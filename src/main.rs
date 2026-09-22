use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::queue::ArrayQueue;
use tracing::{info, warn};

use crate::audio::Audio;

pub mod audio;
pub mod coordinator;
pub mod display;
pub mod input;
pub mod latency;
pub mod modeller;
pub mod nam_ffi;
pub mod state;
pub mod tests;

const BUFFER_SIZE: usize = 32;
const QUEUE_CAPACITY: usize = 256;
const AUDIO_THREAD_RT_PRIORITY: i32 = 80;
const I32_FULL_SCALE: f32 = 2_147_483_648.0;

fn main() {
    // Logging
    tracing_subscriber::fmt::init();
    info!("Starting Antidote.");

    // Lock-free ringbuffers
    let inputs = Arc::new(ArrayQueue::<f32>::new(QUEUE_CAPACITY));
    let outputs = Arc::new(ArrayQueue::<f32>::new(QUEUE_CAPACITY));

    let (_command_sender, command_receiver) =
        crossbeam::channel::unbounded::<audio::AudioCommand>();

    // NAM + Audio subsystem
    let modeller = modeller::NamA2ModelModeller::new();
    let mut audio = Audio::new(
        inputs.clone(),
        outputs.clone(),
        modeller,
        BUFFER_SIZE,
        command_receiver, // Unused for now
    );

    // Start a new thread to run audio processing loop
    let audio_thread = std::thread::spawn(move || {
        promote_current_thread_to_rt(AUDIO_THREAD_RT_PRIORITY);
        loop {
            audio.tick();
        }
    });

    // CPAL Audio
    info!("Starting CPAL.");
    let host = cpal::default_host();

    let wanted = "hw:CARD=sndi2s0,DEV=0";
    let device = host
        .devices()
        .expect("Failed to enumerate audio devices.")
        .find(|d| {
            d.description()
                .ok()
                .and_then(|x| x.driver().map(str::to_owned))
                == Some(wanted.into())
        })
        .expect("device not found");

    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: 48_000,
        buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE as u32),
    };

    info!(
        "Audio config: {:?} Hz, {:?} channels, buffer {:?}",
        config.sample_rate, config.channels, config.buffer_size
    );

    let latency_monitor = Arc::new(latency::LatencyMonitor::new(config.sample_rate as f64));

    // Report latency stats periodically without touching the audio threads
    {
        let latency_monitor = latency_monitor.clone();
        let inputs = inputs.clone();
        let outputs = outputs.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                latency_monitor.report(&inputs, &outputs);
            }
        });
    }

    let input_stream = device.build_input_stream(
        config,
        {
            let latency_monitor = latency_monitor.clone();
            move |data: &[i32], info: &cpal::InputCallbackInfo| {
                latency_monitor.record_input(
                    info.timestamp().callback,
                    info.timestamp().capture,
                    data.len(),
                );
                for &sample in data {
                    inputs.force_push(sample as f32 / I32_FULL_SCALE);
                }
            }
        },
        move |error| {
            warn!("Error in input stream: {:?}", error);
        },
        None, // None waits forever to initialize the stream
    );

    info!(
        "Default input device: {:?}, default output device: {:?}",
        device, device
    );

    info!("Supported audio config: {:?}", device);

    let output_stream = device
        .build_output_stream(
            config,
            move |data: &mut [i32], info: &cpal::OutputCallbackInfo| {
                latency_monitor.record_output(
                    info.timestamp().callback,
                    info.timestamp().playback,
                    data.len(),
                );
                // Copy output data from the ringbuffer
                for sample in data.iter_mut() {
                    *sample = (outputs.pop().unwrap_or(0.0) * I32_FULL_SCALE) as i32;
                }
            },
            move |error| {
                warn!("Error in output stream: {:?}", error);
            },
            // None for waiting forever to initialize the stream
            None,
        )
        .expect("Failed to build output stream.");

    // Run
    let input_stream = input_stream.unwrap();
    let output_stream = output_stream;

    input_stream.play().unwrap();
    output_stream.play().unwrap();

    // Let the audio run for a while
    let playback_duration = std::time::Duration::from_secs(50);
    info!(
        "Playing back audio for {} seconds.",
        playback_duration.as_secs()
    );
    std::thread::sleep(playback_duration);

    info!("Stopping audio playback.");

    // Cleanup
    drop(input_stream);
    drop(output_stream);
    drop(audio_thread);
    info!("Exiting Antidote.");
}

#[cfg(target_os = "linux")]
fn promote_current_thread_to_rt(priority: i32) {
    // SAFETY: `sched_param` is a plain C struct, and `pthread_self` /
    // `pthread_setschedparam` operate on the calling thread only. Failure (e.g. missing
    // CAP_SYS_NICE or an rtprio rlimit) is non-fatal; we stay on the default scheduler.
    unsafe {
        let param = libc::sched_param {
            sched_priority: priority,
        };
        let ret = libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_FIFO, &param);
        if ret != 0 {
            warn!(
                "Failed to promote audio thread to SCHED_FIFO (error {}); running under the default scheduler.",
                ret
            );
        } else {
            info!(
                "Promoted audio thread to SCHED_FIFO priority {}.",
                priority
            );
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn promote_current_thread_to_rt(_priority: i32) {}
