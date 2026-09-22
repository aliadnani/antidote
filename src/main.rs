use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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
const BUFFER_SIZE: usize = 256;
// Queue capacity is measured in stereo frames and provides four callback periods
// of headroom, rather than one period of interleaved scalar samples.
const QUEUE_CAPACITY: usize = BUFFER_SIZE * 4;
const I32_FULL_SCALE: f32 = 2_147_483_648.0;

fn main() {
    // Logging
    tracing_subscriber::fmt::init();
    info!("Starting Antidote.");

    // Lock-free ringbuffers
    let inputs = Arc::new(ArrayQueue::<[f32; AUDIO_CHANNELS]>::new(QUEUE_CAPACITY));
    let outputs = Arc::new(ArrayQueue::<[f32; AUDIO_CHANNELS]>::new(QUEUE_CAPACITY));
    let audio_stats = Arc::new(AudioStats::new());

    let (_command_sender, command_receiver) =
        crossbeam::channel::unbounded::<audio::AudioCommand>();

    // NAM + Audio subsystem
    let mut modeller = modeller::NamA2ModelModeller::new();
    modeller
        .load_nam_a2_model("resources/fender_brown.nam")
        .expect("Could not load NAM A2 model.");

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
        channels: AUDIO_CHANNELS as u16,
        sample_rate: 48_000,
        buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE as u32),
    };

    info!(
        "Audio config: {:?} Hz, {:?} channels, buffer {:?}",
        config.sample_rate, config.channels, config.buffer_size
    );

    // Report queue errors periodically without touching the audio threads.
    {
        let audio_stats = audio_stats.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                audio_stats.report();
            }
        });
    }

    let input_stream = device.build_input_stream(
        config,
        {
            let audio_stats = audio_stats.clone();
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
    );

    info!(
        "Default input device: {:?}, default output device: {:?}",
        device, device
    );

    info!("Supported audio config: {:?}", device);

    let output_stream = device
        .build_output_stream(
            config,
            move |data: &mut [i32], _info: &cpal::OutputCallbackInfo| {
                let mut frames = data.chunks_exact_mut(AUDIO_CHANNELS);
                for frame in &mut frames {
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
