use std::sync::Arc;

use cpal::{
    Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam::queue::ArrayQueue;
use tracing::{info, warn};

use crate::{
    audio::{Audio, AudioStats},
    modeller::Modeller,
};

pub mod audio;
pub mod coordinator;
pub mod display;
pub mod input;
pub mod modeller;
pub mod nam_ffi;
pub mod state;
pub mod tests;

const AUDIO_CHANNELS: usize = 2;
const BUFFER_SIZE: usize = 48;
const QUEUE_CAPACITY: usize = BUFFER_SIZE * 4;
const I32_FULL_SCALE: f32 = 2_147_483_648.0;

fn main() {
    // Logging
    tracing_subscriber::fmt::init();
    info!("Starting Antidote.");

    // Set up shared audio state
    let (inputs, outputs, audio_stats) = setup_shared_audio_state();

    // Set up inter-thread communication channel for audio commands
    let (_, command_receiver) = crossbeam::channel::unbounded::<audio::AudioCommand>();

    // Start
    let audio_thread = setup_nam_processing_with_default_model(
        inputs.clone(),
        outputs.clone(),
        command_receiver,
        audio_stats.clone(),
        Some("resources/fender_brown.nam"),
    );

    // CPAL Audio
    info!("Starting CPAL.");
    let (device, config) = setup_cpal_device_and_config(Some("hw:CARD=sndi2s0,DEV=0"));

    // Set up audio stats reporting thread
    let audio_stats_handle = setup_audio_stats_reporting(audio_stats.clone());

    // Run
    let input_stream = build_input_stream(&device, &config, inputs, audio_stats.clone())
        .expect("Failed to build input stream.");
    let output_stream = build_output_stream(&device, &config, outputs, audio_stats.clone())
        .expect("Failed to build output stream.");

    run_audio(input_stream, output_stream);

    // Exit and cleanup
    drop(audio_thread);
    drop(audio_stats_handle);

    info!("Exiting Antidote.");
}

fn setup_shared_audio_state() -> (
    Arc<ArrayQueue<[f32; AUDIO_CHANNELS]>>,
    Arc<ArrayQueue<[f32; AUDIO_CHANNELS]>>,
    Arc<AudioStats>,
) {
    let inputs = Arc::new(ArrayQueue::<[f32; AUDIO_CHANNELS]>::new(QUEUE_CAPACITY));
    let outputs = Arc::new(ArrayQueue::<[f32; AUDIO_CHANNELS]>::new(QUEUE_CAPACITY));
    let audio_stats = Arc::new(AudioStats::new());

    (inputs, outputs, audio_stats)
}

fn setup_nam_processing_with_default_model(
    inputs: Arc<ArrayQueue<[f32; AUDIO_CHANNELS]>>,
    outputs: Arc<ArrayQueue<[f32; AUDIO_CHANNELS]>>,
    command_receiver: crossbeam::channel::Receiver<audio::AudioCommand>,
    audio_stats: Arc<AudioStats>,
    default_model_path: Option<&str>,
) -> std::thread::JoinHandle<()> {
    // NAM + Audio subsystem
    let mut modeller = modeller::NamA2ModelModeller::new();

    if let Some(model_path) = default_model_path {
        modeller
            .load_nam_a2_model(model_path)
            .expect("Could not load NAM A2 model.");
    }

    let mut audio = Audio::new(
        inputs.clone(),
        outputs.clone(),
        modeller,
        BUFFER_SIZE,
        command_receiver, // Unused for now
        audio_stats.clone(),
    );

    // Start a new thread to run audio processing loop
    let audio_thread = std::thread::spawn(move || {
        loop {
            audio.tick();
        }
    });

    audio_thread
}

fn setup_cpal_device_and_config(target_device: Option<&str>) -> (cpal::Device, cpal::StreamConfig) {
    let host = cpal::default_host();

    let device = match target_device {
        Some(device_name) => find_device_by_name(&host, device_name).or_else(|| {
            warn!(
                "Could not find device with name '{}', falling back to default input device.",
                device_name
            );
            host.default_input_device()
        }),

        None => host.default_input_device()
    };

    let device = device.expect("Failed to find a suitable audio input/output device.");

    let config = cpal::StreamConfig {
        channels: AUDIO_CHANNELS as u16,
        sample_rate: 48_000,
        buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE as u32),
    };

    (device, config)
}

fn find_device_by_name(host: &cpal::Host, device_name: &str) -> Option<cpal::Device> {
    host.devices()
        .expect("Failed to enumerate audio devices.")
        .find(|d| {
            d.description()
                .ok()
                .and_then(|x| x.driver().map(str::to_owned))
                == Some(device_name.into())
        })
}


fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    inputs: Arc<ArrayQueue<[f32; AUDIO_CHANNELS]>>,
    audio_stats: Arc<AudioStats>,
) -> Result<Stream, cpal::Error> {
    device.build_input_stream(
        *config,
        {
            move |data: &[i32], _info: &cpal::InputCallbackInfo| {
                let mut frames = data.chunks_exact(AUDIO_CHANNELS);
                for frame in &mut frames {
                    let frame = [
                        // PCM samples from our I2S lines are i32, so we need to convert them first.
                        frame[0] as f32 / I32_FULL_SCALE,
                        frame[1] as f32 / I32_FULL_SCALE,
                    ];
                    if inputs.force_push(frame).is_some() {
                        audio_stats.record_input_drop();
                    }
                }
            }
        },
        move |error| {
            warn!("Error in input stream: {:?}", error);
        },
        None, // None waits forever to initialize the stream
    )
}

fn build_output_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    outputs: Arc<ArrayQueue<[f32; AUDIO_CHANNELS]>>,
    audio_stats: Arc<AudioStats>,
) -> Result<Stream, cpal::Error> {
    device.build_output_stream(
        *config,
        move |data: &mut [i32], _info: &cpal::OutputCallbackInfo| {
            let mut frames = data.chunks_exact_mut(AUDIO_CHANNELS);
            for frame in &mut frames {
                let outputs = &outputs;
                let sample = match outputs.pop() {
                    Some([sample, ..]) => sample,
                    None => {
                        audio_stats.record_output_underrun();
                        0.0
                    }
                };
                let sample = (sample * I32_FULL_SCALE) as i32;
                frame[0] = sample;
                frame[1] = sample;
            }
        },
        move |error| {
            warn!("Error in output stream: {:?}", error);
        },
        // None for waiting forever to initialize the stream
        None,
    )
}

fn setup_audio_stats_reporting(audio_stats: Arc<AudioStats>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            audio_stats.report();
        }
    })
}

fn run_audio(input_stream: cpal::Stream, output_stream: cpal::Stream) {
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
}
