#![allow(warnings)]

pub mod config;

use color_eyre::eyre;
use futures::{StreamExt, TryStreamExt};
use opentelemetry_sdk::export::logs::LogBatch;
use opentelemetry_sdk::metrics::data::{Metric, ResourceMetrics, ScopeMetrics, Temporality};
use opentelemetry_sdk::{export::trace::SpanData, logs::LogData, metrics::exporter};
use otel_collector_component::factory::ComponentName;
use otel_collector_component::MetricsStream;
use std::sync::Arc;
use tokio::sync::{broadcast, watch};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tracing::{debug, error, trace, warn};

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

pub mod client {
    // use std::sync::Mutex;
    // use opentelemetry_otlp::exporter::to

    use opentelemetry_proto::tonic::collector::metrics::v1::{
        metrics_service_client::MetricsServiceClient, ExportMetricsServiceRequest,
    };
    use opentelemetry_sdk::metrics::data::ResourceMetrics;
    use tonic::{
        codec::CompressionEncoding, metadata::MetadataMap, service::Interceptor,
        transport::Channel, Extensions, Request,
    };

    pub struct TonicMetricsClient {
        client: MetricsServiceClient<Channel>,
        // inner: ClientInner,
        // inner: Mutex<Option<ClientInner>>,
    }

    // pub struct ClientInner {
    //     client: MetricsServiceClient<Channel>,
    //     // interceptor: BoxInterceptor,
    // }

    impl std::fmt::Debug for TonicMetricsClient {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("TonicMetricsClient")
        }
    }

    impl TonicMetricsClient {
        pub(super) fn new(
            channel: Channel,
            // interceptor: BoxInterceptor,
            compression: Option<opentelemetry_otlp::Compression>,
            // compression: Option<CompressionEncoding>,
        ) -> Self {
            let mut client = MetricsServiceClient::new(channel);
            if let Some(compression) = compression {
                let compression = match compression {
                    opentelemetry_otlp::Compression::Gzip => CompressionEncoding::Gzip,
                    opentelemetry_otlp::Compression::Zstd => CompressionEncoding::Zstd,
                };
                client = client
                    .send_compressed(compression.into())
                    .accept_compressed(compression.into());
            }

            TonicMetricsClient {
                client,
                // inner: Mutex::new(Some(ClientInner {
                //     client,
                //     interceptor,
                // })),
            }
        }
    }

    #[async_trait::async_trait]
    pub trait MetricsExporter: Send + Sync + 'static {
        async fn export(
            &mut self,
            metrics: &ResourceMetrics,
        ) -> Result<(), opentelemetry_otlp::Error>;
    }

    // impl opentelemetry_otlp::Metrics MetricsClient for TonicMetricsClient {

    #[async_trait::async_trait]
    impl MetricsExporter for TonicMetricsClient {
        async fn export(
            &mut self,
            metrics: &ResourceMetrics,
        ) -> Result<(), opentelemetry_otlp::Error> {
            // let (m, e, _) = inner
            //     .interceptor
            //     .call(Request::new(()))
            //     .map_err(|e| {
            //         MetricsError::Other(format!("unexpected status while exporting {e:?}"))
            //     })?
            //     .into_parts();

            // let (mut client, metadata, extensions) = self
            //     .inner
            //     .lock()
            //     .map_err(Into::into)
            //     .and_then(|mut inner| match &mut *inner {
            //         Some(inner) => {
            //             let (m, e, _) = inner
            //                 .interceptor
            //                 .call(Request::new(()))
            //                 .map_err(|e| {
            //                     MetricsError::Other(format!(
            //                         "unexpected status while exporting {e:?}"
            //                     ))
            //                 })?
            //                 .into_parts();
            //             Ok((inner.client.clone(), m, e))
            //         }
            //         None => Err(MetricsError::Other("exporter is already shut down".into())),
            //     })?;

            let metadata = MetadataMap::default();
            let extensions = Extensions::default();
            self.client
                .export(Request::from_parts(
                    metadata,
                    extensions,
                    ExportMetricsServiceRequest::from(metrics),
                ))
                .await
                .map_err(opentelemetry_otlp::Error::from)?;

            Ok(())
        }

        // fn shutdown(&self) -> Result<()> {
        //     let _ = self.inner.lock()?.take();
        //
        //     Ok(())
        // }
    }
}

#[derive(Debug)]
pub struct OtlpExporter<ME>
where
    ME: std::fmt::Debug,
{
    pub id: String,
    pub config: config::OtlpExporterConfig,
    // trace_config: Option<sdk::trace::Config>,
    // batch_config: Option<sdk::trace::BatchConfig>,
    trace_exporter: opentelemetry_otlp::SpanExporter,
    log_exporter: opentelemetry_otlp::LogExporter,
    metrics_exporter: ME,
    // metrics_exporter: opentelemetry_otlp::MetricsExporter,
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

// impl OtlpExporter {
// impl<ME> OtlpExporter<ME>
impl OtlpExporter<client::TonicMetricsClient>
// where
//     ME: client::MetricsExporter,
{
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

        let tls_config = None;
        let channel = build_transport_channel(&export_config(), tls_config)?;
        // dbg!(&channel);

        let batch_config = opentelemetry_sdk::trace::BatchConfig::default();
        // userAgent := fmt.Sprintf("%s/%s (%s/%s)",
        //     set.BuildInfo.Description, set.BuildInfo.Version, runtime.GOOS, runtime.GOARCH)

        // TODO: only set settings that make sense here...
        let metrics_exporter = client::TonicMetricsClient::new(
            channel.clone(),
            Some(opentelemetry_otlp::Compression::Gzip),
        );
        // let metrics_exporter = opentelemetry_otlp::new_exporter()
        //     .tonic()
        //     // .with_channel(channel.clone())
        //     .with_compression(opentelemetry_otlp::Compression::Gzip)
        //     // .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        //     .with_metadata(metadata.clone())
        //     // .with_timeout(timeout)
        //     .with_export_config(export_config())
        //     // .with_tls_config(TODO)
        //     .build_metrics_exporter(
        //         Box::new(DefaultAggregationSelector::new()),
        //         Box::new(DefaultTemporalitySelector::new()),
        //     )?;
        let log_exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_channel(channel.clone())
            .with_compression(opentelemetry_otlp::Compression::Gzip)
            // .with_protocol(opentelemetry_otlp::Protocol::Grpc)
            .with_metadata(metadata.clone())
            // .with_export_config(export_config())
            // .with_timeout(timeout)
            // .with_tls_config(TODO)
            .build_log_exporter()?;
        let trace_exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_channel(channel.clone())
            .with_compression(opentelemetry_otlp::Compression::Gzip)
            // .with_protocol(opentelemetry_otlp::Protocol::Grpc)
            .with_metadata(metadata.clone())
            // .with_export_config(export_config())
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
}

// async fn export_metrics(&self, metrics: &mut ResourceMetrics) -> eyre::Result<()> {
// async fn export_metrics(&mut self, metrics: &ResourceMetrics) -> eyre::Result<()> {
// use opentelemetry_sdk::metrics::exporter::PushMetricsExporter;

// let (mut client, metadata, extensions) =
//     self.inner
//         .lock()
//         .map_err(Into::into)
//         .and_then(|mut inner| match &mut *inner {
//             Some(inner) => {
//                 let (m, e, _) = inner
//                     .interceptor
//                     .call(Request::new(()))
//                     .map_err(|e| {
//                         MetricsError::Other(format!(
//                             "unexpected status while exporting {e:?}"
//                         ))
//                     })?
//                     .into_parts();
//                 Ok((inner.client.clone(), m, e))
//             }
//             None => Err(MetricsError::Other("exporter is already shut down".into())),
//         })?;

// self.metrics_exporter.client
//     .export(Request::from_parts(
//         metadata,
//         extensions,
//         ExportMetricsServiceRequest::from(&*metrics),
//     ))
// .await
// .map_err(crate::Error::from)?;

//         use client::MetricsExporter;
//         if let Err(err) = self.metrics_exporter.export(metrics).await {
//             error!("failed to export metrics: {err}");
//         }
//         Ok(())
//     }
//
//
//     async fn export_logs<'a>(
//         &'a mut self,
//         // logs: Vec<std::borrow::Cow<'a, LogData>>,
//         logs: LogBatch<'_>,
//     ) -> eyre::Result<()> {
//         use opentelemetry_sdk::export::logs::LogExporter;
//         if let Err(err) = self.log_exporter.export(logs).await {
//             error!("failed to export logs: {err}");
//         }
//         Ok(())
//     }
//
//     async fn export_traces<'a>(&'a mut self, traces: Vec<SpanData>) -> eyre::Result<()> {
//         use opentelemetry_sdk::export::trace::SpanExporter;
//         if let Err(err) = self.trace_exporter.export(traces).await {
//             error!("failed to export logs: {err}");
//         }
//         Ok(())
//     }
// }

impl<ME> OtlpExporter<ME>
where
    ME: client::MetricsExporter + std::fmt::Debug,
{
    async fn send_metrics(
        &mut self,
        payload: Result<Vec<Arc<ResourceMetrics>>, BroadcastStreamRecvError>,
    ) -> Result<(), opentelemetry_otlp::Error> {
        let resource_metrics = match payload {
            Ok(resource_metrics) => resource_metrics,
            Err(BroadcastStreamRecvError::Lagged(num_skipped)) => {
                warn!("skipped {num_skipped} messages");
                return Ok(());
            }
        };

        // let metrics = resource_metrics.into_iter().flat_map(|metrics| metrics);
        // futures::stream::iter(payload.into_iter().flat_map(|metrics| metrics))
        //     .map(|metric| async {
        //         self.metrics_exporter.export(&*metric).await
        //     })
        //     .buffer_unordered(4)
        //     .try_collect::<Vec<_>>()
        //     .await?;

        // Ok(())

        let mut errors = vec![];
        for metric in resource_metrics {
            use client::MetricsExporter;
            errors.push(self.metrics_exporter.export(&*metric).await);
        }
        errors.into_iter().collect::<Result<_, _>>()?;
        Ok(())
        // metrics.Ok(())
        //     match self.metrics_exporter.export(&*metrics).await {
        //         Ok(()) => {
        //             debug!("otlp send worked");
        //         }
        //         Err(err) => {
        //             error!("failed to export metrics: {err}");
        //         }
        //     }
        // }
    }
}

#[async_trait::async_trait]
impl<ME> otel_collector_component::Exporter for OtlpExporter<ME>
where
    ME: client::MetricsExporter + std::fmt::Debug,
{
    async fn start(
        mut self: Box<Self>,
        mut shutdown_rx: watch::Receiver<bool>,
        mut metrics: MetricsStream,
    ) -> eyre::Result<()> {
        tracing::debug!("{} is running", self.id);
        loop {
            let (from, resource_metrics) = tokio::select! {
                Some(payload) = metrics.next() => payload,
                _ = shutdown_rx.changed() => break,
            };
            trace!(
                "{} received {:?} metrics from {:?} [queue size = {}]",
                self.id,
                resource_metrics.as_ref().ok().map(|metrics| metrics.len()),
                from,
                metrics.len(),
            );
            if let Err(err) = self.send_metrics(resource_metrics).await {
                error!("failed to export metrics: {err}");
            }

            // dbg!(&resource_metrics);

            // TODO: send the metrics here
            // tokio::time::sleep(std::time::Duration::from_secs(10)).await;
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
