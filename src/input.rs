use crossbeam::channel::{unbounded, Receiver, Sender};
use gpiocdev::line::{Bias, EdgeDetection, EdgeKind};
use gpiocdev::Request;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

pub trait Input {
    fn poll_events(&self) -> Result<Vec<InputEvent>, InputError>;
}

// Hold threshold is 1.5 seconds
const HOLD_THRESHOLD: Duration = Duration::from_millis(1500);

// Mechanical switches bounce for up to ~10ms; edges inside this window are ignored.
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(20);

pub struct InputGpioBacked {
    event_receiver: Receiver<InputEvent>,
}

impl InputGpioBacked {
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

impl Input for InputGpioBacked {
    fn poll_events(&self) -> Result<Vec<InputEvent>, InputError> {
        Ok(self.event_receiver.try_iter().collect())
    }
}

#[derive(Debug)]
pub enum InputEvent {
    // TODO: Left foot switch is not physically wired - re-enable once that changes
    // FootSwitchLeftTap,
    // ...
    FootSwitchRightTap,
    FootSwitchRightHold,
}

#[derive(Error, Debug)]
pub enum InputError {
    #[error("GPIO error: {0}")]
    Gpio(#[from] gpiocdev::Error),
    #[error("Failed to spawn GPIO input thread: {0}")]
    ThreadSpawn(#[from] std::io::Error),
    #[error("Unknown error: {message}")]
    UnknownError { message: String },
}
