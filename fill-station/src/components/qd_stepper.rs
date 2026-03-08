use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

#[cfg(any(target_os = "linux", target_os = "android"))]
use async_gpiod::{Chip, LineId, Lines, Options, Output};

// Stepping Configuration
const STEP_FREQUENCY_HZ: u32 = 1000; // 1 KHz step rate (max 12 KHz for full-step ISD02)
const PERIOD_NS: u32 = 1_000_000_000 / STEP_FREQUENCY_HZ;
const DUTY_CYCLE_NS: u32 = PERIOD_NS / 2; // 50% duty cycle (500 us >> 4 us min pulse)
const ENABLE_WAKE_MS: u64 = 2; // Wait after enable before pulsing (spec: 1 ms min)

// Preset step counts and directions (TODO: calibrate on hardware)
pub const QD_OPEN_STEPS: u32 = 200; // 1 full revolution at full-step (200 steps/rev)
pub const QD_CLOSE_STEPS: u32 = 200;
pub const QD_OPEN_DIRECTION: bool = true;
pub const QD_CLOSE_DIRECTION: bool = false;

/// QD Stepper motor controller using ISD02 driver.
/// STEP signal via PWM sysfs, DIR and ENA via GPIO.
pub struct QdStepper {
    name: String,
    #[allow(dead_code)]
    chip_nr: u32,
    #[allow(dead_code)]
    channel_nr: u32,
    #[allow(dead_code)]
    pwm_path: PathBuf,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    dir_line: Lines<Output>,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    ena_line: Lines<Output>,
}

impl QdStepper {
    /// Initialize the QD stepper component.
    ///
    /// Exports PWM channel for STEP signal, configures DIR and ENA GPIO lines.
    /// ENA is set HIGH (driver enabled) at init.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new(
        chip_nr: u32,
        channel_nr: u32,
        chip_dir: &Chip,
        pin_dir: LineId,
        chip_ena: &Chip,
        pin_ena: LineId,
        name: &str,
    ) -> Result<Self> {
        let chip_path = PathBuf::from(format!("/sys/class/pwm/pwmchip{}", chip_nr));
        let pwm_path = chip_path.join(format!("pwm{}", channel_nr));

        // 1. Export the PWM channel if it doesn't exist
        if !pwm_path.exists() {
            info!("QD '{}': Exporting PWM chip {} channel {}", name, chip_nr, channel_nr);
            let export_path = chip_path.join("export");
            let ch = channel_nr.to_string();
            smol::unblock(move || fs::write(export_path, ch))
                .await
                .context("Failed to export PWM channel for QD stepper")?;
        }

        // Wait for sysfs entry to appear
        if !pwm_path.exists() {
            smol::Timer::after(std::time::Duration::from_millis(100)).await;
        }

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

        let stepper = Self {
            name: name.to_string(),
            chip_nr,
            channel_nr,
            pwm_path: pwm_path.clone(),
            dir_line,
            ena_line,
        };

        // 4. Configure PWM: disable first, set period, ensure duty=0 (no stepping)
        stepper.set_enable_pwm(false).await?;
        stepper.write_file("period", &PERIOD_NS.to_string()).await
            .context("Failed to set PWM period")?;
        stepper.write_file("duty_cycle", "0").await
            .context("Failed to set initial duty cycle")?;

        info!(
            "QD '{}' initialized: PWM {}/{} ({}Hz), DIR=pin{}, ENA=pin{}",
            name, chip_nr, channel_nr, STEP_FREQUENCY_HZ, pin_dir, pin_ena
        );

        Ok(stepper)
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new(
        chip_nr: u32,
        channel_nr: u32,
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
            chip_nr,
            channel_nr,
            pwm_path: PathBuf::from("/tmp/mock_qd_stepper"),
        })
    }

    /// Begin stepping in the given direction.
    /// Sets DIR GPIO, ensures ENA is high, waits for driver wake, then starts PWM.
    /// Call `stop_stepping()` after the desired duration to halt.
    pub async fn begin_stepping(&self, direction: bool) -> Result<()> {
        info!(
            "QD '{}': begin stepping, direction={}",
            self.name,
            if direction { "OPEN" } else { "CLOSE" }
        );

        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            // Set direction
            self.dir_line.set_values([direction]).await
                .context("Failed to set DIR GPIO")?;

            // Ensure driver is enabled (ENA HIGH)
            self.ena_line.set_values([true]).await
                .context("Failed to set ENA GPIO")?;

            // Wait for driver to wake from possible idle/shutdown
            smol::Timer::after(std::time::Duration::from_millis(ENABLE_WAKE_MS)).await;

            // Start PWM with 50% duty cycle
            self.write_file("duty_cycle", &DUTY_CYCLE_NS.to_string()).await
                .context("Failed to set duty cycle")?;
            self.set_enable_pwm(true).await?;
        }

        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            let _ = direction;
            info!("[Mock] QD '{}' stepping started", self.name);
        }

        Ok(())
    }

    /// Stop stepping by disabling PWM output.
    pub async fn stop_stepping(&self) -> Result<()> {
        info!("QD '{}': stop stepping", self.name);

        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            // Set duty to 0 then disable PWM
            self.write_file("duty_cycle", "0").await
                .context("Failed to clear duty cycle")?;
            self.set_enable_pwm(false).await?;
        }

        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            info!("[Mock] QD '{}' stepping stopped", self.name);
        }

        Ok(())
    }

    /// Compute how long to run PWM for a given number of steps (milliseconds).
    pub fn step_duration_ms(steps: u32) -> u64 {
        (steps as u64 * 1000) / STEP_FREQUENCY_HZ as u64
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

    // --- Private helpers ---

    #[cfg(any(target_os = "linux", target_os = "android"))]
    async fn set_enable_pwm(&self, enable: bool) -> Result<()> {
        self.write_file("enable", if enable { "1" } else { "0" })
            .await
            .context("Failed to set PWM enable")
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    async fn write_file(&self, file: &str, content: &str) -> Result<()> {
        let path = self.pwm_path.join(file);
        let content = content.to_string();
        smol::unblock(move || fs::write(path, content)).await?;
        Ok(())
    }
}
