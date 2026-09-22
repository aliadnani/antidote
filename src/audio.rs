use crossbeam::{channel::Receiver, queue::ArrayQueue};
use std::sync::Arc;
use std::thread;
use tracing::error;

use crate::modeller::Modeller;

pub struct Audio<T: Modeller> {
    inputs: Arc<ArrayQueue<f32>>,
    outputs: Arc<ArrayQueue<f32>>,
    // Pre-allocated - as to avoid on the hot path
    input_buffer: Vec<f32>,
    output_buffer: Vec<f32>,
    modeller: T,
    command_channel: Receiver<AudioCommand>,
}

pub enum AudioCommand {
    // Hotswap model only for now - not sure if we need other commands but this abstraction is near free anyways.
    HotSwapModel(String),
}

impl<T: Modeller> Audio<T> {
    pub fn new(
        inputs: Arc<ArrayQueue<f32>>,
        outputs: Arc<ArrayQueue<f32>>,
        modeller: T,
        chunk_size: usize,
        command_channel: Receiver<AudioCommand>,
    ) -> Self {
        Audio {
            inputs,
            outputs,
            modeller,
            command_channel,
            input_buffer: vec![0.0; chunk_size],
            output_buffer: vec![0.0; chunk_size],
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
                    self.modeller.unload_nam_a2_model();
                    self.modeller
                        .load_nam_a2_model(&model_path)
                        .inspect_err(|e| {
                            error!(
                                "Failed to load NAM A2 model from path {}: {:?}",
                                model_path, e
                            )
                        })
                        .ok();
                }
            }
        }

        const SPIN_PHASE_CUTOFF: usize = 100;
        const YIELD_PHASE_CUTOFF: usize = 200;
        const PARK_DURATION: std::time::Duration = std::time::Duration::from_micros(500);

        let mut popped_count = 0;
        let mut spun_count = 0;

        while popped_count < self.input_buffer.len() {
            while popped_count < self.input_buffer.len() {
                match self.inputs.pop() {
                    Some(sample) => {
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
            self.outputs.force_push(sample);
        }
    }
}
