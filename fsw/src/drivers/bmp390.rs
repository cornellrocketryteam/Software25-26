use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;
use libm::powf;

// BMP390 I2C address (SDO=1)
const BMP390_ADDR: u8 = 0x77;

// BMP390 Registers
const REG_CHIP_ID: u8 = 0x00;
const REG_PWR_CTRL: u8 = 0x1B;
const REG_OSR: u8 = 0x1C;
const REG_ODR: u8 = 0x1D;
const REG_CONFIG: u8 = 0x1F;
const REG_DATA: u8 = 0x04;

/// BMP390 pressure and temperature sensor
pub struct BMP390<I2C> {
    i2c: I2C,
    ground_pressure: f32,
}

/// Sensor reading data
pub struct SensorReading {
    pub pressure: f32,
    pub temperature: f32,
    pub altitude: f32,
}

impl<I2C, E> BMP390<I2C>
where
    I2C: I2c<Error = E>,
{
    /// Create and initialize a new BMP390 sensor
    pub fn new<D: DelayNs>(mut i2c: I2C, delay: &mut D) -> Result<Self, &'static str> {
        delay.delay_ms(10);

        // Verify chip ID
        let mut chip_id = [0u8];
        i2c.write_read(BMP390_ADDR, &[REG_CHIP_ID], &mut chip_id)
            .map_err(|_| "Failed to read chip ID")?;

        if chip_id[0] != 0x60 {
            return Err("Invalid chip ID");
        }

        delay.delay_ms(10);

        // Configure sensor
        Self::write_reg(&mut i2c, REG_PWR_CTRL, 0x33)?; // Enable pressure and temp, normal mode
        delay.delay_ms(10);
        Self::write_reg(&mut i2c, REG_OSR, 0x03)?; // Oversampling x8
        Self::write_reg(&mut i2c, REG_ODR, 0x04)?; // 50Hz output rate
        Self::write_reg(&mut i2c, REG_CONFIG, 0x02)?; // IIR filter coefficient 3

        delay.delay_ms(50);

        let mut sensor = Self {
            i2c,
            ground_pressure: 101325.0,
        };

        // Calibrate ground level
        sensor.calibrate_ground_level(delay)?;

        Ok(sensor)
    }

    /// Calibrate ground level pressure
    fn calibrate_ground_level<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), &'static str> {
        let mut pressure_sum = 0.0;
        const NUM_SAMPLES: u32 = 10;

        for _ in 0..NUM_SAMPLES {
            let (pressure, _) = self.read_raw(delay)?;
            pressure_sum += pressure;
            delay.delay_ms(10);
        }

        self.ground_pressure = pressure_sum / NUM_SAMPLES as f32;
        Ok(())
    }

    /// Read sensor data
    pub fn read<D: DelayNs>(&mut self, delay: &mut D) -> Result<SensorReading, &'static str> {
        let (pressure, temperature) = self.read_raw(delay)?;
        let altitude = 44330.0 * (1.0 - powf(pressure / self.ground_pressure, 0.1903));

        Ok(SensorReading {
            pressure,
            temperature,
            altitude,
        })
    }

    /// Read raw pressure and temperature
    fn read_raw<D: DelayNs>(&mut self, delay: &mut D) -> Result<(f32, f32), &'static str> {
        delay.delay_ms(40);

        let mut data = [0u8; 6];
        self.i2c
            .write_read(BMP390_ADDR, &[REG_DATA], &mut data)
            .map_err(|_| "Failed to read sensor data")?;

        let press_raw = ((data[2] as u32) << 16) | ((data[1] as u32) << 8) | (data[0] as u32);
        let temp_raw = ((data[5] as u32) << 16) | ((data[4] as u32) << 8) | (data[3] as u32);

        let temperature = (temp_raw as f32 / 16384.0) - 40.0;
        let pressure = press_raw as f32 / 64.0;

        Ok((pressure, temperature))
    }

    fn write_reg(i2c: &mut I2C, reg: u8, value: u8) -> Result<(), &'static str> {
        i2c.write(BMP390_ADDR, &[reg, value])
            .map_err(|_| "I2C write failed")
    }
}
