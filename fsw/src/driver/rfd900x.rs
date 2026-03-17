//! RFD900x radio driver
//!
//! Driver for the RFD900x long-range radio module.
//! Communicates via UART1 at 9600 baud.
//!
//! Hardware connections:
//! - GP8 (Pico) -> TX (UART1) -> RX (RFD900x)
//! - GP9 (Pico) -> RX (UART1) -> TX (RFD900x)

use embassy_rp::uart::{Async, Error, Uart};

/// RFD900x radio driver
pub struct Rfd900x<'a> {
    uart: Uart<'a, Async>,
    sync_word: u32,
}

impl<'a> Rfd900x<'a> {
    const SYNC_WORD: [u8; 4] = [0x67, 0x59, 0x5D, 0x3E]; // "CRT!"

    /// Create a new RFD900x driver instance
    ///
    /// # Arguments
    /// * `uart` - Configured UART1 peripheral (9600 baud)
    pub fn new(uart: Uart<'a, Async>) -> Self {
        Self {
            uart: uart,
            sync_word: 0x3E5D5967, // CRT!
        }
    }
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(())` on transmission error
    /// sync word is written as the first byte
    pub async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        self.uart.write(&Self::SYNC_WORD).await?;
        self.uart.write(data).await?;

        return Ok(());
    }

    /// Read data from the radio into the provided buffer
    ///
    /// This function reads until the buffer is full or an error occurs.
    pub async fn receive(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        self.uart.read(buffer).await
    }

    /// Synchronize and read a full packet
    ///
    /// Scans the UART stream for the SYNC_WORD before reading the rest of the packet.
    pub async fn receive_packet(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        let mut sync_idx = 0;
        let mut byte = [0u8; 1];

        // 1. Scan for sync word
        while sync_idx < Self::SYNC_WORD.len() {
            self.uart.read(&mut byte).await?;
            if byte[0] == Self::SYNC_WORD[sync_idx] {
                sync_idx += 1;
            } else if byte[0] == Self::SYNC_WORD[0] {
                sync_idx = 1;
            } else {
                sync_idx = 0;
            }
        }

        // 2. Read the actual packet data
        self.uart.read(buffer).await
    }
}
