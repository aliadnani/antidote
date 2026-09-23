use crossbeam::channel::{unbounded, Receiver, Sender};
use linux_embedded_hal::gpio_cdev::{
    Chip, EventRequestFlags, EventType, LineEventHandle, LineRequestFlags,
};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

pub trait Input {
    fn poll_events(&self) -> Result<Vec<InputEvent>, InputError>;
}


const HOLD_THRESHOLD: Duration = Duration::from_millis(1500);
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(20);

pub struct InputGpioBacked {
    event_receiver: Receiver<InputEvent>,
}

impl InputGpioBacked {
    pub fn new(chip_name: &str, input_line: u32) -> Result<Self, InputError> {
        let mut chip = Chip::new(chip_name)?;
        let input_line = chip.get_line(input_line)?;
        let button_input_line = input_line.events(
            LineRequestFlags::INPUT,
            EventRequestFlags::BOTH_EDGES,
            "button_input",
        )?;

        let (event_sender, event_receiver) = unbounded();
        thread::Builder::new()
            .name("gpio_input".to_string())
            .spawn(move || poll_gpio_events(button_input_line, event_sender))?;

        Ok(Self { event_receiver })
    }
}

fn poll_gpio_events(mut events: LineEventHandle, event_sender: Sender<InputEvent>) {
    let mut press_started: Option<Instant> = None;
    let mut last_edge_at: Option<Instant> = None;

    loop {
        let event = match events.get_event() {
            Ok(event) => event,
            Err(error) => {
                tracing::error!(?error, "Failed to read GPIO line event");
                return;
            }
        };

        if last_edge_at.is_some_and(|edge_at| edge_at.elapsed() < DEBOUNCE_WINDOW) {
            continue;
        }
        last_edge_at = Some(Instant::now());

        match event.event_type() {
            EventType::RisingEdge => {
                press_started = Some(Instant::now());
            }
            EventType::FallingEdge => {
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
    Gpio(#[from] linux_embedded_hal::gpio_cdev::errors::Error),
    #[error("Failed to spawn GPIO input thread: {0}")]
    ThreadSpawn(#[from] std::io::Error),
    #[error("Unknown error: {message}")]
    UnknownError { message: String },
}
