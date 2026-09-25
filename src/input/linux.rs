use crossbeam::channel::{Receiver, Sender, unbounded};
use gpiocdev::Request;
use gpiocdev::line::{Bias, EdgeDetection, EdgeKind, EventClock};
use std::thread;
use std::time::Duration;

use super::{Input, InputError, InputEvent, event_for_press_duration_ns};

const DEBOUNCE_PERIOD: Duration = Duration::from_millis(20);

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
            .with_debounce_period(DEBOUNCE_PERIOD)
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
    let mut press_started_ns: Option<u64> = None;
    let mut previous_line_seqno: Option<u32> = None;

    for edge in events.edge_events() {
        let edge = match edge {
            Ok(edge) => edge,
            Err(error) => {
                tracing::error!(?error, "Failed to read GPIO line event");
                break;
            }
        };

        let sequence_gap = edge.line_seqno != 0
            && previous_line_seqno
                .is_some_and(|previous| edge.line_seqno != previous.wrapping_add(1));

        if sequence_gap {
            tracing::warn!(
                previous_line_seqno = ?previous_line_seqno,
                line_seqno = edge.line_seqno,
                "GPIO edge events were dropped; resynchronizing button state"
            );
            press_started_ns = (edge.kind == EdgeKind::Rising).then_some(edge.timestamp_ns);
        } else {
            match edge.kind {
                EdgeKind::Rising => press_started_ns = Some(edge.timestamp_ns),
                EdgeKind::Falling => {
                    let Some(press_started_ns) = press_started_ns.take() else {
                        continue;
                    };

                    let duration_ns = edge.timestamp_ns.saturating_sub(press_started_ns);
                    if event_sender
                        .send(event_for_press_duration_ns(duration_ns))
                        .is_err()
                    {
                        return;
                    }
                }
            }
        }

        if edge.line_seqno != 0 {
            previous_line_seqno = Some(edge.line_seqno);
        }
    }
}

impl Input for PlatformInput {
    fn poll_events(&self) -> Result<Vec<InputEvent>, InputError> {
        Ok(self.event_receiver.try_iter().collect())
    }
}
