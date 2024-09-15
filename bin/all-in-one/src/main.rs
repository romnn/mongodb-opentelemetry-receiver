#![allow(warnings)]

use clap::Parser;
use color_eyre::eyre;
use opentelemetry::trace::TracerProvider as TracerProviderTrait;
use opentelemetry_sdk::trace::TracerProvider;
use otel_collector_component::{factory::ProcessorFactory, pipeline::PipelineBuilder};
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::layer::SubscriberExt;

pub const APPLICATION_NAME: &'static str = "mongodb-opentelemetry-receiver";

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogFormat {
    Json,
    Pretty,
}

impl std::str::FromStr for LogFormat {
    type Err = eyre::Report;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            s if s.eq_ignore_ascii_case("json") => Ok(LogFormat::Json),
            s if s.eq_ignore_ascii_case("pretty") => Ok(LogFormat::Pretty),
            other => Err(eyre::eyre!("unknown log format: {other:?}")),
        }
    }
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Options {
    #[arg(long = "log", env = "LOG_LEVEL", aliases = ["log-level"],  help = "Log level. When using a more sophisticated logging setup using RUST_LOG environment variable, this option is overwritten.")]
    pub log_level: Option<tracing::metadata::Level>,

    #[arg(
        long = "log-format",
        env = "LOG_FORMAT",
        help = "Log format (json or pretty)"
    )]
    pub log_format: Option<LogFormat>,

    #[arg(short = 'c', long = "config", env = "CONFIG", aliases = ["conf"],  help = "Path to YAML config file")]
    pub config_path: Option<PathBuf>,
}

struct TelemetryOptions {
    application_name: &'static str,
    log_level: Option<tracing::metadata::Level>,
    log_format: Option<LogFormat>,
}

fn setup_telemetry(options: &TelemetryOptions) -> eyre::Result<()> {
    let provider = TracerProvider::builder()
        // .with_simple_exporter(stdout::SpanExporter::default())
        .build();
    let tracer = provider.tracer(options.application_name);

    // create a tracing layer with the configured tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let default_log_level = options.log_level.unwrap_or(tracing::metadata::Level::INFO);
    let default_env_filter = tracing_subscriber::filter::EnvFilter::builder()
        .with_default_directive(default_log_level.into())
        .parse_lossy("");

    let env_filter_directive = std::env::var("RUST_LOG").ok();
    let env_filter = match env_filter_directive {
        Some(directive) => {
            match tracing_subscriber::filter::EnvFilter::builder()
                .with_env_var(directive)
                .try_from_env()
            {
                Ok(env_filter) => env_filter,
                Err(err) => {
                    eprintln!("invalid log filter: {err}");
                    eprintln!("falling back to default logging");
                    default_env_filter
                }
            }
        }
        None => default_env_filter,
    };

    // autodetect logging format
    let log_format = options.log_format.unwrap_or_else(|| {
        if atty::is(atty::Stream::Stdout) {
            // terminal
            LogFormat::Pretty
        } else {
            // not a terminal
            LogFormat::Json
        }
    });

    let fmt_layer_pretty = tracing_subscriber::fmt::Layer::new()
        .compact()
        .with_writer(std::io::stdout);
    let fmt_layer_json = tracing_subscriber::fmt::Layer::new()
        .json()
        .compact()
        .with_writer(std::io::stdout);

    type BoxedFmtLayer = Box<
        dyn tracing_subscriber::Layer<tracing_subscriber::registry::Registry>
            + Send
            + Sync
            + 'static,
    >;

    // impl<S> Layer<S> for Box<dyn Layer<S> + Send + Sync>
    // let fmt_layer: BoxedFmtLayer = match log_format {
    //     LogFormat::Json => Box::new(fmt_layer_json),
    //     LogFormat::Pretty => Box::new(fmt_layer_pretty),
    // };
    let subscriber = tracing_subscriber::registry()
        .with(telemetry)
        .with(if log_format == LogFormat::Json {
            Some(fmt_layer_json)
        } else {
            None
        })
        .with(if log_format == LogFormat::Pretty {
            Some(fmt_layer_pretty)
        } else {
            None
        })
        .with(env_filter);
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    // parse config
    let options = Options::parse();

    setup_telemetry(&TelemetryOptions {
        application_name: APPLICATION_NAME,
        log_level: options.log_level,
        log_format: options.log_format,
    })?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        warn!("received ctr-c");
        info!("initiate graceful shutdown");
        shutdown_tx.send(true).unwrap();
    });

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
    // use otel_collector_component::pipeline::{BuiltinServiceBuilder, Pipelines};
    let pipelines = PipelineBuilder::new()
        .with_receiver(otel_mongodb_receiver::Factory::default())
        .with_processors(vec![
            Box::new(otel_batch_processor::Factory::default()), // Box::new(otel_batch_processor::Factory::default()) as Box<dyn ProcessorFactory>
        ])
        .with_exporter(otel_otlp_exporter::Factory::default())
        .build(config)
        .await?;

    // let pipelines = Pipelines::from_config::<BuiltinServiceBuilder>(config)?;
    // tracing::debug!(?pipelines);
    tracing::debug!("done building pipelines");
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
