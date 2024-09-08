use clap::Parser;
use color_eyre::eyre;
use mongodb::{bson, Client};

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

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let start = std::time::Instant::now();
    let options = Options::parse();

    let client = Client::with_uri_str(&options.connection_uri).await?;

    // Send a ping to confirm a successful connection
    client
        .database("admin")
        .run_command(bson::doc! { "ping": 1 })
        .await?;
    println!("connected to {}", options.connection_uri);

    mongodb_opentelemetry_receiver::scrape_metrics(&client).await?;

    println!("completed in {:?}", start.elapsed());

    Ok(())
}
