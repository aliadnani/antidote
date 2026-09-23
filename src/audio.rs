use crossbeam::{channel::Receiver, queue::ArrayQueue};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::thread;
use tracing::{error, info};

use crate::{modeller::Modeller, AUDIO_CHANNELS};

pub struct AudioStats {
    input_drops: AtomicU64,
    output_drops: AtomicU64,
    output_underruns: AtomicU64,
}

impl AudioStats {
    pub fn new() -> Self {
        Self {
            input_drops: AtomicU64::new(0),
            output_drops: AtomicU64::new(0),
            output_underruns: AtomicU64::new(0),
        }
    }

    pub fn record_input_drop(&self) {
        self.input_drops.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_output_underrun(&self) {
        self.output_underruns.fetch_add(1, Ordering::Relaxed);
    }

    fn record_output_drop(&self) {
        self.output_drops.fetch_add(1, Ordering::Relaxed);
    }

    pub fn report(&self) {
        tracing::info!(
            input_drops = self.input_drops.load(Ordering::Relaxed),
            output_drops = self.output_drops.load(Ordering::Relaxed),
            output_underruns = self.output_underruns.load(Ordering::Relaxed),
            "Audio errors"
        );
    }
}

pub struct Audio<T: Modeller> {
    inputs: Arc<ArrayQueue<[f32; AUDIO_CHANNELS]>>,
    outputs: Arc<ArrayQueue<[f32; AUDIO_CHANNELS]>>,
    // Pre-allocated - as to avoid on the hot path
    input_buffer: Vec<f32>,
    output_buffer: Vec<f32>,
    modeller: T,
    command_channel: Receiver<AudioCommand>,
    stats: Arc<AudioStats>,
}

pub enum AudioCommand {
    // Hotswap model only for now - not sure if we need other commands but this abstraction is near free anyways.
    HotSwapModel(String),
}

impl<T: Modeller> Audio<T> {
    pub fn new(
        inputs: Arc<ArrayQueue<[f32; AUDIO_CHANNELS]>>,
        outputs: Arc<ArrayQueue<[f32; AUDIO_CHANNELS]>>,
        modeller: T,
        chunk_size: usize,
        command_channel: Receiver<AudioCommand>,
        stats: Arc<AudioStats>,
    ) -> Self {
        Audio {
            inputs,
            outputs,
            modeller,
            command_channel,
            input_buffer: vec![0.0; chunk_size],
            output_buffer: vec![0.0; chunk_size],
            stats,
        }
    }

    pub fn tick(&mut self) {
        while let Ok(audio_command) = self.command_channel.try_recv() {
            /* TODO: Defer this async

            Doing this here - block DSP which will cause underruns.
            Our audio thread will gracefully handle underruns via pass-through, but still this is not ideal.
            */
            match audio_command {
                AudioCommand::HotSwapModel(model_path) => {
                    info!(model_path = %model_path, "Unloading current NAM A2 model");
                    self.modeller.unload_nam_a2_model();

                    let load_result = self.modeller.load_nam_a2_model(&model_path);

                    if load_result.is_err() {
                        error!(
                            "Failed to load NAM A2 model from path {}: {:?}",
                            model_path, load_result
                        )
                    } else {
                        info!(model_path = %model_path, "Loaded NAM A2 model");
                    }
                }
            }
        }

        const SPIN_PHASE_CUTOFF: usize = 100;
        const YIELD_PHASE_CUTOFF: usize = 200;
        const PARK_DURATION: std::time::Duration = std::time::Duration::from_micros(50);

        let mut popped_count = 0;
        let mut spun_count = 0;

        while popped_count < self.input_buffer.len() {
            while popped_count < self.input_buffer.len() {
                match self.inputs.pop() {
                    Some([sample, ..]) => {
                        // NAM is a mono model, we really only care about the first channel
                        self.input_buffer[popped_count] = sample;
                        popped_count += 1;
                    }

                    None => break,
                }
            }

            spun_count += 1;

            // Wee!!
            if spun_count < SPIN_PHASE_CUTOFF {
                std::hint::spin_loop();
            } else if spun_count < YIELD_PHASE_CUTOFF {
                thread::yield_now();
            } else {
                thread::park_timeout(PARK_DURATION);
            }
        }

        self.modeller
            .process_block(&self.input_buffer, &mut self.output_buffer)
            .inspect_err(|e| {
                error!("Failed to process block with NAM A2 model: {:?}", e);
            })
            .ok();

        for &sample in &self.output_buffer {
            // NAM is mono - but our output is not, hence we duplicate.
            if self.outputs.force_push([sample; AUDIO_CHANNELS]).is_some() {
                self.stats.record_output_drop();
            }
        }
    }
}
