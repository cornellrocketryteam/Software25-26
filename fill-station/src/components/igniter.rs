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

    pub async fn has_continuity(&self) -> bool {
        *self
            .continuity_line
            .get_values([false])
            .await
            .expect("The GPIO File Descriptor should not be able to close?")
            .get(0)
            .expect("We know one value exists since we are requesting one value")
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
