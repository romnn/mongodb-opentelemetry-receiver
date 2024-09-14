use duration_string::DurationString;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MongoDbHostConfig {
    pub endpoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MongoDbReceiverConfig {
    pub hosts: Vec<MongoDbHostConfig>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub collection_interval: Option<DurationString>,
    pub initial_delay: Option<DurationString>,
    pub tls: Option<otel_collector_component::config::TlsConfig>,
}
