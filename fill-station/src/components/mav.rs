use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

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

        // 2. Set Period to 330 Hz (3,030,303 ns)
        // Note: Period must be set before duty cycle if current duty_cycle > new period? 
        // Best practice: Disable, set period, set duty, enable.
        mav.set_enable(false).await?;
        mav.write_file("period", "3030303").await.context("Failed to set period")?;
        
        // 3. Initialize to Neutral (1500 us -> 1,500,000 ns)
        mav.set_pulse_width_us(1500).await?;

        // 4. Enable
        mav.set_enable(true).await?;
        
        info!("MAV '{}' initialized on PWM {}/{}", name, chip_nr, channel_nr);

        Ok(mav)
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new(chip_nr: u32, channel_nr: u32, name: &str) -> Result<Self> {
        info!("MAV '{}' mocked for non-Linux platform", name);
        Ok(Self {
            name: name.to_string(),
            chip_nr,
            channel_nr,
            pwm_path: PathBuf::from("/tmp/mock_mav"),
        })
    }

    #[allow(dead_code)] // Internal use for ramping
    async fn get_current_pulse_width_us(&self) -> Result<u32> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
             let content = fs::read_to_string(self.pwm_path.join("duty_cycle")).context("Failed to read duty cycle")?;
             let ns: u32 = content.trim().parse().context("Failed to parse duty cycle")?;
             Ok(ns / 1000)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            Ok(1500) // Mock default
        }
    }

    /// Set pulse width with safe ramping
    async fn set_pulse_width_ramped(&self, target_us: u32) -> Result<()> {
         let current_us = self.get_current_pulse_width_us().await.unwrap_or(1500); // Default to neutral if read fails
         
         // If difference is large, ramp
         if (current_us as i32 - target_us as i32).abs() > 100 {
             let step = 50; // Microseconds per step
             let delay = std::time::Duration::from_millis(20); // 20ms per step
             
             let mut current = current_us as i32;
             let target = target_us as i32;
             
             while (current - target).abs() > step {
                 if current < target {
                     current += step;
                 } else {
                     current -= step;
                 }
                 self.set_pulse_width_us(current as u32).await?;
                 smol::Timer::after(delay).await;
             }
         }
         
         // Set final target
         self.set_pulse_width_us(target_us).await
    }

    /// Set pulse width in microseconds
    /// Limits: 800us to 2200us
    pub async fn set_pulse_width_us(&self, us: u32) -> Result<()> {
        if us < 800 || us > 2200 {
            warn!("MAV '{}' requested pulse width {} us out of safe range (800-2200)", self.name, us);
            return Ok(()); // Ignore unsafe commands
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

    /// Open valve (90 degrees, 2000 us)
    pub async fn open(&self) -> Result<()> {
        // 90 degrees -> 2000 us
        self.set_pulse_width_ramped(2000).await
    }

    /// Close valve (0 degrees, 1000 us)
    pub async fn close(&self) -> Result<()> {
        // 0 degrees -> 1000 us
        self.set_pulse_width_ramped(1000).await
    }

    /// Set valve to neutral position (1500 us)
    pub async fn neutral(&self) -> Result<()> {
        self.set_pulse_width_ramped(1500).await
    }

    /// Set specific angle (0-90 degrees)
    /// Maps 0-90 to 1000-2000 us
    pub async fn set_angle(&self, angle: f32) -> Result<()> {
        // Clamp angle to 0-90
        let angle = angle.max(0.0).min(90.0);
        
        // Map 0-90 -> 1000-2000
        let us = 1000.0 + (angle * (1000.0 / 90.0));
        
        self.set_pulse_width_ramped(us as u32).await
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
}
