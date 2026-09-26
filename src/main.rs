use std::{path::PathBuf, sync::Arc};

use cpal::{
    Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam::queue::ArrayQueue;
use cxx::UniquePtr;
use tracing::{info, warn};

use crate::{
    audio::{Audio, AudioStats},
    coordinator::Coordinator,
    display::PlatformDisplay,
    input::PlatformInput,
    modeller::Modeller,
    nam_ffi::NamA2Model,
    preprocessor::PreProcessor,
    state::State,
};

pub mod audio;
pub mod coordinator;
pub mod display;
pub mod input;
pub mod modeller;
pub mod nam_ffi;
pub mod preprocessor;
pub mod state;
pub mod tests;

const AUDIO_CHANNELS: usize = 2;
const BUFFER_SIZE: usize = 64;
const QUEUE_CAPACITY: usize = BUFFER_SIZE * 4;
const I32_FULL_SCALE: f32 = 2_147_483_648.0;

const GPIO_CHIP: &str = "/dev/gpiochip1";
const FOOT_SWITCH_RIGHT_LINE: u32 = 5;

const NAM_MODELS_DIR: &str = "resources";

fn main() {
    // Logging
    tracing_subscriber::fmt::init();
    info!("Starting Antidote.");

    let nam_models_dir = match nam_models_dir_from_args() {
        Ok(dir) => dir,
        Err(usage) => {
            eprintln!("{usage}");
            return;
        }
    };

    let state = match State::new(&nam_models_dir) {
        Ok(state) => state,
        Err(error) => {
            warn!(?error, path = %nam_models_dir.display(), "Failed to load NAM profiles");
            return;
        }
    };
    let Some(initial_model_path) = state.current_model().map(|model| model.as_str().to_owned())
    else {
        warn!("No NAM profiles available");
        return;
    };

    // Set up shared audio state
    let (inputs, outputs, audio_stats) = setup_shared_audio_state();

    // Set up inter-thread communication channel for audio commands
    let (command_sender, command_receiver) = crossbeam::channel::unbounded::<audio::AudioCommand>();

    // Set up disposal of retired NAM models on a dedicated thread - their destructors
    let (disposal_tx, disposal_rx) = crossbeam::channel::unbounded::<UniquePtr<NamA2Model>>();
    let model_disposal_handle = setup_model_disposal(disposal_rx);

    // Set up input -> state -> audio command coordination
    let coordinator_thread = setup_coordinator(command_sender, state);

    // Start
    let audio_thread = setup_nam_processing_with_default_model(
        inputs.clone(),
        outputs.clone(),
        command_receiver,
        disposal_tx,
        audio_stats.clone(),
        Some(&initial_model_path),
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

    std::thread::park();

    // Exit and cleanup
    drop(audio_thread);
    drop(coordinator_thread);
    drop(audio_stats_handle);
    drop(model_disposal_handle);

    info!("Exiting Antidote.");
}

fn nam_models_dir_from_args() -> Result<PathBuf, String> {
    let mut args = std::env::args_os();
    let program = args.next().unwrap_or_default();
    let nam_models_dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(NAM_MODELS_DIR));

    if args.next().is_some() {
        return Err(format!(
            "Usage: {} [PROFILES_DIR]",
            program.to_string_lossy()
        ));
    }

    Ok(nam_models_dir)
}

fn setup_coordinator(
    command_sender: crossbeam::channel::Sender<audio::AudioCommand>,
    state: State,
) -> Option<std::thread::JoinHandle<()>> {
    let input = match PlatformInput::new(GPIO_CHIP, FOOT_SWITCH_RIGHT_LINE) {
        Ok(input) => input,
        Err(error) => {
            warn!(?error, "No GPIO input available - coordinator not started.");
            return None;
        }
    };

    let display = PlatformDisplay::new();
    let coordinator = Coordinator::new(input, state, command_sender, display);

    Some(std::thread::spawn(move || coordinator.run()))
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
    disposal_tx: crossbeam::channel::Sender<UniquePtr<NamA2Model>>,
    audio_stats: Arc<AudioStats>,
    default_model_path: Option<&str>,
) -> std::thread::JoinHandle<()> {
    // NAM + Audio subsystem
    let mut modeller = modeller::NamA2ModelModeller::new(disposal_tx);

    if let Some(model_path) = default_model_path {
        modeller
            .load_nam_a2_model(model_path)
            .expect("Could not load NAM A2 model.");
    }

    // USB noise filter experiment
    let preprocessor = PreProcessor::new();

    let mut audio = Audio::new(
        inputs.clone(),
        outputs.clone(),
        modeller,
        BUFFER_SIZE,
        command_receiver,
        audio_stats.clone(),
        Some(preprocessor),
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

        None => host.default_input_device(),
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

fn setup_model_disposal(
    disposal_rx: crossbeam::channel::Receiver<UniquePtr<NamA2Model>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(retired_model) = disposal_rx.recv() {
            drop(retired_model);
        }
    })
}

fn run_audio(input_stream: cpal::Stream, output_stream: cpal::Stream) {
    input_stream.play().unwrap();
    output_stream.play().unwrap();
}
