#![allow(warnings)]

pub mod config;

use crate::config::BatchProcessorConfig;
use color_eyre::eyre;
use futures::StreamExt;
use otel_collector_component::factory::ComponentName;
use otel_collector_component::{MetricPayload, MetricsStream};
use tokio::sync::{broadcast, watch};
use tokio_stream::wrappers::BroadcastStream;

pub const DEFAULT_BUFFER_SIZE: usize = 1024;

lazy_static::lazy_static! {
    static ref COMPONENT_NAME: ComponentName = ComponentName::new("batch").unwrap();
}

#[derive(Debug, Default)]
pub struct Factory {}

#[async_trait::async_trait]
impl otel_collector_component::factory::ProcessorFactory for Factory {
    fn component_name(&self) -> &ComponentName {
        &COMPONENT_NAME
    }

    async fn build(
        &self,
        id: String,
        config: serde_yaml::Value,
    ) -> eyre::Result<Box<dyn otel_collector_component::Processor>> {
        let processor = BatchProcessor::from_config(id, config)?;
        Ok(Box::new(processor))
    }
}

#[derive(Debug)]
pub struct BatchProcessor {
    pub id: String,
    pub config: config::BatchProcessorConfig,
    pub metrics_tx: broadcast::Sender<MetricPayload>,
}

impl BatchProcessor {
    pub fn from_config(id: String, config: serde_yaml::Value) -> eyre::Result<Self> {
        let config: BatchProcessorConfig = serde_yaml::from_value(config)?;
        let (metrics_tx, _) = broadcast::channel(DEFAULT_BUFFER_SIZE);
        Ok(Self {
            id,
            config,
            metrics_tx,
        })
    }
}

#[async_trait::async_trait]
impl otel_collector_component::Processor for BatchProcessor {
    fn id(&self) -> &str {
        &self.id
    }

    async fn start(
        self: Box<Self>,
        shutdown_rx: watch::Receiver<bool>,
        mut metrics: MetricsStream,
    ) -> eyre::Result<()> {
        while let Some((from, metric)) = metrics.next().await {
            tracing::debug!("{} received metric {:?} from {:?}", self.id, metric, from);
            let metric = match metric {
                Ok(metric) => metric,
                Err(err) => {
                    continue;
                }
            };
            if let Err(err) = self.metrics_tx.send(metric) {
                tracing::error!("{} failed to send: {err}", self.to_service_id());
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl otel_collector_component::Producer for BatchProcessor {
    fn metrics(&self) -> BroadcastStream<MetricPayload> {
        BroadcastStream::new(self.metrics_tx.subscribe())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {}
}
