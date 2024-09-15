use crate::factory::{ExporterFactory, ProcessorFactory, ReceiverFactory};
use crate::{config, MetricPayload, ServiceError, ServiceIdentifier, ServiceKind};
use color_eyre::eyre;
use futures::{Stream, StreamExt};
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::Resource;
use petgraph::data::{Build, DataMap};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::{net::unix::pipe::pipe, sync::broadcast};
use tokio_stream::wrappers::BroadcastStream;
use tracing::debug;

pub trait GraphExt<N, E, Ix> {
    fn get_or_insert_node(&mut self, weight: N) -> petgraph::graph::NodeIndex<Ix>
    where
        N: PartialEq;
}

impl<N, E, D, Ix> GraphExt<N, E, Ix> for petgraph::Graph<N, E, D, Ix>
where
    D: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType,
{
    fn get_or_insert_node(&mut self, weight: N) -> petgraph::graph::NodeIndex<Ix>
    where
        N: PartialEq,
    {
        let found = self
            .node_indices()
            .find(|idx| match self.node_weight(*idx) {
                Some(node) => weight == *node,
                None => false,
            });
        match found {
            Some(found) => found,
            None => petgraph::Graph::<N, E, D, Ix>::add_node(self, weight),
        }
    }
}

#[derive(Debug)]
pub struct MonotonicSequenceReceiver {
    pub id: String,
    pub metrics_tx: broadcast::Sender<MetricPayload>,
    pub interval: std::time::Duration,
}

pub const DEFAULT_BUFFER_SIZE: usize = 1024;

impl MonotonicSequenceReceiver {
    pub fn new(id: String, interval: std::time::Duration) -> Self {
        let (metrics_tx, _) = broadcast::channel(DEFAULT_BUFFER_SIZE);
        Self {
            id,
            metrics_tx,
            interval,
        }
    }
}

#[async_trait::async_trait]
impl crate::Receiver for MonotonicSequenceReceiver {
    async fn start(self: Box<Self>, mut shutdown_rx: watch::Receiver<bool>) -> eyre::Result<()> {
        // send data each second
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut interval_stream = tokio_stream::wrappers::IntervalStream::new(interval);
        let mut sequence_number = 0;
        loop {
            tokio::select! {
                _ = interval_stream.next() => (),
                _ = shutdown_rx.changed() => break,
            };

            tracing::debug!(
                queue_size = self.metrics_tx.len(),
                num_receivers = self.metrics_tx.receiver_count(),
                "sending sequence number {sequence_number}"
            );
            let metrics = opentelemetry_sdk::metrics::data::ResourceMetrics {
                resource: opentelemetry_sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                    "sequence_number",
                    sequence_number,
                )]),
                scope_metrics: vec![],
            };
            self.metrics_tx.send(Arc::new(vec![metrics]));
            sequence_number += 1;
        }
        Ok(())
    }
}

impl crate::Producer for MonotonicSequenceReceiver {
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

#[derive(Debug, strum::Display)]
pub enum Service {
    Receiver(Box<dyn crate::Receiver>),
    Processor(Box<dyn crate::Processor>),
    Exporter(Box<dyn crate::Exporter>),
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

    pub fn kind(&self) -> ServiceKind {
        match self {
            Self::Receiver(_) => ServiceKind::Receiver,
            Self::Processor(_) => ServiceKind::Processor,
            Self::Exporter(_) => ServiceKind::Exporter,
        }
    }

    // pub fn to_service_id(&self) -> ServiceIdentifier {
    //     match self {
    //         Self::Receiver(receiver) => receiver.to_service_id(),
    //         Self::Processor(processor) => processor.to_service_id(),
    //         Self::Exporter(exporter) => exporter.to_service_id(),
    //     }
    // }
    //
    // pub fn id(&self) -> &str {
    //     match self {
    //         Self::Receiver(receiver) => receiver.id(),
    //         Self::Processor(processor) => processor.id(),
    //         Self::Exporter(exporter) => exporter.id(),
    //     }
    // }
}

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

pub enum Factory {
    Receiver(Box<dyn ReceiverFactory>),
    Processor(Box<dyn ProcessorFactory>),
    Exporter(Box<dyn ExporterFactory>),
}

pub type FactoryMap = HashMap<ServiceIdentifier, Factory>;

pub struct PipelineBuilder {
    pub factories: FactoryMap,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        Self {
            factories: FactoryMap::new(),
        }
    }

    pub fn with_receiver(mut self, factory: impl ReceiverFactory + 'static) -> Self {
        self.factories.insert(
            ServiceIdentifier::Receiver(factory.component_name().into()),
            Factory::Receiver(Box::new(factory)),
        );
        self
    }

    pub fn with_processors(
        mut self,
        factories: impl IntoIterator<Item = Box<dyn ProcessorFactory>>,
    ) -> Self {
        self.factories.extend(factories.into_iter().map(|factory| {
            (
                ServiceIdentifier::Processor(factory.component_name().into()),
                Factory::Processor(factory),
            )
        }));
        self
    }

    pub fn with_processor(mut self, factory: impl ProcessorFactory + 'static) -> Self {
        self.factories.insert(
            ServiceIdentifier::Processor(factory.component_name().into()),
            Factory::Processor(Box::new(factory)),
        );
        self
    }

    pub fn with_exporters(
        mut self,
        factories: impl IntoIterator<Item = Box<dyn ExporterFactory>>,
    ) -> Self {
        self.factories.extend(factories.into_iter().map(|factory| {
            (
                ServiceIdentifier::Exporter(factory.component_name().into()),
                Factory::Exporter(factory),
            )
        }));
        self
    }

    pub fn with_exporter(mut self, factory: impl ExporterFactory + 'static) -> Self {
        self.factories.insert(
            ServiceIdentifier::Exporter(factory.component_name().into()),
            Factory::Exporter(Box::new(factory)),
        );
        self
    }

    pub async fn build(mut self, config: config::Config) -> eyre::Result<Pipelines> {
        let (graph, source_node) = build_pipeline_graph(&config)?;

        if graph.node_indices().len() == 2 {
            // pipeline is empty: only have source and sink
            // TODO: should we fail or connect source to sink and do nothing?
            eyre::bail!("empty pipeline");
        }

        let services = build_services(&graph, &self.factories, config).await?;

        print_pipeline_paths(&graph);

        Ok(Pipelines {
            services,
            pipelines: graph,
        })
    }
}

pub type ServiceMap = HashMap<ServiceIdentifier, Service>;

#[derive(Debug)]
pub struct Pipelines {
    pub services: ServiceMap,
    pub pipelines: PipelineGraph,
}

pub async fn build_services(
    graph: &PipelineGraph,
    factories: &FactoryMap,
    mut config: config::Config,
) -> eyre::Result<ServiceMap> {
    let mut services = HashMap::new();
    for node in graph.node_weights() {
        let PipelineNode::Service(unique_service_id) = node else {
            continue;
        };
        let unique_id = unique_service_id.id();
        let service_id = unique_service_id.to_service_id()?;

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
        let missing_service_error = || ServiceError::MissingService {
            kind: unique_service_id.kind(),
            id: unique_id.to_string(),
        };
        let raw_config = raw_config.ok_or_else(missing_service_error)?;

        let service = match factories.get(&service_id) {
            Some(Factory::Receiver(factory)) => {
                Service::Receiver(factory.build(unique_id.to_string(), raw_config).await?)
            }
            Some(Factory::Processor(factory)) => {
                Service::Processor(factory.build(unique_id.to_string(), raw_config).await?)
            }
            Some(Factory::Exporter(factory)) => {
                Service::Exporter(factory.build(unique_id.to_string(), raw_config).await?)
            }
            None => {
                return Err(missing_service_error().into());
            }
        };

        assert_eq!(unique_service_id.kind(), service.kind());
        services.insert(unique_service_id.clone(), service);
    }
    Ok(services)
}

pub fn build_pipeline_graph(
    config: &config::Config,
) -> eyre::Result<(PipelineGraph, petgraph::graph::NodeIndex)> {
    let mut graph = PipelineGraph::default();

    let source_node = graph.add_node(PipelineNode::Source);
    let sink_node = graph.add_node(PipelineNode::Sink);

    for (pipeline_id, pipeline) in config.service.pipelines.pipelines.iter() {
        let edge = match crate::service_id(&pipeline_id).as_deref() {
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
    }
    Ok((graph, source_node))
}

fn print_pipeline_paths(graph: &PipelineGraph) {
    let mut sources = graph.externals(petgraph::Direction::Incoming);
    let mut sinks = graph.externals(petgraph::Direction::Outgoing);
    debug!(
        "sources: {:?}",
        sources
            .clone()
            .filter_map(|node_idx| graph.node_weight(node_idx))
            .map(|node| node.to_string())
            .collect::<Vec<_>>()
    );
    debug!(
        "sinks: {:?}",
        sinks
            .clone()
            .filter_map(|node_idx| graph.node_weight(node_idx))
            .map(|node| node.to_string())
            .collect::<Vec<_>>()
    );
    let source = sources.next().unwrap();
    assert_eq!(sources.next(), None);
    let sink = sinks.next().unwrap();
    assert_eq!(sinks.next(), None);

    let paths: Vec<Vec<_>> =
        petgraph::algo::all_simple_paths(&graph, source, sink, 0, None).collect();

    for path in paths {
        debug!(
            "pipeline: {}",
            path.into_iter()
                .map(|node_idx| graph.node_weight(node_idx).unwrap().to_string())
                .collect::<Vec<_>>()
                .join(" -> ")
        );
    }
}

pub fn chain_dependencies<Ty, Ix>(
    graph: &petgraph::Graph<PipelineNode, PipelineEdge, Ty, Ix>,
    services: &HashMap<ServiceIdentifier, Service>,
    dependencies: petgraph::graph::Edges<'_, PipelineEdge, Ty, Ix>,
) -> (crate::MetricsStream,)
where
    Ty: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType,
{
    use petgraph::visit::{EdgeRef, NodeRef};
    let mut merged_metrics = crate::MetricsStream::new();
    for dep in dependencies {
        let Some(PipelineNode::Service(dep_id)) = graph.node_weight(dep.target()) else {
            continue;
        };
        let dep_service = services.get(dep_id).unwrap();
        match dep.weight() {
            PipelineEdge::Metrics => {
                if let Some(metrics) = dep_service.metrics() {
                    merged_metrics.insert(dep_id.clone(), metrics);
                    // merged_metrics.insert(dep_service.to_service_id(), metrics);
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
            let node = pipeline_graph
                .node_weight(node_idx)
                .ok_or_else(|| eyre::eyre!("internal: missing node {node_idx:?}"))?;
            let PipelineNode::Service(service_id) = node else {
                continue;
            };
            let service = self
                .pipelines
                .services
                .remove(service_id)
                .ok_or_else(|| eyre::eyre!("internal: missing service {service_id}"))?;

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
