/// Sensor interface module
/// Handles IMU (ICM-42688-P), Altimeter (BMP390), and GPS (uBlox MAX-M10M)
use defmt::*;
use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c::I2c;

use crate::state::SensorData;

/// IMU sensor wrapper for ICM-42688-P
pub struct ImuSensor<I2C> {
    _i2c: core::marker::PhantomData<I2C>,
    initialized: bool,
}

impl<I2C> ImuSensor<I2C>
where
    I2C: I2c,
{
    pub fn new() -> Self {
        Self {
            _i2c: core::marker::PhantomData,
            initialized: false,
        }
    }

    pub async fn init(&mut self, _i2c: &mut I2C) -> Result<(), &'static str> {
        info!("Initializing IMU (ICM-42688-P)");
        Timer::after(Duration::from_millis(100)).await;

        // TODO: Initialize ICM-42688-P sensor
        // For now, mark as initialized
        self.initialized = true;
        info!("IMU initialized successfully");
        Ok(())
    }

    pub async fn read(
        &mut self,
        _i2c: &mut I2C,
    ) -> Result<(f32, f32, f32, f32, f32, f32), &'static str> {
        if !self.initialized {
            return Err("IMU not initialized");
        }

        // TODO: Read actual sensor data
        // Returns (gyro_x, gyro_y, gyro_z, accel_x, accel_y, accel_z)
        Ok((0.0, 0.0, 0.0, 0.0, 0.0, 9.81))
    }
}

/// Altimeter sensor wrapper for BMP390
pub struct AltimeterSensor<I2C> {
    _i2c: core::marker::PhantomData<I2C>,
    initialized: bool,
    ground_pressure: f32,
}

impl<I2C> AltimeterSensor<I2C>
where
    I2C: I2c,
{
    pub fn new() -> Self {
        Self {
            _i2c: core::marker::PhantomData,
            initialized: false,
            ground_pressure: 101325.0, // Standard sea level pressure
        }
    }

    pub async fn init(&mut self, _i2c: &mut I2C) -> Result<(), &'static str> {
        info!("Initializing Altimeter (BMP390)");
        Timer::after(Duration::from_millis(100)).await;

        // TODO: Initialize BMP390 sensor
        self.initialized = true;
        info!("Altimeter initialized successfully");
        Ok(())
    }

    pub async fn calibrate_ground_level(&mut self, _i2c: &mut I2C) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Altimeter not initialized");
        }

        // TODO: Read current pressure and set as ground level
        info!("Calibrating ground level pressure");
        self.ground_pressure = 101325.0;
        Ok(())
    }

    pub async fn read(&mut self, _i2c: &mut I2C) -> Result<(f32, f32, f32), &'static str> {
        if !self.initialized {
            return Err("Altimeter not initialized");
        }

        // TODO: Read actual sensor data
        // Returns (altitude in meters, pressure in Pa, temperature in C)
        Ok((0.0, self.ground_pressure, 20.0))
    }
}

/// GPS sensor wrapper for uBlox MAX-M10M
pub struct GpsSensor {
    initialized: bool,
}

impl GpsSensor {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    pub async fn init(&mut self) -> Result<(), &'static str> {
        info!("Initializing GPS (uBlox MAX-M10M)");
        Timer::after(Duration::from_millis(100)).await;

        // TODO: Initialize GPS via UART
        self.initialized = true;
        info!("GPS initialized successfully");
        Ok(())
    }

    pub async fn read(&mut self) -> Result<(f64, f64, f32, u8, bool), &'static str> {
        if !self.initialized {
            return Err("GPS not initialized");
        }

        // TODO: Read actual GPS data via UART
        // Returns (latitude, longitude, altitude, num_satellites, valid)
        Ok((0.0, 0.0, 0.0, 0, false))
    }
}

/// Main sensor manager
pub struct SensorManager<I2C> {
    pub imu: ImuSensor<I2C>,
    pub altimeter: AltimeterSensor<I2C>,
    pub gps: GpsSensor,
}

impl<I2C> SensorManager<I2C>
where
    I2C: I2c,
{
    pub fn new() -> Self {
        Self {
            imu: ImuSensor::new(),
            altimeter: AltimeterSensor::new(),
            gps: GpsSensor::new(),
        }
    }

    pub async fn init(&mut self, i2c: &mut I2C) -> Result<(), &'static str> {
        info!("Initializing all sensors");

        self.imu.init(i2c).await?;
        self.altimeter.init(i2c).await?;
        self.gps.init().await?;

        // Calibrate altimeter to ground level
        self.altimeter.calibrate_ground_level(i2c).await?;

        info!("All sensors initialized successfully");
        Ok(())
    }

    pub async fn read_all(&mut self, i2c: &mut I2C) -> SensorData {
        let mut data = SensorData::default();

        // Read IMU
        if let Ok((gx, gy, gz, ax, ay, az)) = self.imu.read(i2c).await {
            data.gyro_x = gx;
            data.gyro_y = gy;
            data.gyro_z = gz;
            data.accel_x = ax;
            data.accel_y = ay;
            data.accel_z = az;
        } else {
            warn!("Failed to read IMU");
        }

        // Read Altimeter
        if let Ok((alt, press, temp)) = self.altimeter.read(i2c).await {
            data.altitude = alt;
            data.pressure = press;
            data.temperature = temp;
        } else {
            warn!("Failed to read altimeter");
        }

        // Read GPS
        if let Ok((lat, lon, gps_alt, sats, valid)) = self.gps.read().await {
            data.latitude = lat;
            data.longitude = lon;
            data.gps_altitude = gps_alt;
            data.satellites = sats;
            data.gps_valid = valid;
        } else {
            warn!("Failed to read GPS");
        }

        data
    }
}
