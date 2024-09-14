use color_eyre::eyre;
use regex::Regex;

lazy_static::lazy_static! {
    static ref COMPONENT_NAME_REGEX: Regex = Regex::new(r"^[a-zA-Z][0-9a-zA-Z_]{0,62}$").unwrap();
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentName(String);

impl ComponentName {
    pub fn new(name: impl Into<String>) -> eyre::Result<Self> {
        let name: String = name.into();
        if !COMPONENT_NAME_REGEX.is_match(&name) {
            eyre::bail!("invalid name: {name:?}");
        }
        Ok(Self(name))
    }
}

impl From<&ComponentName> for String {
    fn from(value: &ComponentName) -> Self {
        value.0.to_string()
    }
}

// #[async_trait::async_trait]
// pub trait Factory<T> {
//     fn component_name(&self) -> &ComponentName;
//     async fn build(&self, id: String, config: serde_yaml::Value) -> eyre::Result<T>;
// }

// pub type ReceiverFactory = dyn Factory<Box<dyn crate::Receiver>>;
// pub type ProcessorFactory = dyn Factory<Box<dyn crate::Processor>>;
// pub type ExporterFactory = dyn Factory<Box<dyn crate::Exporter>>;

#[async_trait::async_trait]
pub trait ReceiverFactory {
    fn component_name(&self) -> &ComponentName;
    async fn build(
        &self,
        id: String,
        config: serde_yaml::Value,
    ) -> eyre::Result<Box<dyn crate::Receiver>>;
}

#[async_trait::async_trait]
pub trait ProcessorFactory {
    fn component_name(&self) -> &ComponentName;
    async fn build(
        &self,
        id: String,
        config: serde_yaml::Value,
    ) -> eyre::Result<Box<dyn crate::Processor>>;
}

#[async_trait::async_trait]
pub trait ExporterFactory {
    fn component_name(&self) -> &ComponentName;
    async fn build(
        &self,
        id: String,
        config: serde_yaml::Value,
    ) -> eyre::Result<Box<dyn crate::Exporter>>;
}

// impl ExporterFactory {
//     pub fn build(&self) -> Box<dyn{
//     }
// }
