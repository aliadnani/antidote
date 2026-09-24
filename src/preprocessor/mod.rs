use coeffs::{HP_SECTIONS, LP_SECTIONS, NOTCH_SECTIONS};

mod coeffs;

struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    fn new(sos: [f64; 6]) -> Self {
        let [b0, b1, b2, a0, a1, a2] = sos;
        Biquad {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x
            + self.b1 * self.x1
            + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        let y = if y.abs() < 1e-30 { 0.0 } else { y };
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

pub struct PreProcessor {
    biquads: Vec<Biquad>,
}

impl PreProcessor {
    pub fn new() -> Self {
        let mut biquads = Vec::with_capacity(
            HP_SECTIONS.len() + NOTCH_SECTIONS.len() + LP_SECTIONS.len(),
        );

        biquads.extend(HP_SECTIONS.map(Biquad::new));
        biquads.extend(NOTCH_SECTIONS.map(Biquad::new));
        biquads.extend(LP_SECTIONS.map(Biquad::new));

        PreProcessor { biquads }
    }

    pub fn process_block(&mut self, block: &mut [f32]) {
        for sample in block {
            let mut value = f64::from(*sample);
            for biquad in &mut self.biquads {
                value = biquad.process(value);
            }
            *sample = value as f32;
        }
    }
}

impl Default for PreProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coeffs::{TEST_BLOCK_LEN, TEST_EXPECTED, TEST_INPUT};

    #[test]
    fn matches_scipy_reference() {
        let mut preprocessor = PreProcessor::new();

        let input: Vec<f32> = TEST_INPUT.iter().map(|&v| v as f32).collect();
        let mut output = input;

        for chunk in output.chunks_mut(TEST_BLOCK_LEN) {
            preprocessor.process_block(chunk);
        }

        for (i, (&got, &want)) in output.iter().zip(TEST_EXPECTED.iter()).enumerate() {
            let diff = (f64::from(got) - want).abs();
            assert!(
                diff < 1e-9,
                "sample {i}: got {got}, want {want} (diff {diff})"
            );
        }
    }

    fn sine_steady_state_rms(freq: f64, amp: f64) -> f64 {
        let fs = coeffs::FS;
        let n = 48000;
        let mut preprocessor = PreProcessor::new();

        let mut signal: Vec<f32> = (0..n)
            .map(|i| {
                let t = f64::from(i as u32) / fs;
                (amp * (2.0 * std::f64::consts::PI * freq * t).sin()) as f32
            })
            .collect();

        preprocessor.process_block(&mut signal);

        let steady = &signal[n / 2..];
        (steady.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>()
            / steady.len() as f64)
            .sqrt()
    }

    #[test]
    fn attenuates_mains() {
        let in_rms = 0.4 / std::f64::consts::SQRT_2;
        let out_rms = sine_steady_state_rms(coeffs::MAINS_F0, 0.4);
        assert!(
            out_rms < in_rms * 0.1,
            "mains not attenuated enough: {out_rms} vs input {in_rms}"
        );
    }

    #[test]
    fn preserves_passband() {
        // Halfway between two mains harmonics, clear of the notch bank.
        let freq = coeffs::MAINS_F0 * 16.5;
        let in_rms = 0.4 / std::f64::consts::SQRT_2;
        let out_rms = sine_steady_state_rms(freq, 0.4);
        assert!(
            (out_rms / in_rms - 1.0).abs() < 0.05,
            "passband gain off: {out_rms} vs input {in_rms}"
        );
    }

    #[test]
    fn attenuates_switching_noise_band() {
        let in_rms = 0.4 / std::f64::consts::SQRT_2;
        let out_rms = sine_steady_state_rms(12000.0, 0.4);
        assert!(
            out_rms < in_rms * 0.1,
            "HF not attenuated enough: {out_rms} vs input {in_rms}"
        );
    }
}
