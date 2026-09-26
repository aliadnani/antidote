use crossbeam::channel::Sender;
use cxx::UniquePtr;
use thiserror::Error;
use tracing::info;

use crate::nam_ffi::{self, NamA2Model};

pub trait Modeller {
    fn process_block(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), ModellerError>;

    /// Load a preloaded NAM A2 model.
    fn load_preloaded_nam_a2_model(&mut self, dsp: UniquePtr<NamA2Model>);

    fn load_nam_a2_model(&mut self, model_path: &str) -> Result<(), ModellerError>;

    fn unload_nam_a2_model(&mut self);
}

#[derive(Error, Debug)]
pub enum ModellerError {
    #[error("Unknown error: {message}")]
    UnknownError { message: String },
}

pub struct NamA2ModelModeller {
    dsp: Option<UniquePtr<NamA2Model>>,
    // A dedicated thread to unload models - doing this on the audio hot loop causes dropouts, so hence we do it here
    disposal_tx: Sender<UniquePtr<NamA2Model>>,
}

impl NamA2ModelModeller {
    pub fn new(disposal_tx: Sender<UniquePtr<NamA2Model>>) -> Self {
        NamA2ModelModeller {
            dsp: None,
            disposal_tx,
        }
    }

    fn dispose_dsp(&self, dsp: Option<UniquePtr<NamA2Model>>) {
        if let Some(old) = dsp {
            let _ = self.disposal_tx.send(old);
        }
    }
}

impl Modeller for NamA2ModelModeller {
    fn load_nam_a2_model(&mut self, model_path: &str) -> Result<(), ModellerError> {
        let dsp = nam_ffi::load_nam_a2_model_path(model_path).map_err(|e| {
            ModellerError::UnknownError {
                message: format!("Failed to load NAM A2 model: {:?}", e),
            }
        })?;

        let old = self.dsp.replace(dsp);
        self.dispose_dsp(old);

        info!(model_path = %model_path, "Loaded NAM A2 model.");

        Ok(())
    }

    fn load_preloaded_nam_a2_model(&mut self, dsp: UniquePtr<NamA2Model>) {
        let old = self.dsp.replace(dsp);
        self.dispose_dsp(old);
    }

    // DSP field is mostly infallible - doesn't make sense to return a Result.
    fn unload_nam_a2_model(&mut self) {
        let old = self.dsp.take();
        self.dispose_dsp(old);

        info!("Unloaded NAM A2 model.");
    }

    fn process_block(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), ModellerError> {
        if let Some(dsp) = &mut self.dsp {
            nam_ffi::process_block_with_nam_a2_model(
                dsp.pin_mut(),
                input,
                output,
                input.len() as i32,
            );

            Ok(())
        } else {
            // None means no model is loaded yet - so just pass through for now.
            output.copy_from_slice(input);

            Ok(())
        }
    }
}

pub struct PassThroughMetricsModeller;

impl Modeller for PassThroughMetricsModeller {
    fn load_preloaded_nam_a2_model(&mut self, _dsp: UniquePtr<NamA2Model>) {
        info!("PassThroughMetricsModeller: load_preloaded_nam_a2_model called.");
    }

    fn load_nam_a2_model(&mut self, model_path: &str) -> Result<(), ModellerError> {
        info!(
            "PassThroughMetricsModeller: load_nam_a2_model called with path: {}",
            model_path
        );
        Ok(())
    }

    fn unload_nam_a2_model(&mut self) {
        info!("PassThroughMetricsModeller: unload_nam_a2_model called");
    }

    fn process_block(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), ModellerError> {
        output.copy_from_slice(input);

        info!(
            "Processed block of size: {}, min: {}, max: {}",
            input.len(),
            input
                .iter()
                .min_by(|a, b| f32::total_cmp(a, b))
                .unwrap_or(&0.0),
            input
                .iter()
                .max_by(|a, b| f32::total_cmp(a, b))
                .unwrap_or(&0.0)
        );

        Ok(())
    }
}
