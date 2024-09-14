use color_eyre::eyre;
use opentelemetry_sdk::metrics::data::Metric;

// pub trait MetricsConsumer: std::fmt::Debug + Send + Sync + 'static {
#[async_trait::async_trait]
pub trait MetricsConsumer: std::fmt::Debug {
    async fn consume(&self, metrics: &[Metric]) -> eyre::Result<()>;
}

#[derive(Debug)]
pub struct OtlpExporter {}

#[async_trait::async_trait]
impl MetricsConsumer for OtlpExporter {
    async fn consume(&self, metrics: &[Metric]) -> eyre::Result<()> {
        tracing::debug!("OTLP: exporting {} metrics", metrics.len());
        Ok(())
    }
}
