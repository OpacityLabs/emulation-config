//! Default headers and header order.
//!
//! On the wire both are byte arrays, as the fork writes them:
//! - headers are `[name, value]` pairs, each a list of bytes;
//! - each header-order entry is a list of bytes whose first byte is a tag
//!   (`0` = name with its original casing, `1` = standard lowercase name).

use bytes::Bytes;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use wreq::header::{HeaderMap, HeaderName, HeaderValue, IntoOrigHeaderName, OrigHeaderMap};

const CASED_TAG: u8 = 0;
const STANDARD_TAG: u8 = 1;

/// Serde helpers for a [`HeaderMap`] stored as `[name bytes, value bytes]` pairs.
pub(crate) mod header_pairs {
    use super::{
        Deserialize, Deserializer, HeaderMap, HeaderName, HeaderValue, Serialize, Serializer,
    };
    use serde::de::Error as _;

    // `Vec<u8>` (not `&[u8]`) keeps each name and value a list of numbers in every serde backend,
    // exactly like the fork. `&[u8]` would become a byte string in some backends.
    pub(crate) fn serialize<S: Serializer>(
        headers: &HeaderMap,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let pairs: Vec<Vec<Vec<u8>>> = headers
            .iter()
            .map(|(name, value)| vec![name.as_str().as_bytes().to_vec(), value.as_bytes().to_vec()])
            .collect();
        pairs.serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<HeaderMap, D::Error> {
        let pairs = Vec::<Vec<Vec<u8>>>::deserialize(deserializer)?;
        let mut headers = HeaderMap::new();
        for pair in pairs {
            // Like the fork, anything after the name and value is ignored.
            let [name, value, ..] = pair.as_slice() else {
                return Err(D::Error::custom("invalid header"));
            };
            let name = HeaderName::from_bytes(name).map_err(D::Error::custom)?;
            let value = HeaderValue::from_bytes(value).map_err(D::Error::custom)?;
            headers
                .try_append(name, value)
                .map_err(|_| D::Error::custom("too many headers"))?;
        }
        Ok(headers)
    }
}

/// One entry of the header order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrigHeaderName {
    /// Sent with exactly this casing, e.g. `User-Agent`.
    Cased(Bytes),
    /// Sent as the standard lowercase name.
    Standard(HeaderName),
}

impl OrigHeaderName {
    /// The lowercase name this entry is grouped under, or `None` if it is not a valid header name.
    fn key(&self) -> Option<HeaderName> {
        match self {
            Self::Cased(bytes) => HeaderName::from_bytes(bytes).ok(),
            Self::Standard(name) => Some(name.clone()),
        }
    }

    fn encode(&self) -> Vec<u8> {
        let (tag, name) = match self {
            Self::Cased(bytes) => (CASED_TAG, bytes.as_ref()),
            Self::Standard(name) => (STANDARD_TAG, name.as_str().as_bytes()),
        };
        let mut encoded = Vec::with_capacity(name.len() + 1);
        encoded.push(tag);
        encoded.extend_from_slice(name);
        encoded
    }

    fn decode(encoded: &[u8]) -> Result<Self, &'static str> {
        match encoded.split_first() {
            Some((&CASED_TAG, name)) => Ok(Self::Cased(Bytes::copy_from_slice(name))),
            Some((&STANDARD_TAG, name)) => HeaderName::from_bytes(name)
                .map(Self::Standard)
                .map_err(|_| "invalid header"),
            _ => Err("invalid header"),
        }
    }

    fn to_wreq(&self) -> wreq::header::OrigHeaderName {
        match self {
            Self::Cased(bytes) => bytes.clone().into_orig_header_name(),
            Self::Standard(name) => name.clone().into_orig_header_name(),
        }
    }
}

impl From<&str> for OrigHeaderName {
    fn from(name: &str) -> Self {
        Self::Cased(Bytes::copy_from_slice(name.as_bytes()))
    }
}

impl From<String> for OrigHeaderName {
    fn from(name: String) -> Self {
        Self::Cased(Bytes::from(name))
    }
}

impl From<Bytes> for OrigHeaderName {
    fn from(name: Bytes) -> Self {
        Self::Cased(name)
    }
}

impl From<HeaderName> for OrigHeaderName {
    fn from(name: HeaderName) -> Self {
        Self::Standard(name)
    }
}

/// The order headers are sent in, with their original casing.
///
/// This behaves like wreq's `OrigHeaderMap`: names are grouped by their lowercase form in the
/// order each group first appears, and names that are not valid header names are skipped.
#[derive(Debug, Clone, Default)]
pub struct OrigHeaders(HeaderMap<OrigHeaderName>);

impl OrigHeaders {
    /// Creates an empty header order.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a name to the order.
    ///
    /// Returns `false` and skips the name if it is not a valid header name, as wreq does.
    ///
    /// # Panics
    ///
    /// Panics if the order grows past the maximum size of an `http::HeaderMap`.
    pub fn insert(&mut self, name: impl Into<OrigHeaderName>) -> bool {
        self.try_insert(name.into())
            .expect("header order exceeds the maximum HeaderMap size")
    }

    fn try_insert(&mut self, name: OrigHeaderName) -> Result<bool, wreq::header::MaxSizeReached> {
        match name.key() {
            Some(key) => self.0.try_append(key, name).map(|_| true),
            None => Ok(false),
        }
    }

    /// Number of names in the order.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the order is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The names in the order they are sent.
    pub fn iter(&self) -> impl Iterator<Item = &OrigHeaderName> {
        self.0.iter().map(|(_, name)| name)
    }
}

// `HeaderMap`'s own equality ignores key order, which is the whole point of this type.
impl PartialEq for OrigHeaders {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Eq for OrigHeaders {}

impl<N: Into<OrigHeaderName>> FromIterator<N> for OrigHeaders {
    fn from_iter<I: IntoIterator<Item = N>>(names: I) -> Self {
        let mut order = Self::new();
        for name in names {
            order.insert(name);
        }
        order
    }
}

impl Serialize for OrigHeaders {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let encoded: Vec<Vec<u8>> = self.iter().map(OrigHeaderName::encode).collect();
        encoded.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OrigHeaders {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let entries = Vec::<Vec<u8>>::deserialize(deserializer)?;
        let mut order = Self::new();
        for entry in entries {
            let name = OrigHeaderName::decode(&entry).map_err(D::Error::custom)?;
            order
                .try_insert(name)
                .map_err(|_| D::Error::custom("too many headers"))?;
        }
        Ok(order)
    }
}

impl From<&OrigHeaders> for OrigHeaderMap {
    fn from(order: &OrigHeaders) -> Self {
        let mut map = Self::new();
        for name in order.iter() {
            map.insert(name.to_wreq());
        }
        map
    }
}
