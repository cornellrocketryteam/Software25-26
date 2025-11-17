//! MB85RS2 SPI FRAM driver

use embassy_rp::gpio::Output;
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::Spi;
use embedded_hal::spi::SpiBus;

const CMD_WREN: u8 = 0x06; // Write Enable
const CMD_READ: u8 = 0x03; // Read Memory
const CMD_WRITE: u8 = 0x02; // Write Memory

/// MB85RS2 FRAM driver
pub struct Fram<'a> {
    spi: Spi<'a, SPI0, embassy_rp::spi::Blocking>,
    cs: Output<'a>,
}

impl<'a> Fram<'a> {
    /// Create a new FRAM driver
    pub fn new(spi: Spi<'a, SPI0, embassy_rp::spi::Blocking>, cs: Output<'a>) -> Self {
        Self { spi, cs }
    }

    /// Write enable command
    fn write_enable(&mut self) -> Result<(), ()> {
        self.cs.set_low();
        let result = self.spi.write(&[CMD_WREN]);
        self.cs.set_high();
        result.map_err(|_| ())
    }

    /// Read a u32 value from the specified address
    pub fn read_u32(&mut self, addr: u32) -> Result<u32, ()> {
        // Address is 18 bits for MB85RS2 (256KB)
        let addr_bytes = [
            ((addr >> 16) & 0x03) as u8, // Top 2 bits only
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ];

        let mut buffer = [0u8; 4];

        self.cs.set_low();
        // Send READ command and 3-byte address
        if self.spi.write(&[CMD_READ]).is_err() {
            self.cs.set_high();
            return Err(());
        }
        if self.spi.write(&addr_bytes).is_err() {
            self.cs.set_high();
            return Err(());
        }
        // Read 4 bytes
        if self.spi.read(&mut buffer).is_err() {
            self.cs.set_high();
            return Err(());
        }
        self.cs.set_high();

        // Convert big-endian bytes to u32
        Ok(u32::from_be_bytes(buffer))
    }

    /// Write a u32 value to the specified address
    pub fn write_u32(&mut self, addr: u32, value: u32) -> Result<(), ()> {
        // Enable writes
        self.write_enable()?;

        // Address is 18 bits for MB85RS2 (256KB)
        let addr_bytes = [
            ((addr >> 16) & 0x03) as u8, // Top 2 bits only
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ];

        let value_bytes = value.to_be_bytes();

        self.cs.set_low();
        // Send WRITE command, 3-byte address, and 4-byte value
        if self.spi.write(&[CMD_WRITE]).is_err() {
            self.cs.set_high();
            return Err(());
        }
        if self.spi.write(&addr_bytes).is_err() {
            self.cs.set_high();
            return Err(());
        }
        if self.spi.write(&value_bytes).is_err() {
            self.cs.set_high();
            return Err(());
        }
        self.cs.set_high();

        Ok(())
    }
}
