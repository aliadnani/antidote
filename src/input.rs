use std::time::Duration;
use std::time::Instant;
use thiserror::Error;

pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(20);
#[cfg(any(target_os = "linux", test))]
const HOLD_THRESHOLD: Duration = Duration::from_millis(650);

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
#[derive(Default)]
struct ButtonPoller {
    pressed_since: Option<Instant>,
    released_since: Option<Instant>,
    press_confirmed: bool,
    hold_sent: bool,
}

#[cfg(any(target_os = "linux", test))]
impl ButtonPoller {
    fn sample(&mut self, is_pressed: bool, now: Instant) -> Option<InputEvent> {
        if is_pressed {
            self.released_since = None;
            let pressed_since = *self.pressed_since.get_or_insert(now);
            let pressed_for = now.saturating_duration_since(pressed_since);

            if !self.press_confirmed && pressed_for >= POLL_INTERVAL {
                self.press_confirmed = true;
            }
            if self.press_confirmed && !self.hold_sent && pressed_for >= HOLD_THRESHOLD {
                self.hold_sent = true;
                return Some(InputEvent::FootSwitchRightHold);
            }
            return None;
        }

        if !self.press_confirmed {
            self.pressed_since = None;
            self.released_since = None;
            self.hold_sent = false;
            return None;
        }

        let Some(released_since) = self.released_since else {
            self.released_since = Some(now);
            return None;
        };
        if now.saturating_duration_since(released_since) < POLL_INTERVAL {
            return None;
        }

        self.pressed_since = None;
        self.released_since = None;
        self.press_confirmed = false;
        if self.hold_sent {
            self.hold_sent = false;
            None
        } else {
            Some(InputEvent::FootSwitchRightTap)
        }
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

    use super::{ButtonPoller, HOLD_THRESHOLD, InputEvent, POLL_INTERVAL};

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
    fn emits_hold_when_threshold_passes_and_does_not_repeat_on_release() {
        let start = Instant::now();
        let mut poller = ButtonPoller::default();
        assert!(poller.sample(true, start).is_none());
        assert!(poller.sample(true, start + POLL_INTERVAL).is_none());

        let threshold_sample = start + HOLD_THRESHOLD;
        assert!(matches!(
            poller.sample(true, threshold_sample),
            Some(InputEvent::FootSwitchRightHold)
        ));
        assert!(
            poller
                .sample(true, threshold_sample + POLL_INTERVAL)
                .is_none()
        );

        assert!(
            poller
                .sample(false, threshold_sample + POLL_INTERVAL * 2)
                .is_none()
        );
        assert!(
            poller
                .sample(false, threshold_sample + POLL_INTERVAL * 3)
                .is_none()
        );
    }
}
