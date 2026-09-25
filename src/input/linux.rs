use crossbeam::channel::{Receiver, Sender, unbounded};
use gpiocdev::Request;
use gpiocdev::line::{Bias, EdgeDetection, EdgeKind, EventClock};
use std::thread;
use std::time::{Duration, Instant};

use super::{Input, InputError, InputEvent, event_for_press_duration_ns};

// Accept an edge only after the input has stayed quiet for this long.
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
            .with_event_clock(EventClock::Monotonic)
            .request()?;

        let (event_sender, event_receiver) = unbounded();
        thread::Builder::new()
            .name("gpio_input".to_string())
            .spawn(move || poll_gpio_events(button_events, event_sender))?;

        Ok(Self { event_receiver })
    }
}

fn poll_gpio_events(events: Request, event_sender: Sender<InputEvent>) {
    let mut debouncer = ButtonDebouncer::default();

    loop {
        let edge = match debouncer.time_until_settle(Instant::now()) {
            Some(timeout) => match events.wait_edge_event(timeout) {
                Ok(true) => match events.read_edge_event() {
                    Ok(edge) => Some(edge),
                    Err(error) => {
                        tracing::error!(?error, "Failed to read GPIO line event");
                        break;
                    }
                },
                Ok(false) => None,
                Err(error) => {
                    tracing::error!(?error, "Failed waiting for GPIO line event");
                    break;
                }
            },
            None => match events.read_edge_event() {
                Ok(edge) => Some(edge),
                Err(error) => {
                    tracing::error!(?error, "Failed to read GPIO line event");
                    break;
                }
            },
        };

        if let Some(edge) = edge {
            debouncer.handle_edge(
                edge.kind,
                edge.timestamp_ns,
                edge.line_seqno,
                Instant::now(),
            );
        } else if let Some(input_event) = debouncer.settle(Instant::now())
            && event_sender.send(input_event).is_err()
        {
            return;
        }
    }
}

#[derive(Default)]
struct ButtonDebouncer {
    press_started_ns: Option<u64>,
    candidate: Option<CandidateEdge>,
    queued_candidate: Option<CandidateEdge>,
    previous_line_seqno: Option<u32>,
}

struct CandidateEdge {
    kind: EdgeKind,
    timestamp_ns: u64,
    received_at: Instant,
}

impl ButtonDebouncer {
    fn handle_edge(&mut self, kind: EdgeKind, timestamp_ns: u64, line_seqno: u32, now: Instant) {
        if line_seqno != 0 {
            let sequence_gap = self
                .previous_line_seqno
                .is_some_and(|previous| line_seqno != previous.wrapping_add(1));

            if sequence_gap {
                tracing::warn!(
                    previous_line_seqno = ?self.previous_line_seqno,
                    line_seqno,
                    "GPIO edge events were dropped; invalidating pending transitions"
                );
                // A gap makes pending transitions ambiguous, but it does not
                // invalidate a press that was already confirmed. In
                // particular, a repeated Rising edge must not lose a hold.
                self.candidate = None;
                self.queued_candidate = None;
            }

            self.previous_line_seqno = Some(line_seqno);
        }

        // If a release arrives before a pending press has settled, retain it
        // as the next transition. This lets a short tap complete after the
        // debounce window. A subsequent Rising edge cancels that pair as
        // bounce back to the original state.
        let already_stable = matches!(
            (self.press_started_ns.is_some(), kind),
            (false, EdgeKind::Falling) | (true, EdgeKind::Rising)
        );

        if already_stable {
            if self.press_started_ns.is_none()
                && self
                    .candidate
                    .as_ref()
                    .is_some_and(|candidate| candidate.kind == EdgeKind::Rising)
                && kind == EdgeKind::Falling
            {
                self.queued_candidate = Some(CandidateEdge {
                    kind,
                    timestamp_ns,
                    received_at: now,
                });
            } else if self.queued_candidate.is_some()
                && self.press_started_ns.is_none()
                && kind == EdgeKind::Rising
            {
                self.queued_candidate = None;
                self.candidate = Some(CandidateEdge {
                    kind,
                    timestamp_ns,
                    received_at: now,
                });
            } else {
                self.candidate = None;
                self.queued_candidate = None;
            }
        } else if self
            .candidate
            .as_ref()
            .is_some_and(|candidate| candidate.kind == kind)
        {
            // Duplicate edges do not restart the quiet-period timer or move
            // the timestamp of the original transition.
        } else {
            self.candidate = Some(CandidateEdge {
                kind,
                timestamp_ns,
                received_at: now,
            });
            self.queued_candidate = None;
        }
    }

    fn time_until_settle(&self, now: Instant) -> Option<Duration> {
        self.queued_candidate
            .as_ref()
            .or(self.candidate.as_ref())
            .map(|candidate| {
                DEBOUNCE_WINDOW.saturating_sub(now.saturating_duration_since(candidate.received_at))
            })
    }

    fn settle(&mut self, now: Instant) -> Option<InputEvent> {
        let candidate = self.queued_candidate.as_ref().or(self.candidate.as_ref())?;
        if now.saturating_duration_since(candidate.received_at) < DEBOUNCE_WINDOW {
            return None;
        }

        if let Some(queued_candidate) = self.queued_candidate.take() {
            let press_candidate = self.candidate.take()?;
            let duration_ns = queued_candidate
                .timestamp_ns
                .saturating_sub(press_candidate.timestamp_ns);
            return Some(event_for_press_duration_ns(duration_ns));
        }

        let candidate = self.candidate.take()?;
        match candidate.kind {
            EdgeKind::Rising => {
                self.press_started_ns = Some(candidate.timestamp_ns);
                None
            }
            EdgeKind::Falling => {
                let press_started_ns = self.press_started_ns.take()?;
                let duration_ns = candidate.timestamp_ns.saturating_sub(press_started_ns);
                Some(event_for_press_duration_ns(duration_ns))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use gpiocdev::line::EdgeKind;

    use super::{ButtonDebouncer, DEBOUNCE_WINDOW};
    use crate::input::InputEvent;

    #[test]
    fn bounce_back_to_stable_state_cancels_candidate() {
        let start = Instant::now();
        let mut debouncer = ButtonDebouncer::default();

        debouncer.handle_edge(EdgeKind::Rising, 1_000_000_000, 1, start);
        debouncer.handle_edge(
            EdgeKind::Falling,
            1_005_000_000,
            2,
            start + Duration::from_millis(5),
        );
        assert_eq!(
            debouncer.time_until_settle(start + Duration::from_millis(5)),
            None
        );

        debouncer.handle_edge(
            EdgeKind::Rising,
            1_008_000_000,
            3,
            start + Duration::from_millis(8),
        );
        assert!(
            debouncer
                .settle(start + Duration::from_millis(8) + DEBOUNCE_WINDOW)
                .is_none()
        );

        debouncer.handle_edge(
            EdgeKind::Falling,
            2_008_000_000,
            4,
            start + Duration::from_secs(1),
        );
        assert!(matches!(
            debouncer.settle(start + Duration::from_secs(1) + DEBOUNCE_WINDOW),
            Some(InputEvent::FootSwitchRightTap)
        ));
    }

    #[test]
    fn press_and_release_before_settle_are_reported_as_a_tap() {
        let start = Instant::now();
        let mut debouncer = ButtonDebouncer::default();

        debouncer.handle_edge(EdgeKind::Rising, 1_000_000_000, 1, start);
        debouncer.handle_edge(
            EdgeKind::Falling,
            1_006_000_000,
            2,
            start + Duration::from_millis(6),
        );

        assert!(matches!(
            debouncer.settle(start + Duration::from_millis(16)),
            Some(InputEvent::FootSwitchRightTap)
        ));
        assert_eq!(
            debouncer.time_until_settle(start + Duration::from_millis(16)),
            None
        );
    }

    #[test]
    fn sequence_gap_on_release_preserves_and_classifies_long_press() {
        let start = Instant::now();
        let mut debouncer = ButtonDebouncer::default();

        debouncer.handle_edge(EdgeKind::Rising, 1_000_000_000, 1, start);
        assert!(debouncer.settle(start + DEBOUNCE_WINDOW).is_none());

        // The missing edge may be switch bounce. Preserve the confirmed press
        // if the next observed edge is its release.
        debouncer.handle_edge(
            EdgeKind::Falling,
            2_600_000_000,
            3,
            start + Duration::from_millis(1_600),
        );
        assert!(matches!(
            debouncer.settle(start + Duration::from_millis(1_600) + DEBOUNCE_WINDOW),
            Some(InputEvent::FootSwitchRightHold)
        ));
    }

    #[test]
    fn sequence_gap_rising_edge_does_not_clear_a_confirmed_press() {
        let start = Instant::now();
        let mut debouncer = ButtonDebouncer::default();

        debouncer.handle_edge(EdgeKind::Rising, 1_000_000_000, 1, start);
        assert!(debouncer.settle(start + DEBOUNCE_WINDOW).is_none());

        debouncer.handle_edge(
            EdgeKind::Rising,
            2_600_000_000,
            3,
            start + Duration::from_millis(1_600),
        );
        assert!(
            debouncer
                .settle(start + Duration::from_millis(1_600) + DEBOUNCE_WINDOW)
                .is_none()
        );

        debouncer.handle_edge(
            EdgeKind::Falling,
            2_700_000_000,
            4,
            start + Duration::from_millis(1_700),
        );
        assert!(matches!(
            debouncer.settle(start + Duration::from_millis(1_700) + DEBOUNCE_WINDOW),
            Some(InputEvent::FootSwitchRightHold)
        ));
    }
}

impl Input for PlatformInput {
    fn poll_events(&self) -> Result<Vec<InputEvent>, InputError> {
        Ok(self.event_receiver.try_iter().collect())
    }
}
