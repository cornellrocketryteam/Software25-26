use anyhow::Result;

#[cfg(any(target_os = "linux", target_os = "android"))]
use async_gpiod::Chip;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::components::igniter::Igniter;

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
        
        Ok(Self { ig1, ig2, adc1, adc2 })
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new() -> Result<Self> {
        let adc1 = Ads1015::new(I2C_BUS, ADC1_ADDRESS)?;
        let adc2 = Ads1015::new(I2C_BUS, ADC2_ADDRESS)?;
        
        Ok(Self { adc1, adc2 })
    }
}
