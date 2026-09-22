use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::queue::ArrayQueue;
use tracing::{info, warn};

use crate::audio::Audio;

pub mod audio;
pub mod coordinator;
pub mod display;
pub mod input;
pub mod modeller;
pub mod nam_ffi;
pub mod state;
pub mod tests;

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
        512,
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

    let supported_audio_config = default_input_device
        .supported_input_configs()
        .expect("Error while querying configs.")
        .next()
        .expect("No supported configs.")
        .with_max_sample_rate()
        .config();

    let input_stream = default_input_device.build_input_stream(
        supported_audio_config,
        move |data: &[f32], _| {
            for &sample in data {
                inputs.force_push(sample);
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

    let output_stream = output_device
        .build_output_stream(
            supported_audio_config,
            move |data: &mut [f32], _| {
                // Copy output data from the ringbuffer
                for sample in data.iter_mut() {
                    *sample = outputs.pop().unwrap_or(0.0);
                }
                
                info!(
                    "Output stream callback executed. Output buffer length: {}, max/min values: max: {}, min: {}",
                     data.len(),
                      data.iter().cloned().fold(f32::MIN, f32::max), data.iter().cloned().fold(f32::MAX, f32::min));

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
    let playback_duration = std::time::Duration::from_secs(5);
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
