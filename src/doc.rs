use std::fmt::Pointer;

#[derive(Copy, Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum BsonKey<'a> {
    KeyStr(&'a str),
    Index(usize),
}

impl<'a> std::fmt::Display for BsonKey<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyStr(key) => std::fmt::Display::fmt(key, f),
            Self::Index(idx) => std::fmt::Display::fmt(idx, f),
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum OwnedBsonKey {
    Key(String),
    Index(usize),
}

impl std::fmt::Display for OwnedBsonKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Key(key) => std::fmt::Display::fmt(key, f),
            Self::Index(idx) => std::fmt::Display::fmt(idx, f),
        }
    }
}

impl<'a> BsonKey<'a> {
    fn to_owned(&self) -> OwnedBsonKey {
        match self {
            Self::Index(idx) => OwnedBsonKey::Index(*idx),
            Self::KeyStr(key) => OwnedBsonKey::Key(key.to_string()),
        }
    }
}

impl<'a> From<usize> for BsonKey<'a> {
    fn from(value: usize) -> Self {
        BsonKey::Index(value)
    }
}

impl<'a> From<&'a str> for BsonKey<'a> {
    fn from(value: &'a str) -> Self {
        BsonKey::KeyStr(value)
    }
}

pub fn get<'a, 'b: 'a>(
    value: &'b mongodb::bson::Bson,
    key: BsonKey<'a>,
) -> Option<&'a mongodb::bson::Bson> {
    match (value, key) {
        (mongodb::bson::Bson::Array(arr), BsonKey::Index(idx)) => arr.get(idx),
        (mongodb::bson::Bson::Document(doc), BsonKey::KeyStr(key)) => doc.get(key),
        _ => None,
    }
}

pub trait BsonValue {
    fn get_str(&self) -> Result<&str, InvalidTypeError>;
    fn get_i64(&self) -> Result<i64, InvalidTypeError>;
}

impl BsonValue for mongodb::bson::Bson {
    fn get_str(&self) -> Result<&str, InvalidTypeError> {
        self.as_str().ok_or_else(|| InvalidTypeError {
            expected_type: "string".to_string(),
            value: self.clone(),
        })
    }
    fn get_i64(&self) -> Result<i64, InvalidTypeError> {
        match self {
            mongodb::bson::Bson::Int64(v) => Ok(*v),
            mongodb::bson::Bson::Int32(v) => Ok((*v).into()),
            other => Err(InvalidTypeError {
                expected_type: "integer".to_string(),
                value: self.clone(),
            }),
        }
    }
}

#[derive(Debug)]
pub struct OwnedPath(Vec<OwnedBsonKey>);

impl std::fmt::Display for OwnedPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.0.len();
        for (idx, key) in self.0.iter().enumerate() {
            write!(f, "{}", key)?;
            if idx + 1 < len {
                write!(f, ".")?;
            }
        }
        Ok(())
    }
}

impl<'a> FromIterator<&'a BsonKey<'a>> for OwnedPath {
    fn from_iter<T: IntoIterator<Item = &'a BsonKey<'a>>>(iter: T) -> Self {
        Self(iter.into_iter().map(|k| k.to_owned()).collect())
    }
}

#[derive(Debug)]
pub struct Match {
    pub path: OwnedPath,
    pub value: mongodb::bson::Bson,
}

#[derive(thiserror::Error, Debug)]
#[error("invalid type: expected {expected_type}, found {value:?}")]
pub struct InvalidTypeError {
    pub expected_type: String,
    pub value: mongodb::bson::Bson,
}

#[derive(thiserror::Error, Debug)]
pub struct Error {
    pub path: OwnedPath,
    #[source]
    pub source: QueryError,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.path.to_string(), self.source)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("not found")]
    NotFound { partial_match: Option<Match> },
    #[error(transparent)]
    InvalidType(InvalidTypeError),
}

pub fn get_path<'p, 'k: 'b, 'b: 'p + 'k>(
    document: &'b mongodb::bson::Bson,
    path: impl AsRef<[BsonKey<'k>]>,
) -> Result<&'b mongodb::bson::Bson, Error> {
    let path = path.as_ref();
    let mut value: &mongodb::bson::Bson = document;
    for (idx, key) in path.iter().copied().enumerate() {
        value = get(value, key).ok_or_else(|| Error {
            path: OwnedPath::from_iter(path.iter()),
            source: QueryError::NotFound {
                partial_match: Some(Match {
                    path: OwnedPath::from_iter(path[..idx].iter()),
                    value: value.clone(),
                }),
            },
        })?;
    }
    Ok(value)
}

#[macro_export]
macro_rules! path {
    ( $( $x:expr ),* ) => {
        {
            let mut path: Vec<crate::doc::BsonKey> = Vec::new();
            $(
                let key = crate::doc::BsonKey::from($x);
                path.push(key);
            )*
            path
        }
    }
}

// #[macro_export]
// macro_rules! get {
//     ( $doc:expr, $( $x:expr ),* ) => {
//         {
//             let mut path: Vec<crate::doc::BsonKey> = Vec::new();
//             // let mut last_valid: &mongodb::bson::Bson = $doc;
//             // let mut val: eyre::Result<&mongodb::bson::Bson> = Ok($doc);
//             $(
//                 // if let Ok(valid) = val {
//                 //     last_valid = valid;
//                 //     dbg!(&path);
//                 let key = crate::doc::BsonKey::from($x);
//                 //     val = crate::doc::get(valid, key)
//                 //         .ok_or_else(|| {
//                 //             crate::doc::Error::NotFound{
//                 //                 path: path.into_iter().map(|p| p.to_owned())
//                 //             }
//                 //         });
//                 path.push(key);
//                 // }
//             )*
//         (path, crate::doc::get_path($doc, path.as_slice()))
//             // val
//             // Ok::<&bson::Bson, eyre::Report>(val)
//         }
//     };
// }

#[macro_export]
macro_rules! get_i64 {
    ( $doc:expr, $( $x:expr ),* ) => {{
        let path = crate::path!($($x),*);
        crate::doc::get_path($doc, path.as_slice()).and_then(|v|
            crate::doc::BsonValue::get_i64(v).map_err(|err| {
                crate::doc::Error {
                    path: crate::doc::OwnedPath::from_iter(path.iter()),
                    source: crate::doc::QueryError::InvalidType(err),
                }
        }))
    }};
}

#[macro_export]
macro_rules! get_str{
    ( $doc:expr, $( $x:expr ),* ) => {{
        let path = crate::path!($($x),*);
        crate::doc::get_path($doc, path.as_slice()).and_then(|v|
            crate::doc::BsonValue::get_str(v).map_err(|err| {
                crate::doc::Error {
                    path: crate::doc::OwnedPath::from_iter(path.iter()),
                    source: crate::doc::QueryError::InvalidType(err),
                }
        }))
    }};
}
