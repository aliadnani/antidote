use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub use linux::PlatformInput;
#[cfg(target_os = "macos")]
pub use macos::PlatformInput;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("Antidote input is currently supported on Linux and macOS only");

pub trait Input {
    fn poll_events(&self) -> Result<Vec<InputEvent>, InputError>;
}

#[derive(Debug)]
pub enum InputEvent {
    // TODO: Left foot switch is not physically wired - re-enable once that changes
    // FootSwitchLeftTap,
    // ...
    FootSwitchRightTap,
    FootSwitchRightHold,
}

#[cfg(any(target_os = "linux", test))]
const HOLD_THRESHOLD_NS: u64 = 1_500_000_000;

#[cfg(any(target_os = "linux", test))]
pub(crate) fn event_for_press_duration_ns(duration_ns: u64) -> InputEvent {
    if duration_ns >= HOLD_THRESHOLD_NS {
        InputEvent::FootSwitchRightHold
    } else {
        InputEvent::FootSwitchRightTap
    }
}

#[derive(Error, Debug)]
pub enum InputError {
    #[cfg(target_os = "linux")]
    #[error("GPIO error: {0}")]
    Gpio(#[from] gpiocdev::Error),
    #[error("Failed to spawn GPIO input thread: {0}")]
    ThreadSpawn(#[from] std::io::Error),
    #[error("Unknown error: {message}")]
    UnknownError { message: String },
}

#[cfg(test)]
mod tests {
    use super::{InputEvent, event_for_press_duration_ns};

    #[test]
    fn classifies_taps_and_holds_at_the_hold_threshold() {
        assert!(matches!(
            event_for_press_duration_ns(1_499_999_999),
            InputEvent::FootSwitchRightTap
        ));
        assert!(matches!(
            event_for_press_duration_ns(1_500_000_000),
            InputEvent::FootSwitchRightHold
        ));
    }
}
