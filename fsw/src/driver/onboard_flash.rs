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

#[derive(Debug, defmt::Format)]
pub enum Error {
    Read,
    Write,
    Erase,
    OutOfBounds,
    StorageFull,
    Spi,
}

pub struct OnboardFlash<'a> {
    spi: SpiDeviceType<'a>,
    write_offset: u32,
    needs_header: bool,
}

impl<'a> OnboardFlash<'a> {
    // Commands
    const CMD_WREN: u8 = 0x06;
    const CMD_READ: u8 = 0x03;
    const CMD_PAGE_PROG: u8 = 0x02;
    const CMD_SECTOR_ERASE: u8 = 0x20;
    const CMD_RDSR1: u8 = 0x05;
    const CMD_JEDEC_ID: u8 = 0x9F;

    fn debug_print(s: &str) {
        log::info!("{}", s);
        crate::umbilical::print_str(s);
        crate::umbilical::print_str("\n");
    }

    pub fn new(spi: SpiDeviceType<'a>) -> Self {
        Self {
            spi,
            write_offset: STORAGE_OFFSET,
            needs_header: true,
        }
    }

    async fn wait_busy(&mut self) -> Result<(), Error> {
        loop {
            let mut status = [0u8; 1];
            self.spi.transaction(&mut [
                embedded_hal_async::spi::Operation::Write(&[Self::CMD_RDSR1]),
                embedded_hal_async::spi::Operation::Read(&mut status),
            ]).await.map_err(|_| Error::Spi)?;
            if status[0] & 0x01 == 0 {
                break;
            }
        }
        Ok(())
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
        let mut buffer = [0u8; PAGE_SIZE as usize];
        let mut low = STORAGE_OFFSET;
        let mut high = STORAGE_OFFSET + STORAGE_SIZE - PAGE_SIZE;

        Self::debug_print("Flash(SPI): Init logging scan...");

        // JEDEC ID Check
        if let Ok(id) = self.read_jedec_id().await {
            let mut msg = heapless::String::<64>::new();
            let _ = core::fmt::write(&mut msg, format_args!("Flash: JEDEC ID: {:02x} {:02x} {:02x}", id[0], id[1], id[2]));
            Self::debug_print(msg.as_str());
        }

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
        
        let mut msg = heapless::String::<64>::new();
        let _ = core::fmt::write(&mut msg, format_args!("Flash: Offset set to {:#x}", self.write_offset));
        Self::debug_print(msg.as_str());

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

    pub async fn wipe_storage(&mut self) -> Result<(), Error> {
        let mut addr = STORAGE_OFFSET;
        let end = STORAGE_OFFSET + STORAGE_SIZE;
        while addr < end {
            self.erase_sector(addr).await?;
            addr += SECTOR_SIZE;
        }
        self.write_offset = STORAGE_OFFSET;
        self.needs_header = true;
        Ok(())
    }
}