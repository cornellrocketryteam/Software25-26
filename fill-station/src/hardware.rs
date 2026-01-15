use anyhow::Result;

#[cfg(any(target_os = "linux", target_os = "android"))]
use async_gpiod::Chip;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::components::igniter::Igniter;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::components::solenoid_valve::{SolenoidValve, LinePull};

use crate::components::ads1015::Ads1015;

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
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub sv2: SolenoidValve,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub sv3: SolenoidValve,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub sv4: SolenoidValve,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub sv5: SolenoidValve,
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

        // SV1: Actuate aka "Control" (Signal Pin/42), Check aka "Signal" (GPIO Pin/51)
        let sv1 = SolenoidValve::new(
            &chip0, 42,
            &chip1, 51,
            LinePull::NormallyClosed
        ).await?;

        // SV2: Actuate (Signal/32), Check (GPIO/34)
        let sv2 = SolenoidValve::new(
            &chip0, 32,
            &chip0, 34,
            LinePull::NormallyClosed
        ).await?;

        // SV3: Actuate (Signal/44), Check (GPIO/37) -- Signal=Chip1, GPIO=Chip0
        let sv3 = SolenoidValve::new(
            &chip1, 44,
            &chip0, 37,
            LinePull::NormallyClosed
        ).await?;

        // SV4: Actuate (Signal/65), Check (GPIO/36) -- Signal=Chip1, GPIO=Chip0
        let sv4 = SolenoidValve::new(
            &chip1, 65,
            &chip0, 36,
            LinePull::NormallyClosed
        ).await?;

        // SV5: Actuate (Signal/48), Check (GPIO/46) -- Signal=Chip1, GPIO=Chip1 -- NO
        let sv5 = SolenoidValve::new(
            &chip1, 48,
            &chip1, 46,
            LinePull::NormallyOpen
        ).await?;
        
        Ok(Self { ig1, ig2, adc1, adc2, sv1, sv2, sv3, sv4, sv5 })
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new() -> Result<Self> {
        let adc1 = Ads1015::new(I2C_BUS, ADC1_ADDRESS)?;
        let adc2 = Ads1015::new(I2C_BUS, ADC2_ADDRESS)?;
        
        Ok(Self { adc1, adc2 })
    }
}
