//! MB85RS2 SPI FRAM driver

use crate::module::{SpiDevice, SharedSpi};
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice as SharedSpiDevice;
use embassy_rp::gpio::Output;
use embedded_hal_async::spi::{SpiDevice as _, Operation};

const CMD_WREN: u8 = 0x06; // Write Enable
const CMD_READ: u8 = 0x03; // Read Memory
const CMD_WRITE: u8 = 0x02; // Write Memory
const CMD_RDID: u8 = 0x9F; // Read Device ID (expect 04 7F 48 03)
const CMD_RDSR: u8 = 0x05; // Read Status Register (bit1 = WEL)

/// MB85RS2 FRAM driver
pub struct Fram<'a> {
    spi: SpiDevice<'a>,
}

impl<'a> Fram<'a> {
    /// Create a new FRAM driver using a shared SPI bus
    pub fn new(spi_bus: &'static SharedSpi, cs: Output<'a>) -> Self {
        let spi_device = SharedSpiDevice::new(spi_bus, cs);
        Self { spi: spi_device }
    }

    /// Write enable command
    async fn write_enable(&mut self) -> Result<(), ()> {
        let result = self.spi.write(&[CMD_WREN]).await;
        result.map_err(|_| ())
    }

    /// Read N bytes starting at addr into buf (single SPI transaction)
    pub async fn read_bytes(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), ()> {
        let addr_bytes = [
            ((addr >> 16) & 0x03) as u8,
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ];
        let mut ops = [
            Operation::Write(&[CMD_READ]),
            Operation::Write(&addr_bytes),
            Operation::Read(buf),
        ];
        self.spi.transaction(&mut ops).await.map_err(|_| ())
    }

    /// Write data bytes starting at addr (single WREN + WRITE transaction)
    pub async fn write_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), ()> {
        self.write_enable().await?;
        let addr_bytes = [
            ((addr >> 16) & 0x03) as u8,
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ];
        let mut ops = [
            Operation::Write(&[CMD_WRITE]),
            Operation::Write(&addr_bytes),
            Operation::Write(data),
        ];
        self.spi.transaction(&mut ops).await.map_err(|_| ())
    }

    /// Read a u32 value from the specified address
    pub async fn read_u32(&mut self, addr: u32) -> Result<u32, ()> {
        // Address is 18 bits for MB85RS2 (256KB)
        let addr_bytes = [
            ((addr >> 16) & 0x03) as u8, // Top 2 bits only
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ];

        let mut buffer = [0u8; 4];

        // Send READ command, 3-byte address, and read 4 bytes in a single transaction
        let mut operations = [
            Operation::Write(&[CMD_READ]),
            Operation::Write(&addr_bytes),
            Operation::Read(&mut buffer),
        ];

        if self.spi.transaction(&mut operations).await.is_err() {
            return Err(());
        }

        // Convert big-endian bytes to u32
        Ok(u32::from_be_bytes(buffer))
    }

    /// Write a u32 value to the specified address
    pub async fn write_u32(&mut self, addr: u32, value: u32) -> Result<(), ()> {
        // Enable writes
        self.write_enable().await?;

        // Address is 18 bits for MB85RS2 (256KB)
        let addr_bytes = [
            ((addr >> 16) & 0x03) as u8, // Top 2 bits only
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ];

        let value_bytes = value.to_be_bytes();

        let mut operations = [
            Operation::Write(&[CMD_WRITE]),
            Operation::Write(&addr_bytes),
            Operation::Write(&value_bytes),
        ];

        if self.spi.transaction(&mut operations).await.is_err() {
            return Err(());
        }

        Ok(())
    }

    /// Read Device ID — should return [0x04, 0x7F, 0x48, 0x03]
    pub async fn read_device_id(&mut self) -> Result<[u8; 4], ()> {
        let mut id = [0u8; 4];
        let mut ops = [
            Operation::Write(&[CMD_RDID]),
            Operation::Read(&mut id),
        ];
        self.spi.transaction(&mut ops).await.map_err(|_| ())?;
        Ok(id)
    }

    /// Read Status Register — bit 1 is WEL (Write Enable Latch)
    pub async fn read_status_register(&mut self) -> Result<u8, ()> {
        let mut sr = [0u8; 1];
        let mut ops = [
            Operation::Write(&[CMD_RDSR]),
            Operation::Read(&mut sr),
        ];
        self.spi.transaction(&mut ops).await.map_err(|_| ())?;
        Ok(sr[0])
    }

    /// Reset the FRAM state — zeros all addresses written by state.rs
    pub async fn reset(&mut self) -> Result<(), ()> {
        // Addresses 0–39: FlightMode, CycleCount, Pressure, Temp, Altitude,
        // MAV state, SV state, PT3, PT4, RTD (10 × u32)
        let zeros = [0u8; 40];
        self.write_bytes(0, &zeros).await?;
        // Address 100: fallback altitude log
        self.write_u32(100, 0).await?;
        Ok(())
    }
}
