use std::sync::atomic::{AtomicU64, Ordering};

use cpal::StreamInstant;
use crossbeam::queue::ArrayQueue;
use tracing::info;

pub struct LatencyMonitor {
    sample_rate: f64,
    input_capture_ns: AtomicU64,
    input_callback_ns: AtomicU64,
    input_frames: AtomicU64,
    output_callback_ns: AtomicU64,
    output_playback_ns: AtomicU64,
    output_frames: AtomicU64,
    max_input_callback_jitter_ns: AtomicU64,
}

impl LatencyMonitor {
    pub fn new(sample_rate: f64) -> Self {
        LatencyMonitor {
            sample_rate,
            input_capture_ns: AtomicU64::new(0),
            input_callback_ns: AtomicU64::new(0),
            input_frames: AtomicU64::new(0),
            output_callback_ns: AtomicU64::new(0),
            output_playback_ns: AtomicU64::new(0),
            output_frames: AtomicU64::new(0),
            max_input_callback_jitter_ns: AtomicU64::new(0),
        }
    }

    pub fn record_input(&self, callback: StreamInstant, capture: StreamInstant, num_frames: usize) {
        let callback_ns = callback.as_nanos() as u64;
        let previous_callback_ns = self.input_callback_ns.swap(callback_ns, Ordering::Relaxed);
        if previous_callback_ns != 0 {
            let delta_ns = callback_ns.saturating_sub(previous_callback_ns);
            let expected_ns = (num_frames as f64 * 1_000_000_000.0 / self.sample_rate) as u64;
            let deviation_ns = delta_ns.abs_diff(expected_ns);
            self.max_input_callback_jitter_ns
                .fetch_max(deviation_ns, Ordering::Relaxed);
        }
        self.input_capture_ns
            .store(capture.as_nanos() as u64, Ordering::Relaxed);
        self.input_frames
            .fetch_add(num_frames as u64, Ordering::Relaxed);
    }

    pub fn record_output(
        &self,
        callback: StreamInstant,
        playback: StreamInstant,
        num_frames: usize,
    ) {
        self.output_callback_ns
            .store(callback.as_nanos() as u64, Ordering::Relaxed);
        self.output_playback_ns
            .store(playback.as_nanos() as u64, Ordering::Relaxed);
        self.output_frames
            .fetch_add(num_frames as u64, Ordering::Relaxed);
    }

    pub fn report<T>(&self, inputs: &ArrayQueue<T>, outputs: &ArrayQueue<T>) {
        let input_callback_ns = self.input_callback_ns.load(Ordering::Relaxed);
        let input_capture_ns = self.input_capture_ns.load(Ordering::Relaxed);
        let input_frames = self.input_frames.load(Ordering::Relaxed);
        let output_callback_ns = self.output_callback_ns.load(Ordering::Relaxed);
        let output_playback_ns = self.output_playback_ns.load(Ordering::Relaxed);
        let output_frames = self.output_frames.load(Ordering::Relaxed);
        let max_jitter_ns = self.max_input_callback_jitter_ns.swap(0, Ordering::Relaxed);

        if input_frames == 0 || output_frames == 0 {
            return;
        }

        let input_buffering_ms =
            (input_callback_ns as i64 - input_capture_ns as i64) as f64 / 1_000_000.0;
        let output_scheduling_ms =
            (output_playback_ns as i64 - output_callback_ns as i64) as f64 / 1_000_000.0;
        let sample_count_skew_ns = (input_frames as i64 - output_frames as i64) as f64
            * 1_000_000_000.0
            / self.sample_rate;
        let round_trip_ms = (output_playback_ns as i64 - input_capture_ns as i64) as f64
            / 1_000_000.0
            + sample_count_skew_ns / 1_000_000.0;

        info!(
            "Latency: round-trip ~{:.2}ms, input buffering {:.2}ms, output scheduling {:.2}ms, max input callback jitter {:.3}ms, queues in {}/{} frames, out {}/{} frames",
            round_trip_ms,
            input_buffering_ms,
            output_scheduling_ms,
            max_jitter_ns as f64 / 1_000_000.0,
            inputs.len(),
            inputs.capacity(),
            outputs.len(),
            outputs.capacity(),
        );
    }
}
