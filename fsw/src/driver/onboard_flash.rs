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

/// Storage region offset from flash base (last 64KB of 4 MiB)
/// 4 MiB = 0x400000, so last 64KB starts at 0x3F0000
const STORAGE_OFFSET: u32 = 0x3F0000;

/// Fixed address for packet storage within the storage region
const PACKET_OFFSET: u32 = STORAGE_OFFSET;

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
}

/// Onboard QSPI flash driver
pub struct OnboardFlash<'d> {
    flash: Flash<'d, FLASH, Async, FLASH_SIZE>,
}

impl<'d> OnboardFlash<'d> {
    /// Create a new onboard flash driver
    ///
    /// # Arguments
    /// * `flash` - The FLASH peripheral
    /// * `dma` - A DMA channel for async operations
    pub fn new(flash: Peri<'d, FLASH>, dma: Peri<'d, DMA_CH4>) -> Self {
        Self {
            flash: Flash::new(flash, dma, crate::module::Irqs),
        }
    }

    /// Read bytes from flash at the specified offset
    ///
    /// # Arguments
    /// * `offset` - Offset from flash base address
    /// * `buffer` - Buffer to read into
    pub async fn read(&mut self, offset: u32, buffer: &mut [u8]) -> Result<(), Error> {
        self.flash
            .blocking_read(offset, buffer)
            .map_err(|_| Error::Read)
    }

    /// Write bytes to flash at the specified offset
    ///
    /// This function handles erasing the necessary sectors before writing.
    /// The offset must be aligned to ERASE_SIZE (4KB) boundaries for the erase
    /// to work correctly.
    ///
    /// # Arguments
    /// * `offset` - Offset from flash base address (should be sector-aligned)
    /// * `data` - Data to write
    pub async fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), Error> {
        // Ensure we're writing within the storage region
        if offset < STORAGE_OFFSET {
            return Err(Error::OutOfBounds);
        }

        // Calculate sector-aligned erase boundaries
        let erase_start = offset - (offset % ERASE_SIZE as u32);
        let erase_end = erase_start + ERASE_SIZE as u32;

        // Erase the sector first (NOR flash requirement)
        self.flash
            .blocking_erase(erase_start, erase_end)
            .map_err(|_| Error::Erase)?;

        // Write the data
        self.flash
            .blocking_write(offset, data)
            .map_err(|_| Error::Write)
    }

    /// Write the most recent packet to flash storage
    ///
    /// This stores the packet at a fixed location in the storage region.
    /// The previous packet is overwritten.
    pub async fn write_packet(&mut self, packet: &Packet) -> Result<(), Error> {
        let bytes = packet.to_bytes();
        self.write(PACKET_OFFSET, &bytes).await
    }

    /// Read the stored packet from flash
    ///
    /// Returns the most recently written packet, or a default packet
    /// if the storage has never been written.
    pub async fn read_packet(&mut self) -> Result<Packet, Error> {
        let mut buffer = [0u8; Packet::SIZE];
        self.read(PACKET_OFFSET, &mut buffer).await?;
        Ok(Packet::from_bytes(&buffer))
    }
}