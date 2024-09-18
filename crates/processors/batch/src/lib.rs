#![allow(warnings)]

pub mod config;

use crate::config::BatchProcessorConfig;
use color_eyre::eyre;
use futures::{FutureExt, StreamExt};
use otel_collector_component::factory::ComponentName;
use otel_collector_component::{MetricPayload, MetricsStream, ServiceIdentifier};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};
use tracing::{error, info, trace, warn};

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

// #[pin_project::pin_project(project = TimerProj)]
// pub enum Timer {
//     Infinite,
//     Finite {
//         // #[pin]
//         // inner: tokio::time::Sleep,
//         inner: std::pin::Pin<Box<tokio::time::Sleep>>,
//         deadline: Duration,
//     },
// }
//
// impl std::fmt::Debug for Timer {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::Finite { inner, deadline } => f
//                 .debug_struct("Finite")
//                 .field("deadline", deadline)
//                 .finish(),
//             Self::Infinite => write!(f, "Infinite"),
//         }
//     }
// }
//
// impl Timer {
//     pub fn infinite() -> Self {
//         Self::Infinite
//     }
//
//     pub fn finite(deadline: Duration) -> Self {
//         Self::Finite {
//             inner: Box::pin(tokio::time::sleep(deadline)),
//             deadline,
//         }
//     }
//
//     pub fn reset(&mut self) {
//         match self {
//             Self::Finite { inner, deadline } => {
//                 std::mem::replace(inner, Box::pin(tokio::time::sleep(*deadline)));
//                 inner.poll_unpin(cx);
//             }
//             _ => {}
//         }
//     }
// }
//
// fn assert_unpin<T: Unpin>() {}
// fn test() {
//     assert_unpin::<Box<std::pin::Pin<tokio::time::Sleep>>>();
//     assert_unpin::<Box<tokio::time::Sleep>>();
//     assert_unpin::<Timer>();
// }
//
// impl futures::Future for Timer {
//     type Output = <tokio::time::Sleep as futures::Future>::Output;
//
//     fn poll(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Self::Output> {
//         match self.project() {
//             TimerProj::Infinite => std::task::Poll::Pending,
//             TimerProj::Finite { inner, deadline } => {
//                 dbg!(inner.poll_unpin(cx))
//             }
//         }
//     }
// }

// #[derive(Debug)]
// pub struct Shard {}
//
// impl Shard {
//     pub async fn start(&self) {
//         let timer = Timer::finite(Duration::from_secs(10));
//         timer.await;
//     }
// }

// SingleShardBatcher is used when `metadata_keys` is empty,
// to avoid the additional lock and map operations used in `MultiShardBatcher`.
// #[derive(Debug)]
// pub struct SingleShardBatcher {
//     pub shard: Shard,
//     pub timer: Timer,
// }
//
// impl Shard {
//     pub fn process(&mut self, metrics: MetricPayload) {
//         // let
//     }
// }

// #[derive(Debug)]
// pub struct MultiShardBatcher {
//     pub shards: HashMap<HashSet<String>, Shard>,
//     pub metadata_keys: Vec<String>,
//     pub metadata_cardinality_limit: usize,
// }
//
// impl MultiShardBatcher {
//     fn process(&mut self, metrics: MetricPayload) {
//         // get metadata
//         // for m in metrics.iter() {
//         //     let resource_attrs = m.resource.iter().collect::<Vec<_>>();
//         //     let metric_attrs = m.scope_metrics.iter().map(|sm| sm.scope.attributes).collect:<Vec<_>>();
//         //     let metric_attrs = m.scope_metrics.iter().map(|sm| sm.metrics.map(|mm| mm.a)).collect:<Vec<_>>();
//         // }
//         // md := map[string][]string{}
//         // 	var attrs []attribute.KeyValue
//         // 	for _, k := range mb.metadataKeys {
//         // 		// Lookup the value in the incoming metadata, copy it
//         // 		// into the outgoing metadata, and create a unique
//         // 		// value for the attributeSet.
//         // 		vs := info.Metadata.Get(k)
//         // 		md[k] = vs
//         // 		if len(vs) == 1 {
//         // 			attrs = append(attrs, attribute.String(k, vs[0]))
//         // 		} else {
//         // 			attrs = append(attrs, attribute.StringSlice(k, vs))
//         // 		}
//         // 	}
//         // 	aset := attribute.NewSet(attrs...)
//         //
//         // 	b, ok := mb.batchers.Load(aset)
//         // 	if !ok {
//         // 		mb.lock.Lock()
//         // 		if mb.metadataLimit != 0 && mb.size >= mb.metadataLimit {
//         // 			mb.lock.Unlock()
//         // 			return errTooManyBatchers
//         // 		}
//         //
//         // 		// aset.ToSlice() returns the sorted, deduplicated,
//         // 		// and name-downcased list of attributes.
//         // 		var loaded bool
//         // 		b, loaded = mb.batchers.LoadOrStore(aset, mb.newShard(md))
//         // 		if !loaded {
//         // 			// Start the goroutine only if we added the object to the map, otherwise is already started.
//         // 			b.(*shard).start()
//         // 			mb.size++
//         // 		}
//         // 		mb.lock.Unlock()
//         // 	}
//         // 	b.(*shard).newItem <- data
//     }
// }

// #[derive(Debug)]
// pub enum Batcher {
//     SingleShard(SingleShardBatcher),
//     MultiShard(MultiShardBatcher),
// }

#[derive(Debug)]
pub struct BatchProcessor {
    pub id: String,
    pub send_batch_size: Option<usize>,
    pub send_batch_max_size: Option<usize>,
    pub timeout: Option<Duration>,
    deadlines: tokio_util::time::DelayQueue<()>,
    metrics_tx: broadcast::Sender<MetricPayload>,
    // batcher: Shard,
    current_batch: Vec<MetricPayload>,
    // current_batch_size: usize,
}

impl BatchProcessor {
    pub fn from_config(id: String, config: serde_yaml::Value) -> eyre::Result<Self> {
        let config: BatchProcessorConfig = serde_yaml::from_value(config)?;
        let mut send_batch_size =
            config
                .send_batch_size
                .map_or(Some(8192), |size| if size == 0 { None } else { Some(size) });
        let send_batch_max_size = config.send_batch_max_size.and_then(|size| {
            if size == 0 {
                None
            } else {
                Some(size.max(send_batch_size.unwrap_or(0)))
            }
        });
        let timeout = config
            .timeout
            .map_or(Some(Duration::from_millis(200)), |timeout| {
                let timeout: Duration = timeout.into();
                if timeout.is_zero() {
                    None
                } else {
                    Some(timeout)
                }
            });

        if timeout.is_none() {
            send_batch_size = None;
        }

        // dbg!(&send_batch_size);
        // dbg!(&send_batch_max_size);
        // dbg!(&timeout);

        // create single or multi shard batcher
        // let batcher = Shard {};
        // let batcher = Batcher::SingleShard(SingleShardBatcher { shard: Shard {} });
        // let batcher = if config.metadata_keys.len() == 0 {
        //
        // } else {
        //     let metadata_cardinality_limit = config.metadata_cardinality_limit.unwrap_or(1000);
        //     Batcher::MultiShard(MultiShardBatcher {
        //         shards: HashMap::new(),
        //         metadata_keys: config.metadata_keys,
        //         metadata_cardinality_limit,
        //     })
        // };
        // bpt, err := newBatchProcessorTelemetry(set, bp.batcher.currentMetadataCardinality)
        let queue_buffer_size = config.send_queue_buffer_size.unwrap_or(DEFAULT_BUFFER_SIZE);
        let (metrics_tx, _) = broadcast::channel(queue_buffer_size);

        // let timer = match timeout {
        //     Some(deadline) => Timer::finite(deadline),
        //     None => Timer::infinite(),
        // };
        Ok(Self {
            id,
            send_batch_size,
            send_batch_max_size,
            timeout,
            deadlines: tokio_util::time::DelayQueue::new(),
            // batcher,
            metrics_tx,
            current_batch: Vec::new(),
            // current_batch_size: 0,
        })
    }

    // async fn send_items(&mut self, trigger: Trigger) {
    async fn send_items(&mut self, items: Vec<MetricPayload>, trigger: Trigger) {
        // async fn send_items(&mut self, items: MetricPayload, trigger: Trigger) {
        // warn!(
        //     ?trigger,
        //     current_batch_size = self.current_batch_size(),
        //     "TODO: send batch"
        // );
        let combined_batch: Vec<_> = items.into_iter().flat_map(|metrics| metrics).collect();
        trace!(
            batch_size = combined_batch.len(),
            "{} sending batch",
            self.id
        );
        if let Err(err) = self.metrics_tx.send(combined_batch) {
            error!("failed to send metrics: {err}");
        }
        //     items
        //         .into_iter()
        //         .flat_map(|metrics| std::sync::Arc::unwrap_or_clone(metrics)),
        // );
    }

    fn current_batch_size(&self) -> usize {
        self.current_batch.len()
    }

    fn reset_deadline(&mut self) {
        if let Some(deadline) = self.timeout {
            self.deadlines.insert((), deadline);
        }
    }

    async fn process_item(
        &mut self,
        from: ServiceIdentifier,
        payload: Result<MetricPayload, BroadcastStreamRecvError>,
    ) {
        let resource_metrics = match payload {
            Ok(metrics) => metrics,
            Err(BroadcastStreamRecvError::Lagged(num_skipped)) => {
                warn!("skipped {num_skipped} messages");
                return;
            }
        };
        // self.current_batch_size += resource_metrics.len();
        self.current_batch.push(resource_metrics);

        // for b.batch.itemCount() > 0 && (!b.hasTimer() || b.batch.itemCount() >= b.processor.sendBatchSize) {
        // while self.current_batch_size > 0 && self.current_batch_size >= self.send_batch_size {
        let mut did_send = false;
        // while self.current_batch_size > 0
        while !self.current_batch.is_empty()
            && self
                .send_batch_size
                .is_some_and(|size| size >= self.current_batch_size())
        {
            did_send = true;
            // send a batch
            let idx = self
                .current_batch
                .len()
                .min(self.send_batch_max_size.unwrap_or(usize::MAX));
            let items = self.current_batch.drain(..idx).collect();
            self.send_items(items, Trigger::BatchSize);
        }
        if did_send {
            // reset deadline
            self.reset_deadline();
        }

        // match self.batcher {
        //     Batcher::SingleShard(ref mut batcher) => batcher.shard.process(metric),
        //     Batcher::MultiShard(_) => unimplemented!("multi shard batcher"),
        //     // Batcher::MultiShard(ref mut batcher) => batcher.process(metric),
        // };
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Trigger {
    Timeout,
    BatchSize,
}

#[async_trait::async_trait]
impl otel_collector_component::Processor for BatchProcessor {
    async fn start(
        mut self: Box<Self>,
        mut shutdown_rx: watch::Receiver<bool>,
        mut metrics: MetricsStream,
    ) -> eyre::Result<()> {
        self.reset_deadline();
        // if let Some(deadline) = self.timeout {
        //     self.deadlines.insert((), deadline);
        // }
        loop {
            tokio::select! {
                _ = self.deadlines.next() => {
                    let idx = self
                        .current_batch
                        .len()
                        .min(self.send_batch_max_size.unwrap_or(usize::MAX));
                    if idx > 0 {
                        let items: Vec<_> = self.current_batch.drain(..idx).collect();
                        trace!(batch_size = items.len(), "batch timer expired");
                        self.send_items(items, Trigger::Timeout).await;
                    }
                    self.reset_deadline();
                },
                Some((from, payload)) = metrics.next() => {
                    trace!("batch received payload");
                    self.process_item(from, payload).await;
                },
                _ = shutdown_rx.changed() => break,
            };

            // let resource_metrics = match resource_metrics {
            //     Ok(resource_metrics) => resource_metrics,
            //     Err(err) => {
            //         warn!(
            //             "{} received metric error {:?} from {:?}",
            //             self.id, err, from
            //         );
            //         continue;
            //     }
            // };
            // trace!(
            //     "{} received {} metrics from {:?} [queue size = {}]",
            //     self.id,
            //     resource_metrics.len(),
            //     from,
            //     self.metrics_tx.len(),
            //     // tokio::time::sleep(Duration::from_secs(30)).await;
            // );
            // if let Err(err) = self.metrics_tx.send(resource_metrics) {
            //     tracing::error!("{} failed to send: {err}", self.id);
            // }
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
