use duration_string::DurationString;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BatchProcessorConfig {
    pub send_batch_size: Option<usize>,
    pub timeout: Option<DurationString>,
}
