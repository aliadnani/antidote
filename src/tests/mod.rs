#[cfg(test)]
pub mod nam_tests;

#[cfg(test)]
pub(crate) fn assert_f32_slices_within_ulps(
    actual: &[f32],
    expected: &[f32],
    max_ulps: u32,
    context: &str,
) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{context}: slice lengths differ"
    );

    for (index, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            actual.is_finite() && expected.is_finite(),
            "{context} sample {index}: got {actual:?}, expected {expected:?}"
        );

        let ulp_distance = ordered_f32_bits(actual).abs_diff(ordered_f32_bits(expected));
        assert!(
            ulp_distance <= max_ulps,
            "{context} sample {index}: got {actual:?}, expected {expected:?} \
             ({ulp_distance} ULPs; maximum {max_ulps})"
        );
    }
}

#[cfg(test)]
fn ordered_f32_bits(value: f32) -> u32 {
    let bits = value.to_bits();
    if bits & 0x8000_0000 == 0 {
        bits | 0x8000_0000
    } else {
        !bits
    }
}
