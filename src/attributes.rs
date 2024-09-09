#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
pub enum ConnectionType {
    #[strum(serialize = "active")]
    Active,
    #[strum(serialize = "available")]
    Available,
    #[strum(serialize = "current")]
    Current,
}

impl ConnectionType {
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
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
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn to_mongodb_key(self) -> &'static str {
        match self {
            LockMode::Shared => "R",
            LockMode::Exclusive => "W",
            LockMode::IntentShared => "r",
            LockMode::IntentExclusive => "w",
        }
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
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
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn supported(&self, version: &semver::Version) -> bool {
        match self {
            LockType::ParallelBatchWriteMode | LockType::ReplicationStateTransition
                if version < &semver::Version::new(4, 2, 0) =>
            {
                false
            }
            _ => true,
        }
    }

    pub fn to_mongodb_key(self) -> &'static str {
        match self {
            LockType::ParallelBatchWriteMode => "ParallelBatchWriterMode",
            LockType::ReplicationStateTransition => "ReplicationStateTransition",
            LockType::Global => "Global",
            LockType::Database => "Database",
            LockType::Collection => "Collection",
            LockType::Mutex => "Mutex",
            LockType::Metadata => "Metadata",
            LockType::Oplog => "oplog",
        }
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
pub enum MemoryType {
    #[strum(serialize = "resident")]
    Resident,
    #[strum(serialize = "virtual")]
    Virtual,
}

impl MemoryType {
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn to_mongodb_key(self) -> &'static str {
        match self {
            MemoryType::Resident => "resident",
            MemoryType::Virtual => "virtual",
        }
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
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
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn to_mongodb_key(self) -> &'static str {
        match self {
            Operation::Insert => "insert",
            Operation::Query => "queries",
            Operation::Update => "update",
            Operation::Delete => "remove",
            Operation::Getmore => "getmore",
            Operation::Command => "commands",
        }
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
pub enum OperationLatency {
    #[strum(serialize = "read")]
    Read,
    #[strum(serialize = "write")]
    Write,
    #[strum(serialize = "command")]
    Command,
}

impl OperationLatency {
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn to_mongodb_key(self) -> &'static str {
        match self {
            OperationLatency::Read => "reads",
            OperationLatency::Write => "writes",
            OperationLatency::Command => "commands",
        }
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter, strum::IntoStaticStr,
)]
pub enum CacheStatus {
    #[strum(serialize = "hit")]
    Hit,
    #[strum(serialize = "miss")]
    Miss,
}

impl CacheStatus {
    pub fn iter() -> <Self as strum::IntoEnumIterator>::Iterator {
        <Self as strum::IntoEnumIterator>::iter()
    }

    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}
