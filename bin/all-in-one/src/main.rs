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
    let default_log_directive = format!(
        "none,otel_={}",
        default_log_level.to_string().to_ascii_lowercase()
    );
    let default_env_filter = tracing_subscriber::filter::EnvFilter::builder()
        .with_regex(true)
        .with_default_directive(default_log_level.into())
        .parse(default_log_directive)?;

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

    let config = if let Some(path) = options.config_path {
        let config = otel_collector_component::config::Config::from_file(path)?;
        config
    } else {
        otel_collector_component::config::Config::default()
    };

    let pipelines = PipelineBuilder::new()
        .with_receiver(otel_mongodb_receiver::Factory::default())
        .with_processors([Box::new(otel_batch_processor::Factory::default()) as _])
        .with_exporter(otel_otlp_exporter::Factory::default())
        .build(config)
        .await?;

    let pipeline_executor = otel_collector_component::pipeline::PipelineExecutor { pipelines };
    pipeline_executor.start(shutdown_rx).await?;
    Ok(())
}
