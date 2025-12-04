use anyhow::Result;
use async_gpiod::Chip;

use crate::components::igniter::Igniter;

const GPIO_CHIP: &str = "gpiochip1";

pub struct Hardware {
    pub ig1: Igniter,
    pub ig2: Igniter,
}

impl Hardware {
    pub async fn new() -> Result<Self> {
        let chip = Chip::new(GPIO_CHIP).await?;
        let ig1 = Igniter::new(&chip, 18, 16).await?;
        let ig2 = Igniter::new(&chip, 24, 22).await?;
        Ok(Self { ig1, ig2 })
    }
}
