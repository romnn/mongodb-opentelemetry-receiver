#![allow(warnings)]

pub mod config;

use color_eyre::eyre;
use futures::StreamExt;
use opentelemetry_sdk::export::logs::LogBatch;
use opentelemetry_sdk::metrics::data::{Metric, ResourceMetrics, ScopeMetrics, Temporality};
use opentelemetry_sdk::{export::trace::SpanData, logs::LogData, metrics::exporter};
use otel_collector_component::factory::ComponentName;
use otel_collector_component::MetricsStream;
use tokio::sync::{broadcast, watch};
use tracing::{debug, trace, warn};

lazy_static::lazy_static! {
    static ref COMPONENT_NAME: ComponentName = ComponentName::new("otlp").unwrap();
}

#[derive(Debug, Default)]
pub struct Factory {}

#[async_trait::async_trait]
impl otel_collector_component::factory::ExporterFactory for Factory {
    fn component_name(&self) -> &ComponentName {
        &COMPONENT_NAME
    }

    async fn build(
        &self,
        id: String,
        config: serde_yaml::Value,
    ) -> eyre::Result<Box<dyn otel_collector_component::Exporter>> {
        let receiver = OtlpExporter::from_config(id, config)?;
        Ok(Box::new(receiver))
    }
}

#[derive(Debug)]
pub struct OtlpExporter {
    pub id: String,
    pub config: config::OtlpExporterConfig,
    // trace_config: Option<sdk::trace::Config>,
    // batch_config: Option<sdk::trace::BatchConfig>,
    trace_exporter: opentelemetry_otlp::SpanExporter,
    log_exporter: opentelemetry_otlp::LogExporter,
    metrics_exporter: opentelemetry_otlp::MetricsExporter,
    // metadata       opentelemetry_otlp::Me
    // use opentelemetry_otlp::WithExportConfig;
    // TimeoutSettings: exporterhelper.NewDefaultTimeoutSettings(),
    // RetryConfig:     configretry.NewDefaultBackOffConfig(),
    // QueueConfig:     exporterhelper.NewDefaultQueueSettings(),
    // BatcherConfig:   batcherCfg,
    // ClientConfig: configgrpc.ClientConfig{
    // 	Headers: map[string]configopaque.String{},
    // 	// Default to gzip compression
    // 	Compression: configcompression.TypeGzip,
    // 	// We almost read 0 bytes, so no need to tune ReadBufferSize.
    // 	WriteBufferSize: 512 * 1024,
    // },
}

fn build_transport_channel(
    config: &opentelemetry_otlp::ExportConfig,
    tls_config: Option<tonic::transport::ClientTlsConfig>,
) -> eyre::Result<tonic::transport::Channel> {
    let endpoint = tonic::transport::Channel::from_shared(config.endpoint.clone())?;
    let channel = match tls_config {
        Some(tls_config) => endpoint.tls_config(tls_config)?,
        None => endpoint,
    };

    Ok(channel.timeout(config.timeout).connect_lazy())
}

impl OtlpExporter {
    pub fn from_config(id: String, config: serde_yaml::Value) -> eyre::Result<Self> {
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_sdk::metrics::reader::{
            DefaultAggregationSelector, DefaultTemporalitySelector,
        };

        let config: config::OtlpExporterConfig = serde_yaml::from_value(config)?;

        let metadata = tonic::metadata::MetadataMap::default();

        let export_config = || opentelemetry_otlp::ExportConfig {
            endpoint: config.endpoint.clone(),
            protocol: opentelemetry_otlp::Protocol::Grpc,
            timeout: std::time::Duration::from_secs(60),
        };

        // let tls_config = None;
        // let channel = build_transport_channel(&export_config(), tls_config)?;
        // dbg!(&channel);

        let batch_config = opentelemetry_sdk::trace::BatchConfig::default();
        // userAgent := fmt.Sprintf("%s/%s (%s/%s)",
        //     set.BuildInfo.Description, set.BuildInfo.Version, runtime.GOOS, runtime.GOARCH)

        // TODO: only set settings that make sense here...
        let metrics_exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            // .with_channel(channel.clone())
            .with_compression(opentelemetry_otlp::Compression::Gzip)
            // .with_protocol(opentelemetry_otlp::Protocol::Grpc)
            .with_metadata(metadata.clone())
            // .with_timeout(timeout)
            .with_export_config(export_config())
            // .with_tls_config(TODO)
            .build_metrics_exporter(
                Box::new(DefaultAggregationSelector::new()),
                Box::new(DefaultTemporalitySelector::new()),
            )?;
        let log_exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            // .with_channel(channel.clone())
            .with_compression(opentelemetry_otlp::Compression::Gzip)
            // .with_protocol(opentelemetry_otlp::Protocol::Grpc)
            .with_metadata(metadata.clone())
            .with_export_config(export_config())
            // .with_timeout(timeout)
            // .with_tls_config(TODO)
            .build_log_exporter()?;
        let trace_exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            // .with_channel(channel.clone())
            .with_compression(opentelemetry_otlp::Compression::Gzip)
            // .with_protocol(opentelemetry_otlp::Protocol::Grpc)
            .with_metadata(metadata.clone())
            .with_export_config(export_config())
            // .with_timeout(timeout)
            // .with_tls_config(TODO)
            .build_span_exporter()?;
        Ok(Self {
            id,
            config,
            metrics_exporter,
            trace_exporter,
            log_exporter,
        })
    }

    async fn export_metrics(&self, metrics: &mut ResourceMetrics) -> eyre::Result<()> {
        use opentelemetry_sdk::metrics::exporter::PushMetricsExporter;
        if let Err(err) = self.metrics_exporter.export(metrics).await {
            tracing::error!("failed to export metrics: {err}");
        }
        Ok(())
    }

    async fn export_logs<'a>(
        &'a mut self,
        // logs: Vec<std::borrow::Cow<'a, LogData>>,
        logs: LogBatch<'_>,
    ) -> eyre::Result<()> {
        use opentelemetry_sdk::export::logs::LogExporter;
        if let Err(err) = self.log_exporter.export(logs).await {
            tracing::error!("failed to export logs: {err}");
        }
        Ok(())
    }

    async fn export_traces<'a>(&'a mut self, traces: Vec<SpanData>) -> eyre::Result<()> {
        use opentelemetry_sdk::export::trace::SpanExporter;
        if let Err(err) = self.trace_exporter.export(traces).await {
            tracing::error!("failed to export logs: {err}");
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl otel_collector_component::Exporter for OtlpExporter {
    async fn start(
        self: Box<Self>,
        mut shutdown_rx: watch::Receiver<bool>,
        mut metrics: MetricsStream,
    ) -> eyre::Result<()> {
        tracing::debug!("{} is running", self.id);
        loop {
            let (from, resource_metrics) = tokio::select! {
                Some(payload) = metrics.next() => payload,
                _ = shutdown_rx.changed() => break,
            };
            let resource_metrics = match resource_metrics {
                Ok(resource_metrics) => resource_metrics,
                Err(err) => {
                    warn!(
                        "{} received metric error {:?} from {:?}",
                        self.id, err, from
                    );
                    continue;
                }
            };
            trace!(
                "{} received {} metrics from {:?} [queue size = {}]",
                self.id,
                resource_metrics.len(),
                from,
                metrics.len(),
            );
            // TODO: send the metrics here
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {}
}
