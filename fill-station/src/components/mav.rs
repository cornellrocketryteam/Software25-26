use anyhow::Result;
#[cfg(any(target_os = "linux", target_os = "android"))]
use anyhow::{bail, Context};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use tracing::{info, warn};

// Servo constants mirror the FSW MAV driver (`fsw/src/actuator.rs` `Mav`) so
// both MAVs travel the same distance. Keep the two in sync.
// Servo: ProModeler DS2685BLHV.
const FREQUENCY_HZ: u32 = 330;
const MIN_US: u32 = 800;
const MAX_US: u32 = 2200;
pub const OPEN_US: u32 = 1950;
pub const CLOSE_US: u32 = 883;

#[cfg(any(target_os = "linux", target_os = "android"))]
const PERIOD_NS: u32 = 1_000_000_000 / FREQUENCY_HZ;

/// MAV (Mechanically Actuated Valve) servo driven by a sysfs PWM channel.
pub struct Mav {
    name: String,
    #[allow(dead_code)]
    pwm_path: PathBuf,
    state_open: AtomicBool,
    pulse_us: AtomicU32,
    /// Bumped on every open/close so a pending timed auto-close can tell
    /// whether it has been superseded.
    generation: AtomicU64,
}

impl Mav {
    /// Initialize the MAV on the PWM controller whose platform device name is
    /// `device_addr` (e.g. "23040000.pwm" for epwm4). The pwmchip number is
    /// assigned by probe order, so it is looked up rather than hardcoded.
    ///
    /// Exports the channel, sets the 330 Hz period, closes the valve and enables output.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new(device_addr: &str, channel_nr: u32, name: &str) -> Result<Self> {
        let chip_path = find_pwmchip(device_addr)?;
        let pwm_path = chip_path.join(format!("pwm{}", channel_nr));

        if !pwm_path.exists() {
            info!("Exporting PWM {} channel {}", chip_path.display(), channel_nr);
            let export = chip_path.join("export");
            smol::unblock(move || fs::write(export, channel_nr.to_string()))
                .await
                .context("Failed to export PWM channel")?;
        }

        // The sysfs entry can take a moment to appear after export
        if !pwm_path.exists() {
            smol::Timer::after(std::time::Duration::from_millis(100)).await;
        }

        let mav = Self {
            name: name.to_string(),
            pwm_path,
            state_open: AtomicBool::new(false),
            pulse_us: AtomicU32::new(CLOSE_US),
            generation: AtomicU64::new(0),
        };

        mav.set_enable(false).await?;
        mav.write_file("period", &PERIOD_NS.to_string()).await.context("Failed to set period")?;
        mav.set_pulse_width_us(CLOSE_US).await?;
        mav.set_enable(true).await?;

        info!("MAV '{}' initialized on {} (Freq: {} Hz, closed at {} us)",
              name, mav.pwm_path.display(), FREQUENCY_HZ, CLOSE_US);

        Ok(mav)
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new(device_addr: &str, channel_nr: u32, name: &str) -> Result<Self> {
        info!("MAV '{}' mocked for non-Linux platform ({} ch {}, Freq: {} Hz)",
              name, device_addr, channel_nr, FREQUENCY_HZ);
        Ok(Self {
            name: name.to_string(),
            pwm_path: PathBuf::from("/tmp/mock_mav"),
            state_open: AtomicBool::new(false),
            pulse_us: AtomicU32::new(CLOSE_US),
            generation: AtomicU64::new(0),
        })
    }

    /// Set pulse width in microseconds, clamped to MIN_US..MAX_US (same as FSW).
    pub async fn set_pulse_width_us(&self, us: u32) -> Result<()> {
        let clamped = us.clamp(MIN_US, MAX_US);
        if clamped != us {
            warn!("MAV '{}' pulse width {} us clamped to {} us", self.name, us, clamped);
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let ns = clamped * 1000;
            self.write_file("duty_cycle", &ns.to_string()).await.context("Failed to set duty cycle")?;
        }

        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            info!("[Mock] MAV '{}' set to {} us", self.name, clamped);
        }

        self.pulse_us.store(clamped, Ordering::Relaxed);
        Ok(())
    }

    /// Open valve to OPEN_US. Returns the generation of this open, which a
    /// timed auto-close passes to `close_if_generation`.
    pub async fn open(&self) -> Result<u64> {
        self.set_pulse_width_us(OPEN_US).await?;
        self.state_open.store(true, Ordering::Relaxed);
        Ok(self.generation.fetch_add(1, Ordering::SeqCst) + 1)
    }

    /// Close valve to CLOSE_US. Cancels any pending timed auto-close.
    pub async fn close(&self) -> Result<()> {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.set_pulse_width_us(CLOSE_US).await?;
        self.state_open.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Close only if no open/close has happened since the open that returned `generation`.
    pub async fn close_if_generation(&self, generation: u64) -> Result<bool> {
        if self.generation.load(Ordering::SeqCst) != generation {
            return Ok(false);
        }
        self.close().await?;
        Ok(true)
    }

    pub fn is_open(&self) -> bool {
        self.state_open.load(Ordering::Relaxed)
    }

    /// Last commanded pulse width in microseconds.
    pub fn pulse_width_us(&self) -> u32 {
        self.pulse_us.load(Ordering::Relaxed)
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

/// Find `/sys/class/pwm/pwmchipN` whose `device` symlink points at `device_addr`.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn find_pwmchip(device_addr: &str) -> Result<PathBuf> {
    for entry in fs::read_dir("/sys/class/pwm").context("Failed to read /sys/class/pwm")? {
        let path = entry?.path();
        if let Ok(target) = fs::read_link(path.join("device")) {
            if target.file_name().map_or(false, |n| n == device_addr) {
                return Ok(path);
            }
        }
    }
    bail!("No pwmchip found for device {}", device_addr)
}
