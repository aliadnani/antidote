use thiserror::Error;
use tracing::info;

pub trait Modeller {
    fn process_block(&self, input: &[f32], output: &mut [f32]) -> Result<(), ModellerError>;
}
#[derive(Error, Debug)]
pub enum ModellerError {
    #[error("Unknown error: {message}")]
    UnknownError { message: String },
}

pub struct PassThroughMetricsModeller;

impl Modeller for PassThroughMetricsModeller {
    fn process_block(&self, input: &[f32], output: &mut [f32]) -> Result<(), ModellerError> {
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
