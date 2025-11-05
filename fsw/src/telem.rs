/// Telemetry module for radio communication
/// Handles RFD900 radio communication via UART

use defmt::*;
use embassy_time::Instant;
use heapless::String;

use crate::state::{FlightState, SensorData};
use crate::constants::TELEMETRY_INTERVAL_MS;

pub struct TelemetryManager {
    last_send_time: Option<Instant>,
    initialized: bool,
}

impl TelemetryManager {
    pub fn new() -> Self {
        Self {
            last_send_time: None,
            initialized: false,
        }
    }

    pub async fn init(&mut self) -> Result<(), &'static str> {
        info!("Initializing Telemetry (RFD900 Radio)");

        // TODO: Initialize UART for radio
        self.initialized = true;
        info!("Telemetry initialized successfully");
        Ok(())
    }

    /// Check if it's time to send telemetry
    pub fn should_send(&self) -> bool {
        if !self.initialized {
            return false;
        }

        match self.last_send_time {
            None => true,
            Some(last_time) => {
                let now = Instant::now();
                let elapsed = now.duration_since(last_time);
                elapsed.as_millis() >= TELEMETRY_INTERVAL_MS
            }
        }
    }

    /// Format and send telemetry data
    pub async fn send_telemetry(
        &mut self,
        state: FlightState,
        sensor_data: &SensorData,
        cycle_time: u64,
    ) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Telemetry not initialized");
        }

        // Format telemetry message (using integer representation for simplicity)
        let mut message: String<512> = String::new();

        use core::fmt::Write;
        core::write!(
            &mut message,
            "STATE:{},ALT:{},TEMP:{},PRESS:{},\
            ACCEL:{}|{}|{},GYRO:{}|{}|{},\
            GPS:{}|{}|{}|{},CYCLE:{}\n",
            state.name(),
            sensor_data.altitude as i32,
            sensor_data.temperature as i32,
            sensor_data.pressure as i32,
            sensor_data.accel_x as i32,
            sensor_data.accel_y as i32,
            sensor_data.accel_z as i32,
            sensor_data.gyro_x as i32,
            sensor_data.gyro_y as i32,
            sensor_data.gyro_z as i32,
            sensor_data.latitude as i32,
            sensor_data.longitude as i32,
            sensor_data.gps_altitude as i32,
            sensor_data.satellites,
            cycle_time,
        ).map_err(|_| "Failed to format message")?;

        // TODO: Send via UART to radio
        info!("TX: {}", message.as_str());

        self.last_send_time = Some(Instant::now());
        Ok(())
    }

    /// Send a simple status message
    pub async fn send_status(&mut self, message: &str) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Telemetry not initialized");
        }

        // TODO: Send via UART to radio
        info!("TX: {}", message);
        Ok(())
    }
}
