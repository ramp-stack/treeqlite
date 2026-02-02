use super::{PrimitiveType};
use serde::{Serialize, Deserialize};

use std::cmp::Ordering;
use std::hash::{DefaultHasher, Hasher, Hash};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone)]
pub enum Primitive {
    Bytes(Vec<u8>),
    String(String),
    Integer(i64),
    Float(f64),
}

impl Primitive {
    pub fn as_float(&self) -> Option<f64> {match self {
        Self::Integer(i) => Some(*i as f64),
        Self::Float(f) => Some(*f),
        _ => None,
    }}

    pub fn as_bytes(&self) -> Option<&[u8]> {match self {
        Self::Bytes(b) => Some(b),
        Self::String(s) => Some(s.as_bytes()),
        _ => None
    }}

    pub fn to_type(&self) -> PrimitiveType {match self {
        Self::Bytes(_) => PrimitiveType::Bytes,
        Self::String(_) => PrimitiveType::String,
        Self::Integer(_) => PrimitiveType::Integer,
        Self::Float(_) => PrimitiveType::Float,
    }}
}

impl Eq for Primitive {}
impl PartialEq for Primitive {
    fn eq(&self, other: &Self) -> bool {self.cmp(other) == Ordering::Equal}
}
impl PartialOrd for Primitive {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {Some(self.cmp(other))}
}

impl Ord for Primitive {
    fn cmp(&self, other: &Self) -> Ordering {
        match ((self.as_float(), self.as_bytes()), (other.as_float(), other.as_bytes())) {
            ((Some(a), _), (Some(b), _)) => a.total_cmp(&b),
            ((_, Some(a)), (_, Some(b))) => a.cmp(b),
            ((Some(_), _), (_, Some(_))) => Ordering::Less,
            ((_, Some(_)), (Some(_), _)) => Ordering::Greater,
            _ => unimplemented!()
        }
    }
}

impl Hash for Primitive {
    fn hash<H: Hasher>(&self, state: &mut H) {match self {
        Self::Bytes(b) => b.hash(state),
        Self::String(s) => s.hash(state),
        Self::Integer(i) => i.hash(state),
        Self::Float(f) => f.to_bits().hash(state),
    }}
}

impl std::fmt::Debug for Primitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {match self {
        Self::String(s) => write!(f, "{:?}", s),
        Self::Bytes(b) => write!(f, "{:?}..{}..{:?}", &b[..4], b.len()-8, &b[b.len()-4..]),
        s => write!(f, "{}", s),
    }}
}

impl std::fmt::Display for Primitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {match self {
        Self::String(s) => write!(f, "{}", s),
        Self::Bytes(b) => write!(f, "{}", std::str::from_utf8(b).map(|s| s.to_string()).unwrap_or_else(|_| format!("{:02x?}", b))),
        Self::Integer(i) => write!(f, "{}", i),
        Self::Float(r) => write!(f, "{}", r),
    }}
}
