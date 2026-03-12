use anyhow::{Context, Result};
use std::time::Duration;
use tracing::info;

#[cfg(any(target_os = "linux", target_os = "android"))]
use async_gpiod::{Chip, LineId, Lines, Options, Output};

// Stepping Configuration
const STEP_FREQUENCY_HZ: u32 = 1000; // 1 KHz step rate (max 12 KHz for full-step ISD02)
const HALF_PERIOD_US: u64 = 500; // 500 us HIGH + 500 us LOW = 1 KHz (>> 4 us min pulse)
const ENABLE_WAKE_MS: u64 = 2; // Wait after enable before pulsing (spec: 1 ms min)
const DIR_SETUP_MS: u64 = 5; // DIR must be stable before first STEP rising edge

// Preset step counts and directions (TODO: calibrate on hardware)
pub const QD_OPEN_STEPS: u32 = 200; // 1 full revolution at full-step (200 steps/rev)
pub const QD_CLOSE_STEPS: u32 = 200;
pub const QD_OPEN_DIRECTION: bool = true;
pub const QD_CLOSE_DIRECTION: bool = false;

/// QD Stepper motor controller using ISD02 driver.
/// STEP signal via GPIO bit-bang, DIR and ENA via GPIO.
pub struct QdStepper {
    name: String,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    step_line: Lines<Output>,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    dir_line: Lines<Output>,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    ena_line: Lines<Output>,
}

impl QdStepper {
    /// Initialize the QD stepper component.
    ///
    /// Configures STEP, DIR, and ENA GPIO lines.
    /// ENA is set HIGH (driver enabled) at init. STEP starts LOW.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new(
        chip_step: &Chip,
        pin_step: LineId,
        chip_dir: &Chip,
        pin_dir: LineId,
        chip_ena: &Chip,
        pin_ena: LineId,
        name: &str,
    ) -> Result<Self> {
        // 1. Configure STEP GPIO (output, start LOW)
        let opts_step = Options::output([pin_step])
            .values([false])
            .consumer(format!("{}-step", name));
        let step_line = chip_step.request_lines(opts_step).await
            .context("Failed to request STEP GPIO line")?;

        // 2. Configure DIR GPIO (output, start LOW)
        let opts_dir = Options::output([pin_dir])
            .values([false])
            .consumer(format!("{}-dir", name));
        let dir_line = chip_dir.request_lines(opts_dir).await
            .context("Failed to request DIR GPIO line")?;

        // 3. Configure ENA GPIO (output, start HIGH = driver enabled)
        let opts_ena = Options::output([pin_ena])
            .values([true])
            .consumer(format!("{}-ena", name));
        let ena_line = chip_ena.request_lines(opts_ena).await
            .context("Failed to request ENA GPIO line")?;

        info!(
            "QD '{}' initialized: STEP=pin{}, DIR=pin{}, ENA=pin{} ({}Hz bit-bang)",
            name, pin_step, pin_dir, pin_ena, STEP_FREQUENCY_HZ
        );

        Ok(Self {
            name: name.to_string(),
            step_line,
            dir_line,
            ena_line,
        })
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new(
        _chip_step: &(),
        _pin_step: u32,
        _chip_dir: &(),
        _pin_dir: u32,
        _chip_ena: &(),
        _pin_ena: u32,
        name: &str,
    ) -> Result<Self> {
        info!(
            "QD '{}' mocked for non-Linux platform ({}Hz step rate)",
            name, STEP_FREQUENCY_HZ
        );
        Ok(Self {
            name: name.to_string(),
        })
    }

    /// Move the stepper motor a given number of steps in the specified direction.
    /// Uses GPIO bit-banging for the STEP signal at 1 KHz (500us HIGH + 500us LOW).
    pub async fn move_steps(&self, steps: u32, direction: bool) -> Result<()> {
        info!(
            "QD '{}': moving {} steps, direction={}",
            self.name,
            steps,
            if direction { "OPEN" } else { "CLOSE" }
        );

        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            // Set direction first and let it settle through the opto-coupler
            self.dir_line.set_values([direction]).await
                .context("Failed to set DIR GPIO")?;
            smol::Timer::after(Duration::from_millis(DIR_SETUP_MS)).await;

            // Ensure driver is enabled (ENA HIGH), then wait for wake
            self.ena_line.set_values([true]).await
                .context("Failed to set ENA GPIO")?;
            smol::Timer::after(Duration::from_millis(ENABLE_WAKE_MS)).await;

            // Bit-bang step pulses
            let half_period = Duration::from_micros(HALF_PERIOD_US);
            for _ in 0..steps {
                self.step_line.set_values([true]).await
                    .context("Failed to set STEP HIGH")?;
                smol::Timer::after(half_period).await;
                self.step_line.set_values([false]).await
                    .context("Failed to set STEP LOW")?;
                smol::Timer::after(half_period).await;
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            let _ = (steps, direction);
            info!("[Mock] QD '{}' move complete", self.name);
        }

        info!("QD '{}': move complete ({} steps)", self.name, steps);
        Ok(())
    }

    /// Read current ENA (enabled) state. Returns true if driver is enabled.
    pub async fn is_enabled(&self) -> bool {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            self.ena_line.get_values([false]).await
                .map(|v| *v.get(0).unwrap_or(&false))
                .unwrap_or(false)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            false
        }
    }

    /// Read current DIR state. Returns true if direction is set to "open".
    pub async fn get_direction(&self) -> bool {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            self.dir_line.get_values([false]).await
                .map(|v| *v.get(0).unwrap_or(&false))
                .unwrap_or(false)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            false
        }
    }
}
