use crate::ext::NumDatapoints;
use crate::metrics;
use crate::scrape::Scrape;
use color_eyre::eyre;
use futures::{StreamExt, TryStreamExt};
use metrics::{EmitMetric, Record, StorageEngine};
use mongodb::{bson, event::cmap::ConnectionCheckoutFailedReason, Client};
use opentelemetry::{InstrumentationLibrary, KeyValue, Value};
use opentelemetry_sdk::metrics::data::{Metric, ResourceMetrics, ScopeMetrics};
use opentelemetry_sdk::Resource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::time::{Instant, SystemTime};
use tracing::{debug, info, trace, warn};

pub const DEFAULT_PORT: u16 = 27017;

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

fn omit_values(mut value: bson::Bson, depth: usize) -> bson::Bson {
    omit_values_visitor(&mut value, depth);
    value
}

fn omit_values_visitor(value: &mut bson::Bson, depth: usize) {
    match value {
        bson::Bson::Document(value) => {
            if depth <= 0 {
                *value = bson::doc! {"omitted": true};
            } else {
                for (_, v) in value.iter_mut() {
                    omit_values_visitor(v, depth - 1);
                }
            }
        }
        bson::Bson::Array(value) => {
            if depth <= 0 {
                *value = bson::Array::from_iter([bson::Bson::String("omitted".to_string())]);
            } else {
                for v in value.iter_mut() {
                    omit_values_visitor(v, depth - 1);
                }
            }
        }
        value => {
            // keep
        }
    };
}

fn get_version(doc: &bson::Bson) -> eyre::Result<semver::Version> {
    let version = crate::get_str!(doc, "version")?;
    let version = semver::Version::parse(version)?;
    Ok(version)
}

// pub async fn query_version(client: &Client) -> eyre::Result<semver::Version> {
//     let version = client
//         .database("admin")
//         .run_command(bson::doc! {"buildInfo": 1})
//         .await?;
//     let version = bson::from_document(version)?;
//     get_version(&version)
// }

pub fn get_server_address_and_port<'a>(
    server_status: &'a bson::Bson,
) -> eyre::Result<(&'a str, u16)> {
    let host = crate::get_str!(server_status, "host")?;
    match &*host.split(":").collect::<Vec<_>>() {
        [host] => Ok((host, DEFAULT_PORT)),
        [host, port] => {
            let port = port.parse()?;
            Ok((host, port))
        }
        _ => Err(eyre::eyre!("unexpected host format: {:?}", host)),
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

/// Returns the index stats per collection for a given database
/// more information can be found here:
/// https://www.mongodb.com/docs/manual/reference/operator/aggregation/indexStats/
pub async fn get_index_stats(
    client: &Client,
    database_name: &str,
    collection_name: &str,
) -> eyre::Result<Vec<bson::Bson>> {
    let db = client.database(database_name);
    let collection = db.collection::<bson::Document>(collection_name);
    let cursor = collection.aggregate([bson::doc! {"$indexStats": {}}]);
    let index_stats: Vec<bson::Bson> = cursor
        .await?
        .map_ok(|doc| bson::Bson::from(doc))
        .try_collect()
        .await?;
    Ok(index_stats)
}

fn get_storage_engine(server_status: &bson::Bson) -> Option<StorageEngine> {
    match crate::get_str!(&server_status, "storageEngine", "name") {
        Err(err) => {
            warn!("{}", err);
            if let Some(partial_match) = err.source.partial_match() {
                trace!(
                    "[{}] = {:#}",
                    partial_match.path,
                    omit_values(partial_match.value.clone(), 1)
                );
            }
            None
        }
        Ok("wiredTiger") => Some(StorageEngine::WiredTiger),
        Ok(other) => Some(StorageEngine::Other(other.to_string())),
    }
}

async fn get_server_status(client: &Client, database_name: &str) -> eyre::Result<bson::Bson> {
    let server_status = client
        .database(database_name)
        .run_command(bson::doc! {"serverStatus": 1})
        .await?;
    Ok(bson::Bson::from(server_status))
}

async fn get_db_stats(client: &Client, database_name: &str) -> eyre::Result<bson::Bson> {
    let db_stats = client
        .database(&database_name)
        .run_command(bson::doc! {"dbStats": 1})
        .await?;
    Ok(bson::Bson::from(db_stats))
}

async fn get_top_stats(client: &Client) -> eyre::Result<bson::Bson> {
    let top_stats = client
        .database("admin")
        .run_command(bson::doc! { "top": 1})
        .await?;
    Ok(bson::Bson::from(top_stats))
}

#[derive(Debug)]
pub struct Options {
    pub connection_uri: String,
}

#[derive(Debug)]
pub struct MetricScraper {
    client: Client,
    metrics: Metrics,
    start_time: SystemTime,
}

impl Scrape for MetricScraper {
    async fn scrape(&mut self) -> eyre::Result<Vec<ResourceMetrics>> {
        // async fn scrape(&mut self) -> () {
        let start = std::time::Instant::now();
        let mut errors = Vec::new();
        let metrics = self.record_metrics(&mut errors).await?;
        // todo: do something with the errors
        debug!("completed in {:?}", start.elapsed());
        Ok(metrics)
    }
}

impl MetricScraper {
    pub async fn new(options: &Options) -> eyre::Result<Self> {
        // connect to database
        let client = Client::with_uri_str(&options.connection_uri).await?;

        // send ping to confirm a successful connection
        client
            .database("admin")
            .run_command(bson::doc! { "ping": 1 })
            .await?;

        info!(uri = options.connection_uri, "connected to database");

        Ok(Self {
            client,
            start_time: SystemTime::now(),
            metrics: Metrics::default(),
        })
    }

    pub async fn record_metrics(
        &mut self,
        errors: &mut Vec<metrics::Error>,
    ) -> eyre::Result<Vec<ResourceMetrics>> {
        let now = SystemTime::now();

        let database_names = self.client.list_database_names().await?;
        let database_count = database_names.len();

        let server_status = get_server_status(&self.client, "admin").await?;
        let version = get_version(&server_status)?;
        trace!(version = version.to_string());

        let storage_engine = get_storage_engine(&server_status);
        trace!(?storage_engine);

        let (server_address, server_port) = get_server_address_and_port(&server_status)?;

        let mut config = metrics::Config {
            start_time: self.start_time,
            time: now,
            database_name: None,
            storage_engine,
            version,
        };

        self.metrics.database_count.record(database_count, &config);
        for metric in self.metrics.server_status_metrics.iter_mut() {
            metric.record(&server_status, &config, errors);
        }

        let top_stats = get_top_stats(&self.client).await?;
        self.metrics
            .operation_time
            .record(&top_stats, &config, errors);

        // create resource
        let global_resource = ResourceAttributesConfig {
            server_address: server_address.to_string(),
            server_port,
            database: None,
        }
        .into_resource();

        let mut global_metrics = ScopeMetrics {
            scope: crate::LIBRARY.clone(),
            metrics: vec![],
        };

        global_metrics
            .metrics
            .push(self.metrics.database_count.emit());
        global_metrics
            .metrics
            .push(self.metrics.operation_time.emit());
        for metric in self.metrics.server_status_metrics.iter_mut() {
            global_metrics.metrics.push(metric.emit());
        }

        let mut metric_names = global_metrics
            .metrics
            .iter()
            .filter(|metric| metric.num_datapoints().unwrap_or(0) > 0)
            .map(|metric| &metric.name)
            .collect::<Vec<_>>();
        metric_names.sort();

        debug!("global: emitted {} metrics", metric_names.len());
        // for (idx, metric_name) in metric_names.iter().enumerate() {
        //     debug!("[{}] = {}", idx, metric_name);
        // }

        let mut resources = vec![ResourceMetrics {
            resource: global_resource,
            scope_metrics: vec![global_metrics],
        }];
        // let global_resource_metrics = ResourceMetrics {
        //     resource: global_resource,
        //     scope_metrics: vec![global_metrics],
        // };

        // collect metrics for each database
        for database_name in database_names.iter() {
            config.database_name = Some(database_name.clone());
            let db_stats = get_db_stats(&self.client, database_name).await?;
            // if err != nil {
            // 	errs.AddPartial(1, fmt.Errorf("failed to fetch database stats metrics: %w", err))
            // } else {
            // 	s.recordDBStats(now, dbStats, databaseName, errs)
            // }
            for metric in self.metrics.db_stats_metrics.iter_mut() {
                metric.record(&db_stats, &config, errors);
            }

            let db_server_status = get_server_status(&self.client, database_name).await?;
            for metric in self.metrics.db_server_status_metrics.iter_mut() {
                metric.record(&db_server_status, &config, errors);
            }

            let collection_names = self
                .client
                .database(database_name)
                .list_collection_names()
                .await?;

            if database_name != "local" {
                for collection_name in collection_names {
                    let index_stats =
                        get_index_stats(&self.client, database_name, &collection_name).await?;
                    self.metrics.index_accesses.record(
                        &index_stats,
                        database_name,
                        &collection_name,
                        &config,
                        errors,
                    );
                    // if err != nil {
                    // 	errs.AddPartial(1, fmt.Errorf("failed to fetch index stats metrics: %w", err))
                    // 	return
                    // }
                    // s.recordIndexStats(now, indexStats, databaseName, collectionName, errs)
                }
            }

            let db_resource = ResourceAttributesConfig {
                server_address: server_address.to_string(),
                server_port,
                database: Some(database_name.to_string()),
            }
            .into_resource();

            // let metrics = self.metrics.emit_for_resource(resource);
            let mut db_metrics = ScopeMetrics {
                scope: crate::LIBRARY.clone(),
                metrics: vec![],
            };

            db_metrics.metrics.push(self.metrics.index_accesses.emit());
            for metric in self.metrics.db_stats_metrics.iter_mut() {
                db_metrics.metrics.push(metric.emit());
            }
            for metric in self.metrics.db_server_status_metrics.iter_mut() {
                db_metrics.metrics.push(metric.emit());
            }

            let mut metric_names = db_metrics
                .metrics
                .iter()
                .filter(|metric| metric.num_datapoints().unwrap_or(0) > 0)
                .map(|metric| &metric.name)
                .collect::<Vec<_>>();
            metric_names.sort();

            debug!("{:?} emitted {} metrics", database_name, metric_names.len());
            // for (idx, metric_name) in metric_names.iter().enumerate() {
            //     debug!("[{}] = {}", idx, metric_name);
            // }

            let db_resource_metrics = ResourceMetrics {
                resource: db_resource,
                scope_metrics: vec![db_metrics],
            };
            resources.push(db_resource_metrics);

            // if err != nil {
            // 	errs.AddPartial(1, fmt.Errorf("failed to fetch server status metrics: %w", err))
            // 	return
            // }
        }

        // debug log errors
        for err in errors {
            warn!("{}", err);
            if let Some(partial_match) = err.partial_match() {
                trace!(
                    "[{}] = {:#}",
                    partial_match.path,
                    omit_values(partial_match.value.clone(), 1)
                );
            }
        }

        Ok(resources)
        // Ok(())
    }
}
