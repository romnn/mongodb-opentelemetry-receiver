use color_eyre::eyre;
use mongodb::bson;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::data::{
    Aggregation, DataPoint, Gauge, Histogram, Metric, Sum, Temporality,
};
use std::time::SystemTime;
use tracing::Instrument;

use crate::{
    attributes::{self, Operation},
    doc,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to collect {metric:?}: {source}")]
    CollectMetric {
        metric: &'static str,
        #[source]
        source: doc::Error,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StorageEngine {
    WiredTiger,
    Other(String),
}

pub struct Config {
    pub database_name: Option<String>,
    pub start_time: SystemTime,
    pub time: SystemTime,
    pub storage_engine: Option<StorageEngine>,
    pub version: semver::Version,
}

pub trait Record {
    fn record(
        &mut self,
        server_status: &bson::Bson,
        config: &Config,
        errors: &mut Vec<Error>,
    ) -> ();

    fn emit(&mut self) -> Metric;
}

fn datapoint<T>(value: T, config: &Config) -> DataPoint<T> {
    DataPoint {
        attributes: vec![],
        start_time: Some(config.start_time),
        time: Some(config.time),
        value,
        exemplars: vec![],
    }
}

trait Emit
where
    Self: Sized,
{
    fn emit(&mut self) -> Self;
}

impl<T> Emit for Sum<T> {
    fn emit(&mut self) -> Self {
        Self {
            data_points: std::mem::replace(&mut self.data_points, vec![]),
            is_monotonic: self.is_monotonic,
            temporality: self.temporality,
        }
    }
}

impl<T> Emit for Gauge<T> {
    fn emit(&mut self) -> Self {
        Self {
            data_points: std::mem::replace(&mut self.data_points, vec![]),
        }
    }
}

impl<T> Emit for Histogram<T> {
    fn emit(&mut self) -> Self {
        Self {
            data_points: std::mem::replace(&mut self.data_points, vec![]),
            temporality: self.temporality,
        }
    }
}

#[derive(Debug)]
pub struct Descriptor {
    /// The name of the instrument that created this data.
    pub name: &'static str,
    /// The description of the instrument, which can be used in documentation.
    pub description: &'static str,
    /// The unit in which the instrument reports.
    pub unit: &'static str,
}

#[derive(Debug)]
pub struct MongoMetric<D> {
    pub descriptor: Descriptor,
    pub data: D,
}

impl MongoMetric<Sum<i64>> {
    pub fn sum(descriptor: Descriptor) -> Self {
        Self {
            descriptor,
            data: Sum {
                data_points: Vec::new(),
                is_monotonic: false,
                temporality: Temporality::Cumulative,
            },
        }
    }
}

impl MongoMetric<Gauge<i64>> {
    pub fn gauge(descriptor: Descriptor) -> Self {
        Self {
            descriptor,
            data: Gauge {
                data_points: Vec::new(),
            },
        }
    }
}

impl<D> MongoMetric<D>
where
    D: Aggregation + Emit,
{
    pub fn emit(&mut self) -> Metric {
        Metric {
            name: self.descriptor.name.into(),
            description: self.descriptor.description.into(),
            unit: self.descriptor.unit.into(),
            data: Box::new(self.data.emit()),
        }
    }
}

#[derive(Debug)]
pub struct CollectionCount(MongoMetric<Sum<i64>>);

impl CollectionCount {
    pub fn new() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.collection.count",
            description: "The number of collections.",
            unit: "{collections}",
        }))
    }

    // pub fn record_data_point(&mut self, start_time: SystemTime, time: SystemTime, value: i64) {
    //     self.data.data_points.push(DataPoint {
    //         start_time: Some(start_time),
    //         time: Some(time),
    //         attributes: vec![],
    //         exemplars: vec![],
    //         value,
    //     });
    // }

    // /// UpdateCapacity saves max length of data point slices that will be used for the slice capacity.
    // pub fn update_capacity(&mut self) {
    //     self.capacity = self.data.data_points.len();
    // }
}

impl Record for CollectionCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "collections") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct DataSize(MongoMetric<Sum<i64>>);

impl DataSize {
    pub fn new() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.data.size",
            description: "The size of the collection. Data compression does not affect this value.",
            unit: "By",
        }))
    }
}

impl Record for DataSize {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "mongodb", "data", "size") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct ConnectionCount(MongoMetric<Sum<i64>>);

impl ConnectionCount {
    pub fn new() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.connection.count",
            description: "The number of connections.",
            unit: "{connections}",
        }))
    }
}

impl Record for ConnectionCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for connection_type in attributes::ConnectionType::iter() {
            match crate::get_i64!(stats, "connections", connection_type.as_str()) {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new("type", connection_type.as_str())],
                        ..datapoint(value, config)
                    });
                }
                Err(err) => errors.push(Error::CollectMetric {
                    metric: self.0.descriptor.name,
                    source: err.into(),
                }),
            }
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct CacheOperations(MongoMetric<Sum<i64>>);

impl CacheOperations {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.cache.operations",
            description: "The number of cache operations of the instance.",
            unit: "{operations}",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

impl Record for CacheOperations {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        if config.storage_engine != Some(StorageEngine::WiredTiger) {
            // mongodb is using a different storage engine and this metric can not be collected
            return;
        }

        let cache_misses =
            match crate::get_i64!(stats, "wiredTiger", "cache", "pages read into cache") {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new(
                            "type",
                            attributes::CacheStatus::Miss.as_str(),
                        )],
                        ..datapoint(value, config)
                    });
                    value
                }
                Err(err) => {
                    errors.push(Error::CollectMetric {
                        metric: self.0.descriptor.name,
                        source: err.into(),
                    });
                    return;
                }
            };
        let cache_total = match crate::get_i64!(
            stats,
            "wiredTiger",
            "cache",
            "pages requested from the cache"
        ) {
            Ok(cache_total) => cache_total,
            Err(err) => {
                errors.push(Error::CollectMetric {
                    metric: self.0.descriptor.name,
                    source: err.into(),
                });
                return;
            }
        };
        let cache_hits = cache_total - cache_misses;
        self.0.data.data_points.push(DataPoint {
            // start_time: Some(config.start_time),
            // time: Some(config.time),
            attributes: vec![KeyValue::new("type", attributes::CacheStatus::Hit.as_str())],
            // exemplars: vec![],
            // value: cache_hits,
            ..datapoint(cache_hits, config)
        });
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct CursorCount(MongoMetric<Sum<i64>>);

impl CursorCount {
    pub fn new() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.cursor.count",
            description: "The number of open cursors maintained for clients.",
            unit: "{cursors}",
        }))
    }
}

impl Record for CursorCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "metrics", "cursor", "open", "total") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    // start_time: Some(config.start_time),
                    // time: Some(config.time),
                    // attributes: vec![],
                    // exemplars: vec![],
                    // value: cursor_count,
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct CursorTimeouts(MongoMetric<Sum<i64>>);

impl CursorTimeouts {
    pub fn new() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.cursor.timeout.count",
            description: "The number of cursors that have timed out.",
            unit: "{cursors}",
        }))
    }
}

impl Record for CursorTimeouts {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "metrics", "cursor", "timedOut") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct DatabaseCount(MongoMetric<Sum<i64>>);

impl DatabaseCount {
    pub fn new() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.database.count",
            description: "The number of existing databases.",
            unit: "{databases}",
        }))
    }
}

impl DatabaseCount {
    // fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
    fn record(&mut self, num_databases: usize, config: &Config) -> () {
        self.0.data.data_points.push(DataPoint {
            ..datapoint(num_databases.try_into().unwrap_or(i64::MAX), config)
        });
        //     match crate::get_i64!(stats, "metrics", "cursor", "timedOut") {
        //         Ok(cursor_timeouts) => {
        //             self.0.data.data_points.push(DataPoint {
        //                 ..datapoint(cursor_timeouts, config)
        //             });
        //         }
        //         Err(err) => errors.push(Error::CollectMetric {
        //             metric: self.0.descriptor.name,
        //             source: err.into(),
        //         }),
        //     }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct DocumentOperations(MongoMetric<Sum<i64>>);

impl DocumentOperations {
    pub fn new() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.document.operation.count",
            description: "The number of document operations executed.",
            unit: "{documents}",
        }))
    }
}

impl Record for DocumentOperations {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for operation in attributes::Operation::iter() {
            match crate::get_i64!(stats, "metrics", "document", operation.as_str()) {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new("operation", operation.as_str())],
                        ..datapoint(value, config)
                    });
                }
                Err(err) => errors.push(Error::CollectMetric {
                    metric: self.0.descriptor.name,
                    source: err.into(),
                }),
            }
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct Extent(MongoMetric<Sum<i64>>);

impl Extent {
    pub fn new() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.extent.count",
            description: "The number of extents.",
            unit: "{extents}",
        }))
    }
}

impl Record for Extent {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        if config.version >= semver::Version::new(4, 4, 0) {
            // Mongo version 4.4+ no longer returns numExtents,
            // since it is part of the obsolete MMAPv1 storage engine
            // https://www.mongodb.com/docs/manual/release-notes/4.4-compatibility/#mmapv1-cleanup
            return;
        }
        match crate::get_i64!(stats, "numExtents") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct GlobalLockTime(MongoMetric<Sum<i64>>);

impl GlobalLockTime {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.global_lock.time",
            description: "The time the global lock has been held.",
            unit: "ms",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

impl Record for GlobalLockTime {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "globalLock", "totalTime") {
            Ok(value) => {
                let lock_held_time_millis = value / 1000;
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(lock_held_time_millis, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct Health(MongoMetric<Gauge<i64>>);

impl Health {
    pub fn new() -> Self {
        let mut metric = MongoMetric::gauge(Descriptor {
            name: "mongodb.health",
            description: "The health status of the server.",
            unit: "1",
        });
        Self(metric)
    }
}

impl Record for Health {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "ok") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct IndexAccesses(MongoMetric<Sum<i64>>);

impl IndexAccesses {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.index.access.count",
            description: "The number of times an index has been accessed.",
            unit: "{accesses}",
        });
        Self(metric)
    }
}

impl IndexAccesses {
    pub fn record(
        &mut self,
        index_stats: &[bson::Bson],
        database_name: &str,
        collection_name: &str,
        config: &Config,
        errors: &mut Vec<Error>,
    ) -> eyre::Result<()> {
        let mut index_accesses_total = 0;
        for doc in index_stats {
            match crate::get_i64!(doc, "accesses", "ops") {
                Ok(value) => {
                    index_accesses_total += value;
                }
                Err(err) => errors.push(Error::CollectMetric {
                    metric: self.0.descriptor.name,
                    source: err.into(),
                }),
            }
        }
        self.0.data.data_points.push(DataPoint {
            attributes: vec![KeyValue::new("collection", collection_name.to_string())],
            ..datapoint(index_accesses_total, config)
        });
        Ok(())
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct IndexCount(MongoMetric<Sum<i64>>);

impl IndexCount {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.index.count",
            description: "The number of indexes.",
            unit: "{indexes}",
        });
        Self(metric)
    }
}

impl Record for IndexCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "indexes") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct IndexSize(MongoMetric<Sum<i64>>);

impl IndexSize {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.index.size",
            description: "Sum of the space allocated to all indexes in the database, including free index space.",
            unit: "By",
        });
        Self(metric)
    }
}

impl Record for IndexSize {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "indexSize") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

fn record_lock_metric(
    stats: &bson::Bson,
    key: &str,
    config: &Config,
    metric: &mut MongoMetric<Sum<i64>>,
    errors: &mut Vec<Error>,
) {
    if config.version < semver::Version::new(3, 2, 0) {
        // lock Metrics are only supported by MongoDB v3.2+
        return;
    }
    for lock_type in attributes::LockType::iter() {
        if !lock_type.supported(&config.version) {
            continue;
        }
        for lock_mode in attributes::LockMode::iter() {
            match crate::get_i64!(
                stats,
                "locks",
                lock_type.to_mongodb_key(),
                key,
                lock_mode.to_mongodb_key()
            ) {
                Ok(value) => {
                    metric.data.data_points.push(DataPoint {
                        attributes: vec![
                            KeyValue::new("lock_type", lock_type.as_str()),
                            KeyValue::new("lock_mode", lock_mode.as_str()),
                        ],
                        ..datapoint(value, config)
                    });
                }

                Err(doc::Error {
                    source: doc::QueryError::NotFound { .. },
                    ..
                }) => {
                    // mongoDB only publishes this lock metric is it is available.
                    // do not raise error when key is not found
                }
                Err(err) => {
                    errors.push(Error::CollectMetric {
                        metric: metric.descriptor.name,
                        source: err.into(),
                    });
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct LockAquireCount(MongoMetric<Sum<i64>>);

impl LockAquireCount {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.lock.acquire.count",
            description: "Number of times the lock was acquired in the specified mode.",
            unit: "{count}",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

impl Record for LockAquireCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        record_lock_metric(stats, "acquireCount", config, &mut self.0, errors);
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct LockAquireTime(MongoMetric<Sum<i64>>);

impl LockAquireTime {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.lock.acquire.time",
            description: "Cumulative wait time for the lock acquisitions.",
            unit: "microseconds",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

impl Record for LockAquireTime {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        record_lock_metric(stats, "timeAcquiringMicros", config, &mut self.0, errors);
        // if config.version < semver::Version::new(3, 2, 0) {
        //     // lock Metrics are only supported by MongoDB v3.2+
        //     return;
        // }
        // for lock_type in attributes::LockType::iter() {
        //     if !lock_type.supported(&config.version) {
        //         continue;
        //     }
        //     for lock_mode in attributes::LockMode::iter() {
        //         match crate::get_i64!(
        //             stats,
        //             "locks",
        //             lock_type.to_mongodb_key(),
        //             "timeAcquiringMicros",
        //             lock_mode.to_mongodb_key()
        //         ) {
        //             Ok(value) => {
        //                 self.0.data.data_points.push(DataPoint {
        //                     attributes: vec![
        //                         KeyValue::new("lock_type", lock_type.as_str()),
        //                         KeyValue::new("lock_mode", lock_mode.as_str()),
        //                     ],
        //                     ..datapoint(value, config)
        //                 });
        //             }
        //
        //             Err(doc::Error {
        //                 source: doc::QueryError::NotFound { .. },
        //                 ..
        //             }) => {
        //                 // mongoDB only publishes this lock metric is it is available.
        //                 // do not raise error when key is not found
        //             }
        //             Err(err) => {
        //                 errors.push(Error::CollectMetric {
        //                     metric: self.0.descriptor.name,
        //                     source: err.into(),
        //                 });
        //             }
        //         }
        //     }
        // }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct LockAquireWaitCount(MongoMetric<Sum<i64>>);

impl LockAquireWaitCount {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.lock.acquire.wait_count",
            description: "Number of times the lock acquisitions encountered waits because the locks were held in a conflicting mode.",
            unit: "{count}",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

impl Record for LockAquireWaitCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        record_lock_metric(stats, "acquireWaitCount", config, &mut self.0, errors);
        // if config.version < semver::Version::new(3, 2, 0) {
        //     // lock Metrics are only supported by MongoDB v3.2+
        //     return;
        // }
        // for lock_type in attributes::LockType::iter() {
        //     if !lock_type.supported(&config.version) {
        //         continue;
        //     }
        //     for lock_mode in attributes::LockMode::iter() {
        //         match crate::get_i64!(
        //             stats,
        //             "locks",
        //             lock_type.to_mongodb_key(),
        //             "acquireWaitCount",
        //             lock_mode.to_mongodb_key()
        //         ) {
        //             Ok(value) => {
        //                 self.0.data.data_points.push(DataPoint {
        //                     attributes: vec![
        //                         KeyValue::new("lock_type", lock_type.as_str()),
        //                         KeyValue::new("lock_mode", lock_mode.as_str()),
        //                     ],
        //                     ..datapoint(value, config)
        //                 });
        //             }
        //
        //             Err(doc::Error {
        //                 source: doc::QueryError::NotFound { .. },
        //                 ..
        //             }) => {
        //                 // mongoDB only publishes this lock metric is it is available.
        //                 // do not raise error when key is not found
        //             }
        //             Err(err) => {
        //                 errors.push(Error::CollectMetric {
        //                     metric: self.0.descriptor.name,
        //                     source: err.into(),
        //                 });
        //             }
        //         }
        //     }
        // }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct LockAquireDeadlockCount(MongoMetric<Sum<i64>>);

impl LockAquireDeadlockCount {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.lock.deadlock.count",
            description: "Number of times the lock acquisitions encountered deadlocks.",
            unit: "{count}",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

impl Record for LockAquireDeadlockCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        record_lock_metric(stats, "deadlockCount", config, &mut self.0, errors);
        // if config.version < semver::Version::new(3, 2, 0) {
        //     // lock Metrics are only supported by MongoDB v3.2+
        //     return;
        // }
        // for lock_type in attributes::LockType::iter() {
        //     if !lock_type.supported(&config.version) {
        //         continue;
        //     }
        //     for lock_mode in attributes::LockMode::iter() {
        //         match crate::get_i64!(
        //             stats,
        //             "locks",
        //             lock_type.to_mongodb_key(),
        //             "deadlockCount",
        //             lock_mode.to_mongodb_key()
        //         ) {
        //             Ok(value) => {
        //                 self.0.data.data_points.push(DataPoint {
        //                     attributes: vec![
        //                         KeyValue::new("lock_type", lock_type.as_str()),
        //                         KeyValue::new("lock_mode", lock_mode.as_str()),
        //                     ],
        //                     ..datapoint(value, config)
        //                 });
        //             }
        //
        //             Err(doc::Error {
        //                 source: doc::QueryError::NotFound { .. },
        //                 ..
        //             }) => {
        //                 // mongoDB only publishes this lock metric is it is available.
        //                 // do not raise error when key is not found
        //             }
        //             Err(err) => {
        //                 errors.push(Error::CollectMetric {
        //                     metric: self.0.descriptor.name,
        //                     source: err.into(),
        //                 });
        //             }
        //         }
        //     }
        // }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct MemoryUsage(MongoMetric<Sum<i64>>);

impl MemoryUsage {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.memory.usage",
            description: "The amount of memory used.",
            unit: "By",
        });
        Self(metric)
    }
}

impl Record for MemoryUsage {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for memory_type in attributes::MemoryType::iter() {
            match crate::get_i64!(stats, "mem", memory_type.to_mongodb_key()) {
                Ok(value) => {
                    // convert from mebibytes to bytes
                    let mem_usage_bytes = value * 1024 * 1024;
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new("type", memory_type.as_str())],
                        ..datapoint(mem_usage_bytes, config)
                    });
                }
                Err(err) => errors.push(Error::CollectMetric {
                    metric: self.0.descriptor.name,
                    source: err.into(),
                }),
            }
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct NetworkIn(MongoMetric<Sum<i64>>);

impl NetworkIn {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.network.io.receive",
            description: "The number of bytes received.",
            unit: "By",
        });
        Self(metric)
    }
}

impl Record for NetworkIn {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "network", "bytesIn") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct NetworkOut(MongoMetric<Sum<i64>>);

impl NetworkOut {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.network.io.transmit",
            description: "The number of by transmitted.",
            unit: "By",
        });
        Self(metric)
    }
}

impl Record for NetworkOut {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "network", "bytesOut") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct NetworkRequestCount(MongoMetric<Sum<i64>>);

impl NetworkRequestCount {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.network.request.count",
            description: "The number of requests received by the server.",
            unit: "{requests}",
        });
        Self(metric)
    }
}

impl Record for NetworkRequestCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "network", "numRequests") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct ObjectCount(MongoMetric<Sum<i64>>);

impl ObjectCount {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.object.count",
            description: "The number of objects.",
            unit: "{objects}",
        });
        Self(metric)
    }
}

impl Record for ObjectCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "objects") {
            Ok(value) => {
                self.0.data.data_points.push(DataPoint {
                    ..datapoint(value, config)
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.0.descriptor.name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct OperationCount(MongoMetric<Sum<i64>>);

impl OperationCount {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.operation.count",
            description: "The number of operations executed.",
            unit: "{operations}",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

impl Record for OperationCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for operation in attributes::Operation::iter() {
            match crate::get_i64!(stats, "opcounters", operation.to_mongodb_key()) {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new("operation", operation.as_str())],
                        ..datapoint(value, config)
                    });
                }
                Err(err) => errors.push(Error::CollectMetric {
                    metric: self.0.descriptor.name,
                    source: err.into(),
                }),
            }
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct OperationLatencyTime(MongoMetric<Gauge<i64>>);

impl OperationLatencyTime {
    pub fn new() -> Self {
        let mut metric = MongoMetric::gauge(Descriptor {
            name: "mongodb.operation.latency.time",
            description: "The latency of operations.",
            unit: "us",
        });
        Self(metric)
    }
}

impl Record for OperationLatencyTime {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for operation in attributes::OperationLatency::iter() {
            match crate::get_i64!(stats, "opLatencies", operation.to_mongodb_key(), "latency") {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new("operation", operation.as_str())],
                        ..datapoint(value, config)
                    });
                }
                Err(err) => errors.push(Error::CollectMetric {
                    metric: self.0.descriptor.name,
                    source: err.into(),
                }),
            }
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct OperationLatencyOps(MongoMetric<Sum<i64>>);

impl OperationLatencyOps {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.operation.latency.ops",
            description: "The total number of operations performed since startup.",
            unit: "{ops}",
        });
        Self(metric)
    }
}

impl Record for OperationLatencyOps {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for operation in attributes::OperationLatency::iter() {
            match crate::get_i64!(stats, "opLatencies", operation.to_mongodb_key(), "ops") {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new("operation", operation.as_str())],
                        ..datapoint(value, config)
                    });
                }
                Err(err) => errors.push(Error::CollectMetric {
                    metric: self.0.descriptor.name,
                    source: err.into(),
                }),
            }
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct OperationReplCount(MongoMetric<Sum<i64>>);

impl OperationReplCount {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.operation.repl.count",
            description: "The number of replicated operations executed.",
            unit: "{operations}",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

impl Record for OperationReplCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for operation in attributes::Operation::iter() {
            match crate::get_i64!(stats, "opcountersRepl", operation.to_mongodb_key(), "ops") {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new("operation", operation.as_str())],
                        ..datapoint(value, config)
                    });
                }
                Err(err) => errors.push(Error::CollectMetric {
                    metric: self.0.descriptor.name,
                    source: err.into(),
                }),
            }
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct OperationTime(MongoMetric<Sum<i64>>);

impl OperationTime {
    pub fn new() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.operation.time",
            description: "The total time spent performing operations.",
            unit: "ms",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

fn collection_path_names(stats: &bson::Bson) -> Result<(), doc::Error> {
    let totals = doc::get_path(stats, [doc::BsonKey::KeyStr("totals")])?;
    // for total in totals {}
    Ok(())
    // var collectionPathNames []string
    // for collectionPathName := range docTotals {
    // 	if collectionPathName != "note" {
    // 		collectionPathNames = append(collectionPathNames, collectionPathName)
    // 	}
    // }
    // return collectionPathNames, nil
}

impl Record for OperationTime {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        todo!();
        for operation in attributes::Operation::iter() {
            // match crate::get_i64!(stats, "opcountersRepl", operation.to_mongodb_key(), "ops") {
            //     Ok(value) => {
            //         self.0.data.data_points.push(DataPoint {
            //             attributes: vec![KeyValue::new("operation", operation.as_str())],
            //             ..datapoint(value, config)
            //         });
            //     }
            //     Err(err) => errors.push(Error::CollectMetric {
            //         metric: self.0.descriptor.name,
            //         source: err.into(),
            //     }),
            // }
        }
    }

    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}
