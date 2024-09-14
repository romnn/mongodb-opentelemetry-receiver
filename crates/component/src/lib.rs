#![allow(warnings)]

pub mod bfs;
pub mod config;
pub mod ext;
pub mod factory;
pub mod pipeline;
pub mod telemetry;

use color_eyre::eyre;
use factory::ComponentName;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use std::sync::Arc;
use tokio::sync::watch;
use tokio_stream::wrappers::BroadcastStream;

/// Metric payload that is sent between pipeline nodes.
///
/// `Arc` is used to make the payload `Clone`, which is required to broadcast the payload to
/// multiple distinct nodes down the pipeline.
pub type MetricPayload = Arc<Vec<ResourceMetrics>>;
pub type TracesPayload = String;
pub type LogPayload = String;

pub type MetricsStream = tokio_stream::StreamMap<ServiceIdentifier, BroadcastStream<MetricPayload>>;

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

    pub fn to_service_id(&self) -> Result<ServiceIdentifier, ServiceError> {
        let service_id =
            crate::service_id(self.id()).ok_or_else(|| ServiceError::InvalidFormat {
                kind: self.kind(),
                id: self.id().to_string(),
            })?;
        let service_id = match self {
            Self::Receiver(_) => ServiceIdentifier::Receiver(service_id.to_string()),
            Self::Processor(_) => ServiceIdentifier::Processor(service_id.to_string()),
            Self::Exporter(_) => ServiceIdentifier::Exporter(service_id.to_string()),
        };
        Ok(service_id)
    }
}

#[inline]
pub fn service_id(value: &str) -> Option<String> {
    value.split("/").next().map(|id| id.to_ascii_lowercase())
}

#[async_trait::async_trait]
pub trait Receiver: Producer + std::fmt::Debug + Send + Sync + 'static {
    fn id(&self) -> &str;

    fn to_service_id(&self) -> ServiceIdentifier {
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

    fn to_service_id(&self) -> ServiceIdentifier {
        ServiceIdentifier::Processor(self.id().to_string())
    }

    async fn start(
        self: Box<Self>,
        shutdown_rx: watch::Receiver<bool>,
        metrics: MetricsStream,
    ) -> eyre::Result<()>;
}

#[async_trait::async_trait]
pub trait Exporter: std::fmt::Debug + Send + Sync + 'static {
    fn id(&self) -> &str;

    fn to_service_id(&self) -> ServiceIdentifier {
        ServiceIdentifier::Exporter(self.id().to_string())
    }

    async fn start(
        self: Box<Self>,
        shutdown_rx: watch::Receiver<bool>,
        metrics: MetricsStream,
    ) -> eyre::Result<()>;
}
