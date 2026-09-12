use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tracing::{info, warn};

use crate::modeller::Modeller;

pub mod coordinator;
pub mod display;
pub mod input;
pub mod modeller;
pub mod state;

fn main() {
    // ...
    info!("Starting Antidote.");
    tracing_subscriber::fmt::init();
    let modeller = modeller::PassThroughMetricsModeller;

    // ...
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

    // ...
    let mut output = vec![0.0; (512) as usize];
    let stream = default_input_device.build_input_stream(
        supported_audio_config,
        move |data: &[f32], _| {
            modeller.process_block(data, &mut output).unwrap();
            ()
        },
        move |error| {
            warn!("Error in input stream: {:?}", error);
        },
        // None for waiting forever to initialize the stream
        None,
    );

    // ...
    let stream = stream.unwrap();
    stream.play().unwrap();

    // ...
    let playback_duration = std::time::Duration::from_secs(3);
    info!(
        "Playing back audio for {} seconds.",
        playback_duration.as_secs()
    );
    std::thread::sleep(playback_duration);

    // ...
    drop(stream);

    // ...
    info!("Exiting Antidote.");
}
