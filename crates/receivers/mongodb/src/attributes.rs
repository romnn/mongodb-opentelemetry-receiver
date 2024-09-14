#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr)]
pub enum ConnectionType {
    #[strum(serialize = "active")]
    Active,
    #[strum(serialize = "available")]
    Available,
    #[strum(serialize = "current")]
    Current,
}

impl ConnectionType {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr)]
pub enum LockMode {
    #[strum(serialize = "shared")]
    Shared,
    #[strum(serialize = "exclusive")]
    Exclusive,
    #[strum(serialize = "intent_shared")]
    IntentShared,
    #[strum(serialize = "intent_exclusive")]
    IntentExclusive,
}

impl LockMode {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr)]
pub enum LockType {
    #[strum(serialize = "parallel_batch_write_mode")]
    ParallelBatchWriteMode,
    #[strum(serialize = "replication_state_transition")]
    ReplicationStateTransition,
    #[strum(serialize = "global")]
    Global,
    #[strum(serialize = "database")]
    Database,
    #[strum(serialize = "collection")]
    Collection,
    #[strum(serialize = "mutex")]
    Mutex,
    #[strum(serialize = "metadata")]
    Metadata,
    #[strum(serialize = "oplog")]
    Oplog,
}

impl LockType {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr)]
pub enum MemoryType {
    #[strum(serialize = "resident")]
    Resident,
    #[strum(serialize = "virtual")]
    Virtual,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr)]
pub enum Operation {
    #[strum(serialize = "insert")]
    Insert,
    #[strum(serialize = "query")]
    Query,
    #[strum(serialize = "update")]
    Update,
    #[strum(serialize = "delete")]
    Delete,
    #[strum(serialize = "getmore")]
    Getmore,
    #[strum(serialize = "command")]
    Command,
}

impl Operation {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr)]
pub enum OperationLatency {
    #[strum(serialize = "read")]
    Read,
    #[strum(serialize = "write")]
    Write,
    #[strum(serialize = "command")]
    Command,
}

impl OperationLatency {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr)]
pub enum CacheStatus {
    #[strum(serialize = "hit")]
    Hit,
    #[strum(serialize = "miss")]
    Miss,
}

impl CacheStatus {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}
