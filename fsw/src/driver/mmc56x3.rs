use crate::module::{I2cDevice, SharedI2c};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c::I2c;

/// AK09915 I2C address
const MMC56X3_ADDR: u8 = 0x30;

//need to figure out if there is a solution to burst reading for this sensor 

const REG_OUT_X0: u8 = 0x00; //x-axis data high 

const REG_STATUS1: u8 = 0x18;
const REG_ODR: u8 = 0x1A; //output data register 
const REG_CTRL2: u8 = 0x1D; // Control register for enabling continuous mode


const STATUS1_DRDY: u8 = 0x01; // data ready bit
const REG_CTRL1: u8= 0x1C; 
const SOFT_RESET: u8=0x01; // soft reset bit inside ctrl 1 

/// Sensitivity: 0.00625 µT per LSB
const SENSITIVITY: f32 = 0.00625;


const ODR_200HZ: u8 = 200;

/// MMC56X3 magnometer errors
#[derive(Debug)]
pub enum Mmc56x3Error<E> {
    I2c(E),
    DataNotReady,
}

impl<E> From<E> for Mmc56x3Error<E> {
    fn from(error: E) -> Self {
        Mmc56x3Error::I2c(error)
    }
}


/// MMC56X3 magnetometer driver wrapper
pub struct Mmc56x3Sensor {
    pub i2c: I2cDevice<'static>,
}

impl Mmc56x3Sensor {





pub async fn new(i2c_bus: &'static SharedI2c) -> Self {
    let mut sensor = Self {
        i2c: SharedI2cDevice::new(i2c_bus),
    };

     let _dev = I2cDevice::new(i2c_bus);

       
     // Initialize sensor: configure to 200Hz ODR continous mode 
        match sensor.init().await {
            Ok(_) => log::info!("MMC56X3 initialized at 200Hz"),
            Err(_) => log::error!("Failed to initialize MMC56X3"),
        }

        sensor
}

/// Initialize sensor (set ODR)
    async fn init(&mut self) -> Result<(), Mmc56x3Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {

        log::info!("MMC56X3: Writing soft reset");
        self.write_register(REG_CTRL1, SOFT_RESET).await?;

        Timer::after(Duration::from_millis(25)).await;

        
        log::info!("odr set");

        // set ODR to 200Hz 
        self.write_register(REG_ODR, ODR_200HZ).await?;

        // continous mode 
        let ctrl2 = self.read_register(REG_CTRL2).await?;
        log::info!("MMC56X3: Current CTRL2 = 0x{:02X}", ctrl2);

        self.write_register(REG_CTRL2, 0x10).await?;
        log::info!("MMC56X3: Set continuous mode");



        // Wait a short time for first measurement
        Timer::after(Duration::from_millis(50)).await;
        Ok(())
    }

 /// Write a value to a register
    async fn write_register(&mut self, register: u8, value: u8) -> Result<(), Mmc56x3Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let data = [register, value];
        self.i2c.write(MMC56X3_ADDR, &data).await?;
        Ok(())
    }

    /// Read a single register
    async fn read_register(&mut self, register: u8) -> Result<u8, Mmc56x3Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let mut buffer = [0u8; 1];
        self.i2c.write_read(MMC56X3_ADDR, &[register], &mut buffer).await?;
        Ok(buffer[0])
    }

    /// Read multiple bytes starting from a register
    async fn read_registers(&mut self, register: u8, buffer: &mut [u8]) -> Result<(), Mmc56x3Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        self.i2c.write_read(MMC56X3_ADDR, &[register], buffer).await?;
        Ok(())
    }


// read magnetometer data into packet as one stream starting from a register 
pub async fn read_into_packet(
        &mut self,
        packet: &mut crate::packet::Packet,
    ) -> Result<(), Mmc56x3Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let status = self.read_register(REG_STATUS1).await?;
        if status & STATUS1_DRDY == 0 {
            return Err(Mmc56x3Error::DataNotReady);
        }

      let mut buf = [0u8; 9];

      self.read_registers(REG_OUT_X0, &mut buf).await?;

       //combining the bytes to be in correct order, High 19-12, Mid 11-4, Low 3-0 bits 
      let mut x_raw = ((buf[0] as u32) << 12) | ((buf[1] as u32) << 4) | ((buf[6] as u32) >> 4);
      let mut y_raw = ((buf[2] as u32) << 12) | ((buf[3] as u32) << 4) | ((buf[7] as u32) >> 4);
      let mut z_raw = ((buf[4] as u32) << 12) | ((buf[5] as u32) << 4) | ((buf[8] as u32) >> 4);
       
        // make it signed 
        x_raw = x_raw.wrapping_sub(1 << 19);
        y_raw = y_raw.wrapping_sub(1 << 19);
        z_raw = z_raw.wrapping_sub(1 << 19);


      // Convert to microtesla (µT) using sensitivity of 0.00625 µT/LSB
        packet.mag_x = x_raw as f32 * SENSITIVITY;
        packet.mag_y = y_raw as f32 * SENSITIVITY;
        packet.mag_z = z_raw as f32 * SENSITIVITY;

        Ok(())
        
    }
}