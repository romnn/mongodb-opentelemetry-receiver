#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    // sample_time: Instant,
    // bson::DateTime
    // #[serde(rename="Flattened")]
    // flattened: HashMap<String, bson::Document>,
    host: String,
    version: String,
    process: String,
    pid: u64,
    asserts: Asserts,
    batched_deleted: Option<BatchedDeletes>,
    bucket_catalog: Option<BucketCatalog>,
    catalog_stats: Option<CatalogStats>,
    connections: Option<Connections>,
    #[serde(rename = "defaultRWConcern")]
    default_rw_concern: Option<DefaultRWConcern>,
    // Uptime             int64                  `bson:"uptime"`
    // UptimeMillis       int64                  `bson:"uptimeMillis"`
    // UptimeEstimate     int64                  `bson:"uptimeEstimate"`
    // LocalTime          time.Time              `bson:"localTime"`
    // BackgroundFlushing *FlushStats            `bson:"backgroundFlushing"`
    // ExtraInfo          *ExtraInfo             `bson:"extra_info"`
    // Connections        *ConnectionStats       `bson:"connections"`
    // Dur                *DurStats              `bson:"dur"`
    // GlobalLock         *GlobalLockStats       `bson:"globalLock"`
    // Locks              map[string]LockStats   `bson:"locks,omitempty"`
    // Network            *NetworkStats          `bson:"network"`
    // Opcounters         *OpcountStats          `bson:"opcounters"`
    // OpcountersRepl     *OpcountStats          `bson:"opcountersRepl"`
    // RecordStats        *DBRecordStats         `bson:"recordStats"`
    // Mem                *MemStats              `bson:"mem"`
    // Repl               *ReplStatus            `bson:"repl"`
    // ShardCursorType    map[string]interface{} `bson:"shardCursorType"`
    // StorageEngine      *StorageEngine         `bson:"storageEngine"`
    // WiredTiger         *WiredTiger            `bson:"wiredTiger"`
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asserts {
    pub regular: i64,
    pub warning: i64,
    pub msg: i64,
    pub user: i64,
    pub rollovers: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchedDeletes {
    pub batches: i64,
    pub docs: i64,
    pub staged_size_bytes: i64,
    pub time_in_batch_millis: i64,
    pub refetches_due_to_yield: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketCatalog {
    pub num_buckets: i64,
    pub num_open_buckets: i64,
    pub num_idle_buckets: i64,
    pub memory_usage: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStats {
    pub collections: i64,
    pub capped: i64,
    pub views: i64,
    pub timeseries: i64,
    pub internalCollections: i64,
    pub internalViews: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connections {
    pub current: Option<i64>,
    pub available: Option<i64>,
    pub total_created: Option<i64>,
    pub rejected: Option<i64>,
    pub active: Option<i64>,
    pub threaded: Option<i64>,
    pub exhaust_is_master: Option<i64>,
    pub exhaustHello: Option<i64>,
    pub awaiting_topology_changes: Option<i64>,
    pub load_balanced: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultRWConcern {
    pub default_read_concern: Option<DefaultReadConcern>,
    pub default_write_concern: Option<DefaultWriteConcern>,
    pub default_write_oncern_source: Option<String>,
    pub default_read_concern_source: Option<String>,
    pub update_op_time: Option<bson::Timestamp>,
    pub update_wall_clock_time: Option<bson::DateTime>,
    pub local_update_wall_clock_time: Option<bson::DateTime>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultReadConcern {
    pub level: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultWriteConcern {
    pub w: Option<String>,
    pub wtimeout: Option<i64>,
    pub j: Option<bool>,
}

// #[derive(Debug, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ElectionMetrics {
//    step_up_cmd : {
//       called : Long("<num>"),
//       successful : Long("<num>")
//    },
//    priority_takeover : {
//       called : Long("<num>"),
//       successful : Long("<num>")
//    },
//    catch_up_takeover : {
//       called : Long("<num>"),
//       successful : Long("<num>")
//    },
//    election_timeout : {
//       called : Long("<num>"),
//       successful : Long("<num>")
//    },
//    freeze_timeout : {
//       called : Long("<num>"),
//       successful : Long("<num>")
//    },
//    num_step_downs_caused_by_higher_term : Long("<num>"),
//    num_catch_ups : Long("<num>"),
//    num_catch_ups_succeeded : Long("<num>"),
//    num_catch_ups_already_caught_up : Long("<num>"),
//    num_catch_ups_skipped : Long("<num>"),
//    num_catch_ups_timed_out : Long("<num>"),
//    num_catch_ups_failed_with_error : Long("<num>"),
//    num_catch_ups_failed_with_new_term : Long("<num>"),
//    num_catch_ups_failed_with_repl_set_abort_primary_catch_up_cmd : Long("<num>"),
//    average_catch_up_ops : Option<f64>,
// }
