use std::fmt::Pointer;

#[derive(Copy, Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum BsonKey<'a> {
    // KeyString(String),
    KeyStr(&'a str),
    Index(usize),
}

impl<'a> std::fmt::Display for BsonKey<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyStr(key) => std::fmt::Debug::fmt(key, f),
            Self::Index(idx) => std::fmt::Debug::fmt(idx, f),
        }
    }
}

// impl<'a> std::fmt::Debug for BsonKey<'a> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::KeyStr(key) => std::fmt::Debug::fmt(key, f),
//             Self::Index(idx) => std::fmt::Debug::fmt(idx, f),
//         }
//     }
// }

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum OwnedBsonKey {
    Key(String),
    Index(usize),
}

impl std::fmt::Display for OwnedBsonKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Key(key) => std::fmt::Debug::fmt(key, f),
            Self::Index(idx) => std::fmt::Debug::fmt(idx, f),
        }
    }
}

// impl std::fmt::Display for OwnedBsonKey {
// }

// impl<'a> std::borrow::Borrow<BsonKey<'a>> for OwnedBsonKey {
//     fn borrow(&self) -> &BsonKey<'a> {
//         match &self {
//             Self::Index(idx) => &BsonKey::Index(*idx),
//             Self::Key(key) => &BsonKey::KeyStr(key.as_ref()),
//         }
//     }
//
// }

// impl<'a> ToOwned for BsonKey<'a> {
//     type Owned = OwnedBsonKey;
impl<'a> BsonKey<'a> {
    fn to_owned(&self) -> OwnedBsonKey {
        match self {
            Self::Index(idx) => OwnedBsonKey::Index(*idx),
            // Self::KeyString(key) => OwnedBsonKey::Key(key.to_string()),
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

// impl<'a> From<String> for BsonKey<'a> {
//     fn from(value: String) -> Self {
//         BsonKey::KeyString(value)
//     }
// }

pub fn get<'a, 'b: 'a>(
    value: &'b mongodb::bson::Bson,
    key: BsonKey<'a>,
) -> Option<&'a mongodb::bson::Bson> {
    match (value, key) {
        (mongodb::bson::Bson::Array(arr), BsonKey::Index(idx)) => arr.get(idx),
        (mongodb::bson::Bson::Document(doc), BsonKey::KeyStr(key)) => doc.get(key),
        // (mongodb::bson::Bson::Document(doc), BsonKey::KeyString(key)) => doc.get(key),
        _ => None,
    }
}

pub trait BsonValue {
    fn get_str(&self) -> Result<&str, InvalidTypeError>;
    fn get_i64(&self) -> Result<i64, InvalidTypeError>;
}

impl BsonValue for mongodb::bson::Bson {
    fn get_str(&self) -> Result<&str, InvalidTypeError> {
        // let options = bson::DeserializerOptions::builder()
        //     .(false)
        //     .human_readable(false)
        //     .build();
        // Ok(self.try_into().unwrap())
        // self.deser
        // bson::from_bson(self)
        // bson::from_bson_with_options(self)
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
        // this allows parsing i32 too
        // bson::from_bson(self.clone()).map_err(Into::into)
        // self.as_i64().ok_or_else(|| eyre::eyre!("expected i64, got {self:?}"))
    }
}

#[derive(Debug)]
pub struct OwnedPath(Vec<OwnedBsonKey>);

impl<'a> FromIterator<&'a BsonKey<'a>> for OwnedPath {
    fn from_iter<T: IntoIterator<Item = &'a BsonKey<'a>>>(iter: T) -> Self {
        Self(iter.into_iter().map(|k| k.to_owned()).collect())
    }
}

impl OwnedPath {
    pub fn to_dotted_string(&self) -> String {
        self.0
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(".")
    }
}

// impl From<Vec<BsonKey<'a>>>

#[derive(Debug)]
pub struct Match {
    pub path: OwnedPath,
    pub value: mongodb::bson::Bson,
}

#[derive(thiserror::Error, Debug)]
#[error("invalid type: expected {expected_type}, found {value:?}")]
pub struct InvalidTypeError {
    // path: Path,
    pub expected_type: String,
    pub value: mongodb::bson::Bson,
}

// impl std::fmt::Display for InvalidTypeError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(
//             f,
//             "invalid type: expected {}, found {:?}",
//             self.expected_type,
//             self.value
//         )
//     }
// }

// pub fn dotted_path(path: &[OwnedBsonKey]) -> String {
//     path.iter()
//         .map(|p| p.to_string())
//         .collect::<Vec<_>>()
//         .join(".")
// }

#[derive(thiserror::Error, Debug)]
pub struct Error {
    pub path: OwnedPath,
    #[source]
    pub source: QueryError,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.to_dotted_string(), self.source)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("not found")]
    NotFound { partial_match: Option<Match> },
    #[error(transparent)]
    InvalidType(InvalidTypeError),
    // InvalidType(#[from] InvalidTypeError),
}

// impl std::fmt::Display for QueryError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::NotFound {
//                 path,
//                 partial_match,
//             } => {
//                 write!(f, "not found: {:?}", dotted_path(&*path))
//             }
//             Self::InvalidType(err) => std::fmt::Display::fmt(err, f),
//         }
//     }
// }

// #[inline]
// fn get_helper<'a>(
//     value: &mut Result<&'a mongodb::bson::Bson, Error>,
//     key: impl Into<BsonKey<'a>>,
//     path: &mut Vec<BsonKey<'a>>,
// ) {
//     let Ok(valid) = value else {
//         return;
//     };
//     // if let Ok(valid) = value {
//     // last_valid = valid;
//     dbg!(&path);
//     let key: crate::doc::BsonKey = key.into();
//     value = crate::doc::get(&*valid, key).ok_or_else(|| Error::NotFound {
//         path: path.into_iter().map(|p| p.to_owned()).collect(),
//         partial_match: Match {
//             path: path.into_iter().map(|p| p.to_owned()).collect()
//         },
//     });
//     path.push(key);
//     // path.push($x.to_string());
//     // }
// }

pub fn get_path<'p, 'k: 'b, 'b: 'p + 'k>(
    document: &'b mongodb::bson::Bson,
    path: impl AsRef<[BsonKey<'k>]>,
    // path: impl AsRef<&'p [BsonKey<'k>]>,
    // path: &'p [BsonKey<'k>],
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
        // let value = crate::get!($doc, $($x),*);
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
        // let value = crate::get!($doc, $($x),*);
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
