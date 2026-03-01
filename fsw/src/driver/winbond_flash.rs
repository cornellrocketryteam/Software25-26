//! Winbond W25Q01 128MB SPI Flash driver
//!
//! Used for permanent flight data logging. Packets are written sequentially
//! starting at address 0. Sectors are erased on-demand as the write pointer
//! enters each new sector.
//!
//! JEDEC ID: 0xEF 0x40 0x21 (verified on board)
//!
//! TODO: Update SPI bus, CS pin, and DMA channels to match final board assignment.

use embassy_rp::gpio::Output;
use embassy_rp::peripherals::{DMA_CH5, DMA_CH6, SPI1};
use embassy_rp::spi::{Async, Spi};
use embassy_time::Timer;

use crate::packet::Packet;

/// Expected JEDEC ID for Winbond W25Q01
const JEDEC_ID: [u8; 3] = [0xEF, 0x40, 0x21];

/// Total flash size: 128 MB
const FLASH_SIZE: u32 = 128 * 1024 * 1024;

/// Page size: 256 bytes (maximum bytes per page program command)
const PAGE_SIZE: u32 = 256;

/// Sector size: 4 KB (minimum erase unit)
const SECTOR_SIZE: u32 = 4096;

// SPI commands - 4-byte addressing required for chips >16MB
const CMD_WRITE_ENABLE: u8 = 0x06;
const CMD_READ_STATUS: u8 = 0x05;
const CMD_JEDEC_ID: u8 = 0x9F;
const CMD_READ: u8 = 0x13;         // Read with 4-byte address
const CMD_PAGE_PROGRAM: u8 = 0x12; // Page program with 4-byte address
const CMD_SECTOR_ERASE: u8 = 0x21; // 4KB sector erase with 4-byte address
const CMD_CHIP_ERASE: u8 = 0x60;   // Full chip erase (~200s for 128MB, pre-flight only)

/// Write In Progress bit in status register
const STATUS_WIP: u8 = 0x01;

#[derive(Debug, defmt::Format)]
pub enum Error {
    Spi,
    InvalidJedecId,
    OutOfBounds,
}

/// Winbond W25Q01 128MB SPI flash driver for permanent flight data logging
pub struct WinbondFlash<'d> {
    // TODO: Change SPI1 to the correct bus once board pin assignment is confirmed
    spi: Spi<'d, SPI1, Async>,
    cs: Output<'d>,
    /// Next write address - reset to 0 on each power cycle / new flight
    write_ptr: u32,
}

impl<'d> WinbondFlash<'d> {
    /// Initialize the driver and verify the JEDEC ID
    ///
    /// # Arguments
    /// * `spi` - SPI bus instance (TODO: confirm bus)
    /// * `cs`  - Chip select pin (active low)
    pub async fn new(
        // TODO: Update peripheral types to match final board pin assignment
        spi: Spi<'d, SPI1, Async>,
        cs: Output<'d>,
    ) -> Result<Self, Error> {
        let mut driver = Self { spi, cs, write_ptr: 0 };

        let id = driver.read_jedec_id().await?;
        if id != JEDEC_ID {
            return Err(Error::InvalidJedecId);
        }

        log::info!("Winbond W25Q01 initialized (JEDEC: {:02X} {:02X} {:02X})", id[0], id[1], id[2]);
        Ok(driver)
    }

    /// Read the JEDEC manufacturer/device ID
    async fn read_jedec_id(&mut self) -> Result<[u8; 3], Error> {
        self.cs.set_low();
        self.spi.write(&[CMD_JEDEC_ID]).await.map_err(|_| Error::Spi)?;
        let mut id = [0u8; 3];
        self.spi.read(&mut id).await.map_err(|_| Error::Spi)?;
        self.cs.set_high();
        Ok(id)
    }

    /// Poll status register until the Write In Progress bit clears
    async fn wait_ready(&mut self) -> Result<(), Error> {
        loop {
            self.cs.set_low();
            self.spi.write(&[CMD_READ_STATUS]).await.map_err(|_| Error::Spi)?;
            let mut status = [0u8; 1];
            self.spi.read(&mut status).await.map_err(|_| Error::Spi)?;
            self.cs.set_high();

            if status[0] & STATUS_WIP == 0 {
                break;
            }

            // Yield to executor while waiting (sector erases take ~45ms)
            Timer::after_micros(500).await;
        }
        Ok(())
    }

    /// Send Write Enable command (required before every write or erase)
    async fn write_enable(&mut self) -> Result<(), Error> {
        self.cs.set_low();
        self.spi.write(&[CMD_WRITE_ENABLE]).await.map_err(|_| Error::Spi)?;
        self.cs.set_high();
        Ok(())
    }

    /// Read bytes from flash at the given address
    pub async fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Error> {
        if addr as usize + buf.len() > FLASH_SIZE as usize {
            return Err(Error::OutOfBounds);
        }
        self.cs.set_low();
        let cmd = [
            CMD_READ,
            (addr >> 24) as u8,
            (addr >> 16) as u8,
            (addr >> 8) as u8,
            addr as u8,
        ];
        self.spi.write(&cmd).await.map_err(|_| Error::Spi)?;
        self.spi.read(buf).await.map_err(|_| Error::Spi)?;
        self.cs.set_high();
        Ok(())
    }

    /// Erase a 4KB sector. Address must be sector-aligned (multiple of 4096).
    pub async fn erase_sector(&mut self, addr: u32) -> Result<(), Error> {
        if addr >= FLASH_SIZE {
            return Err(Error::OutOfBounds);
        }
        self.write_enable().await?;
        self.cs.set_low();
        let cmd = [
            CMD_SECTOR_ERASE,
            (addr >> 24) as u8,
            (addr >> 16) as u8,
            (addr >> 8) as u8,
            addr as u8,
        ];
        self.spi.write(&cmd).await.map_err(|_| Error::Spi)?;
        self.cs.set_high();
        self.wait_ready().await
    }

    /// Write up to 256 bytes within a single page.
    /// Caller is responsible for ensuring data fits within the page boundary.
    async fn write_page(&mut self, addr: u32, data: &[u8]) -> Result<(), Error> {
        self.write_enable().await?;
        self.cs.set_low();
        let cmd = [
            CMD_PAGE_PROGRAM,
            (addr >> 24) as u8,
            (addr >> 16) as u8,
            (addr >> 8) as u8,
            addr as u8,
        ];
        self.spi.write(&cmd).await.map_err(|_| Error::Spi)?;
        self.spi.write(data).await.map_err(|_| Error::Spi)?;
        self.cs.set_high();
        self.wait_ready().await
    }

    /// Write arbitrary data, splitting across page boundaries as needed.
    async fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), Error> {
        if addr as usize + data.len() > FLASH_SIZE as usize {
            return Err(Error::OutOfBounds);
        }

        let mut written = 0;
        let mut current_addr = addr;

        while written < data.len() {
            // Bytes remaining until end of current page
            let page_remaining = (PAGE_SIZE - (current_addr % PAGE_SIZE)) as usize;
            let chunk = (data.len() - written).min(page_remaining);

            self.write_page(current_addr, &data[written..written + chunk]).await?;

            written += chunk;
            current_addr += chunk as u32;
        }

        Ok(())
    }

    /// Append a packet to the permanent flight log.
    ///
    /// Erases the sector automatically when the write pointer enters a new one.
    /// Returns OutOfBounds if the flash is full.
    pub async fn write_packet(&mut self, packet: &Packet) -> Result<(), Error> {
        let addr = self.write_ptr;

        if addr as usize + Packet::SIZE > FLASH_SIZE as usize {
            return Err(Error::OutOfBounds);
        }

        // Erase sector when write pointer first enters it
        if addr % SECTOR_SIZE == 0 {
            self.erase_sector(addr).await?;
        }

        self.write(addr, &packet.to_bytes()).await?;
        self.write_ptr += Packet::SIZE as u32;

        Ok(())
    }

    /// Read a packet by index (0 = first packet written)
    pub async fn read_packet(&mut self, index: u32) -> Result<Packet, Error> {
        let addr = index * Packet::SIZE as u32;
        if addr as usize + Packet::SIZE > FLASH_SIZE as usize {
            return Err(Error::OutOfBounds);
        }
        let mut buf = [0u8; Packet::SIZE];
        self.read(addr, &mut buf).await?;
        Ok(Packet::from_bytes(&buf))
    }

    /// Erase the entire chip. Call once before each flight.
    ///
    /// WARNING: Takes a long time (~200s for 128MB) and should only be done pre-flight.
    pub async fn erase_all(&mut self) -> Result<(), Error> {
        self.write_enable().await?;
        self.cs.set_low();
        self.spi.write(&[CMD_CHIP_ERASE]).await.map_err(|_| Error::Spi)?;
        self.cs.set_high();
        self.wait_ready().await?;
        self.write_ptr = 0;
        Ok(())
    }

    /// Number of packets written to the log since boot
    pub fn packet_count(&self) -> u32 {
        self.write_ptr / Packet::SIZE as u32
    }
}
