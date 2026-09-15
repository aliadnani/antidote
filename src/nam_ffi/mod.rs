#[cxx::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("antidote/src/nam_ffi/nam_shim.h");

        type NamA2Model;

        // Hard code the path to the JSON file for now. In the future
        // TODO: Make this configurable once we figure out CXX works
        fn load_nam_a2_model_path(model_path_str: &str) -> Result<UniquePtr<NamA2Model>>;

        fn get_nam_a2_model_expected_sample_rate(model: &NamA2Model) -> f64;

        fn process_block_with_nam_a2_model(
            model: Pin<&mut NamA2Model>,
            input: &[f32],
            output: &mut [f32],
            num_frames: i32,
        );
    }
}

pub use self::ffi::*;
