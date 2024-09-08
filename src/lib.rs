#![allow(warnings)]

use std::time::Instant;

use color_eyre::eyre;
use mongodb::{bson, event::cmap::ConnectionCheckoutFailedReason, Client};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DEFAULT_PORT: u16 = 27017;

lazy_static::lazy_static! {
    static ref VERSION_REGEX: Regex = Regex::new(r"v?([0-9]+(\.[0-9]+)*?)(-([0-9]+[0-9A-Za-z\-~]*(\.[0-9A-Za-z\-~]+)*)|(-?([A-Za-z\-~]+[0-9A-Za-z\-~]*(\.[0-9A-Za-z\-~]+)*)))?(\+([0-9A-Za-z\-~]+(\.[0-9A-Za-z\-~]+)*))?").unwrap();
}

// regexp.MustCompile("^" + VersionRegexpRaw + "$")

#[derive(Debug, Deserialize)]
struct Version {
    pub version: String,
}

pub async fn get_version(client: &Client) -> eyre::Result<String> {
    let version = client
        .database("admin")
        .run_command(bson::doc! {"buildInfo": 1})
        .await?;
    let version: Version = bson::from_document(version)?;
    Ok(version.version)
}

pub fn get_server_address_and_port<'a>(
    // server_status: &'a bson::Document,
    server_status: &'a bson::Bson,
) -> eyre::Result<(&'a str, u16)> {
    let host = get!(server_status, "host")?.get_string()?;
    match &*host.split(":").collect::<Vec<_>>() {
        [host] => Ok((host, DEFAULT_PORT)),
        [host, port] => {
            let port = port.parse()?;
            Ok((host, port))
        }
        _ => Err(eyre::eyre!("unexpected host format: {:?}", host)),
    }
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metrics {
    database_count: usize,
}

enum BsonKey {
    Key(String),
    Index(usize),
}

impl From<usize> for BsonKey {
    fn from(value: usize) -> Self {
        BsonKey::Index(value)
    }
}

impl From<&str> for BsonKey {
    fn from(value: &str) -> Self {
        BsonKey::Key(value.to_string())
    }
}

fn get(value: &bson::Bson, key: BsonKey) -> Option<&bson::Bson> {
    match (value, key) {
        (bson::Bson::Array(arr), BsonKey::Index(idx)) => arr.get(idx),
        (bson::Bson::Document(doc), BsonKey::Key(key)) => doc.get(key),
        _ => None,
    }
}

#[macro_export]
macro_rules! get {
    ( $doc:expr, $( $x:expr ),* ) => {
        {
            // let mut val: Option<bson::Bson> = Some(bson::Bson::from($doc));
            // let mut val: bson::Bson = bson::Bson::from($doc);
            let mut path: Vec<String> = Vec::new();
            let mut val: &bson::Bson = $doc;
            // let mut val: bson::Bson = bson::Bson::from($doc);
            $(
                // val = get(val, $x)?;
                // val = get(&val, $x) else {
                val = get(val, BsonKey::from($x)).ok_or(color_eyre::eyre::eyre!("error"))?;
                // val = get(&val, BsonKey::from($x)).ok_or(color_eyre::eyre::eyre!("error")).cloned()?;
                    // return None;
                // };
            )*
            Ok::<&bson::Bson, eyre::Report>(val)
        }
    };
}

trait GetBsonValue {
    fn get_string(&self) -> eyre::Result<&str>;
    fn get_i64(&self) -> eyre::Result<i64>;
}

impl GetBsonValue for bson::Bson {
    fn get_string(&self) -> eyre::Result<&str> {
        // let options = bson::DeserializerOptions::builder()
        //     .(false)
        //     .human_readable(false)
        //     .build();
        // Ok(self.try_into().unwrap())
        // self.deser
        // bson::from_bson(self)
        // bson::from_bson_with_options(self)
        self.as_str()
            .ok_or_else(|| eyre::eyre!("expected str, got {self:?}"))
    }
    fn get_i64(&self) -> eyre::Result<i64> {
        match self {
            bson::Bson::Int64(v) => Ok(*v),
            bson::Bson::Int32(v) => Ok((*v).into()),
            other => Err(eyre::eyre!("expected integer, got {other:?}")),
        }
        // this allows parsing i32 too
        // bson::from_bson(self.clone()).map_err(Into::into)
        // self.as_i64().ok_or_else(|| eyre::eyre!("expected i64, got {self:?}"))
    }
}

pub fn collect_admin_metrics(
    server_status: &bson::Bson,
    // server_status: &bson::Document,
    errors: &mut [String],
) -> eyre::Result<()> {
    // let test = bson::Bson::from(server_status);
    // match test {
    // }
    let storageEngine = get!(server_status, "storageEngine", "name")?.get_string()?;
    if storageEngine != "wiredTiger" {
        return Ok(());
    }
    println!("storage engine: {storageEngine}");

    let cache_misses = get!(
        server_status,
        "wiredTiger",
        "cache",
        "pages read into cache"
    )?
    .get_i64()?;
    println!("cache misses: {cache_misses}");

    let cache_hits = get!(
        server_status,
        "wiredTiger",
        "cache",
        "pages requested from the cache"
    )?
    .get_i64()?;
    println!("cache hits: {cache_hits}");

    // "mongodb.cache.operations"

    let cache_hits = cache_hits - cache_misses;
    // s.mb.RecordMongodbCacheOperationsDataPoint(now, cacheHits, metadata.AttributeTypeHit)

    // s.recordCursorCount(now, document, errs)
    // metricName := "mongodb.cursor.count"
    let cursor_count = get!(server_status, "metrics", "cursor", "open", "total")?.get_i64()?;
    println!("cursor count: {cursor_count}");

    // s.recordCursorTimeoutCount(now, document, errs)
    // metricName := "mongodb.cursor.timeout.count"
    let cursor_timeouts = get!(server_status, "metrics", "cursor", "timedOut")?.get_i64()?;
    println!("cursor timeouts: {cursor_timeouts}");
    // s.mb.RecordMongodbCursorTimeoutCountDataPoint(now, val)
    // errs.AddPartial(1, fmt.Errorf(collectMetricError, metricName, err))

    // s.recordGlobalLockTime(now, document, errs)
    // metricName := "mongodb.global_lock.time"
    let global_lock_time = get!(server_status, "globalLock", "totalTime")?.get_i64()?;
    let global_lock_held_millis = global_lock_time / 1000;
    println!("cursor timeouts: {global_lock_time}");
    // s.mb.RecordMongodbGlobalLockTimeDataPoint(now, heldTimeMilliseconds)

    // s.recordNetworkCount(now, document, errs)
    let network_bytes_in = get!(server_status, "network", "bytesIn")?.get_i64()?;
    let network_bytes_out = get!(server_status, "network", "bytesOut")?.get_i64()?;
    let network_num_requests = get!(server_status, "network", "numRequests")?.get_i64()?;

    // s.recordOperations(now, document, errs)
    // s.recordOperationsRepl(now, document, errs)
    // s.recordSessionCount(now, document, errs)
    // s.recordLatencyTime(now, document, errs)
    // s.recordUptime(now, document, errs)
    // s.recordHealth(now, document, errs)
    Ok(())
}

pub async fn collect_metrics(client: &Client, errors: &mut [String]) -> eyre::Result<()> {
    let database_names = client.list_database_names().await?;
    let raw_server_status = client
        .database("admin")
        .run_command(bson::doc! {"serverStatus": 1})
        .await?;
    let raw_server_status = bson::Bson::from(raw_server_status);
    // println!("{:#?}", server_status);
    // let server_status: ServerStatus = bson::from_document(raw_server_status.clone())?;
    // println!("{:#?}", server_status);

    // let mut metrics = Metrics::default();
    let (serverAddress, serverPort) = get_server_address_and_port(&raw_server_status)?;

    let database_count = database_names.len();

    collect_admin_metrics(&raw_server_status, errors)?;
    Ok(())
}

pub async fn scrape_metrics(client: &Client) -> eyre::Result<()> {
    let version = get_version(client).await?;
    println!("version is {version}");

    let mut errors = Vec::new();
    collect_metrics(client, &mut errors).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {}
}
