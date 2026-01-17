//! Math utilities for audio processing

use napi_derive::napi;

/// Converts a value from decibels to linear gain (multiplier).
///
/// # Example
/// ```
/// // -6dB is approximately 0.5
/// assert!(db_to_linear(-6.0) < 0.51 && db_to_linear(-6.0) > 0.49);
/// // 0dB is 1.0
/// assert_eq!(db_to_linear(0.0), 1.0);
/// ```
///
/// # Arguments
/// * `db` - Value in decibels
///
/// # Returns
/// Linear gain multiplier (0.0 to infinity)
#[napi]
pub fn db_to_linear(db: f64) -> f64 {
    10.0_f64.powf(db / 20.0)
}

/// Converts a linear gain multiplier to decibels.
///
/// # Example
/// ```
/// // 0.5 is approximately -6dB
/// assert!((linear_to_db(0.5) + 6.0).abs() < 0.1);
/// // 1.0 is 0dB
/// assert_eq!(linear_to_db(1.0), 0.0);
/// ```
///
/// # Arguments
/// * `linear` - Linear gain multiplier
///
/// # Returns
/// Value in decibels
#[napi]
pub fn linear_to_db(linear: f64) -> f64 {
    20.0 * linear.log10()
}
