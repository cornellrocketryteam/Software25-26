use anyhow::Result;
use std::sync::Arc;

#[cfg(any(target_os = "linux", target_os = "android"))]
use async_gpiod::Chip;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::components::igniter::Igniter;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::components::solenoid_valve::{SolenoidValve, LinePull};

use crate::components::ads1015::Ads1015;
use crate::components::ball_valve::BallValve;
use crate::components::qd_stepper::QdStepper;

const GPIO_CHIP0: &str = "gpiochip1";
const GPIO_CHIP1: &str = "gpiochip2";
const I2C_BUS: &str = "/dev/i2c-2";
const ADC1_ADDRESS: u16 = 0x48;
const ADC2_ADDRESS: u16 = 0x49;

/// Hardware actuators only — the two ADCs are owned by the dedicated
/// sampler thread (see `spawn_adc_sampler` in `main.rs`).
pub struct Hardware {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub ig1: Arc<Igniter>,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub ig2: Arc<Igniter>,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub sv1: Arc<SolenoidValve>,
    pub ball_valve: Arc<BallValve>,
    pub qd_stepper: Arc<QdStepper>,
}

impl Hardware {
    /// Initialize all hardware. Returns the actuator container plus the two
    /// ADCs as separate values so the caller can hand the ADCs to a
    /// dedicated sampler thread that owns them outright (no shared mutex).
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new() -> Result<(Self, Ads1015, Ads1015)> {
        let chip0 = Chip::new(GPIO_CHIP0).await?;
        let chip1 = Chip::new(GPIO_CHIP1).await?;
        let ig1 = Arc::new(Igniter::new(&chip0, 39, &chip0, 38).await?); // 38 is signal, 39 is continuity
        let ig2 = Arc::new(Igniter::new(&chip1, 42, &chip0, 40).await?); // 42 is continuity on chip 1, 40 is signal on chip 0

        let adc1 = Ads1015::new(I2C_BUS, ADC1_ADDRESS)?;
        let adc2 = Ads1015::new(I2C_BUS, ADC2_ADDRESS)?;

        // SV1 (Normally Closed)
        let sv1 = Arc::new(SolenoidValve::new(
            &chip0, 42, // pin to actuate
            &chip1, 51, // pin to sense
            LinePull::NormallyClosed
        ).await?);

        // Ball Valve
        // Signal: Chip 1, Line 62
        // ON_OFF: Chip 1, Line 63
        let ball_valve = Arc::new(BallValve::new(
            &chip1, 63, // ON_OFF Pin
            &chip1, 62, // Signal Pin
            "BallValve"
        ).await?);

        // QD Stepper (STEP via GPIO bit-bang, DIR/ENA via GPIO)
        let qd_stepper = Arc::new(QdStepper::new(
            &chip1, 58,   // STEP: gpiochip2, line 58 (GPIO1_58, Pull Down)
            &chip1, 43,   // DIR:  gpiochip2, line 43 (GPIO1_43, Pull Down)
            &chip1, 64,   // ENA:  gpiochip2, line 64 (GPIO1_64, No Pull)
            "QD"
        ).await?);

        Ok((Self { ig1, ig2, sv1, ball_valve, qd_stepper }, adc1, adc2))
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new() -> Result<(Self, Ads1015, Ads1015)> {
        let adc1 = Ads1015::new(I2C_BUS, ADC1_ADDRESS)?;
        let adc2 = Ads1015::new(I2C_BUS, ADC2_ADDRESS)?;
        let ball_valve = Arc::new(BallValve::new(&(), 0, &(), 0, "BallValve").await?);
        let qd_stepper = Arc::new(QdStepper::new(&(), 0, &(), 0, &(), 0, "QD").await?);

        Ok((Self { ball_valve, qd_stepper }, adc1, adc2))
    }
}
