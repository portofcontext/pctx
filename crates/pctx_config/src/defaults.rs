// Various default functions to be used by serde

#[cfg(feature = "telemetry")]
use crate::telemetry::SamplingStrategy;

pub(crate) fn default_true() -> bool {
    true
}

#[cfg(feature = "telemetry")]
pub(crate) fn default_sampling_strategy() -> SamplingStrategy {
    SamplingStrategy::Always
}

#[cfg(feature = "telemetry")]
pub(crate) fn default_sampling_rate() -> f64 {
    1.0
}

pub(crate) fn default_timeout_ms() -> u64 {
    10000
}
