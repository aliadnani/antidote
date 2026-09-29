use crate::{
    modeller::{Modeller, NamA2ModelModeller},
    nam_ffi,
    tests::assert_f32_slices_within_ulps,
};

// Test fixture copied from NeuralAmpModelerCore's example_models/A2.nam.
const TEST_NAM_MODEL_PATH: &str = "resources/tests/a2.nam";

#[test]
fn test_nam_a2_model_load() {
    let dsp =
        nam_ffi::load_nam_a2_model_path(TEST_NAM_MODEL_PATH).expect("Could not load NAM A2 model.");

    let sample_rate = nam_ffi::get_nam_a2_model_expected_sample_rate(&dsp);

    assert_eq!(sample_rate, 48000.0);
}

fn generate_sine_wave(frequency: f32, sample_rate: f32, duration_secs: f32) -> Vec<f32> {
    let num_samples = (sample_rate * duration_secs) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    for n in 0..num_samples {
        let t = n as f32 / sample_rate;
        let sample = (2.0 * std::f32::consts::PI * frequency * t).sin();
        samples.push(sample);
    }

    samples
}

#[test]
fn test_nam_a2_model_inference() {
    let mut dsp =
        nam_ffi::load_nam_a2_model_path(TEST_NAM_MODEL_PATH).expect("Could not load NAM A2 model.");

    let input: Vec<f32> = generate_sine_wave(110.0, 48000.0, 2.0);
    let mut output = vec![0.0; input.len()];

    nam_ffi::process_block_with_nam_a2_model(
        dsp.pin_mut(),
        &input,
        &mut output,
        input.len() as i32,
    );

    // This is a sequential model, so check both the head and the tail:
    // The tail confirms that per-sample error does not accumulate over the block.
    assert_f32_slices_within_ulps(
        &input[..10],
        &[
            0.0,
            0.014398469,
            0.028793951,
            0.04318347,
            0.057564024,
            0.07193266,
            0.08628637,
            0.100622185,
            0.11493715,
            0.1292283,
        ],
        4,
        "NAM inference input head",
    );
    assert_f32_slices_within_ulps(
        &input[input.len() - 10..],
        &[
            -0.14353184,
            -0.1292623,
            -0.11496593,
            -0.10052426,
            -0.08630461,
            -0.0719456,
            -0.057571664,
            -0.043185785,
            -0.028790943,
            -0.01439013,
        ],
        4,
        "NAM inference input tail",
    );
    assert_f32_slices_within_ulps(
        &output[..10],
        &[
            -0.0034633796,
            -0.006979144,
            -0.013950484,
            -0.03504115,
            -0.07285835,
            -0.114483975,
            -0.14344479,
            -0.15780656,
            -0.16182058,
            -0.15715274,
        ],
        4,
        "NAM inference output head",
    );
    assert_f32_slices_within_ulps(
        &output[output.len() - 10..],
        &[
            -0.20916332,
            -0.21214621,
            -0.21525365,
            -0.2183323,
            -0.2212951,
            -0.22409782,
            -0.22671857,
            -0.22935577,
            -0.2325447,
            -0.2371557,
        ],
        4,
        "NAM inference output tail",
    );
}

#[test]
fn test_unload_load_unload() {
    let (disposal_tx, _disposal_rx) = crossbeam::channel::unbounded();
    let mut modeller = NamA2ModelModeller::new(disposal_tx);

    // Test unloaded: should be a pass-through; sine wave untouched
    let input = generate_sine_wave(110.0, 48000.0, 2.0);
    let mut output = vec![0.0; input.len()];

    modeller.process_block(&input, &mut output).unwrap();
    // Test only the first 20 samples to avoid printing a huge array in case of failure
    assert_eq!(input[..20], output[..20]);

    // Load the model
    modeller
        .load_nam_a2_model(TEST_NAM_MODEL_PATH)
        .expect("Could not load NAM A2 model.");

    // Test loaded: should be processed; sine wave should be different
    let mut output = vec![0.0; input.len()];
    modeller.process_block(&input, &mut output).unwrap();

    assert_f32_slices_within_ulps(
        &output[..10],
        &[
            -0.0034633796,
            -0.006979144,
            -0.013950484,
            -0.03504115,
            -0.07285835,
            -0.114483975,
            -0.14344479,
            -0.15780656,
            -0.16182058,
            -0.15715274,
        ],
        4,
        "NAM modeller output head",
    );

    // Unload the model
    modeller.unload_nam_a2_model();

    // Test unloaded again: should be a pass-through; sine wave untouched
    let mut output = vec![0.0; input.len()];
    modeller.process_block(&input, &mut output).unwrap();

    assert_eq!(input[..20], output[..20]);
}
