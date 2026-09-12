use thiserror::Error;

pub trait Input {
    fn poll_events(&self) -> Result<Vec<InputEvent>, InputError>;
}

pub enum InputEvent {
    FootSwitchLeftTap,
    // ...
    FootSwitchRightTap,
    FootSwitchRightHold,
}

#[derive(Error, Debug)]
pub enum InputError {
    #[error("Unknown error: {message}")]
    UnknownError { message: String },
}
