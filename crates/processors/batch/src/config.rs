use duration_string::DurationString;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BatchProcessorConfig {
    /// Number of spans, metric data points, or log records after which a batch will
    /// be sent regardless of the timeout.
    ///
    /// `send_batch_size` acts as a trigger and does not affect the size of the batch.
    /// If you need to enforce batch size limits sent to the next component in the pipeline,
    /// see `send_batch_max_size`.
    pub send_batch_size: Option<usize>,
    /// The upper limit of the batch size.
    ///
    /// 0 means no upper limit of the batch size.
    /// This property ensures that larger batches are split into smaller units.
    /// It must be greater than or equal to `send_batch_size`.
    pub send_batch_max_size: Option<usize>,
    /// Time duration after which a batch will be sent regardless of size.
    ///
    /// If set to zero, send_batch_size is ignored as data will be sent immediately,
    /// subject to only send_batch_max_size
    pub timeout: Option<DurationString>,
    /// When set, this processor will create one batcher instance per distinct combination
    /// of values in the client.Metadata.
    #[serde(default)]
    pub metadata_keys: Vec<String>,
    /// When metadata_keys is not empty, this setting limits the number of unique combinations
    /// of metadata key values that will be processed over the lifetime of the process.
    pub metadata_cardinality_limit: Option<usize>,
}
