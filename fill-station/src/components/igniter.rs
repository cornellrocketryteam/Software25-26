use anyhow::Result;
use async_gpiod::{Chip, Input, LineId, Lines, Options, Output};
use smol::Timer;
use std::time::Duration;

const CONSUMER: &str = "fill-station-igniter";

pub struct Igniter {
    continuity_pin: LineId,
    continuity_line: Lines<Input>,
    signal_pin: LineId,
    signal_line: Lines<Output>,
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

    pub async fn ignite(&self) {
        self.signal_line
            .set_values([true])
            .await
            .expect("The GPIO File Descriptor should not be able to close?");

        Timer::after(Duration::from_secs(3)).await;

        self.signal_line
            .set_values([false])
            .await
            .expect("The GPIO File Descriptor should not be able to close?");
    }

    pub async fn is_igniting(&self) -> bool {
        *self
            .signal_line
            .get_values([false])
            .await
            .expect("The GPIO File Descriptor should not be able to close?")
            .get(0)
            .expect("We know one value exists since we are requesting one value")
    }
}
