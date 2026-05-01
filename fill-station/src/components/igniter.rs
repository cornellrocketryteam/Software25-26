use anyhow::Result;
use async_gpiod::{Chip, Input, LineId, Lines, Options, Output};
use smol::Timer;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};

const CONSUMER: &str = "fill-station-igniter";

pub struct Igniter {
    continuity_pin: LineId,
    continuity_line: Lines<Input>,
    signal_pin: LineId,
    signal_line: Lines<Output>,
    firing: AtomicBool,
}

impl Igniter {
    pub async fn new(
        continuity_chip: &Chip,
        continuity_pin: LineId,
        signal_chip: &Chip,
        signal_pin: LineId,
    ) -> Result<Self> {
        let continuity_options = Options::input([continuity_pin]).consumer(CONSUMER);
        let continuity_line = continuity_chip.request_lines(continuity_options).await?;

        let signal_options = Options::output([signal_pin])
            .values([false])
            .consumer(CONSUMER);
        let signal_line = signal_chip.request_lines(signal_options).await?;

        Ok(Self {
            continuity_pin,
            continuity_line,
            signal_pin,
            signal_line,
            firing: AtomicBool::new(false),
        })
    }

    /// Read the continuity sense pin. On any GPIO read error, log and
    /// return `false` rather than panicking — a transient kernel-side
    /// failure during a ground continuity check shouldn't kill the
    /// process mid-fill.
    pub async fn has_continuity(&self) -> bool {
        match self.continuity_line.get_values([false]).await {
            Ok(values) => values.get(0).copied().unwrap_or(false),
            Err(e) => {
                tracing::warn!("Igniter continuity GPIO read failed: {}", e);
                false
            }
        }
    }

    /// Set the ignition state (true = firing, false = off)
    pub async fn set_actuated(&self, enable: bool) -> Result<()> {
        self.signal_line
            .set_values([enable])
            .await?;
        self.firing.store(enable, Ordering::Relaxed);
        Ok(())
    }

    pub async fn is_igniting(&self) -> bool {
        self.firing.load(Ordering::Relaxed)
    }
}
