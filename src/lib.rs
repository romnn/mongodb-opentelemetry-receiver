#![allow(warnings)]

pub mod attributes;
pub mod doc;
pub mod metrics;
pub mod otlp;
pub mod prometheus;
pub mod scrape;

use color_eyre::eyre;
use futures::{StreamExt, TryStreamExt};
use metrics::{EmitMetric, Record, StorageEngine};
use mongodb::bson::spec::ElementType;
use mongodb::Cursor;
use mongodb::{bson, event::cmap::ConnectionCheckoutFailedReason, Client};
use opentelemetry::{InstrumentationLibrary, KeyValue, Value};
use opentelemetry_sdk::metrics::data::{ResourceMetrics, ScopeMetrics};
use opentelemetry_sdk::Resource;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::time::{Instant, SystemTime};
use tracing::{debug, trace, warn};

pub const DEFAULT_PORT: u16 = 27017;

// lazy_static::lazy_static! {
//     static ref VERSION_REGEX: Regex = Regex::new(r"v?([0-9]+(\.[0-9]+)*?)(-([0-9]+[0-9A-Za-z\-~]*(\.[0-9A-Za-z\-~]+)*)|(-?([A-Za-z\-~]+[0-9A-Za-z\-~]*(\.[0-9A-Za-z\-~]+)*)))?(\+([0-9A-Za-z\-~]+(\.[0-9A-Za-z\-~]+)*))?").unwrap();
// }

// regexp.MustCompile("^" + VersionRegexpRaw + "$")

trait NumDatapoints {
    fn num_datapoints(&self) -> Option<usize>;
}

impl NumDatapoints for opentelemetry_sdk::metrics::data::Metric {
    fn num_datapoints(&self) -> Option<usize> {
        if let Some(sum) = self
            .data
            .as_any()
            .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<i64>>()
        {
            return Some(sum.data_points.len());
        }
        if let Some(sum) = self
            .data
            .as_any()
            .downcast_ref::<opentelemetry_sdk::metrics::data::Gauge<i64>>()
        {
            return Some(sum.data_points.len());
        }
        None
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

// type metricMongodbCollectionCount struct {
// 	data     pmetric.Metric // data buffer for generated metric.
// 	config   MetricConfig   // metric config provided by user.
// 	capacity int            // max observed number of data points added to the metric.
// }
//
// // init fills mongodb.collection.count metric with initial data.
// func (m *metricMongodbCollectionCount) init() {
// 	m.data.SetName("mongodb.collection.count")
// 	m.data.SetDescription("The number of collections.")
// 	m.data.SetUnit("{collections}")
// 	m.data.SetEmptySum()
// 	m.data.Sum().SetIsMonotonic(false)
// 	m.data.Sum().SetAggregationTemporality(pmetric.AggregationTemporalityCumulative)
// }
//
// func (m *metricMongodbCollectionCount) recordDataPoint(start pcommon.Timestamp, ts pcommon.Timestamp, val int64) {
// 	if !m.config.Enabled {
// 		return
// 	}
// 	dp := m.data.Sum().DataPoints().AppendEmpty()
// 	dp.SetStartTimestamp(start)
// 	dp.SetTimestamp(ts)
// 	dp.SetIntValue(val)
// }

// // emit appends recorded metric data to a metrics slice and prepares it for recording another set of data points.
// func (m *metricMongodbCollectionCount) emit(metrics pmetric.MetricSlice) {
// 	if m.config.Enabled && m.data.Sum().DataPoints().Len() > 0 {
// 		m.updateCapacity()
// 		m.data.MoveTo(metrics.AppendEmpty())
// 		m.init()
// 	}
// }
//
// func newMetricMongodbCollectionCount(cfg MetricConfig) metricMongodbCollectionCount {
// 	m := metricMongodbCollectionCount{config: cfg}
// 	if cfg.Enabled {
// 		m.data = pmetric.NewMetric()
// 		m.init()
// 	}
// 	return m
// }

// #[derive(Debug, Deserialize)]
// struct Version {
//     pub version: String,
// }

pub fn get_version(doc: &bson::Bson) -> eyre::Result<semver::Version> {
    let version = get_str!(doc, "version")?;
    let version = semver::Version::parse(version)?;
    Ok(version)
}

pub async fn query_version(client: &Client) -> eyre::Result<semver::Version> {
    let version = client
        .database("admin")
        .run_command(bson::doc! {"buildInfo": 1})
        .await?;
    let version = bson::from_document(version)?;
    get_version(&version)

    // let version: Version = bson::from_document(version)?;
    // Ok(version.version)
}

pub fn get_server_address_and_port<'a>(
    server_status: &'a bson::Bson,
) -> eyre::Result<(&'a str, u16)> {
    let host = get_str!(server_status, "host")?;
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

lazy_static::lazy_static! {
    static ref LIBRARY: InstrumentationLibrary = opentelemetry_sdk::InstrumentationLibrary::builder("mongodb-opentelemetry-collector")
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
        .build();
}

impl Metrics {
    // pub fn record_admin_metrics(
    //     &mut self,
    //     server_status: &bson::Bson,
    //     config: &metrics::Config,
    //     errors: &mut Vec<metrics::Error>,
    // ) -> eyre::Result<()> {
    //     if config.storage_engine != Some(StorageEngine::WiredTiger) {
    //         return Ok(());
    //     }
    //
    //     // dbg!(doc::get_path(
    //     //     server_status,
    //     //     path!(
    //     //         "connections",
    //     //         attributes::ConnectionType::Available.as_str()
    //     //     )
    //     // ));
    //     // self.collection_count.record(server_status, config, errors);
    //     // self.data_size.record(server_status, config, errors);
    //     // self.connection_count.record(server_status, config, errors);
    //
    //     // let mut collection_count = metrics::CollectionCount::new();
    //     // collection_count.record(server_status);
    //     // match get_i64!(
    //     //     server_status,
    //     //     "wiredTiger",
    //     //     "cache",
    //     //     "pages read into cache"
    //     // ) {
    //     //     Ok(cache_misses) => {
    //     //         println!("cache misses: {cache_misses}");
    //     //     }
    //     //     Err(err) => errors.push(metrics::Error::CollectMetric {
    //     //         metric: "mongodb.cache.operations".to_string(),
    //     //         attributes: "miss, hit".to_string(),
    //     //         source: err.into(),
    //     //     }),
    //     // }
    //     //
    //     // // collectMetricWithAttributes = "failed to collect metric {} with attribute(s) %s: %w"
    //     //
    //     // let cache_hits = get!(
    //     //     server_status,
    //     //     "wiredTiger",
    //     //     "cache",
    //     //     "pages requested from the cache"
    //     // )?
    //     // .get_i64()?;
    //     // println!("cache hits: {cache_hits}");
    //     //
    //     // // "mongodb.cache.operations"
    //     //
    //     // let cache_hits = cache_hits - cache_misses;
    //     // // s.mb.RecordMongodbCacheOperationsDataPoint(now, cacheHits, metadata.AttributeTypeHit)
    //     //
    //     // // s.recordCursorCount(now, document, errs)
    //     // // metricName := "mongodb.cursor.count"
    //     // let cursor_count = get!(server_status, "metrics", "cursor", "open", "total")?.get_i64()?;
    //     // println!("cursor count: {cursor_count}");
    //     //
    //     // // s.recordCursorTimeoutCount(now, document, errs)
    //     // // metricName := "mongodb.cursor.timeout.count"
    //     // let cursor_timeouts = get!(server_status, "metrics", "cursor", "timedOut")?.get_i64()?;
    //     // println!("cursor timeouts: {cursor_timeouts}");
    //     // // s.mb.RecordMongodbCursorTimeoutCountDataPoint(now, val)
    //     // // errs.AddPartial(1, fmt.Errorf(collectMetricError, metricName, err))
    //     //
    //     // // s.recordGlobalLockTime(now, document, errs)
    //     // // metricName := "mongodb.global_lock.time"
    //     // let global_lock_time = get!(server_status, "globalLock", "totalTime")?.get_i64()?;
    //     // let global_lock_held_millis = global_lock_time / 1000;
    //     // println!("cursor timeouts: {global_lock_time}");
    //     // // s.mb.RecordMongodbGlobalLockTimeDataPoint(now, heldTimeMilliseconds)
    //     //
    //     // // s.recordNetworkCount(now, document, errs)
    //     // let network_bytes_in = get!(server_status, "network", "bytesIn")?.get_i64()?;
    //     // let network_bytes_out = get!(server_status, "network", "bytesOut")?.get_i64()?;
    //     // let network_num_requests = get!(server_status, "network", "numRequests")?.get_i64()?;
    //
    //     // s.recordOperations(now, document, errs)
    //     // s.recordOperationsRepl(now, document, errs)
    //     // s.recordSessionCount(now, document, errs)
    //     // s.recordLatencyTime(now, document, errs)
    //     // s.recordUptime(now, document, errs)
    //     // s.recordHealth(now, document, errs)
    //     Ok(())
    // }

    // pub fn emittable_metrics_iter_mut(
    //     &mut self,
    // ) -> impl Iterator<Item = &mut dyn metrics::EmitMetric> {
    //     [&mut self.database_count as &mut dyn metrics::EmitMetric]
    //         .into_iter()
    //         .chain(
    //             self.admin_metrics
    //                 .iter_mut()
    //                 .map(|metric| metric.as_mut() as &mut dyn metrics::EmitMetric),
    //         )
    // }

    // /// EmitForResource saves all the generated metrics under a new resource and updates the internal state to be ready for
    // // recording another set of data points as part of another resource. This function can be helpful when one scraper
    // // needs to emit metrics from several resources. Otherwise calling this function is not required,
    // // just `Emit` function can be called instead.
    // // Resource attributes should be provided as ResourceMetricsOption arguments.
    // pub fn emit_for_resource(&mut self, resource: Resource) -> ResourceMetrics {
    //     let mut metrics = ScopeMetrics {
    //         scope: LIBRARY.clone(),
    //         metrics: vec![],
    //     };
    //
    //     metrics.metrics.push(self.database_count.emit());
    //     metrics.metrics.push(self.operation_time.emit());
    //     metrics.metrics.push(self.index_accesses.emit());
    //
    //     for metric in self.server_status_metrics.iter_mut() {
    //         metrics.metrics.push(metric.emit());
    //     }
    //     for metric in self.db_stats_metrics.iter_mut() {
    //         metrics.metrics.push(metric.emit());
    //     }
    //     for metric in self.db_server_status_metrics.iter_mut() {
    //         metrics.metrics.push(metric.emit());
    //     }
    //     debug!("emitted {} metrics", metrics.metrics.len());
    //
    //     let resource_metrics = ResourceMetrics {
    //         resource,
    //         scope_metrics: vec![metrics],
    //     };
    //     resource_metrics
    // }
}

pub struct ResourceAttributesConfig {
    database: Option<String>,
    server_address: String,
    server_port: u16,
}

impl ResourceAttributesConfig {
    pub fn to_resource(self) -> Resource {
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

/// IndexStats returns the index stats per collection for a given database
/// more information can be found here: https://www.mongodb.com/docs/manual/reference/operator/aggregation/indexStats/
pub async fn get_index_stats(
    client: &Client,
    database_name: &str,
    collection_name: &str,
) -> eyre::Result<Vec<bson::Bson>> {
    // ) -> eyre::Result<Vec<bson::Document>> {
    let db = client.database(database_name);
    let collection = db.collection::<bson::Document>(collection_name);
    let cursor = collection.aggregate([bson::doc! {"$indexStats": {}}]);
    // let index_stats: Cursor<bson::Bson> = cursor.await?;
    // let index_stats = index_stats.try_collect::<bson::Bson>().await?;
    let index_stats: Vec<bson::Bson> = cursor
        .await?
        .map_ok(|doc| bson::Bson::from(doc))
        // .map(|doc| bson::Bson::from(doc))
        // .map(|doc| {
        //     doc.map_err(eyre::Report::from)
        //         .and_then(|doc| bson::from_document(doc).map_err(eyre::Report::from))
        // })
        .try_collect()
        .await?;
    Ok(index_stats)
    // let index_stats = cursor.strea?).collect();
    // let index_stats = futures::stream::iter(cursor.await?).collect();
    // let index_stats: Cursor<> = cursor.await?;
    // var indexStats []bson.M
    // if err = cursor.All(context.Background(), &indexStats); err != nil {
    // 	return nil, err
    // }
    // return indexStats, nil
    // Ok(vec![])
}

#[derive(Debug)]
pub struct MetricScraper {
    client: Client,
    metrics: Metrics,
    start_time: SystemTime,
}

fn get_storage_engine(server_status: &bson::Bson) -> Option<StorageEngine> {
    match get_str!(&server_status, "storageEngine", "name") {
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

impl MetricScraper {
    // pub async fn record_index_stats(
    //     &mut self,
    //     database_name: &str,
    //     collection_name: &str,
    //     config: &metrics::Config,
    //     errors: &mut Vec<metrics::Error>,
    // ) -> eyre::Result<()> {
    //     if database_name == "local" {
    //         return Ok(());
    //     }
    //     let index_stats = get_index_stats(&self.client, database_name, collection_name).await?;
    //     let mut index_accesses = metrics::IndexAccesses::new();
    //     index_accesses.record(&index_stats, database_name, collection_name, config, errors);
    //     // self.metrics.record
    //     // s.recordIndexStats(now, indexStats, databaseName, collectionName, errs)
    //     Ok(())
    // }

    pub async fn record_metrics(&mut self, errors: &mut Vec<metrics::Error>) -> eyre::Result<()> {
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

        let top_stats = self
            .client
            .database("admin")
            .run_command(bson::doc! { "top": 1})
            .await?;

        let top_stats = bson::from_document(top_stats)?;
        self.metrics
            .operation_time
            .record(&top_stats, &config, errors);

        let database_name = "luup";
        let collection_name = "users";

        // create resource
        let global_resource = ResourceAttributesConfig {
            server_address: server_address.to_string(),
            server_port,
            database: None,
        }
        .to_resource();

        // let metrics = self.metrics.emit_for_resource(resource);
        let mut global_metrics = ScopeMetrics {
            scope: LIBRARY.clone(),
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
        for (idx, metric_name) in metric_names.iter().enumerate() {
            debug!("[{}] = {}", idx, metric_name);
        }

        let global_resource_metrics = ResourceMetrics {
            resource: global_resource,
            scope_metrics: vec![global_metrics],
        };

        // resource_metrics
        // }

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
            .to_resource();

            // let metrics = self.metrics.emit_for_resource(resource);
            let mut db_metrics = ScopeMetrics {
                scope: LIBRARY.clone(),
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
            for (idx, metric_name) in metric_names.iter().enumerate() {
                debug!("[{}] = {}", idx, metric_name);
            }

            let db_resource_metrics = ResourceMetrics {
                resource: db_resource,
                scope_metrics: vec![db_metrics],
            };

            // if err != nil {
            // 	errs.AddPartial(1, fmt.Errorf("failed to fetch server status metrics: %w", err))
            // 	return
            // }
        }

        // // create resource
        // let resource = ResourceAttributesConfig {
        //     server_address: server_address.to_string(),
        //     server_port,
        //     database: Some(database_name.to_string()),
        // }
        // .to_resource();
        // let metrics = self.metrics.emit_for_resource(resource);

        // rb.SetServerAddress(serverAddress)
        // rb.SetServerPort(serverPort)
        // rb.SetDatabase(dbName)
        // s.mb.EmitForResource(metadata.WithResource(rb.Emit()))
        // for metric in self.per_db_stats_metrics.iter_mut() {
        //     metrics.metrics.push(metric.emit());
        // }
        // for metric in self.per_db_server_stats_metrics.iter_mut() {
        //     metrics.metrics.push(metric.emit());
        // }

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

        Ok(())
    }
}

pub async fn record_metrics(client: Client) -> eyre::Result<()> {
    let mut errors = Vec::new();
    // // this is one way to record the metrics, easy for parallelization but all metrics must
    // // use the same input?
    // let mut metrics: Vec<Box<dyn metrics::Record>> = vec![
    //     Box::new(metrics::CollectionCount::new()) as Box<dyn metrics::Record>,
    //     Box::new(metrics::DataSize::new()) as Box<dyn metrics::Record>,
    // ];
    // // let mut errors = vec![];
    // let start_time = SystemTime::now();
    // let now = SystemTime::now();
    // let doc = bson::bson!({});
    // for metric in metrics.iter_mut() {
    //     metric.record(&doc, None, start_time, now, &mut errors);
    // }
    let mut scraper = MetricScraper {
        client,
        start_time: SystemTime::now(),
        metrics: Metrics::default(),
    };
    scraper.record_metrics(&mut errors).await?;
    Ok(())
}
