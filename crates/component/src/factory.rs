use color_eyre::eyre;
use regex::Regex;

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentName(String);

lazy_static::lazy_static! {
    static ref COMPONENT_NAME_REGEX: Regex = Regex::new(r"^[a-zA-Z][0-9a-zA-Z_]{0,62}$").unwrap();
}

impl ComponentName {
    pub fn new(name: impl Into<String>) -> eyre::Result<Self> {
        let name: String = name.into();
        if !COMPONENT_NAME_REGEX.is_match(&name) {
            eyre::bail!("invalid name: {name:?}");
        }
        Ok(Self(name))
    }
}
