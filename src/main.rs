use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tracing::{info, warn};

use crate::modeller::Modeller;

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

    // NAM
    let modeller = modeller::PassThroughMetricsModeller;

    info!("Loading NAM A2 model via FFI.");
    let dsp = nam_ffi::load_nam_a2_model_path("resources/fender_clean.nam").expect("Could not load NAM A2 model.");
    let sample_rate = nam_ffi::get_nam_a2_model_expected_sample_rate(&dsp);

    info!(
        "Loaded NAM A2 model with expected sample rate: {}",
        sample_rate
    );

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

    let mut output = vec![0.0; (512) as usize];
    let stream = default_input_device.build_input_stream(
        supported_audio_config,
        move |data: &[f32], _| {
            modeller.process_block(data, &mut output).unwrap();
        },
        move |error| {
            warn!("Error in input stream: {:?}", error);
        },
        // None for waiting forever to initialize the stream
        None,
    );

    // Run
    let stream = stream.unwrap();
    stream.play().unwrap();

    let playback_duration = std::time::Duration::from_secs(3);
    info!(
        "Playing back audio for {} seconds.",
        playback_duration.as_secs()
    );
    std::thread::sleep(playback_duration);

    info!("Stopping audio playback.");

    // Cleanup
    drop(stream);
    info!("Exiting Antidote.");
}
