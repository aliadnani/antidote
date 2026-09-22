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

fn main() {
    // Logging
    tracing_subscriber::fmt::init();
    info!("Starting Antidote.");

    // Lock-free ringbuffers
    let inputs = Arc::new(ArrayQueue::<f32>::new(1024));
    let outputs = Arc::new(ArrayQueue::<f32>::new(1024));

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
        loop {
            audio.tick();
        }
    });

    // CPAL Audio
    info!("Starting CPAL.");
    let host = cpal::default_host();
    let default_input_device = host
        .default_input_device()
        .inspect(|d| info!("Acquired default input device: {:?}", d))
        .expect("Failed to acquire default input device.");

    let mut supported_audio_config = default_input_device
        .supported_input_configs()
        .expect("Error while querying configs.")
        .next()
        .expect("No supported configs.")
        .with_max_sample_rate()
        .config();

    supported_audio_config.buffer_size = cpal::BufferSize::Fixed(BUFFER_SIZE as u32);

    info!(
        "Audio config: {:?} Hz, {:?} channels, buffer {:?}",
        supported_audio_config.sample_rate,
        supported_audio_config.channels,
        supported_audio_config.buffer_size
    );

    let latency_monitor = Arc::new(latency::LatencyMonitor::new(
        supported_audio_config.sample_rate as f64,
    ));

    // Report latency stats periodically without touching the audio threads
    {
        let latency_monitor = latency_monitor.clone();
        let inputs = inputs.clone();
        let outputs = outputs.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            latency_monitor.report(&inputs, &outputs);
        });
    }

    let input_stream = default_input_device.build_input_stream(
        supported_audio_config,
        {
            let latency_monitor = latency_monitor.clone();
            move |data: &[f32], info: &cpal::InputCallbackInfo| {
                latency_monitor.record_input(
                    info.timestamp().callback,
                    info.timestamp().capture,
                    data.len(),
                );
                for &sample in data {
                    inputs.force_push(sample);
                }
            }
        },
        move |error| {
            warn!("Error in input stream: {:?}", error);
        },
        None, // None waits forever to initialize the stream
    );
    let output_device = host
        .default_output_device()
        .expect("Failed to acquire default output device.");

    info!(
        "Default input device: {:?}, default output device: {:?}",
        default_input_device, output_device
    );

    info!("Supported audio config: {:?}", supported_audio_config);

    let output_stream = output_device
        .build_output_stream(
            supported_audio_config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                latency_monitor.record_output(
                    info.timestamp().callback,
                    info.timestamp().playback,
                    data.len(),
                );
                // Copy output data from the ringbuffer
                for sample in data.iter_mut() {
                    *sample = outputs.pop().unwrap_or(0.0);
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
