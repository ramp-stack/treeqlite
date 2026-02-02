use serde::{Serialize, Deserialize};

use std::cmp::Ordering;
use std::hash::{DefaultHasher, Hasher, Hash};
use std::collections::BTreeMap;

mod primitive;
pub use primitive::Primitive;

mod ser;
pub use ser::Serializer;
mod de;

//  #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
//  pub struct Object(pub String, pub Vec<String>, pub Data);

//  impl Object {
//      pub fn to_type(&self) -> Option<ObjectType> {
//          self.2.to_type().map(|d| ObjectType(self.0.clone(), BTreeMap::from([(self.1.clone(), d)]), false))
//      }
//  }

//  #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
//  pub enum Data {
//      Primitive(Primitive),//Point
//      Collection(BTreeMap<String, Primitive>),//1D///Optional Primitive to store maps of null values
//      Struct(BTreeMap<String, Value>),//2D
//  }

//  impl Data {
//      pub fn to_type(&self) -> Option<DataType> {match self {
//          Self::Primitive(p) => Some(DataType::Primitive(p.to_type())),
//          Self::Collection(c) => c.values().next().map(|n| DataType::Collection(n.to_type())),
//          Self::Struct(s) => Some(DataType::Struct(s.into_iter().map(|(k, v)| (k.to_string(), v.to_type())).collect())),
//      }}
//  }

//  #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
//  pub enum Value {
//      Primitive(Primitive),//Point
//      Map(BTreeMap<String, Object>),//1D
//      Object(Object),//Named
//      Null
//  }

//  impl Value {
//      ///Empty maps and are treated as Null
//      pub fn to_type(&self) -> Type {match self {
//          Value::Primitive(p) => Type::Primitive(p.to_type()),
//          Value::Map(m) => m.values().filter_map(|v| v.to_type()).next().map(|o| Type::Map(o)).unwrap_or(Type::Null),
//          Value::Object(o) => o.to_type().map(|o| Type::Object(o)).unwrap_or(Type::Null),
//          //Value::Object(o) => Type::Object(o.to_type()),
//          Value::Null => Type::Null,
//      }}
//  }

//  #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
//  pub enum DataType {
//      Primitive(PrimitiveType),//Point
//      Collection(PrimitiveType),//1D
//      Struct(BTreeMap<String, Type>),//2D
//  }

//  #[derive(Serialize, Deserialize, Debug, Clone, PartialOrd, Eq, Ord, Hash)]
//  pub struct ObjectType(String, BTreeMap<Vec<String>, DataType>, bool);
//  impl PartialEq for ObjectType {
//      fn eq(&self, other: &Self) -> bool {
//          self.0 == other.0 &&
//          (self.2 || other.2 && self.1 == other.1) ||
//          (
//              !self.2 && !other.2 &&
//              self.1.iter().all(|(k, d)| other.1.get(k).map(|od| od == d).unwrap_or(true)) &&
//              other.1.iter().all(|(k, d)| self.1.get(k).map(|od| od == d).unwrap_or(true))
//          )
//      }
//  }

//  #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
//  pub enum Type {
//      Primitive(PrimitiveType),//Point
//      Map(ObjectType),//1D
//      Object(ObjectType),//Named
//      Null
//  }

//  #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Copy)]
//  pub enum PrimitiveType {String, Bytes, Integer, Float}

//  impl std::fmt::Debug for Object {
//      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//          let name = if self.1.is_empty() {&self.0} else {&format!("{}::{}", self.0, self.1.join("::"))};
//          match &self.2 {
//              Data::Collection(c) => {
//                  let mut s = f.debug_struct(&name);
//                  c.iter().for_each(|(n, v)| {s.field(n, v);});
//                  s.finish()
//              },
//              Data::Primitive(p) => {
//                  write!(f, "{name}({p:?})")
//              },
//              Data::Struct(v) => {
//                  let mut s = f.debug_struct(&name);
//                  v.iter().for_each(|(n, v)| {s.field(n, v);});
//                  s.finish()
//              }
//          }
//      }
//  }

//  impl std::fmt::Display for Object {
//      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//          let mut hasher = DefaultHasher::new();
//          self.hash(&mut hasher);
//          write!(f, "{}", hasher.finish())
//      }
//  }

//  impl std::fmt::Debug for Value {
//      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//          match self {
//              Self::Object(object) => object.fmt(f),
//              Self::Map(c) => {
//                  let mut s = f.debug_struct("Map");
//                  c.iter().for_each(|(n, v)| {s.field(n, v);});
//                  s.finish()
//              },
//              Self::Primitive(p) => p.fmt(f),
//              Self::Null => write!(f, "Null")
//          }
//      }
//  }

//  impl std::fmt::Display for Value {
//      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//          match self {
//              Self::Object(o) => write!(f, "{o}"),
//              Self::Map(c) => {
//                  let mut hasher = DefaultHasher::new();
//                  c.hash(&mut hasher);
//                  write!(f, "{}", hasher.finish())
//              },
//              Self::Primitive(p) => write!(f, "{p}"),
//              Self::Null => write!(f, "Null")
//          }
//      }
//  }















#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    Null,//Point
    Primitive(Primitive),//Point
    Collection(BTreeMap<String, Value>),//1D single typed non exaustive
    Object(BTreeMap<String, Value>),//2D multi typed exaustive
    Named(String, Box<Value>),//Named
}

impl Value {
    pub fn named(name: &str, value: Value) -> Self {
        Value::Named(name.to_string(), Box::new(value))
    }

    pub fn get_name(self) -> (Option<String>, Value) {
        match self {
            Self::Named(name, i) => {
                let (iname, v) = i.get_name();
                (Some(iname.map(|i| format!("{name}::{i}")).unwrap_or(name)), v)
            },
            o => (None, o)
        }
    }

    pub fn unwrap_named(self) -> Self {match self {
        Self::Named(_, v) => *v,
        unnamed => unnamed
    }}
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => write!(f, "Null"),
            Self::Primitive(p) => write!(f, "{p}"),
            other => {
                let mut hasher = DefaultHasher::new();
                other.hash(&mut hasher);
                write!(f, "{}", hasher.finish())
            },
        }
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => write!(f, "Null"),
            Self::Primitive(p) => p.fmt(f),
            Self::Object(c) | Self::Collection(c) => {
                let mut s = f.debug_struct("");
                c.iter().for_each(|(n, v)| {s.field(n, v);});
                s.finish()
            },
            Self::Named(name, i) => match &**i {
                Self::Collection(c) | Self::Object(c) => {
                    let mut s = f.debug_struct(name);
                    c.iter().for_each(|(n, v)| {s.field(n, v);});
                    s.finish()
                },
                o if matches!(o, Self::Named(_, _)) => {write!(f, "{name}::")?; o.fmt(f)},
                p => write!(f, "{name}({p:?})") 
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Copy)]
pub enum PrimitiveType {String, Bytes, Integer, Float}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    Primitive(PrimitiveType),
    Collection(Box<Type>),
    Object(BTreeMap<String, Type>),
    Named(String, Vec<Type>),
}

pub enum Schema {
    Valid(bool),
    Primitive(PrimitiveSchema),
    Collection(Box<Schema>),
    Object(BTreeMap<String, Schema>),
    Named(String, Vec<Schema>),
}

pub type Validate<T> = Box<dyn Fn(&T) -> bool>;

pub enum PrimitiveSchema {
    Bytes(Validate<Vec<u8>>),
    String(Validate<String>),
    Integer(Validate<i64>),
    Float(Validate<f64>),
}


//air_author, air_published, air_revision fields are all enforced by the air servers storage location and set automatically when published

//  pub trait Contract {
//      fn name() -> String;
//      fn sub_contracts() -> BTreeMap<PathBuf, String>;
//      fn required() -> BTreeSet<PathBuf> where Self: Sized;//These queries point to required views of self and sub contracts
//      fn validate(views: BTreeMap<PathBuf, Value>) -> bool;
//  }
