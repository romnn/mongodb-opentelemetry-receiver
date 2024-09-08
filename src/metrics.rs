use color_eyre::eyre;
use mongodb::bson;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::data::{
    Aggregation, DataPoint, Histogram, Metric, Sum, Temporality,
};
use std::time::SystemTime;

use crate::attributes;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to collect metric {metric:?} with attribute(s) {attributes:?}")]
    CollectMetric {
        metric: &'static str,
        attributes: Option<String>,
        #[source]
        source: Box<dyn std::error::Error>,
    },
}

pub trait Record {
    fn record(
        &mut self,
        server_status: &bson::Bson,
        database_name: Option<String>,
        start_time: SystemTime,
        time: SystemTime,
        errors: &mut Vec<Error>,
    ) -> ();

    fn emit(&mut self) -> Metric;
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
pub struct CollectionCount {
    inner: MongoMetric<Sum<i64>>,
}

impl CollectionCount {
    pub fn new() -> Self {
        Self {
            inner: MongoMetric {
                descriptor: Descriptor {
                    name: "mongodb.collection.count".into(),
                    description: "The number of collections.".into(),
                    unit: "{collections}".into(),
                },
                data: Sum {
                    data_points: vec![],
                    is_monotonic: false,
                    temporality: Temporality::Cumulative,
                },
            },
        }
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
    fn record(
        &mut self,
        stats: &bson::Bson,
        database_name: Option<String>,
        start_time: SystemTime,
        time: SystemTime,
        errors: &mut Vec<Error>,
    ) -> () {
        match crate::get_i64!(stats, "collections") {
            Ok(collection_count) => {
                self.inner.data.data_points.push(DataPoint {
                    start_time: Some(start_time),
                    time: Some(time),
                    attributes: vec![],
                    exemplars: vec![],
                    value: collection_count,
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.inner.descriptor.name,
                attributes: Some("miss, hit".to_string()),
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.inner.emit()
    }
}

#[derive(Debug)]
pub struct DataSize {
    pub inner: MongoMetric<Sum<i64>>,
}

impl DataSize {
    pub fn new() -> Self {
        Self {
            inner: MongoMetric {
                descriptor: Descriptor {
                    name: "mongodb.data.size".into(),
                    description:
                        "The size of the collection. Data compression does not affect this value."
                            .into(),
                    unit: "By".into(),
                },
                data: Sum {
                    data_points: vec![],
                    is_monotonic: false,
                    temporality: Temporality::Cumulative,
                },
            },
        }
    }
}

impl Record for DataSize {
    fn record(
        &mut self,
        stats: &bson::Bson,
        database_name: Option<String>,
        start_time: SystemTime,
        time: SystemTime,
        errors: &mut Vec<Error>,
    ) -> () {
        match crate::get_i64!(stats, "mongodb", "data", "size") {
            Ok(data_size) => {
                self.inner.data.data_points.push(DataPoint {
                    start_time: Some(start_time),
                    time: Some(time),
                    attributes: vec![],
                    exemplars: vec![],
                    value: data_size,
                });
            }
            Err(err) => errors.push(Error::CollectMetric {
                metric: self.inner.descriptor.name,
                attributes: database_name,
                source: err.into(),
            }),
        }
    }

    fn emit(&mut self) -> Metric {
        self.inner.emit()
    }
}

#[derive(Debug)]
pub struct ConnectionCount {
    pub inner: MongoMetric<Sum<i64>>,
}

impl ConnectionCount {
    pub fn new() -> Self {
        Self {
            inner: MongoMetric {
                descriptor: Descriptor {
                    name: "mongodb.connection.count",
                    description: "The number of connections.".into(),
                    unit: "{connections}",
                },
                data: Sum {
                    data_points: vec![],
                    is_monotonic: false,
                    temporality: Temporality::Cumulative,
                },
            },
        }
    }
}

impl Record for ConnectionCount {
    fn record(
        &mut self,
        stats: &bson::Bson,
        database_name: Option<String>,
        start_time: SystemTime,
        time: SystemTime,
        errors: &mut Vec<Error>,
    ) -> () {
        for connection_type in attributes::ConnectionType::iter() {
            match crate::get_i64!(stats, "connections", connection_type.as_str()) {
                Ok(data_size) => {
                    self.inner.data.data_points.push(DataPoint {
                        start_time: Some(start_time),
                        time: Some(time),
                        attributes: vec![KeyValue::new("type", connection_type.as_str())],
                        exemplars: vec![],
                        value: data_size,
                    });
                }
                Err(err) => errors.push(Error::CollectMetric {
                    metric: self.inner.descriptor.name,
                    attributes: database_name.clone(),
                    source: err.into(),
                }),
            }
        }
    }

    fn emit(&mut self) -> Metric {
        self.inner.emit()
    }
}
