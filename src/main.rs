use clap::Parser;
use color_eyre::eyre;
use mongodb::{bson, Client};

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::TracerProvider;
use tracing::{debug, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Options {
    #[arg(long = "uri", help = "MongoDB connection URI")]
    pub connection_uri: String,
    // #[arg(long = "database", aliases = ["db"], help = "MongoDB database name")]
    // pub database_name: Option<String>,
    // #[arg(long = "collection", help = "MongoDB collection names")]
    // pub collection_names: Vec<String>,
    // #[arg(long = "confirm", help = "Confirm changes interactively")]
    // pub confirm: Option<bool>,
    // #[arg(
    //     long = "dry-run",
    //     default_value = "false",
    //     help = "Run in dry run mode"
    // )]
    // pub dry_run: bool,
}

pub const APPLICATION_NAME: &'static str = "mongodb-opentelemetry-receiver";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    // Create a new OpenTelemetry trace pipeline that prints to stdout
    let provider = TracerProvider::builder()
        // .with_simple_exporter(stdout::SpanExporter::default())
        .build();
    let tracer = provider.tracer(APPLICATION_NAME);

    // Create a tracing layer with the configured tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Use the tracing subscriber `Registry`, or any other subscriber
    // that impls `LookupSpan`
    // let registry = Registry::default().with(telemetry);
    // tracing_subscriber::fmt().json().init();
    let subscriber = tracing_subscriber::registry()
        .with(telemetry)
        // .with(
        //     tracing_subscriber::filter::::from_default_env()
        //         .add_directive(tracing::Level::DEBUG.into()),
        // )
        .with(
            tracing_subscriber::fmt::Layer::new()
                .pretty()
                .compact()
                .with_writer(std::io::stdout),
        );
    // .init()
    // .unwrap();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    // .with(
    //     tracing_subscriber::fmt::Layer::new()
    //         .json()
    //         .with_writer(non_blocking),
    // );

    let start = std::time::Instant::now();
    let options = Options::parse();

    let client = Client::with_uri_str(&options.connection_uri).await?;

    // Send a ping to confirm a successful connection
    client
        .database("admin")
        .run_command(bson::doc! { "ping": 1 })
        .await?;
    info!(uri = options.connection_uri, "connected to database");

    mongodb_opentelemetry_receiver::record_metrics(client).await?;

    debug!("completed in {:?}", start.elapsed());

    Ok(())
}
