use anyhow::{Result, bail};
use smol::Timer;
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "android"))]
use async_gpiod::{Chip, LineId, Lines, Options, Output};

/// Time to hold ON_OFF high to ensure valve movement (default 3 seconds)
const VALVE_ACTUATION_TIME: Duration = Duration::from_secs(3);

pub struct BallValve {
    name: String,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    on_off_line: Lines<Output>,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    signal_line: Lines<Output>,
}

impl BallValve {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new(
        chip_on_off: &Chip, 
        pin_on_off: LineId, 
        chip_signal: &Chip, 
        pin_signal: LineId, 
        name: &str
    ) -> Result<Self> {
        // Initialize ON_OFF line (start LOW/OFF)
        let opts_on_off = Options::output([pin_on_off])
            .values([false])
            .consumer(format!("{}-on-off", name));
        let on_off_line = chip_on_off.request_lines(opts_on_off).await?;

        // Initialize Signal line (start LOW/CLOSED probably, but we don't know current state. Start LOW to be safe)
        let opts_signal = Options::output([pin_signal])
            .values([false])
            .consumer(format!("{}-signal", name));
        let signal_line = chip_signal.request_lines(opts_signal).await?;
        
        Ok(Self {
            name: name.to_string(),
            on_off_line,
            signal_line,
        })
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new(_chip_on_off: &(), _pin_on_off: u32, _chip_signal: &(), _pin_signal: u32, name: &str) -> Result<Self> {
        Ok(Self {
            name: name.to_string(),
        })
    }

    /// Open sequence: Signal=HIGH, ON_OFF=HIGH (wait), ON_OFF=LOW
    pub async fn open_sequence(&self) -> Result<()> {
        // 1. Ensure Signal is HIGH (Open)
        // We can only change signal if ON_OFF is currently low, which we assume/enforce.
        // But here we are IN the sequence to actuate, so we set signal first.
        self.set_signal_internal(true).await?;
        
        // 2. Turn ON_OFF High to start movement
        self.set_on_off_internal(true).await?;
        
        tracing::info!("BallValve {} opening (waiting {:?})", self.name, VALVE_ACTUATION_TIME);
        Timer::after(VALVE_ACTUATION_TIME).await;
        
        // 3. Turn ON_OFF Low to stop/finish
        self.set_on_off_internal(false).await?;
        tracing::info!("BallValve {} open sequence complete", self.name);
        
        Ok(())
    }

    /// Close sequence: Signal=LOW, ON_OFF=HIGH (wait), ON_OFF=LOW
    pub async fn close_sequence(&self) -> Result<()> {
        // 1. Ensure Signal is LOW (Close)
        self.set_signal_internal(false).await?;
        
        // 2. Turn ON_OFF High
        self.set_on_off_internal(true).await?;
        
        tracing::info!("BallValve {} closing (waiting {:?})", self.name, VALVE_ACTUATION_TIME);
        Timer::after(VALVE_ACTUATION_TIME).await;
        
        // 3. Turn ON_OFF Low
        self.set_on_off_internal(false).await?;
        tracing::info!("BallValve {} close sequence complete", self.name);
        
        Ok(())
    }

    /// Safely set Signal line. Only allowed if ON_OFF is LOW.
    pub async fn set_signal_safe(&self, high: bool) -> Result<()> {
        if self.is_on_off_high().await? {
            bail!("Cannot change Signal while ON_OFF is HIGH");
        }
        self.set_signal_internal(high).await
    }

    /// Directly set ON_OFF line
    pub async fn set_on_off(&self, high: bool) -> Result<()> {
        self.set_on_off_internal(high).await
    }

    // --- Internal Helpers ---

    async fn set_signal_internal(&self, high: bool) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            self.signal_line.set_values([high]).await?;
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            // Mock
            let _ = high; 
        }
        Ok(())
    }

    async fn set_on_off_internal(&self, high: bool) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            self.on_off_line.set_values([high]).await?;
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            // Mock
            let _ = high;
        }
        Ok(())
    }
    
    async fn is_on_off_high(&self) -> Result<bool> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let values = self.on_off_line.get_values([false]).await?;
            Ok(*values.get(0).unwrap_or(&false))
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            Ok(false) // Mock: always safe
        }
    }
}
