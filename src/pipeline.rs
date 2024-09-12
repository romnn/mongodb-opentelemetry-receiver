use crate::config;
use color_eyre::eyre;
use opentelemetry_sdk::{
    export::{logs::LogData, trace::SpanData},
    metrics::{
        data::{ResourceMetrics, Temporality},
        exporter,
    },
};
use petgraph::data::{Build, DataMap};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::{net::unix::pipe::pipe, sync::broadcast};

// #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
// pub enum ServiceIdentifier<'a> {
//     Receiver(&'a str),
//     Processor(&'a str),
//     Exporter(&'a str),
// }
//
// impl<'a> std::fmt::Display for ServiceIdentifier<'a> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         std::fmt::Debug::fmt(self, f)
//     }
// }

// impl<'a> From<ServiceIdentifier<'a>> for ServiceKind {
//     fn from(value: ServiceIdentifier<'a>) -> Self {
//         match value {
//             ServiceIdentifier::Receiver(_) => Self::Receiver,
//             ServiceIdentifier::Processor(_) => Self::Processor,
//             ServiceIdentifier::Exporter(_) => Self::Exporter,
//         }
//     }
// }
//
// impl<'a> ServiceIdentifier<'a> {
//     pub fn id(&self) -> &'a str {
//         match self {
//             Self::Receiver(id) | Self::Exporter(id) | Self::Processor(id) => id,
//         }
//     }
// }

// impl<'a> std::fmt::Display for ServiceIdentifier<'a> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}")
//         match self {
//             Self::Receiver(id) => write!("),
//         }
//     }
// }

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceIdentifier {
    Receiver(String),
    Exporter(String),
    Processor(String),
    // Pipeline(String),
}

// impl<'a> AsRef<ServiceIdentifier<'a>> for ServiceIdentifier {
//     fn as_ref(&self) -> ServiceIdentifier<'a> {

// impl<'a> std::ops::Deref for ServiceIdentifier {
//     type Target = ServiceIdentifier<'a>;
//     fn deref(&self) -> Self::Target {
//         match self {
//             Self::Receiver(id) => ServiceIdentifier::Receiver(id.as_str()),
//             Self::Processor(id) => ServiceIdentifier::Processor(id.as_str()),
//             Self::Exporter(id) => ServiceIdentifier::Exporter(id.as_str()),
//         }
//     }
// }

impl std::fmt::Display for ServiceIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

// impl std::fmt::Display for ServiceIdentifier {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         std::fmt::Debug::fmt(self, f)
//     }
// }

impl From<ServiceIdentifier> for ServiceKind {
    fn from(value: ServiceIdentifier) -> Self {
        match value {
            ServiceIdentifier::Receiver(_) => Self::Receiver,
            ServiceIdentifier::Processor(_) => Self::Processor,
            ServiceIdentifier::Exporter(_) => Self::Exporter,
        }
    }
}

impl ServiceIdentifier {
    pub fn kind(&self) -> ServiceKind {
        match self {
            Self::Receiver(_) => ServiceKind::Receiver,
            Self::Processor(_) => ServiceKind::Processor,
            Self::Exporter(_) => ServiceKind::Exporter,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Receiver(id) | Self::Exporter(id) | Self::Processor(id) => id.as_str(),
        }
    }
}

#[inline]
pub fn service_id(value: &str) -> Option<String> {
    value.split("/").next().map(|id| id.to_ascii_lowercase())
}

#[derive(Debug)]
pub struct MongoDbReceiver {
    pub id: String,
    pub config: config::MongoDbReceiverConfig,
}

impl MongoDbReceiver {
    pub fn from_config(id: String, config: serde_yaml::Value) -> eyre::Result<Self> {
        let config: config::MongoDbReceiverConfig = serde_yaml::from_value(config)?;
        Ok(Self { id, config })
    }
}

#[async_trait::async_trait]
impl Receiver for MongoDbReceiver {
    fn id(&self) -> &str {
        &self.id
    }
    async fn start(&self, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        // async fn start(self: Arc<Self>, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct OtlpReceiver {
    pub id: String,
    pub config: config::OtlpReceiverConfig,
}

impl OtlpReceiver {
    pub fn from_config(id: String, config: serde_yaml::Value) -> eyre::Result<Self> {
        let config: config::OtlpReceiverConfig = serde_yaml::from_value(config)?;
        Ok(Self { id, config })
    }
}

#[async_trait::async_trait]
impl Receiver for OtlpReceiver {
    fn id(&self) -> &str {
        &self.id
    }

    async fn start(&self, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        // async fn start(self: Arc<Self>, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        Ok(())
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

    // return exporterhelper.NewTracesExporter(ctx, set, cfg,
    // 	oce.pushTraces,
    // 	exporterhelper.WithCapabilities(consumer.Capabilities{MutatesData: false}),
    // 	exporterhelper.WithTimeout(oCfg.TimeoutSettings),
    // 	exporterhelper.WithRetry(oCfg.RetryConfig),
    // 	exporterhelper.WithQueue(oCfg.QueueConfig),
    // 	exporterhelper.WithBatcher(oCfg.BatcherConfig),
    // 	exporterhelper.WithStart(oce.start),
    // 	exporterhelper.WithShutdown(oce.shutdown),
    // )
}
// type baseExporter struct {
// 	// Input configuration.
// 	config *Config
//
// 	// gRPC clients and connection.
// 	traceExporter  ptraceotlp.GRPCClient
// 	metricExporter pmetricotlp.GRPCClient
// 	logExporter    plogotlp.GRPCClient
// 	clientConn     *grpc.ClientConn
// 	metadata       metadata.MD
// 	callOptions    []grpc.CallOption
//
// 	settings component.TelemetrySettings
//
// 	// Default user-agent header.
// 	userAgent string
// }
//
// func newExporter(cfg component.Config, set exporter.Settings) *baseExporter {
// 	oCfg := cfg.(*Config)
//
// 	userAgent := fmt.Sprintf("%s/%s (%s/%s)",
// 		set.BuildInfo.Description, set.BuildInfo.Version, runtime.GOOS, runtime.GOARCH)
//
// 	return &baseExporter{config: oCfg, settings: set.TelemetrySettings, userAgent: userAgent}
// }
//
// // start actually creates the gRPC connection. The client construction is deferred till this point as this
// // is the only place we get hold of Extensions which are required to construct auth round tripper.
// func (e *baseExporter) start(ctx context.Context, host component.Host) (err error) {
// 	if e.clientConn, err = e.config.ClientConfig.ToClientConn(ctx, host, e.settings, grpc.WithUserAgent(e.userAgent)); err != nil {
// 		return err
// 	}
// 	e.traceExporter = ptraceotlp.NewGRPCClient(e.clientConn)
// 	e.metricExporter = pmetricotlp.NewGRPCClient(e.clientConn)
// 	e.logExporter = plogotlp.NewGRPCClient(e.clientConn)
// 	headers := map[string]string{}
// 	for k, v := range e.config.ClientConfig.Headers {
// 		headers[k] = string(v)
// 	}
// 	e.metadata = metadata.New(headers)
// 	e.callOptions = []grpc.CallOption{
// 		grpc.WaitForReady(e.config.ClientConfig.WaitForReady),
// 	}
//
// 	return
// }
//
// func (e *baseExporter) shutdown(context.Context) error {
// 	if e.clientConn != nil {
// 		return e.clientConn.Close()
// 	}
// 	return nil
// }
//
// func (e *baseExporter) pushTraces(ctx context.Context, td ptrace.Traces) error {
// 	req := ptraceotlp.NewExportRequestFromTraces(td)
// 	resp, respErr := e.traceExporter.Export(e.enhanceContext(ctx), req, e.callOptions...)
// 	if err := processError(respErr); err != nil {
// 		return err
// 	}
// 	partialSuccess := resp.PartialSuccess()
// 	if !(partialSuccess.ErrorMessage() == "" && partialSuccess.RejectedSpans() == 0) {
// 		e.settings.Logger.Warn("Partial success response",
// 			zap.String("message", resp.PartialSuccess().ErrorMessage()),
// 			zap.Int64("dropped_spans", resp.PartialSuccess().RejectedSpans()),
// 		)
// 	}
// 	return nil
// }
//
// func (e *baseExporter) pushMetrics(ctx context.Context, md pmetric.Metrics) error {
// 	req := pmetricotlp.NewExportRequestFromMetrics(md)
// 	resp, respErr := e.metricExporter.Export(e.enhanceContext(ctx), req, e.callOptions...)
// 	if err := processError(respErr); err != nil {
// 		return err
// 	}
// 	partialSuccess := resp.PartialSuccess()
// 	if !(partialSuccess.ErrorMessage() == "" && partialSuccess.RejectedDataPoints() == 0) {
// 		e.settings.Logger.Warn("Partial success response",
// 			zap.String("message", resp.PartialSuccess().ErrorMessage()),
// 			zap.Int64("dropped_data_points", resp.PartialSuccess().RejectedDataPoints()),
// 		)
// 	}
// 	return nil
// }
//
// func (e *baseExporter) pushLogs(ctx context.Context, ld plog.Logs) error {
// 	req := plogotlp.NewExportRequestFromLogs(ld)
// 	resp, respErr := e.logExporter.Export(e.enhanceContext(ctx), req, e.callOptions...)
// 	if err := processError(respErr); err != nil {
// 		return err
// 	}
// 	partialSuccess := resp.PartialSuccess()
// 	if !(partialSuccess.ErrorMessage() == "" && partialSuccess.RejectedLogRecords() == 0) {
// 		e.settings.Logger.Warn("Partial success response",
// 			zap.String("message", resp.PartialSuccess().ErrorMessage()),
// 			zap.Int64("dropped_log_records", resp.PartialSuccess().RejectedLogRecords()),
// 		)
// 	}
// 	return nil
// }
//
// func (e *baseExporter) enhanceContext(ctx context.Context) context.Context {
// 	if e.metadata.Len() > 0 {
// 		return metadata.NewOutgoingContext(ctx, e.metadata)
// 	}
// 	return ctx
// }
//
// func processError(err error) error {
// 	if err == nil {
// 		// Request is successful, we are done.
// 		return nil
// 	}
//
// 	// We have an error, check gRPC status code.
// 	st := status.Convert(err)
// 	if st.Code() == codes.OK {
// 		// Not really an error, still success.
// 		return nil
// 	}
//
// 	// Now, this is a real error.
// 	retryInfo := getRetryInfo(st)
//
// 	if !shouldRetry(st.Code(), retryInfo) {
// 		// It is not a retryable error, we should not retry.
// 		return consumererror.NewPermanent(err)
// 	}
//
// 	// Check if server returned throttling information.
// 	throttleDuration := getThrottleDuration(retryInfo)
// 	if throttleDuration != 0 {
// 		// We are throttled. Wait before retrying as requested by the server.
// 		return exporterhelper.NewThrottleRetry(err, throttleDuration)
// 	}
//
// 	// Need to retry.
// 	return err
// }
//
// func shouldRetry(code codes.Code, retryInfo *errdetails.RetryInfo) bool {
// 	switch code {
// 	case codes.Canceled,
// 		codes.DeadlineExceeded,
// 		codes.Aborted,
// 		codes.OutOfRange,
// 		codes.Unavailable,
// 		codes.DataLoss:
// 		// These are retryable errors.
// 		return true
// 	case codes.ResourceExhausted:
// 		// Retry only if RetryInfo was supplied by the server.
// 		// This indicates that the server can still recover from resource exhaustion.
// 		return retryInfo != nil
// 	}
// 	// Don't retry on any other code.
// 	return false
// }
//
// func getRetryInfo(status *status.Status) *errdetails.RetryInfo {
// 	for _, detail := range status.Details() {
// 		if t, ok := detail.(*errdetails.RetryInfo); ok {
// 			return t
// 		}
// 	}
// 	return nil
// }
//
// func getThrottleDuration(t *errdetails.RetryInfo) time.Duration {
// 	if t == nil || t.RetryDelay == nil {
// 		return 0
// 	}
// 	if t.RetryDelay.Seconds > 0 || t.RetryDelay.Nanos > 0 {
// 		return time.Duration(t.RetryDelay.Seconds)*time.Second + time.Duration(t.RetryDelay.Nanos)*time.Nanosecond
// 	}
// 	return 0
// }

fn build_transport_channel(
    config: &opentelemetry_otlp::ExportConfig,
    tls_config: Option<tonic::transport::ClientTlsConfig>,
) -> eyre::Result<tonic::transport::Channel> {
    // let config = self.exporter_config;

    // resolving endpoint string
    // grpc doesn't have a "path" like http(See https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md)
    // the path of grpc calls are based on the protobuf service definition
    // so we won't append one for default grpc endpoints
    // If users for some reason want to use a custom path, they can use env var or builder to pass it
    // let endpoint = match env::var(signal_endpoint_var)
    //     .ok()
    //     .or(env::var(OTEL_EXPORTER_OTLP_ENDPOINT).ok())
    // {
    //     Some(val) => val,
    //     None => {
    //         if config.endpoint.is_empty() {
    //             OTEL_EXPORTER_OTLP_GRPC_ENDPOINT_DEFAULT.to_string()
    //         } else {
    //             config.endpoint
    //         }
    //     }
    // };

    let endpoint = tonic::transport::Channel::from_shared(config.endpoint.clone())?;
    // .map_err(crate::Error::from)?;
    // let timeout = match env::var(signal_timeout_var)
    //     .ok()
    //     .or(env::var(OTEL_EXPORTER_OTLP_TIMEOUT).ok())
    // {
    //     Some(val) => match val.parse() {
    //         Ok(seconds) => Duration::from_secs(seconds),
    //         Err(_) => config.timeout,
    //     },
    //     None => config.timeout,
    // };

    // #[cfg(feature = "tls")]
    let channel = match tls_config {
        Some(tls_config) => endpoint.tls_config(tls_config)?,
        // .map_err(crate::Error::from)?,
        None => endpoint,
    };

    Ok(channel.timeout(config.timeout).connect_lazy())

    // #[cfg(not(feature = "tls"))]
    // let channel = endpoint.timeout(timeout).connect_lazy();
}

impl OtlpExporter {
    pub fn from_config(id: String, config: serde_yaml::Value) -> eyre::Result<Self> {
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_sdk::metrics::reader::{
            DefaultAggregationSelector, DefaultTemporalitySelector,
        };

        let config: config::OtlpExporterConfig = serde_yaml::from_value(config)?;

        // use tonic::metadata::{KeyAndValueRef, MetadataMap};
        let metadata = tonic::metadata::MetadataMap::default();
        // let timeout = std::time::Duration::from_secs(60);
        // let user_agent = "";

        let export_config = || opentelemetry_otlp::ExportConfig {
            endpoint: config.endpoint.clone(),
            protocol: opentelemetry_otlp::Protocol::Grpc,
            timeout: std::time::Duration::from_secs(60),
        };

        let tls_config = None;
        let channel = build_transport_channel(&export_config(), tls_config)?;

        // TODO: create tls config from config
        // let tls_config = tonic::transport::ClientTlsConfig {};

        // let trace_config: Option<sdk::trace::Config>,
        // let trace_config = opentelemetry_sdk::trace::Trace::default();
        // let trace_config = opentelemetry_sdk::meter::Trace::default();
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
        logs: Vec<std::borrow::Cow<'a, LogData>>,
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
impl Exporter for OtlpExporter {
    fn id(&self) -> &str {
        &self.id
    }

    async fn start(&self, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        // async fn start(self: Arc<Self>, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct BatchProcessor {
    pub id: String,
    pub config: config::BatchProcessorConfig,
}

impl BatchProcessor {
    pub fn from_config(id: String, config: serde_yaml::Value) -> eyre::Result<Self> {
        let config: config::BatchProcessorConfig = serde_yaml::from_value(config)?;
        Ok(Self { id, config })
    }
}

#[async_trait::async_trait]
impl Processor for BatchProcessor {
    fn id(&self) -> &str {
        &self.id
    }

    async fn start(&self, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        // async fn start(self: Arc<Self>, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait Receiver: std::fmt::Debug + Send + Sync + 'static {
    fn id(&self) -> &str;
    async fn start(&self, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()>;
    // fn logs(&self) -> Option<broadcast::Receiver<LogRecord>>;
    // async fn start(self: Arc<Self>, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()>;
}

#[async_trait::async_trait]
pub trait Processor: std::fmt::Debug + Send + Sync + 'static {
    fn id(&self) -> &str;
    async fn start(&self, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()>;
    // async fn start(self: Arc<Self>, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()>;
}

#[async_trait::async_trait]
pub trait Exporter: std::fmt::Debug + Send + Sync + 'static {
    fn id(&self) -> &str;
    async fn start(&self, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()>;
    // async fn start(self: Arc<Self>, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()>;
}

#[derive(Debug, strum::Display)]
pub enum Service {
    Receiver(Arc<dyn Receiver>),
    Processor(Arc<dyn Processor>),
    Exporter(Arc<dyn Exporter>),
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::Display)]
pub enum ServiceKind {
    #[strum(serialize = "receiver")]
    Receiver,
    #[strum(serialize = "processor")]
    Processor,
    #[strum(serialize = "exporter")]
    Exporter,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("missing {kind} {id:?}")]
    MissingService { kind: ServiceKind, id: String },
    #[error("invalid {kind} identifier {id:?}")]
    InvalidFormat { kind: ServiceKind, id: String },
    #[error("unknown {kind} {id:?}")]
    UnknownService { kind: ServiceKind, id: String },
    #[error(transparent)]
    Service(Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub struct BuiltinServiceBuilder {}

#[async_trait::async_trait]
pub trait ServiceBuilder {
    fn build_service(
        unique_id: &ServiceIdentifier,
        service: String,
        raw_config: serde_yaml::Value,
    ) -> Result<Service, ServiceError> {
        match (unique_id.kind(), unique_id.id()) {
            (ServiceKind::Processor, "debug") => {
                todo!()
            }
            (ServiceKind::Exporter, "otlp") => {
                let otlp_exporter = OtlpExporter::from_config(unique_id.to_string(), raw_config)
                    .map_err(|err| ServiceError::Service(err.into()))?;
                dbg!(&otlp_exporter);
                Ok(Service::Exporter(Arc::new(otlp_exporter)))
                // all_exporters.push(Arc::new(otlp_exporter));
            }
            (ServiceKind::Processor, "batch") => {
                let batch_processor =
                    BatchProcessor::from_config(unique_id.to_string(), raw_config)
                        .map_err(|err| ServiceError::Service(err.into()))?;
                dbg!(&batch_processor);
                Ok(Service::Processor(Arc::new(batch_processor)))
                // all_processors.push(Arc::new(batch_processor));
            }
            (ServiceKind::Receiver, "mongodb") => {
                let mongodb_receiver =
                    MongoDbReceiver::from_config(unique_id.to_string(), raw_config)
                        .map_err(|err| ServiceError::Service(err.into()))?;
                dbg!(&mongodb_receiver);
                Ok(Service::Receiver(Arc::new(mongodb_receiver)))
                // all_receivers.push(Arc::new(mongodb_receiver));
            }
            (ServiceKind::Receiver, "otlp") => {
                let otlp_receiver = OtlpReceiver::from_config(unique_id.to_string(), raw_config)
                    .map_err(|err| ServiceError::Service(err.into()))?;
                dbg!(&otlp_receiver);
                Ok(Service::Receiver(Arc::new(otlp_receiver)))
                // all_receivers.push(Arc::new(otlp_receiver));
            }
            (kind, service) => {
                tracing::warn!("unknown {kind}: {service:?}");
                Err(ServiceError::UnknownService {
                    kind,
                    id: unique_id.to_string(),
                }
                .into())
            }
        }
    }
}

#[async_trait::async_trait]
impl ServiceBuilder for BuiltinServiceBuilder {}

pub type PipelineGraph = petgraph::graph::DiGraph<ServiceIdentifier, ()>;

#[derive(Debug)]
pub struct Pipelines {
    pub services: HashMap<ServiceIdentifier, Service>,
    pub pipelines: HashMap<String, PipelineGraph>,
}

impl Pipelines {
    pub fn from_config<B>(mut config: config::Config) -> eyre::Result<Self>
    where
        B: ServiceBuilder,
    {
        let mut services = HashMap::new();

        // build the graph first
        let mut pipelines = HashMap::new();

        for (pipeline_id, pipeline) in config.service.pipelines.pipelines {
            if pipelines.contains_key(&pipeline_id) {
                tracing::warn!("skipping duplicate pipeline {pipeline_id:?}");
                continue;
            }

            let mut graph = PipelineGraph::default();
            let receiver_nodes = pipeline
                .receivers
                .iter()
                .map(|receiver_id| {
                    graph.add_node(ServiceIdentifier::Receiver(receiver_id.to_string()))
                })
                .collect::<Vec<_>>();

            if receiver_nodes.is_empty() {
                tracing::warn!("skipping pipeline {pipeline_id:?} without receivers");
                continue;
            };

            let processor_nodes = pipeline
                .processors
                .iter()
                .map(|processor_id| {
                    graph.add_node(ServiceIdentifier::Processor(processor_id.to_string()))
                })
                .collect::<Vec<_>>();

            let exporter_nodes = pipeline
                .exporters
                .iter()
                .map(|exporter_id| {
                    graph.add_node(ServiceIdentifier::Exporter(exporter_id.to_string()))
                })
                .collect::<Vec<_>>();

            if exporter_nodes.is_empty() {
                tracing::warn!("skipping pipeline {pipeline_id:?} without exporters");
                continue;
            }

            // connect receiver nodes to the first processor
            if let Some(first_processor_node) = processor_nodes.first() {
                for receiver_node in receiver_nodes.iter() {
                    graph.add_edge(*receiver_node, *first_processor_node, ());
                }
            }

            // chain the processors
            for (last_processor_node, processor_node) in
                processor_nodes.iter().zip(processor_nodes.iter().skip(1))
            {
                graph.add_edge(*last_processor_node, *processor_node, ());
            }

            // connect the last processor to all exporters
            if let Some(last_processor_node) = processor_nodes.last() {
                for exporter_node in exporter_nodes.iter() {
                    graph.add_edge(*last_processor_node, *exporter_node, ());
                }
            }

            if processor_nodes.is_empty() {
                // connect each receiver to each exporter
                for receiver_node in receiver_nodes.iter() {
                    for exporter_node in exporter_nodes.iter() {
                        graph.add_edge(*receiver_node, *exporter_node, ());
                    }
                }
            }

            // we want to build mutliple pipeline graphs, but for now just construct
            let pipeline_receivers = pipeline
                .receivers
                .into_iter()
                .map(|id| ServiceIdentifier::Receiver(id));
            let pipeline_processors = pipeline
                .processors
                .into_iter()
                .map(|id| ServiceIdentifier::Processor(id));
            let pipeline_exporters = pipeline
                .exporters
                .into_iter()
                .map(|id| ServiceIdentifier::Exporter(id));

            for unique_service_id in pipeline_receivers
                .chain(pipeline_processors)
                .chain(pipeline_exporters)
            {
                if services.contains_key(&unique_service_id) {
                    continue;
                }
                let unique_id = unique_service_id.id();
                let service =
                    service_id(&unique_id).ok_or_else(|| ServiceError::InvalidFormat {
                        kind: unique_service_id.kind(),
                        id: unique_id.to_string(),
                    })?;
                let raw_config = match &unique_service_id {
                    ServiceIdentifier::Receiver(receiver_id) => {
                        config.receivers.receivers.remove(receiver_id)
                    }
                    ServiceIdentifier::Processor(processor_id) => {
                        config.processors.processors.remove(processor_id)
                    }
                    ServiceIdentifier::Exporter(exporter_id) => {
                        config.exporters.exporters.remove(exporter_id)
                    }
                };
                let raw_config = raw_config.ok_or_else(|| ServiceError::MissingService {
                    kind: unique_service_id.kind(),
                    id: unique_id.to_string(),
                })?;

                // build the service
                match B::build_service(&unique_service_id, service, raw_config) {
                    Ok(service) => {
                        services.insert(unique_service_id, service);
                    }
                    Err(err) => {
                        tracing::error!("failed to build {unique_service_id}: {err}");
                    }
                }
            }
            // for receiver_id in pipeline.receivers {
            //     let kind = ServiceKind::Receiver;
            //     let service =
            //         service_id(&receiver_id).ok_or_else(|| ServiceError::InvalidFormat {
            //             kind,
            //             id: receiver_id.clone(),
            //         })?;
            //     let raw_config =
            //         config
            //             .receivers
            //             .receivers
            //             .remove(&receiver_id)
            //             .ok_or_else(|| ServiceError::MissingService {
            //                 kind,
            //                 id: receiver_id.clone(),
            //             })?;
            //     match B::build_service(ServiceKind::Receiver, service, &receiver_id, raw_config) {
            //         Ok(service) => {
            //             services.insert(ServiceIdentifier::Receiver(receiver_id), service);
            //         }
            //         Err(err) => {
            //             tracing::error!("failed to build {receiver_id}");
            //         }
            //     }
            // }

            // TODO: find all paths to leafs using dfs here, but for now its fine
            let start_end: Vec<(petgraph::graph::NodeIndex, petgraph::graph::NodeIndex)> =
                receiver_nodes
                    .iter()
                    .flat_map(|receiver_node| {
                        exporter_nodes
                            .iter()
                            .map(|exporter_node| (*receiver_node, *exporter_node))
                    })
                    .collect();

            let ways: Vec<Vec<&ServiceIdentifier>> = start_end
                .iter()
                .flat_map(|(receiver_node, exporter_node)| {
                    petgraph::algo::all_simple_paths::<Vec<_>, _>(
                        &graph,
                        *receiver_node,
                        *exporter_node,
                        0,
                        None,
                    )
                })
                .map(|path| {
                    path.into_iter()
                        .map(|idx| graph.node_weight(idx).unwrap())
                        .collect()
                })
                .collect();

            let unique_ways = std::collections::HashSet::<Vec<&ServiceIdentifier>>::from_iter(ways);

            for path in unique_ways {
                tracing::trace!(
                    "{}",
                    path.iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                );
            }

            pipelines.insert(pipeline_id, graph);
        }

        // dbg!(&pipelines);

        Ok(Self {
            services,
            pipelines,
            // receivers: all_receivers
            //     .into_iter()
            //     .map(|v| (v.id().to_string(), v))
            //     .collect(),
            // processors: all_processors
            //     .into_iter()
            //     .map(|v| (v.id().to_string(), v))
            //     .collect(),
            // exporters: all_exporters
            //     .into_iter()
            //     .map(|v| (v.id().to_string(), v))
            //     .collect(),
        })
    }
}

#[derive(Debug)]
pub struct PipelineExecutor {
    pub pipelines: Pipelines,
}

impl PipelineExecutor {
    pub async fn start(&self, mut shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        use futures::stream::FuturesUnordered;
        use futures::stream::StreamExt;
        use tokio::task::JoinHandle;

        let mut task_handles: FuturesUnordered<JoinHandle<(ServiceIdentifier, eyre::Result<()>)>> =
            FuturesUnordered::new();

        // for exporter in self.exporters.values() {
        //     let shutdown_rx_clone = shutdown_rx.clone();
        //     let exporter_clone = exporter.clone();
        //     task_handles.push(tokio::spawn(async move {
        //         let task_id = ServiceIdentifier::Exporter(exporter_clone.id().to_string());
        //         tracing::debug!("starting {task_id:?}");
        //         let res = exporter_clone.start(shutdown_rx_clone).await;
        //         (task_id, res)
        //     }));
        // }
        //
        // for processor in self.processors.values() {
        //     let shutdown_rx_clone = shutdown_rx.clone();
        //     let processor_clone = processor.clone();
        //     task_handles.push(tokio::spawn(async move {
        //         let task_id = ServiceIdentifier::Processor(processor_clone.id().to_string());
        //         tracing::debug!("starting {task_id:?}");
        //         let res = processor_clone.start(shutdown_rx_clone).await;
        //         (task_id, res)
        //     }));
        // }
        //
        // for receiver in self.receivers.values() {
        //     let shutdown_rx_clone = shutdown_rx.clone();
        //     let receiver_clone = receiver.clone();
        //     task_handles.push(tokio::spawn(async move {
        //         let task_id = ServiceIdentifier::Receiver(receiver_clone.id().to_string());
        //         tracing::debug!("starting {task_id:?}");
        //         let res = receiver_clone.start(shutdown_rx_clone).await;
        //         (task_id, res)
        //     }));
        // }
        //
        // // setup the pipelines
        // for receiver in self.services.values() {}

        // wait for all tasks to complete
        while let Some(Ok((task_id, res))) = task_handles.next().await {
            match res {
                Err(err) => tracing::error!("{task_id:?} exited with error: {err}"),
                Ok(_) => tracing::debug!("{task_id:?} exited"),
            }
        }

        // wait for shutdown signal
        shutdown_rx.changed().await;
        Ok(())
    }
}
