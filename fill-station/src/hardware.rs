use anyhow::Result;

#[cfg(any(target_os = "linux", target_os = "android"))]
use async_gpiod::Chip;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::components::igniter::Igniter;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::components::solenoid_valve::{SolenoidValve, LinePull};

use crate::components::ads1015::Ads1015;
use crate::components::mav::Mav;
use crate::components::ball_valve::BallValve;
use crate::components::qd_stepper::QdStepper;

const GPIO_CHIP0: &str = "gpiochip1";
const GPIO_CHIP1: &str = "gpiochip2";
const I2C_BUS: &str = "/dev/i2c-2";
const ADC1_ADDRESS: u16 = 0x48;
const ADC2_ADDRESS: u16 = 0x49;

pub struct Hardware {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub ig1: Igniter,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub ig2: Igniter,
    pub adc1: Ads1015,
    pub adc2: Ads1015,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub sv1: SolenoidValve,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub sv2: SolenoidValve,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub sv3: SolenoidValve,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub sv4: SolenoidValve,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub sv5: SolenoidValve,
    pub mav: Mav,
    pub ball_valve: BallValve,
    pub qd_stepper: Option<QdStepper>,
}

impl Hardware {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new() -> Result<Self> {
        let chip0 = Chip::new(GPIO_CHIP0).await?;
        let chip1 = Chip::new(GPIO_CHIP1).await?;
        let ig1 = Igniter::new(&chip0, 39, &chip0, 38).await?; // 38 is signal, 39 is continuity
        let ig2 = Igniter::new(&chip1, 42, &chip0, 40).await?; // 42 is continuity on chip 1, 40 is signal on chip 0
        
        let adc1 = Ads1015::new(I2C_BUS, ADC1_ADDRESS)?;
        let adc2 = Ads1015::new(I2C_BUS, ADC2_ADDRESS)?;

        // SV1
        let sv1 = SolenoidValve::new(
            &chip0, 42, // pin to actuate
            &chip1, 51, // pin to sense
            LinePull::NormallyClosed
        ).await?;

        // SV2
        let sv2 = SolenoidValve::new(
            &chip0, 32,
            &chip0, 35,
            LinePull::NormallyClosed
        ).await?;
        
        // SV3
        let sv3 = SolenoidValve::new(
            &chip1, 44, 
            &chip0, 37,
            LinePull::NormallyClosed
        ).await?;

        // SV4
        let sv4 = SolenoidValve::new(
            &chip0, 41, // Signal4 - GPIO0_41 (R19)
            &chip0, 36, // GPIO4 - GPIO0_36 (T19)
            LinePull::NormallyClosed
        ).await?;

        // SV5
        let sv5 = SolenoidValve::new(
            &chip1, 48,
            &chip1, 46,
            LinePull::NormallyClosed
        ).await?;

        // MAV (Chip 0, Channel 1 for B)
        let mav = Mav::new(0, 1, "MAV").await?;

        // Ball Valve
        // Signal: Chip 1, Line 62
        // ON_OFF: Chip 1, Line 63
        let ball_valve = BallValve::new(
            &chip1, 63, // ON_OFF Pin
            &chip1, 62, // Signal Pin
            "BallValve"
        ).await?;

        // QD Stepper (STEP via GPIO bit-bang, DIR/ENA via GPIO)
        // Non-fatal: if QD init fails, the rest of the system still works
        let qd_stepper = match QdStepper::new(
            &chip0, 33,   // STEP: gpiochip1 (SoC GPIO0), line 33
            &chip1, 43,   // DIR:  gpiochip2 (SoC GPIO1), line 43
            &chip1, 64,   // ENA:  gpiochip2 (SoC GPIO1), line 64
            "QD"
        ).await {
            Ok(qd) => Some(qd),
            Err(e) => {
                warn!("QD Stepper init failed (non-fatal): {}", e);
                None
            }
        };

        Ok(Self { ig1, ig2, adc1, adc2, sv1, sv2, sv3, sv4, sv5, mav, ball_valve, qd_stepper })
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new() -> Result<Self> {
        let adc1 = Ads1015::new(I2C_BUS, ADC1_ADDRESS)?;
        let adc2 = Ads1015::new(I2C_BUS, ADC2_ADDRESS)?;
        let mav = Mav::new(0, 0, "MAV").await?;
        let ball_valve = BallValve::new(&(), 0, &(), 0, "BallValve").await?;
        let qd_stepper = QdStepper::new(&(), 0, &(), 0, &(), 0, "QD").await.ok();

        Ok(Self { adc1, adc2, mav, ball_valve, qd_stepper })
    }
}
