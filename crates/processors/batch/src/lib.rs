#![allow(warnings)]

pub mod config;

use crate::config::BatchProcessorConfig;
use color_eyre::eyre;
use futures::StreamExt;
use otel_collector_component::factory::ComponentName;
use otel_collector_component::{MetricPayload, MetricsStream};
use tokio::sync::{broadcast, watch};
use tokio_stream::wrappers::BroadcastStream;
use tracing::{trace, warn};

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
    async fn start(
        self: Box<Self>,
        mut shutdown_rx: watch::Receiver<bool>,
        mut metrics: MetricsStream,
    ) -> eyre::Result<()> {
        loop {
            let (from, resource_metrics) = tokio::select! {
                Some(payload) = metrics.next() => payload,
                _ = shutdown_rx.changed() => break,
            };
            let resource_metrics = match resource_metrics {
                Ok(resource_metrics) => resource_metrics,
                Err(err) => {
                    warn!(
                        "{} received metric error {:?} from {:?}",
                        self.id, err, from
                    );
                    continue;
                }
            };
            trace!(
                "{} received {} metrics from {:?} [queue size = {}]",
                self.id,
                resource_metrics.len(),
                from,
                self.metrics_tx.len(),
                // tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            );
            if let Err(err) = self.metrics_tx.send(resource_metrics) {
                tracing::error!("{} failed to send: {err}", self.id);
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
