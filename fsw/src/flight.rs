/// Flight control module implementing the state machine
/// Based on Cornell Rocketry Team Flight-Software24-25

use defmt::*;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_async::i2c::I2c;

use crate::actuator::ActuatorManager;
use crate::constants::*;
use crate::sensor::SensorManager;
use crate::state::{FlightState, SensorData};
use crate::telem::TelemetryManager;

pub struct FlightController<'a, I2C> {
    state: FlightState,
    sensors: SensorManager<I2C>,
    telemetry: TelemetryManager,
    actuators: ActuatorManager<'a>,

    // State tracking
    state_start_time: Instant,
    max_altitude: f32,
    ground_altitude: f32,
    last_altitude: f32,

    // Timing
    cycle_start: Instant,
}

impl<'a, I2C> FlightController<'a, I2C>
where
    I2C: I2c,
{
    pub fn new() -> Self {
        Self {
            state: FlightState::Startup,
            sensors: SensorManager::new(),
            telemetry: TelemetryManager::new(),
            actuators: ActuatorManager::new(),
            state_start_time: Instant::now(),
            max_altitude: 0.0,
            ground_altitude: 0.0,
            last_altitude: 0.0,
            cycle_start: Instant::now(),
        }
    }

    pub async fn init(&mut self, i2c: &mut I2C) -> Result<(), &'static str> {
        info!("Initializing Flight Controller");

        // Initialize subsystems
        self.sensors.init(i2c).await?;
        self.telemetry.init().await?;

        // Get ground altitude reference
        let sensor_data = self.sensors.read_all(i2c).await;
        self.ground_altitude = sensor_data.altitude;
        self.last_altitude = sensor_data.altitude;

        info!("Flight Controller initialized - Ground altitude: {}m", self.ground_altitude as i32);
        Ok(())
    }

    pub fn init_actuators(
        &mut self,
        drogue_pin: embassy_rp::gpio::Output<'a>,
        main_pin: embassy_rp::gpio::Output<'a>,
    ) {
        self.actuators.init(drogue_pin, main_pin);
    }

    /// Main execution loop
    pub async fn execute(&mut self, i2c: &mut I2C) {
        self.cycle_start = Instant::now();

        // Read all sensors
        let sensor_data = self.sensors.read_all(i2c).await;

        // Update state machine
        self.update_state(&sensor_data);

        // Send telemetry if needed
        if self.telemetry.should_send() {
            let cycle_duration = Instant::now().duration_since(self.cycle_start).as_millis();
            let _ = self.telemetry.send_telemetry(self.state, &sensor_data, cycle_duration).await;
        }

        // Maintain cycle timing
        let elapsed = Instant::now().duration_since(self.cycle_start);
        if elapsed.as_millis() < CYCLE_TIME_MS {
            let sleep_duration = CYCLE_TIME_MS - elapsed.as_millis();
            Timer::after(Duration::from_millis(sleep_duration)).await;
        } else {
            warn!("Cycle overrun: {}ms", elapsed.as_millis());
        }
    }

    /// Update the flight state machine
    fn update_state(&mut self, sensor_data: &SensorData) {
        let current_altitude_agl = sensor_data.altitude - self.ground_altitude;
        let time_in_state = Instant::now().duration_since(self.state_start_time).as_millis();

        // Update max altitude
        if current_altitude_agl > self.max_altitude {
            self.max_altitude = current_altitude_agl;
        }

        // Calculate vertical velocity (simple derivative)
        let vertical_velocity = (sensor_data.altitude - self.last_altitude) / (CYCLE_TIME_MS as f32 / 1000.0);
        self.last_altitude = sensor_data.altitude;

        // State machine logic
        match self.state {
            FlightState::Startup => {
                // Stay in startup for a fixed duration
                if time_in_state >= STARTUP_DURATION_MS {
                    self.transition_to(FlightState::Standby);
                }
            }

            FlightState::Standby => {
                // Detect liftoff by acceleration
                let total_accel = libm::sqrtf(
                    sensor_data.accel_x * sensor_data.accel_x +
                    sensor_data.accel_y * sensor_data.accel_y +
                    sensor_data.accel_z * sensor_data.accel_z
                ) / 9.81; // Convert to G's

                if total_accel > ASCENT_DETECT_ACCEL_G {
                    info!("LIFTOFF DETECTED - Acceleration: {}G", (total_accel * 100.0) as i32);
                    self.transition_to(FlightState::Ascent);
                }
            }

            FlightState::Ascent => {
                // Detect apogee (altitude stops increasing and starts decreasing)
                if current_altitude_agl > 100.0 && // Minimum altitude to consider apogee
                   (self.max_altitude - current_altitude_agl) > APOGEE_THRESHOLD_M
                {
                    info!("APOGEE DETECTED - Max altitude: {}m", self.max_altitude as i32);
                    self.actuators.deploy_drogue();
                    self.transition_to(FlightState::DrogueDeployed);
                }
            }

            FlightState::DrogueDeployed => {
                // Deploy main parachute at target altitude
                if current_altitude_agl <= MAIN_DEPLOY_ALTITUDE_M {
                    info!("Main deployment altitude reached: {}m AGL", current_altitude_agl as i32);
                    self.actuators.deploy_main();
                    self.transition_to(FlightState::MainDeployed);
                }
            }

            FlightState::MainDeployed => {
                // Detect landing (low altitude and low velocity)
                if current_altitude_agl < GROUND_ALTITUDE_M &&
                   vertical_velocity.abs() < LANDED_VELOCITY_THRESHOLD
                {
                    info!("LANDING DETECTED");
                    // Stay in MainDeployed state after landing
                }
            }

            FlightState::Fault => {
                // Stay in fault state
                warn!("In fault state");
            }
        }
    }

    /// Transition to a new state
    fn transition_to(&mut self, new_state: FlightState) {
        if self.state != new_state {
            info!("State transition: {} -> {}", self.state.name(), new_state.name());
            self.state = new_state;
            self.state_start_time = Instant::now();
        }
    }

    pub fn get_state(&self) -> FlightState {
        self.state
    }
}
