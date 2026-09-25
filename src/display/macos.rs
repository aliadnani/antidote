use super::Display;

/// No physical display peripherals are available while running on macOS.
pub struct PlatformDisplay;

impl PlatformDisplay {
    pub fn new() -> Self {
        Self
    }
}

impl Display for PlatformDisplay {
    fn render(&mut self, _profile_name: &str, _profile_index: usize) {}
}
