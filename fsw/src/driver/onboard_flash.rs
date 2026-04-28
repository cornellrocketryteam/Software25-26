//! Onboard SPI Flash driver for W25Q128JV (16 MiB) on shared SPI bus
//!
//! Features:
//! - Page Programming (256 bytes)
//! - Sector Erasing (4 KB)
//! - Binary search for write offset on initialization
//! - Shared SPI bus support with embassy-embedded-hal

use embedded_hal_async::spi::SpiDevice;
use crate::packet::Packet;
use crate::module::SpiDevice as SpiDeviceType;

/// Total flash size: 16 MiB
const FLASH_SIZE: u32 = 16 * 1024 * 1024;
const PAGE_SIZE: u32 = 256;      // W25Q128JV Page Size 
const SECTOR_SIZE: u32 = 4096;   // W25Q128JV Uniform 4KB Sector Erase 

/// Storage region offset from flash base
const STORAGE_OFFSET: u32 = 0x200000;
const STORAGE_SIZE: u32 = 0xE00000;

/// Snapshot ring: small persistent state for crash recovery (replaces FRAM).
/// 64 KB = 16 sectors × 64 records/sector = 1024 record slots.
const SNAPSHOT_RING_BASE: u32 = 0x100000;
const SNAPSHOT_RING_SIZE: u32 = 0x10000;
pub const SNAPSHOT_RECORD_SIZE: u32 = 64;
const SNAPSHOT_MAGIC: [u8; 2] = [0x5A, 0xA5];
const SNAPSHOT_EMPTY_SEQ: u32 = 0xFFFF_FFFF;

#[derive(Debug, defmt::Format)]
pub enum Error {
    Read,
    Write,
    Erase,
    OutOfBounds,
    StorageFull,
    Spi,
}

/// 64-byte snapshot record: 10 × u32 of flight state + magic + seq + crc.
#[derive(Copy, Clone, Default, Debug)]
pub struct Snapshot {
    pub seq: u32,
    pub flight_mode: u32,
    pub cycle_count: u32,
    pub pressure: f32,
    pub temp: f32,
    pub altitude: f32,
    pub mav_open: u32,
    pub sv_open: u32,
    pub pt3: f32,
    pub pt4: f32,
    pub rtd: f32,
}

impl Snapshot {
    fn to_bytes(&self) -> [u8; 64] {
        let mut b = [0xFFu8; 64];
        b[0..2].copy_from_slice(&SNAPSHOT_MAGIC);
        b[2..6].copy_from_slice(&self.seq.to_le_bytes());
        b[6..10].copy_from_slice(&self.flight_mode.to_le_bytes());
        b[10..14].copy_from_slice(&self.cycle_count.to_le_bytes());
        b[14..18].copy_from_slice(&self.pressure.to_le_bytes());
        b[18..22].copy_from_slice(&self.temp.to_le_bytes());
        b[22..26].copy_from_slice(&self.altitude.to_le_bytes());
        b[26..30].copy_from_slice(&self.mav_open.to_le_bytes());
        b[30..34].copy_from_slice(&self.sv_open.to_le_bytes());
        b[34..38].copy_from_slice(&self.pt3.to_le_bytes());
        b[38..42].copy_from_slice(&self.pt4.to_le_bytes());
        b[42..46].copy_from_slice(&self.rtd.to_le_bytes());
        let crc = Self::crc(&b[0..46]);
        b[46..50].copy_from_slice(&crc.to_le_bytes());
        b
    }

    fn from_bytes(b: &[u8; 64]) -> Option<Self> {
        if b[0..2] != SNAPSHOT_MAGIC {
            return None;
        }
        let u32at = |i: usize| u32::from_le_bytes([b[i], b[i+1], b[i+2], b[i+3]]);
        let f32at = |i: usize| f32::from_le_bytes([b[i], b[i+1], b[i+2], b[i+3]]);
        let stored_crc = u32::from_le_bytes([b[46], b[47], b[48], b[49]]);
        if stored_crc != Self::crc(&b[0..46]) {
            return None;
        }
        Some(Self {
            seq: u32at(2),
            flight_mode: u32at(6),
            cycle_count: u32at(10),
            pressure: f32at(14),
            temp: f32at(18),
            altitude: f32at(22),
            mav_open: u32at(26),
            sv_open: u32at(30),
            pt3: f32at(34),
            pt4: f32at(38),
            rtd: f32at(42),
        })
    }

    fn crc(data: &[u8]) -> u32 {
        // Cheap Fletcher-ish checksum — enough to catch torn writes / bit rot.
        let mut a: u32 = 0;
        let mut s: u32 = 0;
        for &byte in data {
            a = a.wrapping_add(byte as u32);
            s = s.wrapping_add(a);
        }
        (s << 16) ^ a
    }
}

pub struct OnboardFlash<'a> {
    spi: SpiDeviceType<'a>,
    write_offset: u32,
    needs_header: bool,
    /// True iff the flash chip is accessible (JEDEC ID responded).
    /// Snapshot ring reads/writes are gated on this flag.
    pub flash_ok: bool,
    /// True iff the data-log region (0x200000+) is exhausted.
    /// The snapshot ring at 0x100000 is a separate region and still works when this is set.
    pub storage_full: bool,
    snapshot_offset: u32,
    snapshot_next_seq: u32,
}

impl<'a> OnboardFlash<'a> {
    // Commands
    const CMD_WREN: u8 = 0x06;
    const CMD_READ: u8 = 0x03;
    const CMD_PAGE_PROG: u8 = 0x02;
    const CMD_SECTOR_ERASE: u8 = 0x20;
    const CMD_RDSR1: u8 = 0x05;
    const CMD_JEDEC_ID: u8 = 0x9F;

    pub fn new(spi: SpiDeviceType<'a>) -> Self {
        Self {
            spi,
            write_offset: STORAGE_OFFSET,
            needs_header: true,
            flash_ok: false,
            storage_full: false,
            snapshot_offset: SNAPSHOT_RING_BASE,
            snapshot_next_seq: 1,
        }
    }

    async fn wait_busy(&mut self) -> Result<(), Error> {
        for _ in 0..500_000 {
            let mut status = [0u8; 1];
            self.spi.transaction(&mut [
                embedded_hal_async::spi::Operation::Write(&[Self::CMD_RDSR1]),
                embedded_hal_async::spi::Operation::Read(&mut status),
            ]).await.map_err(|_| Error::Spi)?;
            if status[0] & 0x01 == 0 {
                return Ok(());
            }
        }
        Err(Error::Spi)
    }

    async fn write_enable(&mut self) -> Result<(), Error> {
        self.spi.write(&[Self::CMD_WREN]).await.map_err(|_| Error::Spi)
    }

    pub async fn read_jedec_id(&mut self) -> Result<[u8; 3], Error> {
        let mut id = [0u8; 3];
        self.spi.transaction(&mut [
            embedded_hal_async::spi::Operation::Write(&[Self::CMD_JEDEC_ID]),
            embedded_hal_async::spi::Operation::Read(&mut id),
        ]).await.map_err(|_| Error::Spi)?;
        Ok(id)
    }

    pub async fn initialize_logging(&mut self) -> Result<(), Error> {
        // Validate chip presence via JEDEC ID before doing anything else
        let id = self.read_jedec_id().await.map_err(|_| Error::Spi)?;
        log::info!("Flash: JEDEC ID: {:02x} {:02x} {:02x}", id[0], id[1], id[2]);
        if id[0] == 0xFF && id[1] == 0xFF && id[2] == 0xFF {
            log::error!("Flash: no chip detected (JEDEC = ff ff ff)");
            return Err(Error::Spi);
        }

        log::info!("Flash(SPI): Init logging scan...");
        let mut buffer = [0u8; PAGE_SIZE as usize];
        let mut low = STORAGE_OFFSET;
        let mut high = STORAGE_OFFSET + STORAGE_SIZE - PAGE_SIZE;

        while low <= high {
            let mid = low + ((high - low) / (2 * PAGE_SIZE)) * PAGE_SIZE;
            self.read(mid, &mut buffer).await?;

            if buffer.iter().all(|&x| x == 0xFF) {
                if mid == STORAGE_OFFSET {
                    self.write_offset = STORAGE_OFFSET;
                    self.needs_header = true;
                    return Ok(());
                }
                high = mid - PAGE_SIZE;
            } else {
                low = mid + PAGE_SIZE;
            }
        }

        self.write_offset = low;
        self.needs_header = self.write_offset == STORAGE_OFFSET;
        
        log::info!("Flash: Offset set to {:#x}", self.write_offset);

        if self.write_offset >= STORAGE_OFFSET + STORAGE_SIZE {
             return Err(Error::StorageFull);
        }
        Ok(())
    }

    pub async fn append_packet_csv(&mut self, packet: &Packet) -> Result<(), Error> {
        if self.needs_header {
            let header = Packet::CSV_HEADER;
            self.append_raw(header.as_bytes()).await?;
            self.needs_header = false;
        }
        let mut buf = [0u8; 256];
        let len = packet.to_csv(&mut buf);
        self.append_raw(&buf[..len]).await
    }

    async fn append_raw(&mut self, data: &[u8]) -> Result<(), Error> {
        let mut current_data = data;
        while !current_data.is_empty() {
            if self.write_offset + current_data.len() as u32 > STORAGE_OFFSET + STORAGE_SIZE {
                return Err(Error::StorageFull);
            }

            // Sector Erase if aligned
            if self.write_offset % SECTOR_SIZE == 0 {
                self.erase_sector(self.write_offset).await?;
            }

            let page_offset = self.write_offset % PAGE_SIZE;
            let remaining_in_page = (PAGE_SIZE - page_offset) as usize;
            let write_len = core::cmp::min(current_data.len(), remaining_in_page);

            self.program_page(self.write_offset, &current_data[..write_len]).await?;

            self.write_offset += write_len as u32;
            current_data = &current_data[write_len..];
        }
        Ok(())
    }

    async fn erase_sector(&mut self, addr: u32) -> Result<(), Error> {
        self.write_enable().await?;
        let cmd = [
            Self::CMD_SECTOR_ERASE,
            ((addr >> 16) & 0xFF) as u8,
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ];
        self.spi.write(&cmd).await.map_err(|_| Error::Spi)?;
        // A single sector erase can take 50-400 ms — longer than the flight
        // loop watchdog timeout. Pet the watchdog before waiting for BUSY
        // to clear so inline callers (e.g. wipe_storage from the umbilical
        // command path) don't trigger a reset mid-erase.
        crate::watchdog::feed();
        self.wait_busy().await
    }

    async fn program_page(&mut self, addr: u32, data: &[u8]) -> Result<(), Error> {
        self.write_enable().await?;
        let cmd = [
            Self::CMD_PAGE_PROG,
            ((addr >> 16) & 0xFF) as u8,
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ];
        
        // Multi-write to send header then data
        self.spi.transaction(&mut [
            embedded_hal_async::spi::Operation::Write(&cmd),
            embedded_hal_async::spi::Operation::Write(data),
        ]).await.map_err(|_| Error::Spi)?;
        
        self.wait_busy().await
    }

    pub async fn read(&mut self, addr: u32, buffer: &mut [u8]) -> Result<(), Error> {
        let cmd = [
            Self::CMD_READ,
            ((addr >> 16) & 0xFF) as u8,
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ];
        
        self.spi.transaction(&mut [
            embedded_hal_async::spi::Operation::Write(&cmd),
            embedded_hal_async::spi::Operation::Read(buffer),
        ]).await.map_err(|_| Error::Spi)
    }

    // Legacy support
    const PACKET_OFFSET: u32 = STORAGE_OFFSET - SECTOR_SIZE;
    pub async fn write_packet(&mut self, packet: &Packet) -> Result<(), Error> {
        self.erase_sector(Self::PACKET_OFFSET).await?;
        let bytes = packet.to_bytes();
        self.program_page(Self::PACKET_OFFSET, &bytes).await
    }

    pub async fn read_packet(&mut self) -> Result<Packet, Error> {
        let mut buffer = [0u8; Packet::SIZE];
        self.read(Self::PACKET_OFFSET, &mut buffer).await?;
        Ok(Packet::from_bytes(&buffer))
    }

    pub fn get_write_offset(&self) -> u32 { self.write_offset }
    pub fn get_storage_offset(&self) -> u32 { STORAGE_OFFSET }
    pub fn get_usage(&self) -> (u32, u32) {
        (self.write_offset.saturating_sub(STORAGE_OFFSET), STORAGE_SIZE)
    }

    /// Scan the snapshot ring to locate the newest record. Sets
    /// `snapshot_offset` to the next slot to write and `snapshot_next_seq`
    /// to one past the highest observed sequence number.
    pub async fn initialize_snapshot_ring(&mut self) -> Result<(), Error> {
        let slot_count = SNAPSHOT_RING_SIZE / SNAPSHOT_RECORD_SIZE;
        let mut max_seq: u32 = 0;
        let mut max_slot: Option<u32> = None;
        let mut buf = [0u8; SNAPSHOT_RECORD_SIZE as usize];

        for i in 0..slot_count {
            let addr = SNAPSHOT_RING_BASE + i * SNAPSHOT_RECORD_SIZE;
            self.read(addr, &mut buf).await?;
            // Feed watchdog every 64 slots — full scan is ~1024 reads.
            crate::watchdog::feed();
            if buf[0..2] != SNAPSHOT_MAGIC {
                continue;
            }
            let seq = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);
            if seq == SNAPSHOT_EMPTY_SEQ {
                continue;
            }
            if seq >= max_seq {
                max_seq = seq;
                max_slot = Some(i);
            }
        }

        match max_slot {
            None => {
                self.snapshot_offset = SNAPSHOT_RING_BASE;
                self.snapshot_next_seq = 1;
            }
            Some(slot) => {
                let next_slot = (slot + 1) % slot_count;
                self.snapshot_offset = SNAPSHOT_RING_BASE + next_slot * SNAPSHOT_RECORD_SIZE;
                self.snapshot_next_seq = max_seq.wrapping_add(1);
            }
        }
        log::info!(
            "Flash: snapshot ring: next_offset={:#x} next_seq={}",
            self.snapshot_offset, self.snapshot_next_seq
        );
        Ok(())
    }

    /// Append a snapshot to the ring. Assigns the next sequence number.
    /// If the write crosses a sector boundary, the destination sector is
    /// erased first (ring behavior — old records in that sector are lost).
    pub async fn write_snapshot(&mut self, snap: &mut Snapshot) -> Result<(), Error> {
        snap.seq = self.snapshot_next_seq;
        let bytes = snap.to_bytes();

        if self.snapshot_offset % SECTOR_SIZE == 0 {
            self.erase_sector(self.snapshot_offset).await?;
        }
        self.program_page(self.snapshot_offset, &bytes).await?;

        let slot_count = SNAPSHOT_RING_SIZE / SNAPSHOT_RECORD_SIZE;
        let next_idx = ((self.snapshot_offset - SNAPSHOT_RING_BASE) / SNAPSHOT_RECORD_SIZE + 1)
            % slot_count;
        self.snapshot_offset = SNAPSHOT_RING_BASE + next_idx * SNAPSHOT_RECORD_SIZE;
        self.snapshot_next_seq = self.snapshot_next_seq.wrapping_add(1);
        if self.snapshot_next_seq == SNAPSHOT_EMPTY_SEQ {
            self.snapshot_next_seq = 1;
        }
        Ok(())
    }

    /// Read the newest valid snapshot from the ring, if any.
    pub async fn read_latest_snapshot(&mut self) -> Result<Option<Snapshot>, Error> {
        let slot_count = SNAPSHOT_RING_SIZE / SNAPSHOT_RECORD_SIZE;
        let mut best: Option<Snapshot> = None;
        let mut buf = [0u8; SNAPSHOT_RECORD_SIZE as usize];

        for i in 0..slot_count {
            let addr = SNAPSHOT_RING_BASE + i * SNAPSHOT_RECORD_SIZE;
            self.read(addr, &mut buf).await?;
            crate::watchdog::feed();
            if let Some(snap) = Snapshot::from_bytes(&buf) {
                if snap.seq != SNAPSHOT_EMPTY_SEQ
                    && best.as_ref().map_or(true, |b| snap.seq > b.seq)
                {
                    best = Some(snap);
                }
            }
        }
        Ok(best)
    }

    /// Erase all 16 sectors of the snapshot ring and reset counters.
    pub async fn reset_snapshot_ring(&mut self) -> Result<(), Error> {
        let mut addr = SNAPSHOT_RING_BASE;
        while addr < SNAPSHOT_RING_BASE + SNAPSHOT_RING_SIZE {
            self.erase_sector(addr).await?;
            crate::watchdog::feed();
            addr += SECTOR_SIZE;
        }
        self.snapshot_offset = SNAPSHOT_RING_BASE;
        self.snapshot_next_seq = 1;
        Ok(())
    }

    pub async fn wipe_storage(&mut self) -> Result<(), Error> {
        // Only erase sectors that have been written to, not all 3584 sectors
        let end = (self.write_offset + SECTOR_SIZE - 1) / SECTOR_SIZE * SECTOR_SIZE;
        let total_sectors = ((end - STORAGE_OFFSET) / SECTOR_SIZE) as u32;
        let mut addr = STORAGE_OFFSET;
        let mut done: u32 = 0;
        // Print progress every PROGRESS_EVERY sectors so the serial monitor
        // shows liveness during a multi-second wipe.
        const PROGRESS_EVERY: u32 = 16;
        while addr < end {
            self.erase_sector(addr).await?;
            // `erase_sector` already pets once, but do so again here so a
            // wipe that spans many sectors keeps the watchdog armed between
            // iterations regardless of how long each erase took.
            crate::watchdog::feed();
            addr += SECTOR_SIZE;
            done += 1;
            if done % PROGRESS_EVERY == 0 || done == total_sectors {
                let mut msg = heapless::String::<48>::new();
                let _ = core::fmt::write(
                    &mut msg,
                    format_args!("Wipe progress: {}/{} sectors\n", done, total_sectors),
                );
                crate::umbilical::print_str(msg.as_str());
            }
        }
        self.write_offset = STORAGE_OFFSET;
        self.needs_header = true;
        Ok(())
    }
}