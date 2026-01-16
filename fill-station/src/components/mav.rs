use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

// Motor Configuration Constants (DS2685BLHV)
#[allow(dead_code)]
const FREQUENCY_HZ: u32 = 330;
#[allow(dead_code)]
const NEUTRAL_US: u32 = 1300;
#[allow(dead_code)]
const OPEN_90_US: u32 = 1800;
#[allow(dead_code)]
const CLOSE_0_US: u32 = 800;
#[allow(dead_code)]
const MAX_US: u32 = 2200;
#[allow(dead_code)]
const MIN_US: u32 = 800;

// Calculated Period in Nanoseconds
#[allow(dead_code)]
const PERIOD_NS: u32 = 1_000_000_000 / FREQUENCY_HZ;

/// MAV (Mechanically Actuated Valve) component controlling a servo via PWM
pub struct Mav {
    name: String,
    #[allow(dead_code)]
    chip_nr: u32, 
    #[allow(dead_code)]
    channel_nr: u32,
    #[allow(dead_code)]
    pwm_path: PathBuf,
}

impl Mav {
    /// Initialize the MAV component
    ///
    /// This will export the PWM channel, set the period, and enable it.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new(chip_nr: u32, channel_nr: u32, name: &str) -> Result<Self> {
        let chip_path = PathBuf::from(format!("/sys/class/pwm/pwmchip{}", chip_nr));
        let pwm_path = chip_path.join(format!("pwm{}", channel_nr));

        // 1. Export the channel if it doesn't exist
        if !pwm_path.exists() {
            info!("Exporting PWM chip {} channel {}", chip_nr, channel_nr);
            // Use smol::unblock to perform blocking I/O on thread pool
            smol::unblock(move || {
                fs::write(chip_path.join("export"), channel_nr.to_string())
            }).await.context("Failed to export PWM channel")?;
        }

        // Wait a bit for the sysfs entry to appear (kernel race condition sometimes)
        if !pwm_path.exists() {
            smol::Timer::after(std::time::Duration::from_millis(100)).await;
        }

        let mav = Self {
            name: name.to_string(),
            chip_nr,
            channel_nr,
            pwm_path: pwm_path.clone(),
        };

        // 2. Set Period
        mav.set_enable(false).await?;
        mav.write_file("period", &PERIOD_NS.to_string()).await.context("Failed to set period")?;
        
        // 3. Initialize to Neutral
        mav.set_pulse_width_us(NEUTRAL_US).await?;

        // 4. Enable
        mav.set_enable(true).await?;
        
        info!("MAV '{}' initialized on PWM {}/{} (Freq: {} Hz)", name, chip_nr, channel_nr, FREQUENCY_HZ);

        Ok(mav)
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new(chip_nr: u32, channel_nr: u32, name: &str) -> Result<Self> {
        info!("MAV '{}' mocked for non-Linux platform (Freq: {} Hz)", name, FREQUENCY_HZ);
        Ok(Self {
            name: name.to_string(),
            chip_nr,
            channel_nr,
            pwm_path: PathBuf::from("/tmp/mock_mav"),
        })
    }

    /// Set pulse width in microseconds
    pub async fn set_pulse_width_us(&self, us: u32) -> Result<()> {
        if us < MIN_US || us > MAX_US {
            warn!("MAV '{}' requested pulse width {} us out of safe range ({}-{})", 
                  self.name, us, MIN_US, MAX_US);
            return Ok(()); 
        }

        let ns = us * 1000;
        
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            self.write_file("duty_cycle", &ns.to_string()).await.context("Failed to set duty cycle")?;
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            info!("[Mock] MAV '{}' set to {} us", self.name, us);
        }

        Ok(())
    }

    /// Open valve (90 degrees)
    pub async fn open(&self) -> Result<()> {
        self.set_pulse_width_us(OPEN_90_US).await
    }

    /// Close valve (0 degrees)
    pub async fn close(&self) -> Result<()> {
        self.set_pulse_width_us(CLOSE_0_US).await
    }

    /// Set valve to neutral position
    pub async fn neutral(&self) -> Result<()> {
        self.set_pulse_width_us(NEUTRAL_US).await
    }

    /// Set specific angle (0-90 degrees)
    pub async fn set_angle(&self, angle: f32) -> Result<()> {
        // Clamp angle to 0-90
        let angle = angle.max(0.0).min(90.0);
        
        // Map 0-90 -> CLOSE_0_US - OPEN_90_US
        let range_us = (OPEN_90_US as f32) - (CLOSE_0_US as f32);
        let us = (CLOSE_0_US as f32) + (angle * (range_us / 90.0));
        
        self.set_pulse_width_us(us as u32).await
    }

    /// Get current pulse width in microseconds
    pub async fn get_pulse_width_us(&self) -> Result<u32> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let content = self.read_file("duty_cycle").await.context("Failed to read duty cycle")?;
            let ns: u32 = content.trim().parse().context("Failed to parse duty cycle")?;
            Ok(ns / 1000)
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            // Return a dummy value for mock
            Ok(NEUTRAL_US)
        }
    }

    /// Get current angle in degrees
    pub async fn get_angle(&self) -> Result<f32> {
        let us = self.get_pulse_width_us().await?;
        
        let close_0 = CLOSE_0_US as f32;
        let open_90 = OPEN_90_US as f32;
        let range_us = open_90 - close_0;

        if us <= CLOSE_0_US {
            Ok(0.0)
        } else if us >= OPEN_90_US {
            Ok(90.0)
        } else {
            // Angle = (us - close_0) * (90 / range)
            Ok((us as f32 - close_0) * (90.0 / range_us))
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    async fn set_enable(&self, enable: bool) -> Result<()> {
        self.write_file("enable", if enable { "1" } else { "0" }).await.context("Failed to set enable")
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    async fn write_file(&self, file: &str, content: &str) -> Result<()> {
        let path = self.pwm_path.join(file);
        let content = content.to_string();
        smol::unblock(move || fs::write(path, content)).await?;
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    async fn read_file(&self, file: &str) -> Result<String> {
        let path = self.pwm_path.join(file);
        let content = smol::unblock(move || fs::read_to_string(path)).await?;
        Ok(content)
    }
}
