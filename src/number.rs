//! Time-based exponential growth for bounded integer controls.
//!
//! [`ExponentialGrowth`] is useful for values such as adaptive limits that
//! should rise smoothly from a baseline while remaining aligned to a fixed
//! step and capped at a maximum.

use n0_future::time::Instant;

/// Computes a stepped exponential value from elapsed monotonic time.
pub struct ExponentialGrowth {
    start_time: Instant,
    initial_value: i64,
    step: i64,
    growth_rate: f64,
    max: i64,
}

impl ExponentialGrowth {
    /// Starts a growth curve at the current instant.
    ///
    /// Values are rounded down to a multiple of `step`, never fall below
    /// `initial_value`, and never exceed `max`. Callers should use a positive,
    /// non-zero step and a finite growth rate.
    pub fn new(step: i64, growth_rate: f64, initial_value: i64, max: i64) -> Self {
        Self {
            start_time: Instant::now(),
            initial_value,
            step,
            growth_rate,
            max,
        }
    }

    /// Samples the curve using the time elapsed since construction.
    ///
    /// Sampling does not mutate the curve, so repeated calls within the same
    /// step interval may return the same value.
    pub fn next(&self) -> i64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let growth = (self.growth_rate * elapsed * 0.1).exp();
        let raw_value = self.initial_value as f64 * growth;

        // Round down to nearest multiple of step
        let stepped_value = ((raw_value / self.step as f64).floor() * self.step as f64) as i64;

        // Ensure the value is at least initial_value
        stepped_value.max(self.initial_value).min(self.max)
    }
}
