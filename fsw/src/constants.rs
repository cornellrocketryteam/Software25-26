/// Flight software constants
/// Based on Cornell Rocketry Team Flight-Software24-25

/// Cycle time in milliseconds - main loop update rate
pub const CYCLE_TIME_MS: u64 = 100;

/// Altitude thresholds (in meters)
pub const APOGEE_THRESHOLD_M: f32 = 10.0; // Minimum altitude change to detect apogee
pub const MAIN_DEPLOY_ALTITUDE_M: f32 = 200.0; // Main parachute deployment altitude (AGL)
pub const GROUND_ALTITUDE_M: f32 = 50.0; // Ground detection threshold

/// State transition timing
pub const STARTUP_DURATION_MS: u64 = 5000; // Time in startup before going to standby
pub const ASCENT_DETECT_ACCEL_G: f32 = 2.0; // Acceleration threshold for liftoff detection (in G's)
pub const LANDED_VELOCITY_THRESHOLD: f32 = 1.0; // Vertical velocity threshold for landing (m/s)

/// Sensor configuration
pub const IMU_SAMPLE_RATE_HZ: u16 = 100;
pub const BMP390_SAMPLE_RATE_HZ: u16 = 50;
pub const GPS_BAUD_RATE: u32 = 9600;

/// Radio configuration
pub const RADIO_BAUD_RATE: u32 = 57600;
pub const TELEMETRY_INTERVAL_MS: u64 = 500; // How often to send telemetry

/// Watchdog timeout
pub const WATCHDOG_TIMEOUT_MS: u32 = 1000;
