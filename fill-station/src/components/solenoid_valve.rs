use anyhow::Result;
use async_gpiod::{Chip, Input, LineId, Lines, Options, Output};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

const CONSUMER: &str = "fill-station-solenoid";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinePull {
    NormallyOpen,
    NormallyClosed,
}

pub struct SolenoidValve {
    control_line: Lines<Output>,
    control_pin: LineId,
    signal_line: Lines<Input>,
    signal_pin: LineId,
    line_pull: LinePull,
    current_level: AtomicBool,
}

impl SolenoidValve {
    pub async fn new(
        control_chip: &Chip,
        control_pin: LineId,
        signal_chip: &Chip,
        signal_pin: LineId,
        line_pull: LinePull,
    ) -> Result<Self> {
        let default_level = match line_pull {
            LinePull::NormallyOpen => true,   // Default HIGH (Actuated = LOW)
            LinePull::NormallyClosed => false, // Default LOW (Actuated = HIGH)
        };

        let control_options = Options::output([control_pin])
            .values([default_level])
            .consumer(CONSUMER);
        let control_line = control_chip.request_lines(control_options).await?;

        let signal_options = Options::input([signal_pin]).consumer(CONSUMER);
        let signal_line = signal_chip.request_lines(signal_options).await?;

        Ok(Self {
            control_line,
            control_pin,
            signal_line,
            signal_pin,
            line_pull,
            current_level: AtomicBool::new(default_level),
        })
    }

    /// Actuate the valve.
    /// If enable is true, the valve moves to the "actuated" state.
    /// Actuated state depends on line_pull:
    /// - NC: Actuated = HIGH
    /// - NO: Actuated = LOW
    pub async fn actuate(&self, enable: bool) -> Result<()> {
        let level = match self.line_pull {
            LinePull::NormallyClosed => enable,          // Actuate -> HIGH
            LinePull::NormallyOpen => !enable,           // Actuate -> LOW
        };

        self.control_line.set_values([level]).await?;
        self.current_level.store(level, Ordering::Relaxed);
        Ok(())
    }

    /// Check continuity signal.
    /// Returns true if signal is HIGH (connected).
    pub async fn check_continuity(&self) -> Result<bool> {
        let values = self.signal_line.get_values([false]).await?;
        Ok(*values.get(0).unwrap_or(&false))
    }
    
    // Helper to get current actuation state (logical)
    pub async fn is_actuated(&self) -> Result<bool> {
        // Use software tracked state instead of reading back hardware register
        // which can be unreliable for output pins on some platforms
        let level = self.current_level.load(Ordering::Relaxed);
        
        match self.line_pull {
             LinePull::NormallyClosed => Ok(level),     // HIGH == Actuated
             LinePull::NormallyOpen => Ok(!level),      // LOW == Actuated
        }
    }
}
