use gpiocdev::Request;
use gpiocdev::line::{Bias, Value};
use std::time::{Duration, Instant};

use super::{ButtonPoller, Input, InputError, InputEvent, event_for_press_duration_ns};

pub struct PlatformInput {
    button: Request,
    poller: ButtonPoller,
}

impl PlatformInput {
    pub fn new(chip_path: &str, line_offset: u32) -> Result<Self, InputError> {
        // The switch shorts the line to ground when pressed, so it is biased
        // pull-up and active-low. gpiocdev reports the logical active state.
        let button = Request::builder()
            .on_chip(chip_path)
            .with_consumer("button_input")
            .with_line(line_offset)
            .as_input()
            .with_bias(Bias::PullUp)
            .as_active_low()
            .request()?;

        Ok(Self {
            button,
            poller: ButtonPoller::default(),
        })
    }
}

impl Input for PlatformInput {
    fn poll_events(&mut self) -> Result<Vec<InputEvent>, InputError> {
        let is_pressed = self.button.lone_value()? == Value::Active;
        Ok(self
            .poller
            .sample(is_pressed, Instant::now())
            .into_iter()
            .collect())
    }
}
