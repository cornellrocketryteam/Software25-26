//! RFD900x radio driver
//!
//! Driver for the RFD900x long-range radio module.
//! Communicates via UART1 at 9600 baud.
//!
//! Hardware connections:
//! - GP4 (Pico) -> TX (UART1) -> RX (RFD900x)
//! - GP5 (Pico) -> RX (UART1) -> TX (RFD900x)

use embassy_rp::uart::{Async, Error, Uart};
use embassy_time::{with_timeout, Duration};

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

    /// Read data from the radio into the provided buffer
    /// 
    /// This function reads until the buffer is full or an error occurs.
    pub async fn receive(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        self.uart.read(buffer).await
    }

    /// Wait for an ACK from the radio
    /// 
    /// This function waits up to `timeout_ms` for the sequence b"ACK".
    pub async fn wait_for_ack(&mut self, timeout_ms: u64) -> Result<bool, Error> {
        let mut buf = [0u8; 3];
        match with_timeout(Duration::from_millis(timeout_ms), self.uart.read(&mut buf)).await {
            Ok(Ok(_)) => {
                if &buf == b"ACK" {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(false), // Timeout
        }
    }

    /// Send an ACK via the radio
    pub async fn send_ack(&mut self) -> Result<(), Error> {
        self.uart.write(b"ACK").await
    }
}
