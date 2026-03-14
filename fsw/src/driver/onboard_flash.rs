//! Onboard QSPI Flash driver for W25Q32RV (4 MiB)
//!
//! This driver provides temporary storage for the most recent packet.
//! The flash is accessed via the RP2350's dedicated QSPI controller.

use embassy_rp::flash::{Async, Flash, ERASE_SIZE};
use embassy_rp::peripherals::{DMA_CH4, FLASH};
use embassy_rp::Peri;

use crate::packet::Packet;

/// Total flash size: 4 MiB
const FLASH_SIZE: usize = 4 * 1024 * 1024;

/// Storage region offset from flash base (last 2MB of 4 MiB)
/// 4 MiB = 0x400000, 2 MiB = 0x200000
const STORAGE_OFFSET: u32 = 0x200000;
const STORAGE_SIZE: u32 = 0x200000;

/// Error type for flash operations
#[derive(Debug, defmt::Format)]
pub enum Error {
    /// Flash read failed
    Read,
    /// Flash write failed
    Write,
    /// Flash erase failed
    Erase,
    /// Address out of bounds
    OutOfBounds,
    /// Storage full
    StorageFull,
}

/// Onboard QSPI flash driver
pub struct OnboardFlash<'d> {
    flash: Flash<'d, FLASH, Async, FLASH_SIZE>,
    write_offset: u32,
}

impl<'d> OnboardFlash<'d> {
    pub fn new(flash: Peri<'d, FLASH>, dma: Peri<'d, DMA_CH4>) -> Self {
        Self {
            flash: Flash::new(flash, dma, crate::module::Irqs),
            write_offset: STORAGE_OFFSET,
        }
    }

    /// Finds the next available write position by scanning page by page.
    pub async fn initialize_logging(&mut self) -> Result<(), Error> {
        let mut buffer = [0u8; 256]; // Page size
        let mut low = STORAGE_OFFSET;
        let mut high = STORAGE_OFFSET + STORAGE_SIZE - 256;

        // Binary search for the first unwritten (0xFF) page
        while low <= high {
            let mid = low + ((high - low) / (2 * 256)) * 256;
            self.read(mid, &mut buffer).await?;

            if buffer.iter().all(|&x| x == 0xFF) {
                // If it's the first page or the previous page has data, this is it
                if mid == STORAGE_OFFSET {
                    self.write_offset = STORAGE_OFFSET;
                    // Write header if brand new
                    self.write_csv_header().await?;
                    return Ok(());
                }
                
                let mut prev_buffer = [0u8; 256];
                self.read(mid - 256, &mut prev_buffer).await?;
                if prev_buffer.iter().any(|&x| x != 0xFF) {
                    // Found the boundary. Now scan forward byte by byte in the previous page
                    // actually, let's just use mid as the start if it's empty.
                    // To be more precise, we can scan the previous page to find the exact end.
                    self.write_offset = mid;
                    // But wait, if we want to append to the same page, we need to find the null or 0xFF
                    for (i, &b) in prev_buffer.iter().enumerate() {
                        if b == 0xFF {
                            self.write_offset = mid - 256 + i as u32;
                            return Ok(());
                        }
                    }
                    return Ok(());
                }
                high = mid - 256;
            } else {
                low = mid + 256;
            }
        }

        self.write_offset = low;
        if self.write_offset >= STORAGE_OFFSET + STORAGE_SIZE {
             return Err(Error::StorageFull);
        }
        Ok(())
    }

    async fn write_csv_header(&mut self) -> Result<(), Error> {
        let header = Packet::CSV_HEADER;
        self.append_raw(header.as_bytes()).await
    }

    pub async fn append_packet_csv(&mut self, packet: &Packet) -> Result<(), Error> {
        let mut buf = [0u8; 256];
        let len = packet.to_csv(&mut buf);
        self.append_raw(&buf[..len]).await
    }

    async fn append_raw(&mut self, data: &[u8]) -> Result<(), Error> {
        if self.write_offset + data.len() as u32 > STORAGE_OFFSET + STORAGE_SIZE {
            return Err(Error::StorageFull);
        }

        // Check if we need to erase a new sector
        let sector_start = self.write_offset - (self.write_offset % ERASE_SIZE as u32);
        let next_sector_start = sector_start + ERASE_SIZE as u32;

        // If we are about to write into a new sector, erase it
        // Or if we are exactly at the start of a sector
        if self.write_offset % ERASE_SIZE as u32 == 0 {
            self.flash.blocking_erase(self.write_offset, self.write_offset + ERASE_SIZE as u32)
                .map_err(|_| Error::Erase)?;
        } else if self.write_offset + data.len() as u32 > next_sector_start {
             // This append crosses a sector boundary. 
             // We need to erase the next sector.
             self.flash.blocking_erase(next_sector_start, next_sector_start + ERASE_SIZE as u32)
                .map_err(|_| Error::Erase)?;
        }

        self.flash.blocking_write(self.write_offset, data)
            .map_err(|_| Error::Write)?;
        
        self.write_offset += data.len() as u32;
        Ok(())
    }

    /// Read bytes from flash at the specified offset
    pub async fn read(&mut self, offset: u32, buffer: &mut [u8]) -> Result<(), Error> {
        self.flash
            .blocking_read(offset, buffer)
            .map_err(|_| Error::Read)
    }

    // Keep legacy packet read/write for now just in case, but redirected to fixed location
    const PACKET_OFFSET: u32 = STORAGE_OFFSET - ERASE_SIZE as u32; // Use one sector before CSV log

    pub async fn write_packet(&mut self, packet: &Packet) -> Result<(), Error> {
        let bytes = packet.to_bytes();
        self.flash.blocking_erase(Self::PACKET_OFFSET, Self::PACKET_OFFSET + ERASE_SIZE as u32)
            .map_err(|_| Error::Erase)?;
        self.flash.blocking_write(Self::PACKET_OFFSET, &bytes)
            .map_err(|_| Error::Write)
    }

    pub async fn read_packet(&mut self) -> Result<Packet, Error> {
        let mut buffer = [0u8; Packet::SIZE];
        self.read(Self::PACKET_OFFSET, &mut buffer).await?;
        Ok(Packet::from_bytes(&buffer))
    }

    pub fn get_write_offset(&self) -> u32 {
        self.write_offset
    }

    pub fn get_storage_offset(&self) -> u32 {
        STORAGE_OFFSET
    }

    /// Erases the entire logging region and resets the write pointer
    pub async fn wipe_storage(&mut self) -> Result<(), Error> {
        let mut addr = STORAGE_OFFSET;
        let end = STORAGE_OFFSET + STORAGE_SIZE;
        while addr < end {
            self.flash.blocking_erase(addr, addr + ERASE_SIZE as u32)
                .map_err(|_| Error::Erase)?;
            addr += ERASE_SIZE as u32;
        }
        self.write_offset = STORAGE_OFFSET;
        self.write_csv_header().await?;
        Ok(())
    }

    /// Returns (used_bytes, total_bytes) for the storage region
    pub fn get_usage(&self) -> (u32, u32) {
        (self.write_offset - STORAGE_OFFSET, STORAGE_SIZE)
    }
}