use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub use linux::PlatformInput;
#[cfg(target_os = "macos")]
pub use macos::PlatformInput;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("Antidote input is currently supported on Linux and macOS only");

pub trait Input {
    fn poll_events(&self) -> Result<Vec<InputEvent>, InputError>;
}

#[derive(Debug)]
pub enum InputEvent {
    // TODO: Left foot switch is not physically wired - re-enable once that changes
    // FootSwitchLeftTap,
    // ...
    FootSwitchRightTap,
    FootSwitchRightHold,
}

#[derive(Error, Debug)]
pub enum InputError {
    #[cfg(target_os = "linux")]
    #[error("GPIO error: {0}")]
    Gpio(#[from] gpiocdev::Error),
    #[error("Failed to spawn GPIO input thread: {0}")]
    ThreadSpawn(#[from] std::io::Error),
    #[error("Unknown error: {message}")]
    UnknownError { message: String },
}
