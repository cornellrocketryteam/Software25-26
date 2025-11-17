//! RFD900x radio driver
//!
//! Driver for the RFD900x long-range radio module.
//! Communicates via UART1 at 9600 baud.
//!
//! Hardware connections:
//! - GP4 (Pico) -> TX (UART1) -> RX (RFD900x)
//! - GP5 (Pico) -> RX (UART1) -> TX (RFD900x)

use embassy_rp::uart::{Uart, Async};

/// RFD900x radio driver
pub struct Rfd900x<'a> {
    uart: Uart<'a, Async>,
}

impl<'a> Rfd900x<'a> {
    /// Create a new RFD900x driver instance
    ///
    /// # Arguments
    /// * `uart` - Configured UART1 peripheral (9600 baud)
    pub fn new(uart: Uart<'a, Async>) -> Self {
        Self { uart }
    }

    /// Send data over the radio
    ///
    /// # Arguments
    /// * `data` - Byte slice to transmit
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(())` on transmission error
    pub async fn send(&mut self, data: &[u8]) -> Result<(), ()> {
        self.uart.write(data).await.map_err(|_| ())
    }
}
