use color_eyre::eyre;
use mongodb::bson;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::data::{
    Aggregation, DataPoint, Gauge, Histogram, Metric, Sum, Temporality,
};
use std::collections::HashMap;
use std::time::SystemTime;
use tracing::trace;

use crate::{attributes, doc};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to collect {metric:?}: {source}")]
    CollectMetric {
        metric: &'static str,
        #[source]
        source: doc::Error,
    },
}

impl Error {
    pub fn partial_match(&self) -> Option<&doc::Match> {
        match self {
            Self::CollectMetric {
                source: doc::Error { path, source },
                ..
            } => source.partial_match(),
            _ => None,
        }
    }
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

pub trait Record: std::fmt::Debug + EmitMetric {
    fn record(
        &mut self,
        server_status: &bson::Bson,
        config: &Config,
        errors: &mut Vec<Error>,
    ) -> ();
}

pub trait EmitMetric {
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

impl Default for CollectionCount {
    fn default() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.collection.count",
            description: "The number of collections.",
            unit: "{collections}",
        }))
    }
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
}

impl EmitMetric for CollectionCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct DataSize(MongoMetric<Sum<i64>>);

impl Default for DataSize {
    fn default() -> Self {
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
}

impl EmitMetric for DataSize {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct ConnectionCount(MongoMetric<Sum<i64>>);

impl Default for ConnectionCount {
    fn default() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.connection.count",
            description: "The number of connections.",
            unit: "{connections}",
        }))
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
pub enum ConnectionType {
    #[strum(serialize = "active")]
    Active,
    #[strum(serialize = "available")]
    Available,
    #[strum(serialize = "current")]
    Current,
}

impl ConnectionType {
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn attribute(&self) -> attributes::ConnectionType {
        match self {
            Self::Available => attributes::ConnectionType::Available,
            Self::Active => attributes::ConnectionType::Active,
            Self::Current => attributes::ConnectionType::Current,
        }
    }
}

impl Record for ConnectionCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for connection_type in ConnectionType::iter() {
            match crate::get_i64!(stats, "connections", connection_type.as_str()) {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new(
                            "type",
                            connection_type.attribute().as_str(),
                        )],
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
}

impl EmitMetric for ConnectionCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct CacheOperations(MongoMetric<Sum<i64>>);

impl Default for CacheOperations {
    fn default() -> Self {
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
            attributes: vec![KeyValue::new("type", attributes::CacheStatus::Hit.as_str())],
            ..datapoint(cache_hits, config)
        });
    }
}

impl EmitMetric for CacheOperations {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct CursorCount(MongoMetric<Sum<i64>>);

impl Default for CursorCount {
    fn default() -> Self {
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

impl EmitMetric for CursorCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct CursorTimeouts(MongoMetric<Sum<i64>>);

impl Default for CursorTimeouts {
    fn default() -> Self {
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
}

impl EmitMetric for CursorTimeouts {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct DatabaseCount(MongoMetric<Sum<i64>>);

impl Default for DatabaseCount {
    fn default() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.database.count",
            description: "The number of existing databases.",
            unit: "{databases}",
        }))
    }
}

impl DatabaseCount {
    pub fn record(&mut self, num_databases: usize, config: &Config) -> () {
        self.0.data.data_points.push(DataPoint {
            ..datapoint(num_databases.try_into().unwrap_or(i64::MAX), config)
        });
    }
}

impl EmitMetric for DatabaseCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct DocumentOperations(MongoMetric<Sum<i64>>);

impl Default for DocumentOperations {
    fn default() -> Self {
        Self(MongoMetric::sum(Descriptor {
            name: "mongodb.document.operation.count",
            description: "The number of document operations executed.",
            unit: "{documents}",
        }))
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
pub enum Operation {
    #[strum(serialize = "insert")]
    Insert,
    #[strum(serialize = "query")]
    Query,
    #[strum(serialize = "update")]
    Update,
    #[strum(serialize = "delete")]
    Delete,
    #[strum(serialize = "getmore")]
    Getmore,
    #[strum(serialize = "command")]
    Command,
}

impl Operation {
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn attribute(&self) -> attributes::Operation {
        match self {
            Self::Insert => attributes::Operation::Insert,
            Self::Query => attributes::Operation::Query,
            Self::Update => attributes::Operation::Update,
            Self::Delete => attributes::Operation::Delete,
            Self::Getmore => attributes::Operation::Getmore,
            Self::Command => attributes::Operation::Command,
        }
    }
}

impl Record for DocumentOperations {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for operation in Operation::iter() {
            match crate::get_i64!(stats, "metrics", "document", operation.as_str()) {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new(
                            "operation",
                            operation.attribute().as_str(),
                        )],
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
}

impl EmitMetric for DocumentOperations {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct Extent(MongoMetric<Sum<i64>>);

impl Default for Extent {
    fn default() -> Self {
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
}

impl EmitMetric for Extent {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct GlobalLockTime(MongoMetric<Sum<i64>>);

impl Default for GlobalLockTime {
    fn default() -> Self {
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
}

impl EmitMetric for GlobalLockTime {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct Health(MongoMetric<Gauge<i64>>);

impl Default for Health {
    fn default() -> Self {
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
}

impl EmitMetric for Health {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct IndexAccesses(MongoMetric<Sum<i64>>);

impl Default for IndexAccesses {
    fn default() -> Self {
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
}

impl EmitMetric for IndexAccesses {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct IndexCount(MongoMetric<Sum<i64>>);

impl Default for IndexCount {
    fn default() -> Self {
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
}

impl EmitMetric for IndexCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct IndexSize(MongoMetric<Sum<i64>>);

impl Default for IndexSize {
    fn default() -> Self {
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
}

impl EmitMetric for IndexSize {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
pub enum LockMode {
    #[strum(serialize = "R")]
    Shared,
    #[strum(serialize = "W")]
    Exclusive,
    #[strum(serialize = "r")]
    IntentShared,
    #[strum(serialize = "w")]
    IntentExclusive,
}

impl LockMode {
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn attribute(&self) -> attributes::LockMode {
        match self {
            LockMode::Shared => attributes::LockMode::Shared,
            LockMode::Exclusive => attributes::LockMode::Exclusive,
            LockMode::IntentShared => attributes::LockMode::IntentShared,
            LockMode::IntentExclusive => attributes::LockMode::IntentExclusive,
        }
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
pub enum LockType {
    #[strum(serialize = "ParallelBatchWriterMode")]
    ParallelBatchWriteMode,
    #[strum(serialize = "ReplicationStateTransition")]
    ReplicationStateTransition,
    #[strum(serialize = "Global")]
    Global,
    #[strum(serialize = "Database")]
    Database,
    #[strum(serialize = "Collection")]
    Collection,
    #[strum(serialize = "Mutex")]
    Mutex,
    #[strum(serialize = "Metadata")]
    Metadata,
    #[strum(serialize = "oplog")]
    Oplog,
}

impl LockType {
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn supported(&self, version: &semver::Version) -> bool {
        match self {
            LockType::ParallelBatchWriteMode | LockType::ReplicationStateTransition
                if version < &semver::Version::new(4, 2, 0) =>
            {
                false
            }
            _ => true,
        }
    }

    pub fn attribute(&self) -> attributes::LockType {
        match self {
            Self::ParallelBatchWriteMode => attributes::LockType::ParallelBatchWriteMode,
            Self::ReplicationStateTransition => attributes::LockType::ReplicationStateTransition,
            Self::Global => attributes::LockType::Global,
            Self::Database => attributes::LockType::Database,
            Self::Collection => attributes::LockType::Collection,
            Self::Mutex => attributes::LockType::Mutex,
            Self::Metadata => attributes::LockType::Metadata,
            Self::Oplog => attributes::LockType::Oplog,
        }
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
    for lock_type in LockType::iter() {
        if !lock_type.supported(&config.version) {
            continue;
        }
        for lock_mode in LockMode::iter() {
            match crate::get_i64!(stats, "locks", lock_type.as_str(), key, lock_mode.as_str()) {
                Ok(value) => {
                    metric.data.data_points.push(DataPoint {
                        attributes: vec![
                            KeyValue::new("lock_type", lock_type.attribute().as_str()),
                            KeyValue::new("lock_mode", lock_mode.attribute().as_str()),
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

impl Default for LockAquireCount {
    fn default() -> Self {
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
}

impl EmitMetric for LockAquireCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct LockAquireTime(MongoMetric<Sum<i64>>);

impl Default for LockAquireTime {
    fn default() -> Self {
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
    }
}

impl EmitMetric for LockAquireTime {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct LockAquireWaitCount(MongoMetric<Sum<i64>>);

impl Default for LockAquireWaitCount {
    fn default() -> Self {
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
    }
}

impl EmitMetric for LockAquireWaitCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct LockAquireDeadlockCount(MongoMetric<Sum<i64>>);

impl Default for LockAquireDeadlockCount {
    fn default() -> Self {
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
    }
}

impl EmitMetric for LockAquireDeadlockCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct MemoryUsage(MongoMetric<Sum<i64>>);

impl Default for MemoryUsage {
    fn default() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.memory.usage",
            description: "The amount of memory used.",
            unit: "By",
        });
        Self(metric)
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
pub enum MemoryType {
    #[strum(serialize = "resident")]
    Resident,
    #[strum(serialize = "virtual")]
    Virtual,
}

impl MemoryType {
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn attribute(&self) -> attributes::MemoryType {
        match self {
            Self::Resident => attributes::MemoryType::Resident,
            Self::Virtual => attributes::MemoryType::Virtual,
        }
    }
}

impl Record for MemoryUsage {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for memory_type in MemoryType::iter() {
            match crate::get_i64!(stats, "mem", memory_type.as_str()) {
                Ok(value) => {
                    // convert from mebibytes to bytes
                    let mem_usage_bytes = value * 1024 * 1024;
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new("type", memory_type.attribute().as_str())],
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
}

impl EmitMetric for MemoryUsage {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct NetworkIn(MongoMetric<Sum<i64>>);

impl Default for NetworkIn {
    fn default() -> Self {
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
}

impl EmitMetric for NetworkIn {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct NetworkOut(MongoMetric<Sum<i64>>);

impl Default for NetworkOut {
    fn default() -> Self {
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
}

impl EmitMetric for NetworkOut {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct NetworkRequestCount(MongoMetric<Sum<i64>>);

impl Default for NetworkRequestCount {
    fn default() -> Self {
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
}

impl EmitMetric for NetworkRequestCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct ObjectCount(MongoMetric<Sum<i64>>);

impl Default for ObjectCount {
    fn default() -> Self {
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
}

impl EmitMetric for ObjectCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct OperationCount(MongoMetric<Sum<i64>>);

impl Default for OperationCount {
    fn default() -> Self {
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
        for operation in Operation::iter() {
            match crate::get_i64!(stats, "opcounters", operation.as_str()) {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new(
                            "operation",
                            operation.attribute().as_str(),
                        )],
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
}

impl EmitMetric for OperationCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct OperationLatencyTime(MongoMetric<Gauge<i64>>);

impl Default for OperationLatencyTime {
    fn default() -> Self {
        let mut metric = MongoMetric::gauge(Descriptor {
            name: "mongodb.operation.latency.time",
            description: "The latency of operations.",
            unit: "us",
        });
        Self(metric)
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
pub enum OperationLatency {
    #[strum(serialize = "reads")]
    Read,
    #[strum(serialize = "writes")]
    Write,
    #[strum(serialize = "commands")]
    Command,
}

impl OperationLatency {
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn attribute(&self) -> attributes::OperationLatency {
        match self {
            Self::Read => attributes::OperationLatency::Read,
            Self::Write => attributes::OperationLatency::Write,
            Self::Command => attributes::OperationLatency::Command,
        }
    }
}

impl Record for OperationLatencyTime {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        for operation in OperationLatency::iter() {
            match crate::get_i64!(stats, "opLatencies", operation.as_str(), "latency") {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new(
                            "operation",
                            operation.attribute().as_str(),
                        )],
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
}

impl EmitMetric for OperationLatencyTime {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct OperationLatencyOps(MongoMetric<Sum<i64>>);

impl Default for OperationLatencyOps {
    fn default() -> Self {
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
        for operation in OperationLatency::iter() {
            match crate::get_i64!(stats, "opLatencies", operation.as_str(), "ops") {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new(
                            "operation",
                            operation.attribute().as_str(),
                        )],
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
}

impl EmitMetric for OperationLatencyOps {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct OperationReplCount(MongoMetric<Sum<i64>>);

impl Default for OperationReplCount {
    fn default() -> Self {
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
        for operation in Operation::iter() {
            match crate::get_i64!(stats, "opcountersRepl", operation.as_str()) {
                Ok(value) => {
                    self.0.data.data_points.push(DataPoint {
                        attributes: vec![KeyValue::new(
                            "operation",
                            operation.attribute().as_str(),
                        )],
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
}

impl EmitMetric for OperationReplCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct OperationTime(MongoMetric<Sum<i64>>);

impl Default for OperationTime {
    fn default() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.operation.time",
            description: "The total time spent performing operations.",
            unit: "ms",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

fn collection_path_names(stats: &bson::Bson) -> Result<Vec<&str>, doc::Error> {
    let path = [doc::BsonKey::KeyStr("totals")];
    let totals = doc::get_path(stats, path)?;
    let totals = totals.as_document().ok_or_else(|| doc::Error {
        path: doc::OwnedPath::from_iter(path.iter()),
        source: doc::QueryError::InvalidType(doc::InvalidTypeError {
            expected_type: "map".to_string(),
            value: totals.clone(),
        }),
    })?;
    let collection_path_names = totals
        .keys()
        .map(|name| name.as_str())
        .filter(|name| *name != "note")
        .collect();
    Ok(collection_path_names)
}

// TODO: using a hash map is so overkill
fn aggregate_operation_time_values(
    stats: &bson::Bson,
    collection_path_names: &[&str],
) -> Result<HashMap<Operation, i64>, doc::Error> {
    let mut aggregated: HashMap<Operation, i64> = HashMap::new();
    for collection_path_name in collection_path_names {
        for operation in Operation::iter() {
            let key = match operation {
                Operation::Insert => "insert",
                Operation::Query => "queries",
                Operation::Update => "update",
                Operation::Delete => "remove",
                Operation::Getmore => "getmore",
                Operation::Command => "commands",
            };
            let value = crate::get_i64!(stats, "totals", *collection_path_name, key, "time")?;
            *aggregated.entry(operation).or_insert(0) += value;
        }
    }
    Ok(aggregated)
}

impl Record for OperationTime {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        let collection_path_names = match collection_path_names(stats) {
            Ok(collection_path_names) => collection_path_names,
            Err(err) => {
                errors.push(Error::CollectMetric {
                    metric: self.0.descriptor.name,
                    source: err.into(),
                });
                return;
            }
        };
        trace!("{:#?}", collection_path_names);
        // trace!(?collection_path_names);

        let aggregated_operation_times =
            match aggregate_operation_time_values(stats, &collection_path_names) {
                Ok(collection_path_names) => collection_path_names,
                Err(err) => {
                    errors.push(Error::CollectMetric {
                        metric: self.0.descriptor.name,
                        source: err.into(),
                    });
                    return;
                }
            };

        for (operation, value) in aggregated_operation_times {
            self.0.data.data_points.push(DataPoint {
                attributes: vec![KeyValue::new("operation", operation.attribute().as_str())],
                ..datapoint(value, config)
            });
        }
    }
}

impl EmitMetric for OperationTime {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct SessionCount(MongoMetric<Sum<i64>>);

impl Default for SessionCount {
    fn default() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.session.count",
            description: "The total number of active sessions.",
            unit: "{sessions}",
        });
        Self(metric)
    }
}

impl Record for SessionCount {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        if config.storage_engine != Some(StorageEngine::WiredTiger) {
            // metric can not be collected: mongodb uses different storage engine
            return;
        }
        match crate::get_i64!(stats, "wiredTiger", "session", "open session count") {
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
}

impl EmitMetric for SessionCount {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct StorageSize(MongoMetric<Sum<i64>>);

impl Default for StorageSize {
    fn default() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.storage.size",
            description: "The total amount of storage allocated to this collection.",
            unit: "By",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

impl Record for StorageSize {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "storageSize") {
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
}

impl EmitMetric for StorageSize {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}

#[derive(Debug)]
pub struct Uptime(MongoMetric<Sum<i64>>);

impl Default for Uptime {
    fn default() -> Self {
        let mut metric = MongoMetric::sum(Descriptor {
            name: "mongodb.uptime",
            description: "The amount of time that the server has been running.",
            unit: "ms",
        });
        metric.data.is_monotonic = true;
        Self(metric)
    }
}

impl Record for Uptime {
    fn record(&mut self, stats: &bson::Bson, config: &Config, errors: &mut Vec<Error>) -> () {
        match crate::get_i64!(stats, "uptimeMillis") {
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
}

impl EmitMetric for Uptime {
    fn emit(&mut self) -> Metric {
        self.0.emit()
    }
}
