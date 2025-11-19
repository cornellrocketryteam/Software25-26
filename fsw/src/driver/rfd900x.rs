//! RFD900x radio driver
//!
//! Driver for the RFD900x long-range radio module.
//! Communicates via UART1 at 9600 baud.
//!
//! Hardware connections:
//! - GP4 (Pico) -> TX (UART1) -> RX (RFD900x)
//! - GP5 (Pico) -> RX (UART1) -> TX (RFD900x)

use embassy_rp::uart::{Async, Error, Uart};

/// RFD900x radio driver
pub struct Rfd900x<'a> {
    uart: Uart<'a, Async>,
    sync_word: u32,
}

impl<'a> Rfd900x<'a> {
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
        self.uart.write(&self.sync_word.to_le_bytes()).await?;
        self.uart.write(data).await?;

        return Ok(());
    }
}
