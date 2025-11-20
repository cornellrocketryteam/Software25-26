use crate::module::{I2cDevice, SharedI2c};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embedded_hal_async::i2c::I2c as I2cTrait;
use ublox::{FixedLinearBuffer, PacketRef, Parser};

/// I2C address for MAX-M10S GPS module
const GPS_I2C_ADDR: u8 = 0x42;

/// Register to read data stream from GPS
const GPS_DATA_STREAM_REG: u8 = 0xFF;

/// Maximum bytes to read per I2C transaction
const MAX_READ_BYTES: usize = 255;

/// Error types for GPS operations
#[derive(Debug)]
pub enum GpsError {
    I2cError,
    ParseError,
    NoData,
    InvalidData,
}

/// Driver for ublox MAX-M10S GPS module over I2C
pub struct UbloxMaxM10s<'a, I2C> {
    i2c: I2C,
    parser: Parser<FixedLinearBuffer<'a>>,
}

impl UbloxMaxM10s<'static, I2cDevice<'static>> {
    /// Create a new GPS driver instance
    ///
    /// Takes a shared I2C bus and returns a GPS driver instance for reading position, time, and satellite data
    pub fn new(i2c_bus: &'static SharedI2c) -> Self {
        let i2c_device = SharedI2cDevice::new(i2c_bus);

        // Create a static buffer for the GPS parser (512 bytes)
        static GPS_BUFFER: static_cell::StaticCell<[u8; 512]> = static_cell::StaticCell::new();
        let buffer = GPS_BUFFER.init([0u8; 512]);

        let buf = FixedLinearBuffer::new(buffer);

        log::info!("ublox MAX-M10S GPS initialized");

        Self {
            i2c: i2c_device,
            parser: Parser::new(buf),
        }
    }
}

impl<'a, I2C> UbloxMaxM10s<'a, I2C>
where
    I2C: I2cTrait,
{

    /// Read available bytes from GPS module via I2C
    async fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, GpsError> {
        // First, read 2 bytes to get number of available bytes
        let mut avail_bytes = [0u8; 2];
        self.i2c
            .write_read(GPS_I2C_ADDR, &[0xFD], &mut avail_bytes)
            .await
            .map_err(|_| GpsError::I2cError)?;

        let available = u16::from_be_bytes(avail_bytes) as usize;

        if available == 0 || available == 0xFFFF {
            return Ok(0);
        }

        // Read actual data (limit to buffer size and MAX_READ_BYTES)
        let bytes_to_read = available.min(MAX_READ_BYTES).min(buffer.len());

        self.i2c
            .write_read(
                GPS_I2C_ADDR,
                &[GPS_DATA_STREAM_REG],
                &mut buffer[..bytes_to_read],
            )
            .await
            .map_err(|_| GpsError::I2cError)?;

        Ok(bytes_to_read)
    }

    /// Read GPS data and update the packet
    ///
    /// This function reads data from the GPS module, parses NAV-PVT messages,
    /// and directly updates the provided packet with GPS data.
    pub async fn read_into_packet(
        &mut self,
        packet: &mut crate::packet::Packet,
    ) -> Result<(), GpsError> {
        let mut buffer = [0u8; MAX_READ_BYTES];

        // Read available data from GPS
        let bytes_read = self.read_bytes(&mut buffer).await?;

        if bytes_read == 0 {
            return Err(GpsError::NoData);
        }

        // Feed bytes to parser
        let mut it = self.parser.consume(&buffer[..bytes_read]);

        // Process the iterator and extract packets
        let mut found_packet = false;
        loop {
            match it.next() {
                Some(Ok(ubx_packet)) => {
                    match ubx_packet {
                        PacketRef::NavPvt(pvt) => {
                            // Update packet directly
                            packet.latitude = pvt.lat_degrees() as f32;
                            packet.longitude = pvt.lon_degrees() as f32;
                            packet.num_satellites = pvt.num_satellites() as u32;

                            // Calculate timestamp from GPS time
                            // Simple timestamp: hours * 3600 + minutes * 60 + seconds
                            packet.timestamp = (pvt.hour() as f32 * 3600.0)
                                + (pvt.min() as f32 * 60.0)
                                + (pvt.sec() as f32);

                            found_packet = true;
                        }
                        _ => {
                            // Ignore other packet types
                        }
                    }
                }
                Some(Err(_)) => {
                    // Malformed packet, continue
                }
                None => {
                    // No more packets
                    break;
                }
            }
        }

        if found_packet {
            Ok(())
        } else {
            Err(GpsError::NoData)
        }
    }
}
