/// Values closer to zero than this are treated as zero.
pub const THRESHOLD: f64 = 1e-9;

/// Whether `value` is approximately zero.
pub fn is_zero(value: f64) -> bool {
    value.abs() < THRESHOLD
}

/// Whether `value` is not approximately zero.
pub fn is_nonzero(value: f64) -> bool {
    !is_zero(value)
}

/// Whether `value` is finite and has no fractional part.
pub fn is_integer(value: f64) -> bool {
    value.is_finite() && is_zero(value - value.round())
}

pub fn to_u8(value: f64) -> Option<u8> {
    let i = value.round() as u8;
    is_zero(i as f64 - value).then_some(i)
}
