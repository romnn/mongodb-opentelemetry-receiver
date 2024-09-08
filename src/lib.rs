#![allow(warnings)]

pub mod attributes;
pub mod doc;
pub mod metrics;
pub mod otlp;
pub mod prometheus;

use color_eyre::eyre;
use metrics::Record;
use mongodb::{bson, event::cmap::ConnectionCheckoutFailedReason, Client};
use opentelemetry::{KeyValue, Value};
use opentelemetry_sdk::metrics::data::{ResourceMetrics, ScopeMetrics};
use opentelemetry_sdk::Resource;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Instant, SystemTime};

pub const DEFAULT_PORT: u16 = 27017;

lazy_static::lazy_static! {
    static ref VERSION_REGEX: Regex = Regex::new(r"v?([0-9]+(\.[0-9]+)*?)(-([0-9]+[0-9A-Za-z\-~]*(\.[0-9A-Za-z\-~]+)*)|(-?([A-Za-z\-~]+[0-9A-Za-z\-~]*(\.[0-9A-Za-z\-~]+)*)))?(\+([0-9A-Za-z\-~]+(\.[0-9A-Za-z\-~]+)*))?").unwrap();
}

// regexp.MustCompile("^" + VersionRegexpRaw + "$")

// trait EmptyAggregation {
//     fn empty() -> Self;
// }
//
// impl<T> EmptyAggregation for Sum<T> {
//     fn empty() -> Self {
//         Self {
//             data_points: vec![],
//             is_monotonic: false,
//             temporality: Temporality::Cumulative,
//         }
//     }
// }

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

#[derive(Debug, Deserialize)]
struct Version {
    pub version: String,
}

pub async fn get_version(client: &Client) -> eyre::Result<String> {
    let version = client
        .database("admin")
        .run_command(bson::doc! {"buildInfo": 1})
        .await?;
    let version: Version = bson::from_document(version)?;
    Ok(version.version)
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
    collection_count: metrics::CollectionCount,
    connection_count: metrics::ConnectionCount,
    data_size: metrics::DataSize,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            collection_count: metrics::CollectionCount::new(),
            connection_count: metrics::ConnectionCount::new(),
            data_size: metrics::DataSize::new(),
        }
    }
}

#[derive(Debug)]
pub struct MetricScraper {
    metrics: Metrics,
    start_time: SystemTime,
}

impl Metrics {
    pub fn record_admin_metrics(
        &mut self,
        server_status: &bson::Bson,
        start_time: SystemTime,
        time: SystemTime,
        errors: &mut Vec<metrics::Error>,
    ) -> eyre::Result<()> {
        let storageEngine = get_str!(server_status, "storageEngine", "name")?;
        if storageEngine != "wiredTiger" {
            return Ok(());
        }
        println!("storage engine: {storageEngine}");

        // dbg!(doc::get_path(
        //     server_status,
        //     path!(
        //         "connections",
        //         attributes::ConnectionType::Available.as_str()
        //     )
        // ));
        self.collection_count
            .record(server_status, None, start_time, time, errors);
        self.data_size
            .record(server_status, None, start_time, time, errors);
        self.connection_count
            .record(server_status, None, start_time, time, errors);

        // let mut collection_count = metrics::CollectionCount::new();
        // collection_count.record(server_status);
        // match get_i64!(
        //     server_status,
        //     "wiredTiger",
        //     "cache",
        //     "pages read into cache"
        // ) {
        //     Ok(cache_misses) => {
        //         println!("cache misses: {cache_misses}");
        //     }
        //     Err(err) => errors.push(metrics::Error::CollectMetric {
        //         metric: "mongodb.cache.operations".to_string(),
        //         attributes: "miss, hit".to_string(),
        //         source: err.into(),
        //     }),
        // }
        //
        // // collectMetricWithAttributes = "failed to collect metric {} with attribute(s) %s: %w"
        //
        // let cache_hits = get!(
        //     server_status,
        //     "wiredTiger",
        //     "cache",
        //     "pages requested from the cache"
        // )?
        // .get_i64()?;
        // println!("cache hits: {cache_hits}");
        //
        // // "mongodb.cache.operations"
        //
        // let cache_hits = cache_hits - cache_misses;
        // // s.mb.RecordMongodbCacheOperationsDataPoint(now, cacheHits, metadata.AttributeTypeHit)
        //
        // // s.recordCursorCount(now, document, errs)
        // // metricName := "mongodb.cursor.count"
        // let cursor_count = get!(server_status, "metrics", "cursor", "open", "total")?.get_i64()?;
        // println!("cursor count: {cursor_count}");
        //
        // // s.recordCursorTimeoutCount(now, document, errs)
        // // metricName := "mongodb.cursor.timeout.count"
        // let cursor_timeouts = get!(server_status, "metrics", "cursor", "timedOut")?.get_i64()?;
        // println!("cursor timeouts: {cursor_timeouts}");
        // // s.mb.RecordMongodbCursorTimeoutCountDataPoint(now, val)
        // // errs.AddPartial(1, fmt.Errorf(collectMetricError, metricName, err))
        //
        // // s.recordGlobalLockTime(now, document, errs)
        // // metricName := "mongodb.global_lock.time"
        // let global_lock_time = get!(server_status, "globalLock", "totalTime")?.get_i64()?;
        // let global_lock_held_millis = global_lock_time / 1000;
        // println!("cursor timeouts: {global_lock_time}");
        // // s.mb.RecordMongodbGlobalLockTimeDataPoint(now, heldTimeMilliseconds)
        //
        // // s.recordNetworkCount(now, document, errs)
        // let network_bytes_in = get!(server_status, "network", "bytesIn")?.get_i64()?;
        // let network_bytes_out = get!(server_status, "network", "bytesOut")?.get_i64()?;
        // let network_num_requests = get!(server_status, "network", "numRequests")?.get_i64()?;

        // s.recordOperations(now, document, errs)
        // s.recordOperationsRepl(now, document, errs)
        // s.recordSessionCount(now, document, errs)
        // s.recordLatencyTime(now, document, errs)
        // s.recordUptime(now, document, errs)
        // s.recordHealth(now, document, errs)
        Ok(())
    }

    /// EmitForResource saves all the generated metrics under a new resource and updates the internal state to be ready for
    // recording another set of data points as part of another resource. This function can be helpful when one scraper
    // needs to emit metrics from several resources. Otherwise calling this function is not required,
    // just `Emit` function can be called instead.
    // Resource attributes should be provided as ResourceMetricsOption arguments.
    pub fn emit_for_resource(&mut self, resource: Resource) -> ResourceMetrics {
        let library =
            opentelemetry_sdk::InstrumentationLibrary::builder("mongodb-opentelemetry-collector")
                .with_version(env!("CARGO_PKG_VERSION"))
                .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
                .build();

        let mut metrics = ScopeMetrics {
            scope: library,
            metrics: vec![],
        };

        metrics.metrics.push(self.data_size.emit());
        metrics.metrics.push(self.collection_count.emit());
        metrics.metrics.push(self.connection_count.emit());

        let resource_metrics = ResourceMetrics {
            resource,
            scope_metrics: vec![metrics],
        };
        resource_metrics
        // 	ils.Metrics().EnsureCapacity(mb.metricsCapacity)
        // 	mb.metricMongodbCacheOperations.emit(ils.Metrics())
        // 	mb.metricMongodbCollectionCount.emit(ils.Metrics())
        // 	mb.metricMongodbConnectionCount.emit(ils.Metrics())
        // 	mb.metricMongodbCursorCount.emit(ils.Metrics())
        // 	mb.metricMongodbCursorTimeoutCount.emit(ils.Metrics())
        // 	mb.metricMongodbDataSize.emit(ils.Metrics())
        // 	mb.metricMongodbDatabaseCount.emit(ils.Metrics())
        // 	mb.metricMongodbDocumentOperationCount.emit(ils.Metrics())
        // 	mb.metricMongodbExtentCount.emit(ils.Metrics())
        // 	mb.metricMongodbGlobalLockTime.emit(ils.Metrics())
        // 	mb.metricMongodbHealth.emit(ils.Metrics())
        // 	mb.metricMongodbIndexAccessCount.emit(ils.Metrics())
        // 	mb.metricMongodbIndexCount.emit(ils.Metrics())
        // 	mb.metricMongodbIndexSize.emit(ils.Metrics())
        // 	mb.metricMongodbLockAcquireCount.emit(ils.Metrics())
        // 	mb.metricMongodbLockAcquireTime.emit(ils.Metrics())
        // 	mb.metricMongodbLockAcquireWaitCount.emit(ils.Metrics())
        // 	mb.metricMongodbLockDeadlockCount.emit(ils.Metrics())
        // 	mb.metricMongodbMemoryUsage.emit(ils.Metrics())
        // 	mb.metricMongodbNetworkIoReceive.emit(ils.Metrics())
        // 	mb.metricMongodbNetworkIoTransmit.emit(ils.Metrics())
        // 	mb.metricMongodbNetworkRequestCount.emit(ils.Metrics())
        // 	mb.metricMongodbObjectCount.emit(ils.Metrics())
        // 	mb.metricMongodbOperationCount.emit(ils.Metrics())
        // 	mb.metricMongodbOperationLatencyTime.emit(ils.Metrics())
        // 	mb.metricMongodbOperationReplCount.emit(ils.Metrics())
        // 	mb.metricMongodbOperationTime.emit(ils.Metrics())
        // 	mb.metricMongodbSessionCount.emit(ils.Metrics())
        // 	mb.metricMongodbStorageSize.emit(ils.Metrics())
        // 	mb.metricMongodbUptime.emit(ils.Metrics())
        //
        // 	for _, op := range rmo {
        // 		op(rm)
        // 	}
        // 	for attr, filter := range mb.resourceAttributeIncludeFilter {
        // 		if val, ok := rm.Resource().Attributes().Get(attr); ok && !filter.Matches(val.AsString()) {
        // 			return
        // 		}
        // 	}
        // 	for attr, filter := range mb.resourceAttributeExcludeFilter {
        // 		if val, ok := rm.Resource().Attributes().Get(attr); ok && filter.Matches(val.AsString()) {
        // 			return
        // 		}
        // 	}
        //
        // 	if ils.Metrics().Len() > 0 {
        // 		mb.updateCapacity(rm)
        // 		rm.MoveTo(mb.metricsBuffer.ResourceMetrics().AppendEmpty())
        // 	}
    }
}

// return ResourceAttributesConfig{
// 		Database: ResourceAttributeConfig{
// 			Enabled: true,
// 		},
// 		ServerAddress: ResourceAttributeConfig{
// 			Enabled: true,
// 		},
// 		ServerPort: ResourceAttributeConfig{
// 			Enabled: false,
// 		},
// 	}

// // ResourceAttributeConfig provides common config for a particular resource attribute.
// type ResourceAttributeConfig struct {
// 	Enabled bool `mapstructure:"enabled"`
// 	// Experimental: MetricsInclude defines a list of filters for attribute values.
// 	// If the list is not empty, only metrics with matching resource attribute values will be emitted.
// 	MetricsInclude []filter.Config `mapstructure:"metrics_include"`
// 	// Experimental: MetricsExclude defines a list of filters for attribute values.
// 	// If the list is not empty, metrics with matching resource attribute values will not be emitted.
// 	// MetricsInclude has higher priority than MetricsExclude.
// 	MetricsExclude []filter.Config `mapstructure:"metrics_exclude"`
//
// 	enabledSetByUser bool
// }

pub struct ResourceAttributesConfig {
    database: String,
    server_address: String,
    server_port: u16,
}

impl ResourceAttributesConfig {
    pub fn to_resource(self) -> Resource {
        Resource::new([
            KeyValue::new("database", self.database),
            KeyValue::new("server.address", self.server_address),
            KeyValue::new("server.port", Value::I64(self.server_port.into())),
        ])
    }
}

impl MetricScraper {
    pub async fn record_metrics(
        &mut self,
        client: &Client,
        errors: &mut Vec<metrics::Error>,
    ) -> eyre::Result<()> {
        let now = SystemTime::now();

        let database_names = client.list_database_names().await?;
        let database_count = database_names.len();

        let server_status = client
            .database("admin")
            .run_command(bson::doc! {"serverStatus": 1})
            .await?;
        let server_status = bson::Bson::from(server_status);
        // println!("{:#?}", server_status);
        // let server_status: ServerStatus = bson::from_document(raw_server_status.clone())?;
        // println!("{:#?}", server_status);

        // let mut metrics = Metrics::default();
        let (server_address, server_port) = get_server_address_and_port(&server_status)?;

        self.metrics
            .record_admin_metrics(&server_status, self.start_time, now, errors)?;
        dbg!(errors.iter().map(|err| err.to_string()).collect::<Vec<_>>());
        // create resource
        let resource = ResourceAttributesConfig {
            server_address: server_address.to_string(),
            server_port,
            database: "".to_string(),
        }
        .to_resource();
        let metrics = self.metrics.emit_for_resource(resource);
        dbg!(metrics);

        Ok(())
    }
}

pub async fn record_metrics(client: &Client) -> eyre::Result<()> {
    let version = get_version(client).await?;
    println!("version is {version}");

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
        start_time: SystemTime::now(),
        metrics: Metrics::new(),
    };
    scraper.record_metrics(client, &mut errors).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {}
}
