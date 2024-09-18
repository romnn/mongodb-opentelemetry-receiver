#![allow(warnings)]

pub mod attributes;
pub mod config;
pub mod document;
pub mod metrics;
pub mod scrape;

use color_eyre::eyre;
use futures::{StreamExt, TryStreamExt};
use metrics::{EmitMetric, Record, StorageEngine};
use mongodb::{bson, event::cmap::ConnectionCheckoutFailedReason, Client};
use opentelemetry::{InstrumentationLibrary, KeyValue, Value};
use opentelemetry_sdk::metrics::data::{Metric, ResourceMetrics, ScopeMetrics};
use opentelemetry_sdk::Resource;
use otel_collector_component::ext::NumDatapoints;
use otel_collector_component::factory::ComponentName;
use otel_collector_component::{MetricPayload, MetricsStream};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::sync::{broadcast, watch, Mutex};
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, error, info, trace, warn};

pub const DEFAULT_PORT: u16 = 27017;

lazy_static::lazy_static! {
    static ref COMPONENT_NAME: ComponentName = ComponentName::new("mongodb").unwrap();
    static ref LIBRARY: InstrumentationLibrary = opentelemetry_sdk::InstrumentationLibrary::builder("mongodb-opentelemetry-collector")
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
        .build();
    // var (
    //     Type      = component.MustNewType("otlp")
    //     ScopeName = "go.opentelemetry.io/collector/exporter/otlpexporter"
    // )
}

pub struct ResourceAttributesConfig {
    database: Option<String>,
    server_address: String,
    server_port: u16,
}

// TODO: use From<> ?
impl ResourceAttributesConfig {
    pub fn into_resource(self) -> Resource {
        let mut attributes = [
            Some(KeyValue::new("server.address", self.server_address)),
            Some(KeyValue::new(
                "server.port",
                Value::I64(self.server_port.into()),
            )),
            self.database.map(|db| KeyValue::new("database", db)),
        ];
        Resource::new(attributes.into_iter().filter_map(|attr| attr))
    }
}

#[derive(Debug)]
pub struct Metrics {
    database_count: metrics::DatabaseCount,
    operation_time: metrics::OperationTime,
    index_accesses: metrics::IndexAccesses,
    server_status_metrics: Vec<Box<dyn metrics::Record>>,
    db_stats_metrics: Vec<Box<dyn metrics::Record>>,
    db_server_status_metrics: Vec<Box<dyn metrics::Record>>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            database_count: metrics::DatabaseCount::default(),
            operation_time: metrics::OperationTime::default(),
            index_accesses: metrics::IndexAccesses::default(),
            server_status_metrics: vec![
                Box::new(metrics::CacheOperations::default()),
                Box::new(metrics::CursorCount::default()),
                Box::new(metrics::CursorTimeouts::default()),
                Box::new(metrics::GlobalLockTime::default()),
                Box::new(metrics::NetworkIn::default()),
                Box::new(metrics::NetworkOut::default()),
                Box::new(metrics::NetworkRequestCount::default()),
                Box::new(metrics::OperationCount::default()),
                Box::new(metrics::OperationReplCount::default()),
                Box::new(metrics::SessionCount::default()),
                Box::new(metrics::OperationLatencyTime::default()),
                Box::new(metrics::OperationLatencyOps::default()),
                Box::new(metrics::Uptime::default()),
                Box::new(metrics::Health::default()),
            ],
            db_stats_metrics: vec![
                Box::new(metrics::CollectionCount::default()),
                Box::new(metrics::DataSize::default()),
                Box::new(metrics::Extent::default()),
                Box::new(metrics::IndexSize::default()),
                Box::new(metrics::IndexCount::default()),
                Box::new(metrics::ObjectCount::default()),
                Box::new(metrics::StorageSize::default()),
            ],
            db_server_status_metrics: vec![
                Box::new(metrics::ConnectionCount::default()),
                Box::new(metrics::DocumentOperations::default()),
                Box::new(metrics::MemoryUsage::default()),
                Box::new(metrics::LockAquireCount::default()),
                Box::new(metrics::LockAquireWaitCount::default()),
                Box::new(metrics::LockAquireTime::default()),
                Box::new(metrics::LockAquireDeadlockCount::default()),
            ],
        }
    }
}

#[derive(Debug, Default)]
pub struct Factory {}

#[async_trait::async_trait]
impl otel_collector_component::factory::ReceiverFactory for Factory {
    fn component_name(&self) -> &ComponentName {
        &COMPONENT_NAME
    }

    async fn build(
        &self,
        id: String,
        config: serde_yaml::Value,
    ) -> eyre::Result<Box<dyn otel_collector_component::Receiver>> {
        let receiver = Receiver::from_config(id, config).await?;
        Ok(Box::new(receiver))
    }
}

pub struct Receiver {
    pub id: String,
    pub config: config::MongoDbReceiverConfig,
    pub scraper: crate::scrape::MetricScraper,
    pub metrics_tx: broadcast::Sender<MetricPayload>,
}

impl std::fmt::Debug for Receiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Receiver")
            .field("id", &self.id)
            .field("config", &self.config)
            .finish()
    }
}

pub const DEFAULT_BUFFER_SIZE: usize = 1024;

impl Receiver {
    pub async fn from_config(id: String, config: serde_yaml::Value) -> eyre::Result<Self> {
        let config: config::MongoDbReceiverConfig = serde_yaml::from_value(config)?;
        let connection_uri = config
            .hosts
            .first()
            .map(|host| host.endpoint.to_string())
            .ok_or_else(|| eyre::eyre!("missing mongodb host"))?;
        let options = crate::scrape::Options { connection_uri };
        let scraper = crate::scrape::MetricScraper::new(&options).await?;
        let (metrics_tx, _) = broadcast::channel(DEFAULT_BUFFER_SIZE);
        Ok(Self {
            id,
            config,
            scraper,
            metrics_tx,
        })
    }
}

#[async_trait::async_trait]
impl otel_collector_component::Receiver for Receiver {
    async fn start(
        mut self: Box<Self>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> eyre::Result<()> {
        let interval = self
            .config
            .collection_interval
            .map(Into::into)
            .unwrap_or(std::time::Duration::from_secs(30));
        let mut interval = tokio::time::interval(interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = interval.tick() => (),
                _ = shutdown_rx.changed() => break,
            };
            match self.scraper.scrape().await {
                Ok(metrics) => {
                    trace!("mongodb scraped {} metrics", metrics.len());
                    self.metrics_tx
                        .send(metrics.into_iter().map(Arc::new).collect());
                }
                Err(err) => {
                    error!("error scraping metrics: {}", err);
                }
            };
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl otel_collector_component::Producer for Receiver {
    fn metrics(&self) -> BroadcastStream<MetricPayload> {
        BroadcastStream::new(self.metrics_tx.subscribe())
    }
}
