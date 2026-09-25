#[cxx::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("antidote/src/nam_ffi/nam_shim.h");

        pub type NamA2Model;

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

// No idea if NamA2Models are actually thread safe - but so far no issues.
unsafe impl Send for ffi::NamA2Model {}

pub use self::ffi::*;
