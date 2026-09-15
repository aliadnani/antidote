use crate::nam_ffi;

#[test]
fn test_nam_a2_model_load() {
    let dsp = nam_ffi::load_nam_a2_model_path("resources/fender_clean.nam")
        .expect("Could not load NAM A2 model.");

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
    let mut dsp = nam_ffi::load_nam_a2_model_path("resources/fender_clean.nam")
        .expect("Could not load NAM A2 model.");

    let input = generate_sine_wave(110.0, 48000.0, 2.0);
    let mut output = vec![0.0; input.len()];

    nam_ffi::process_block_with_nam_a2_model(
        dsp.pin_mut(),
        &input,
        &mut output,
        input.len() as i32,
    );

    // This is a sequential model, so check both the head and the tail:
    // The tail confirms that per-sample error does not accumulate over the block.
    assert_eq!(
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
            0.1292283
        ]
    );
    assert_eq!(
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
            -0.01439013
        ]
    );
    assert_eq!(
        &output[..10],
        &[
            0.00052450894,
            0.00023981501,
            -0.0008804427,
            -0.0037150017,
            -0.008411667,
            -0.013675765,
            -0.017835798,
            -0.019450665,
            -0.018403785,
            -0.0155082615
        ]
    );
    assert_eq!(
        &output[output.len() - 10..],
        &[
            -0.2014261,
            -0.20054963,
            -0.20059547,
            -0.20158094,
            -0.20348664,
            -0.20627508,
            -0.2098254,
            -0.21388681,
            -0.2181065,
            -0.2220757
        ]
    );
}
