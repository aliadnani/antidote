use crossbeam::channel::{Receiver, Sender, unbounded};
use gpiocdev::Request;
use gpiocdev::line::{Bias, EdgeDetection, EdgeKind};
use std::thread;
use std::time::{Duration, Instant};

use super::{Input, InputError, InputEvent};

// Hold threshold is 1.5 seconds
const HOLD_THRESHOLD: Duration = Duration::from_millis(1500);
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(10);

pub struct PlatformInput {
    event_receiver: Receiver<InputEvent>,
}

impl PlatformInput {
    pub fn new(chip_path: &str, line_offset: u32) -> Result<Self, InputError> {
        // The switch shorts the line to ground when pressed, so it is biased
        // pull-up and active-low. Edge semantics are inverted by the kernel,
        // so a press is reported as a rising edge.
        let button_events = Request::builder()
            .on_chip(chip_path)
            .with_consumer("button_input")
            .with_line(line_offset)
            .with_bias(Bias::PullUp)
            .as_active_low()
            .with_edge_detection(EdgeDetection::BothEdges)
            .request()?;

        let (event_sender, event_receiver) = unbounded();
        thread::Builder::new()
            .name("gpio_input".to_string())
            .spawn(move || poll_gpio_events(button_events, event_sender))?;

        Ok(Self { event_receiver })
    }
}

fn poll_gpio_events(events: Request, event_sender: Sender<InputEvent>) {
    let mut press_started: Option<Instant> = None;
    let mut last_edge_at: Option<Instant> = None;

    for edge in events.edge_events() {
        let edge = match edge {
            Ok(edge) => edge,
            Err(error) => {
                tracing::error!(?error, "Failed to read GPIO line event");
                return;
            }
        };

        if last_edge_at.is_some_and(|edge_at| edge_at.elapsed() < DEBOUNCE_WINDOW) {
            continue;
        }
        last_edge_at = Some(Instant::now());

        match edge.kind {
            EdgeKind::Rising => {
                press_started = Some(Instant::now());
            }
            EdgeKind::Falling => {
                let Some(declared_at) = press_started.take() else {
                    continue;
                };

                let input_event = if declared_at.elapsed() >= HOLD_THRESHOLD {
                    InputEvent::FootSwitchRightHold
                } else {
                    InputEvent::FootSwitchRightTap
                };

                if event_sender.send(input_event).is_err() {
                    return;
                }
            }
        }
    }
}

impl Input for PlatformInput {
    fn poll_events(&self) -> Result<Vec<InputEvent>, InputError> {
        Ok(self.event_receiver.try_iter().collect())
    }
}
