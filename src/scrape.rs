use color_eyre::eyre;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use std::sync::Arc;
use tokio::sync::{watch::Receiver, Mutex};

use crate::consumer::MetricsConsumer;

pub trait Scrape {
    async fn scrape(&mut self) -> eyre::Result<Vec<ResourceMetrics>>;
    // async fn scrape(&mut self) -> ();
}

#[derive(Debug)]
pub struct ScrapeScheduler<S> {
    pub scraper: Arc<Mutex<S>>,
    pub interval: std::time::Duration,
    pub consumers: Vec<Box<dyn MetricsConsumer>>,
    pub shutdown_rx: Receiver<bool>,
}

impl<S> ScrapeScheduler<S> {
    pub fn new(scraper: S, interval: std::time::Duration, shutdown_rx: Receiver<bool>) -> Self {
        Self {
            scraper: Arc::new(Mutex::new(scraper)),
            interval,
            consumers: Vec::new(),
            shutdown_rx,
        }
    }
}

impl<S> ScrapeScheduler<S>
where
    S: Scrape,
{
    pub async fn start(&self) {
        let mut shutdown_rx_clone = self.shutdown_rx.clone();
        let mut interval = tokio::time::interval(self.interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let scrape = || async {
            // telemetry like in (sc *controller) scrapeMetricsAndReport() would be cool
            match self.scraper.lock().await.scrape().await {
                Ok(metrics) => {
                    // consume
                    for consumer in self.consumers.iter() {
                        // todo
                    }
                }
                Err(err) => {
                    tracing::warn!("error scraping metrics: {}", err);
                }
            };
        };

        loop {
            tokio::select! {
                _ = interval.tick() => scrape().await,
                _ = shutdown_rx_clone.changed() => return,
            }
        }
    }

    // pub async fn stop(&self) {
    //     // todo: send to channel or use a mutex condition variable?
    // }
}
