#![allow(warnings)]

pub mod attributes;
pub mod config;
pub mod consumer;
pub mod doc;
pub mod ext;
pub mod metrics;
pub mod mongodb;
pub mod otlp;
pub mod pipeline;
pub mod prometheus;
pub mod scrape;
pub mod telemetry;

use opentelemetry::{InstrumentationLibrary, KeyValue, Value};

lazy_static::lazy_static! {
    static ref LIBRARY: InstrumentationLibrary = opentelemetry_sdk::InstrumentationLibrary::builder("mongodb-opentelemetry-collector")
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
        .build();
}
