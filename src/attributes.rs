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
