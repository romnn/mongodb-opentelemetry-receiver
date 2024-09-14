use crate::{config, ext::GraphExt};
use color_eyre::eyre;
use futures::{Stream, StreamExt};
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
use tokio_stream::wrappers::BroadcastStream;

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceIdentifier {
    Receiver(String),
    Exporter(String),
    Processor(String),
}

impl std::fmt::Display for ServiceIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

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

#[derive()]
pub struct MongoDbReceiver {
    pub id: String,
    pub config: config::MongoDbReceiverConfig,
    pub metrics_tx: broadcast::Sender<MetricPayload>,
    // pub metrics_rx: broadcast::Sender<MetricPayload>,
}

impl std::fmt::Debug for MongoDbReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MongoDbReceiver")
            .field("id", &self.id)
            .field("config", &self.config)
            .finish()
    }
}

pub const BUFFER_SIZE: usize = 1024;

impl MongoDbReceiver {
    pub fn from_config(id: String, config: serde_yaml::Value) -> eyre::Result<Self> {
        let config: config::MongoDbReceiverConfig = serde_yaml::from_value(config)?;
        let (metrics_tx, _) = broadcast::channel(BUFFER_SIZE);
        Ok(Self {
            id,
            config,
            metrics_tx,
        })
    }
}

#[async_trait::async_trait]
impl Receiver for MongoDbReceiver {
    fn id(&self) -> &str {
        &self.id
    }
    async fn start(self: Box<Self>, mut shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        // send data each second
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut interval_stream = tokio_stream::wrappers::IntervalStream::new(interval);
        loop {
            tokio::select! {
                tick = interval_stream.next() => match tick {
                    Some(instant) => {
                        tracing::debug!(queue_size = self.metrics_tx.len(), num_receivers = self.metrics_tx.receiver_count(), "sending tick {instant:?}");
                        self.metrics_tx.send(format!("mock metric: {instant:?}"));
                    }
                    None => break,
                },
                _ = shutdown_rx.changed() => break,
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Producer for MongoDbReceiver {
    fn metrics(&self) -> BroadcastStream<MetricPayload> {
        BroadcastStream::new(self.metrics_tx.subscribe())
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

// TODO:
// #[async_trait::async_trait]
// impl Receiver for OtlpReceiver {
//     fn id(&self) -> &str {
//         &self.id
//     }
//
//     async fn start(&self, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
//         // async fn start(self: Arc<Self>, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
//         Ok(())
//     }
// }

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

        let tls_config = None;
        let channel = build_transport_channel(&export_config(), tls_config)?;

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

    async fn start(
        // &self,
        self: Box<Self>,
        shutdown_rx: watch::Receiver<bool>,
        mut metrics: MetricsStream,
    ) -> eyre::Result<()> {
        tracing::debug!("{} is running", self.id);
        while let Some(metric) = metrics.next().await {
            tracing::debug!("{} received metric {:?}", self.id, metric);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct BatchProcessor {
    pub id: String,
    pub config: config::BatchProcessorConfig,
    pub metrics_tx: broadcast::Sender<MetricPayload>,
}

impl BatchProcessor {
    pub fn from_config(id: String, config: serde_yaml::Value) -> eyre::Result<Self> {
        let config: config::BatchProcessorConfig = serde_yaml::from_value(config)?;
        let (metrics_tx, _) = broadcast::channel(BUFFER_SIZE);
        Ok(Self {
            id,
            config,
            metrics_tx,
        })
    }
}

#[async_trait::async_trait]
impl Processor for BatchProcessor {
    fn id(&self) -> &str {
        &self.id
    }

    async fn start(
        self: Box<Self>,
        shutdown_rx: watch::Receiver<bool>,
        mut metrics: MetricsStream,
    ) -> eyre::Result<()> {
        while let Some((from, metric)) = metrics.next().await {
            tracing::debug!("{} received metric {:?} from {:?}", self.id, metric, from);
            let metric = match metric {
                Ok(metric) => metric,
                Err(err) => {
                    continue;
                }
            };
            if let Err(err) = self.metrics_tx.send(metric) {
                tracing::error!("{} failed to send: {err}", self.service_id());
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Producer for BatchProcessor {
    fn metrics(&self) -> BroadcastStream<MetricPayload> {
        BroadcastStream::new(self.metrics_tx.subscribe())
    }
}

#[async_trait::async_trait]
pub trait Receiver: Producer + std::fmt::Debug + Send + Sync + 'static {
    fn id(&self) -> &str;

    fn service_id(&self) -> ServiceIdentifier {
        ServiceIdentifier::Receiver(self.id().to_string())
    }

    async fn start(self: Box<Self>, shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()>;
}

#[async_trait::async_trait]
pub trait Producer: Send + Sync + 'static {
    fn metrics(&self) -> BroadcastStream<MetricPayload>;
}

#[async_trait::async_trait]
pub trait Processor: Producer + std::fmt::Debug + Send + Sync + 'static {
    fn id(&self) -> &str;

    fn service_id(&self) -> ServiceIdentifier {
        ServiceIdentifier::Processor(self.id().to_string())
    }

    async fn start(
        self: Box<Self>,
        shutdown_rx: watch::Receiver<bool>,
        metrics: MetricsStream,
    ) -> eyre::Result<()>;
}

pub type MetricPayload = String;
pub type TracesPayload = String;
pub type LogPayload = String;

pub type MetricsStream = tokio_stream::StreamMap<ServiceIdentifier, BroadcastStream<MetricPayload>>;

#[async_trait::async_trait]
pub trait Exporter: std::fmt::Debug + Send + Sync + 'static {
    // var (
    //     Type      = component.MustNewType("otlp")
    //     ScopeName = "go.opentelemetry.io/collector/exporter/otlpexporter"
    // )

    fn id(&self) -> &str;

    fn service_id(&self) -> ServiceIdentifier {
        ServiceIdentifier::Exporter(self.id().to_string())
    }

    async fn start(
        self: Box<Self>,
        shutdown_rx: watch::Receiver<bool>,
        metrics: MetricsStream,
    ) -> eyre::Result<()>;
}

#[derive(Debug, strum::Display)]
pub enum Service {
    Receiver(Box<dyn Receiver>),
    Processor(Box<dyn Processor>),
    Exporter(Box<dyn Exporter>),
}

impl Service {
    pub fn metrics(&self) -> Option<BroadcastStream<MetricPayload>> {
        match self {
            Self::Receiver(receiver) => Some(receiver.metrics()),
            Self::Processor(processor) => Some(processor.metrics()),
            // exporters do not produce metrics
            Self::Exporter(_) => None,
        }
    }

    pub fn service_id(&self) -> ServiceIdentifier {
        match self {
            Self::Receiver(receiver) => receiver.service_id(),
            Self::Processor(processor) => processor.service_id(),
            Self::Exporter(exporter) => exporter.service_id(),
        }
    }
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
        // service: String,
        service: &str,
        raw_config: serde_yaml::Value,
    ) -> Result<Service, ServiceError> {
        match (unique_id.kind(), service) {
            (ServiceKind::Processor, "debug") => {
                todo!()
            }
            (ServiceKind::Exporter, "otlp") => {
                let otlp_exporter = OtlpExporter::from_config(unique_id.to_string(), raw_config)
                    .map_err(|err| ServiceError::Service(err.into()))?;
                // dbg!(&otlp_exporter);
                Ok(Service::Exporter(Box::new(otlp_exporter)))
                // Ok(Service::Exporter(Arc::new(otlp_exporter)))
                // all_exporters.push(Arc::new(otlp_exporter));
            }
            (ServiceKind::Processor, "batch") => {
                let batch_processor =
                    BatchProcessor::from_config(unique_id.to_string(), raw_config)
                        .map_err(|err| ServiceError::Service(err.into()))?;
                // dbg!(&batch_processor);
                Ok(Service::Processor(Box::new(batch_processor)))
                // Ok(Service::Processor(Arc::new(batch_processor)))
                // all_processors.push(Arc::new(batch_processor));
            }
            (ServiceKind::Receiver, "mongodb") => {
                let mongodb_receiver =
                    MongoDbReceiver::from_config(unique_id.to_string(), raw_config)
                        .map_err(|err| ServiceError::Service(err.into()))?;
                // dbg!(&mongodb_receiver);
                Ok(Service::Receiver(Box::new(mongodb_receiver)))
                // Ok(Service::Receiver(Arc::new(mongodb_receiver)))
                // all_receivers.push(Arc::new(mongodb_receiver));
            }
            // (ServiceKind::Receiver, "otlp") => {
            //     let otlp_receiver = OtlpReceiver::from_config(unique_id.to_string(), raw_config)
            //         .map_err(|err| ServiceError::Service(err.into()))?;
            //     // dbg!(&otlp_receiver);
            //     Ok(Service::Receiver(Arc::new(otlp_receiver)))
            //     // all_receivers.push(Arc::new(otlp_receiver));
            // }
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

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::Display)]
pub enum PipelineEdge {
    #[strum(serialize = "metrics")]
    Metrics,
    #[strum(serialize = "logs")]
    Logs,
    #[strum(serialize = "traces")]
    Traces,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum PipelineNode {
    Source,
    Sink,
    Service(ServiceIdentifier),
}

impl std::fmt::Display for PipelineNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source => write!(f, "SOURCE"),
            Self::Sink => write!(f, "SINK"),
            Self::Service(service) => std::fmt::Display::fmt(service, f),
        }
    }
}

pub type PipelineGraph = petgraph::graph::DiGraph<PipelineNode, PipelineEdge>;

#[derive(Debug)]
pub struct Pipelines {
    pub services: HashMap<ServiceIdentifier, Service>,
    pub pipelines: PipelineGraph,
}

impl Pipelines {
    pub fn from_config<B>(mut config: config::Config) -> eyre::Result<Self>
    where
        B: ServiceBuilder,
    {
        let mut services = HashMap::new();

        let mut graph = PipelineGraph::default();

        let source_node = graph.add_node(PipelineNode::Source);
        let sink_node = graph.add_node(PipelineNode::Sink);

        for (pipeline_id, pipeline) in config.service.pipelines.pipelines {
            let edge = match service_id(&pipeline_id).as_deref() {
                Some("metrics") => PipelineEdge::Metrics,
                Some("logs") => PipelineEdge::Logs,
                Some("traces") => PipelineEdge::Traces,
                Some(other) => {
                    tracing::warn!("skipping pipeline {pipeline_id:?} of unknown type {other:?}");
                    continue;
                }
                None => {
                    tracing::warn!("skipping pipeline {pipeline_id:?} with invalid format");
                    continue;
                }
            };

            let receiver_nodes = pipeline
                .receivers
                .iter()
                .map(|receiver_id| {
                    graph.get_or_insert_node(PipelineNode::Service(ServiceIdentifier::Receiver(
                        receiver_id.to_string(),
                    )))
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
                    graph.get_or_insert_node(PipelineNode::Service(ServiceIdentifier::Processor(
                        processor_id.to_string(),
                    )))
                })
                .collect::<Vec<_>>();

            let exporter_nodes = pipeline
                .exporters
                .iter()
                .map(|exporter_id| {
                    graph.get_or_insert_node(PipelineNode::Service(ServiceIdentifier::Exporter(
                        exporter_id.to_string(),
                    )))
                })
                .collect::<Vec<_>>();

            if exporter_nodes.is_empty() {
                tracing::warn!("skipping pipeline {pipeline_id:?} without exporters");
                continue;
            }

            // connect source node to all receivers
            for receiver_node in receiver_nodes.iter() {
                graph.add_edge(source_node, *receiver_node, edge);
            }

            // connect receiver nodes to the first processor
            if let Some(first_processor_node) = processor_nodes.first() {
                for receiver_node in receiver_nodes.iter() {
                    graph.add_edge(*receiver_node, *first_processor_node, edge);
                }
            }

            // chain the processors
            for (last_processor_node, processor_node) in
                processor_nodes.iter().zip(processor_nodes.iter().skip(1))
            {
                graph.add_edge(*last_processor_node, *processor_node, edge);
            }

            // connect the last processor to all exporters
            if let Some(last_processor_node) = processor_nodes.last() {
                for exporter_node in exporter_nodes.iter() {
                    graph.add_edge(*last_processor_node, *exporter_node, edge);
                }
            }

            if processor_nodes.is_empty() {
                // connect each receiver to each exporter
                for receiver_node in receiver_nodes.iter() {
                    for exporter_node in exporter_nodes.iter() {
                        graph.add_edge(*receiver_node, *exporter_node, edge);
                    }
                }
            }

            // connect exporters to sink node
            for exporter_node in exporter_nodes.iter() {
                graph.add_edge(*exporter_node, sink_node, edge);
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
                match B::build_service(&unique_service_id, &service, raw_config) {
                    Ok(service) => {
                        services.insert(unique_service_id, service);
                    }
                    Err(err) => {
                        tracing::error!("failed to build {unique_service_id}: {err}");
                    }
                }
            }
        }

        print_pipeline_paths(&graph);

        Ok(Self {
            services,
            pipelines: graph,
        })
    }
}

fn print_pipeline_paths(graph: &PipelineGraph) {
    tracing::debug!("=== PIPELINES:");
    let mut sources = graph.externals(petgraph::Direction::Incoming);
    let mut sinks = graph.externals(petgraph::Direction::Outgoing);
    let source = sources.next().unwrap();
    assert_eq!(sources.next(), None);
    let sink = sinks.next().unwrap();
    assert_eq!(sinks.next(), None);

    let start_end = [(source, sink)];
    dbg!(start_end);
    let ways: Vec<Vec<&PipelineNode>> = start_end
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

    let unique_ways = std::collections::HashSet::<Vec<&PipelineNode>>::from_iter(ways);

    for path in unique_ways {
        tracing::debug!(
            "{}",
            path.iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(" -> ")
        );
    }
}

pub fn chain_dependencies<Ty, Ix>(
    graph: &petgraph::Graph<PipelineNode, PipelineEdge, Ty, Ix>,
    services: &HashMap<ServiceIdentifier, Service>,
    dependencies: petgraph::graph::Edges<'_, PipelineEdge, Ty, Ix>,
) -> (MetricsStream,)
where
    Ty: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType,
{
    use petgraph::visit::{EdgeRef, NodeRef};
    let mut merged_metrics = MetricsStream::new();
    for dep in dependencies {
        let Some(PipelineNode::Service(dep_id)) = graph.node_weight(dep.target()) else {
            continue;
        };
        let dep_service = services.get(dep_id).unwrap();
        match dep.weight() {
            PipelineEdge::Metrics => {
                if let Some(metrics) = dep_service.metrics() {
                    merged_metrics.insert(dep_service.service_id(), metrics);
                }
            }
            PipelineEdge::Traces => {}
            PipelineEdge::Logs => {}
        }
    }
    (merged_metrics,)
}

#[derive(Debug)]
pub struct PipelineExecutor {
    pub pipelines: Pipelines,
}

impl PipelineExecutor {
    pub async fn start(mut self, mut shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        use futures::stream::FuturesUnordered;
        use futures::stream::StreamExt;
        use tokio::task::JoinHandle;

        let mut task_handles: FuturesUnordered<JoinHandle<(ServiceIdentifier, eyre::Result<()>)>> =
            FuturesUnordered::new();

        let mut pipeline_graph = self.pipelines.pipelines;

        // start with the sinks, then go up to the sources
        // this way, nodes up the pipeline do not exit prematurely due to having zero subscribers
        pipeline_graph.reverse();

        let mut sources = pipeline_graph.externals(petgraph::Direction::Incoming);
        let source = sources.next().unwrap();
        assert_eq!(sources.next(), None);

        let mut bfs = petgraph::visit::Bfs::new(&pipeline_graph, source);
        while let Some(node_idx) = bfs.next(&pipeline_graph) {
            let node = pipeline_graph.node_weight(node_idx).unwrap();
            let PipelineNode::Service(service_id) = node else {
                continue;
            };
            let service = self.pipelines.services.remove(service_id).unwrap();

            tracing::debug!("starting {:?}", pipeline_graph.node_weight(node_idx));

            let dependencies =
                pipeline_graph.edges_directed(node_idx, petgraph::Direction::Outgoing);

            let shutdown_rx_clone = shutdown_rx.clone();
            let service_id_clone = service_id.clone();

            let (metrics,) =
                chain_dependencies(&pipeline_graph, &self.pipelines.services, dependencies);

            // start the service
            match service {
                Service::Receiver(receiver) => {
                    task_handles.push(tokio::spawn(async move {
                        let res = receiver.start(shutdown_rx_clone).await;
                        (service_id_clone, res)
                    }));
                }
                Service::Processor(processor) => {
                    task_handles.push(tokio::spawn(async move {
                        let res = processor.start(shutdown_rx_clone, metrics).await;
                        (service_id_clone, res)
                    }));
                }
                Service::Exporter(exporter) => {
                    task_handles.push(tokio::spawn(async move {
                        let res = exporter.start(shutdown_rx_clone, metrics).await;
                        (service_id_clone, res)
                    }));
                }
            }
        }

        // wait for all tasks to complete
        while let Some(Ok((task_id, res))) = task_handles.next().await {
            match res {
                Err(err) => tracing::error!("{task_id:?} exited with error: {err}"),
                Ok(_) => tracing::warn!("{task_id:?} exited"),
            }
        }

        // wait for shutdown signal
        shutdown_rx.changed().await;
        Ok(())
    }
}
