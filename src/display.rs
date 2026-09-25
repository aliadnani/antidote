#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub use linux::PlatformDisplay;
#[cfg(target_os = "macos")]
pub use macos::PlatformDisplay;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("Antidote display is currently supported on Linux and macOS only");

pub trait Display: Send {
    fn render(&mut self, profile_name: &str, profile_index: usize);
}
