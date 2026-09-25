use std::time::Duration;
use std::time::Instant;
use thiserror::Error;

pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(20);

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
    fn poll_events(&mut self) -> Result<Vec<InputEvent>, InputError>;
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
const HOLD_THRESHOLD_NS: u64 = 1_200_000_000;

#[cfg(any(target_os = "linux", test))]
pub(crate) fn event_for_press_duration_ns(duration_ns: u64) -> InputEvent {
    if duration_ns >= HOLD_THRESHOLD_NS {
        InputEvent::FootSwitchRightHold
    } else {
        InputEvent::FootSwitchRightTap
    }
}

#[cfg(any(target_os = "linux", test))]
#[derive(Default)]
struct ButtonPoller {
    pressed_since: Option<Instant>,
    released_since: Option<Instant>,
    press_confirmed: bool,
}

#[cfg(any(target_os = "linux", test))]
impl ButtonPoller {
    fn sample(&mut self, is_pressed: bool, now: Instant) -> Option<InputEvent> {
        if is_pressed {
            self.released_since = None;
            match self.pressed_since {
                Some(pressed_since)
                    if !self.press_confirmed
                        && now.saturating_duration_since(pressed_since) >= POLL_INTERVAL =>
                {
                    self.press_confirmed = true;
                }
                None => self.pressed_since = Some(now),
                _ => {}
            }
            return None;
        }

        if !self.press_confirmed {
            self.pressed_since = None;
            self.released_since = None;
            return None;
        }

        let Some(released_since) = self.released_since else {
            self.released_since = Some(now);
            return None;
        };
        if now.saturating_duration_since(released_since) < POLL_INTERVAL {
            return None;
        }

        let pressed_since = self.pressed_since.take()?;
        self.released_since = None;
        self.press_confirmed = false;
        let duration_ns = released_since
            .saturating_duration_since(pressed_since)
            .as_nanos() as u64;
        Some(event_for_press_duration_ns(duration_ns))
    }
}

#[derive(Error, Debug)]
pub enum InputError {
    #[cfg(target_os = "linux")]
    #[error("GPIO error: {0}")]
    Gpio(#[from] gpiocdev::Error),
    #[error("Unknown error: {message}")]
    UnknownError { message: String },
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{ButtonPoller, InputEvent, POLL_INTERVAL, event_for_press_duration_ns};

    #[test]
    fn classifies_taps_and_holds_at_the_hold_threshold() {
        assert!(matches!(
            event_for_press_duration_ns(1_199_999_999),
            InputEvent::FootSwitchRightTap
        ));
        assert!(matches!(
            event_for_press_duration_ns(1_200_000_000),
            InputEvent::FootSwitchRightHold
        ));
    }

    #[test]
    fn requires_two_pressed_samples_and_two_released_samples() {
        let start = Instant::now();
        let mut poller = ButtonPoller::default();

        assert!(poller.sample(true, start).is_none());
        assert!(poller.sample(true, start + POLL_INTERVAL).is_none());
        assert!(
            poller
                .sample(false, start + Duration::from_millis(40))
                .is_none()
        );
        assert!(matches!(
            poller.sample(false, start + Duration::from_millis(60)),
            Some(InputEvent::FootSwitchRightTap)
        ));
    }

    #[test]
    fn unconfirmed_press_transient_is_ignored() {
        let start = Instant::now();
        let mut poller = ButtonPoller::default();

        assert!(poller.sample(true, start).is_none());
        assert!(poller.sample(false, start + POLL_INTERVAL).is_none());
        assert!(poller.sample(false, start + POLL_INTERVAL * 2).is_none());
    }

    #[test]
    fn classifies_press_longer_than_twelve_hundred_milliseconds_as_hold() {
        let start = Instant::now();
        let mut poller = ButtonPoller::default();
        assert!(poller.sample(true, start).is_none());
        assert!(poller.sample(true, start + POLL_INTERVAL).is_none());
        let release_at = start + Duration::from_millis(1_220);
        assert!(poller.sample(false, release_at).is_none());
        assert!(matches!(
            poller.sample(false, release_at + POLL_INTERVAL),
            Some(InputEvent::FootSwitchRightHold)
        ));
    }
}
