use super::{Input, InputError, InputEvent};

/// No physical footswitch is available while running on macOS.
pub struct PlatformInput;

impl PlatformInput {
    pub fn new(_chip_path: &str, _line_offset: u32) -> Result<Self, InputError> {
        Ok(Self)
    }
}

impl Input for PlatformInput {
    fn poll_events(&mut self) -> Result<Vec<InputEvent>, InputError> {
        Ok(Vec::new())
    }
}
