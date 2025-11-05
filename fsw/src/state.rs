/// Flight states following the reference architecture
/// Based on Cornell Rocketry Team Flight-Software24-25

use defmt::Format;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum FlightState {
    /// Initial startup state - sensor initialization and checks
    Startup,
    /// Standby state - waiting for liftoff
    Standby,
    /// Ascent state - rocket is ascending
    Ascent,
    /// Drogue deployed state - drogue parachute has been deployed
    DrogueDeployed,
    /// Main deployed state - main parachute has been deployed
    MainDeployed,
    /// Fault state - system error detected
    Fault,
}

impl FlightState {
    pub fn name(&self) -> &'static str {
        match self {
            FlightState::Startup => "Startup",
            FlightState::Standby => "Standby",
            FlightState::Ascent => "Ascent",
            FlightState::DrogueDeployed => "DrogueDeployed",
            FlightState::MainDeployed => "MainDeployed",
            FlightState::Fault => "Fault",
        }
    }
}

/// Sensor data structure
#[derive(Debug, Clone, Copy, Format)]
pub struct SensorData {
    /// IMU gyroscope data (rad/s)
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub gyro_z: f32,

    /// IMU accelerometer data (m/s²)
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,

    /// Altimeter data
    pub altitude: f32,        // meters above sea level
    pub pressure: f32,        // Pascals
    pub temperature: f32,     // Celsius

    /// GPS data
    pub latitude: f64,        // degrees
    pub longitude: f64,       // degrees
    pub gps_altitude: f32,    // meters above sea level
    pub satellites: u8,       // number of satellites
    pub gps_valid: bool,      // GPS fix valid
}

impl Default for SensorData {
    fn default() -> Self {
        Self {
            gyro_x: 0.0,
            gyro_y: 0.0,
            gyro_z: 0.0,
            accel_x: 0.0,
            accel_y: 0.0,
            accel_z: 0.0,
            altitude: 0.0,
            pressure: 0.0,
            temperature: 0.0,
            latitude: 0.0,
            longitude: 0.0,
            gps_altitude: 0.0,
            satellites: 0,
            gps_valid: false,
        }
    }
}
