use color_eyre::eyre;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TlsConfig {
    pub insecure: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OtlpReceiverConfig {
    pub endpoint: String,
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct Receivers {
    #[serde(flatten)]
    pub receivers: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct Processors {
    #[serde(flatten)]
    pub processors: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum DebugVerbosity {
    #[serde(rename = "detailed")]
    Detailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DebugExporterConfig {
    pub verbosity: Option<DebugVerbosity>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct Exporters {
    #[serde(flatten)]
    pub exporters: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PipelineConfig {
    #[serde(default)]
    pub receivers: Vec<String>,
    #[serde(default)]
    pub processors: Vec<String>,
    #[serde(default)]
    pub exporters: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct Pipelines {
    #[serde(flatten)]
    pub pipelines: HashMap<String, PipelineConfig>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct Services {
    pub pipelines: Pipelines,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub receivers: Receivers,
    #[serde(default)]
    pub processors: Processors,
    #[serde(default)]
    pub exporters: Exporters,
    #[serde(default)]
    pub service: Services,
}

impl Config {
    pub fn from_file(path: impl AsRef<std::path::Path>) -> eyre::Result<Self> {
        let file = std::fs::OpenOptions::new().read(true).open(path)?;
        let mut reader = std::io::BufReader::new(file);
        Self::from_reader(reader)
    }

    pub fn from_reader(reader: impl std::io::BufRead) -> eyre::Result<Self> {
        let config = serde_yaml::from_reader(reader)?;
        Ok(config)
    }
}
