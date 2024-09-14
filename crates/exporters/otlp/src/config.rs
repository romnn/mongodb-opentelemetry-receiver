use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OtlpExporterConfig {
    pub endpoint: String,
    pub tls: Option<otel_collector_component::config::TlsConfig>,
}
