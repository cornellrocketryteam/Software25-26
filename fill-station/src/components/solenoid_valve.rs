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
        // All valves start CLOSED for safety.
        // NC: LOW = closed (de-energized, natural state)
        // NO: HIGH = closed (energized to hold shut)
        let default_level = match line_pull {
            LinePull::NormallyClosed => false, // LOW = closed
            LinePull::NormallyOpen => true,    // HIGH = closed
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

    /// Open or close the valve.
    /// The driver translates the logical open/close into the correct GPIO level
    /// based on the valve's NO/NC configuration:
    /// - NC: open = HIGH (energize), close = LOW (de-energize)
    /// - NO: open = LOW (de-energize), close = HIGH (energize)
    pub async fn set_open(&self, open: bool) -> Result<()> {
        let level = match self.line_pull {
            LinePull::NormallyClosed => open,   // open -> HIGH, close -> LOW
            LinePull::NormallyOpen => !open,    // open -> LOW, close -> HIGH
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

    /// Returns true if the valve is currently open.
    pub async fn is_open(&self) -> Result<bool> {
        let level = self.current_level.load(Ordering::Relaxed);

        match self.line_pull {
            LinePull::NormallyClosed => Ok(level),  // HIGH = open
            LinePull::NormallyOpen => Ok(!level),   // LOW = open
        }
    }
}
