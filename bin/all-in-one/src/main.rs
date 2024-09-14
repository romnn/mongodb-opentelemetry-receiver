use clap::Parser;
use color_eyre::eyre;
// use mongodb_opentelemetry_receiver as collector;
use opentelemetry::trace::TracerProvider as TracerProviderTrait;
use opentelemetry_sdk::trace::TracerProvider;
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::layer::SubscriberExt;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Options {
    #[arg(long = "uri", help = "MongoDB connection URI")]
    pub connection_uri: String,
    #[arg(long = "interval", help = "Scrape interval")]
    pub interval_secs: Option<usize>,
    #[arg(short = 'c', long = "config", aliases = ["conf"],  help = "Path to YAML config file")]
    pub config_path: Option<PathBuf>,
}

pub const APPLICATION_NAME: &'static str = "mongodb-opentelemetry-receiver";

fn setup_telemetry() {
    // create a new OpenTelemetry trace pipeline that prints to stdout
    let provider = TracerProvider::builder()
        // .with_simple_exporter(stdout::SpanExporter::default())
        .build();
    let tracer = provider.tracer(APPLICATION_NAME);

    // create a tracing layer with the configured tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Use the tracing subscriber `Registry`, or any other subscriber
    // that impls `LookupSpan`
    // let registry = Registry::default().with(telemetry);
    // tracing_subscriber::fmt().json().init();
    let subscriber = tracing_subscriber::registry()
        .with(telemetry)
        // .with(
        // tracing_subscriber::filter::de()
        //     .add_directive(tracing::Level::DEBUG.into()),
        // )
        .with(
            tracing_subscriber::fmt::Layer::new()
                .pretty()
                .compact()
                // .with_level(tracing::Level::DEBUG)
                // .with_max_level(tracing_subscriber::Level::DEBUG)
                .with_writer(std::io::stdout),
        )
        .with(tracing_subscriber::filter::EnvFilter::from_default_env());
    // .init()
    // .unwrap();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    // .with(
    //     tracing_subscriber::fmt::Layer::new()
    //         .json()
    //         .with_writer(non_blocking),
    // );
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    setup_telemetry();

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        warn!("received ctr-c");
        info!("initiate graceful shutdown");
        shutdown_tx.send(true).unwrap();
    });

    // parse config
    let options = Options::parse();
    // dbg!(&options);

    let config = if let Some(path) = options.config_path {
        let config = otel_collector_component::config::Config::from_file(path)?;
        config
    } else {
        otel_collector_component::config::Config::default()
    };

    // dbg!(&config);
    // info!(?config);

    // use collector::pipeline::PipelineBuilder;
    use otel_collector_component::pipeline::{BuiltinServiceBuilder, Pipelines};
    let pipelines = Pipelines::from_config::<BuiltinServiceBuilder>(config)?;
    // tracing::debug!(?pipelines);
    // dbg!(&pipelines);
    // let pipeline_manager = collector::pipeline::PipelineManager::new(pipelines)?;
    let pipeline_executor = otel_collector_component::pipeline::PipelineExecutor { pipelines };
    pipeline_executor.start(shutdown_rx).await?;

    // let options = collector::mongodb::Options {
    //     connection_uri: options.connection_uri,
    // };
    //
    // // let scraper_future = MetricScraper::new(&options).await?;
    // let scraper_future = collector::mongodb::MetricScraper::new(&options);
    // let scraper = tokio::select! {
    //     scraper = scraper_future => scraper,
    //     _ = shutdown_rx.changed() => return Ok(()),
    // };
    // // _ = tokio::signal::ctrl_c() => return Ok(()),
    // let scraper = scraper?;
    //
    // let interval = std::time::Duration::from_secs(30);
    // let scheduler = collector::scrape::ScrapeScheduler::new(scraper, interval, shutdown_rx);
    // let scheduler = std::sync::Arc::new(scheduler);
    // // let scheduler_clone = scheduler.clone();
    //
    // info!(?interval, "starting scraper");
    // scheduler.start().await;
    Ok(())
}
