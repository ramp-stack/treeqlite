use serde::ser::{SerializeSeq, SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, SerializeMap, SerializeStruct, SerializeStructVariant};
use serde::Serialize;

use std::collections::BTreeMap;

use super::{Primitive, Value, Type};

const US: &str = "␟";
const RS: &str = "␞";
const GS: &str = "␝";
const FS: &str = "␜";
const TEMP: &str = "␜";

#[derive(Debug)]
pub enum Error{
    Custom(String),
    Unnamed(String)
}
impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "{self:?}")}
}
impl serde::ser::Error for Error {
    fn custom<D: std::fmt::Display>(error: D) -> Error {
        Error::Custom(error.to_string())
    }
}

type Result<O> = std::result::Result<O, Error>;

pub struct Serializer;

impl serde::Serializer for Serializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = Sequence;
    type SerializeTuple = Tuple;
    type SerializeTupleStruct = TupleStruct;
    type SerializeTupleVariant = TupleVariant;
    type SerializeMap = Map;
    type SerializeStruct = Struct;
    type SerializeStructVariant = StructVariant;

    fn serialize_bool(self, v: bool) -> Result<Value> {Ok(Value::Primitive(Primitive::Integer(v as i64)))}
    fn serialize_i8(self, v: i8) -> Result<Value> {Ok(Value::Primitive(Primitive::Integer(v as i64)))}
    fn serialize_i16(self, v: i16) -> Result<Value> {Ok(Value::Primitive(Primitive::Integer(v as i64)))}
    fn serialize_i32(self, v: i32) -> Result<Value> {Ok(Value::Primitive(Primitive::Integer(v as i64)))}
    fn serialize_i64(self, v: i64) -> Result<Value> {Ok(Value::Primitive(Primitive::Integer(v)))}
    fn serialize_u8(self, v: u8) -> Result<Value> {Ok(Value::Primitive(Primitive::Integer(v as i64)))}
    fn serialize_u16(self, v: u16) -> Result<Value> {Ok(Value::Primitive(Primitive::Integer(v as i64)))}
    fn serialize_u32(self, v: u32) -> Result<Value> {Ok(Value::Primitive(Primitive::Integer(v as i64)))}
    fn serialize_u64(self, v: u64) -> Result<Value> {Ok(Value::Primitive(Primitive::Integer(v as i64)))}
    fn serialize_f32(self, v: f32) -> Result<Value> {Ok(Value::Primitive(Primitive::Float(v as f64)))}
    fn serialize_f64(self, v: f64) -> Result<Value> {Ok(Value::Primitive(Primitive::Float(v)))}
    fn serialize_char(self, v: char) -> Result<Value> {Ok(Value::Primitive(Primitive::String(v.to_string())))}
    fn serialize_str(self, v: &str) -> Result<Value> {Ok(Value::Primitive(Primitive::String(v.to_string())))}
    fn serialize_bytes(self, v: &[u8]) -> Result<Value> {Ok(Value::Primitive(Primitive::Bytes(v.to_vec())))}

    fn serialize_none(self) -> Result<Value> {
        Ok(Value::Null)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Value> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<Value> {Ok(Value::Null)}

    fn serialize_unit_struct(self, name: &'static str) -> Result<Value> {
        Ok(Value::named(name, Value::Null))
    }
    fn serialize_unit_variant(self, name: &'static str, _: u32, variant: &'static str) -> Result<Value> {
        Ok(Value::named(name, Value::named(variant, Value::Null)))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, name: &'static str, value: &T) -> Result<Value> {
        Ok(Value::named(name, value.serialize(Serializer)?.unwrap_named()))
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self, name: &'static str, _: u32, variant: &'static str, value: &T
    ) -> Result<Value> {
        Ok(Value::named(name, Value::named(variant, value.serialize(Serializer)?.unwrap_named())))
    }

    fn serialize_struct(self, name: &'static str, _: usize) -> Result<Struct> {
        Ok(Struct(name.to_string(), BTreeMap::new()))
    }
    fn serialize_struct_variant(self, name: &'static str, _: u32, variant: &'static str, _: usize) -> Result<StructVariant> {
        Ok(StructVariant(name.to_string(), variant.to_string(), BTreeMap::new()))
    }

    fn serialize_tuple_struct(self, name: &'static str, _: usize) -> Result<TupleStruct> {
        Ok(TupleStruct(name.to_string(), Vec::new()))
    }
    fn serialize_tuple_variant(self, name: &'static str, _: u32, variant: &'static str, _: usize) -> Result<TupleVariant> {
        Ok(TupleVariant(name.to_string(), variant.to_string(), Vec::new()))
    }

    fn serialize_tuple(self, len: usize) -> Result<Tuple> {Ok(Tuple(Vec::new()))}
    fn serialize_seq(self, _: Option<usize>) -> Result<Sequence> {Ok(Sequence(Vec::new()))}
    fn serialize_map(self, _: Option<usize>) -> Result<Map> {Ok(Map(None, BTreeMap::new()))}
}

pub struct Map(Option<Value>, BTreeMap<String, Value>);
impl SerializeMap for Map {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        self.0 = Some(key.serialize(Serializer)?);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.1.insert(self.0.take().unwrap().to_string(), value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {
        Ok(Value::Collection(self.1))
    }
}

pub struct Sequence(Vec<Value>);
impl SerializeSeq for Sequence {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.0.push(value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {
        Ok(Value::Collection(self.0.into_iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect()))
    }
}

pub struct Tuple(Vec<Value>);
impl SerializeTuple for Tuple {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.0.push(value.serialize(Serializer)?);
        Ok(())
    }
    fn end(mut self) -> Result<Value> {
        Ok(Value::Object(self.0.into_iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect()))
    }
}

pub struct TupleStruct(String, Vec<Value>);
impl SerializeTupleStruct for TupleStruct {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.1.push(value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {
        Ok(Value::named(&self.0, Value::Object(self.1.into_iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect())))
    }
}

pub struct TupleVariant(String, String, Vec<Value>);
impl SerializeTupleVariant for TupleVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.2.push(value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {
        Ok(Value::named(&self.0, Value::named(&self.1, Value::Object(
            self.2.into_iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect()
        ))))
    }
}

pub struct Struct(String, BTreeMap<String, Value>);
impl SerializeStruct for Struct {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()> {
        self.1.insert(key.to_string(), value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {Ok(Value::named(&self.0, Value::Object(self.1)))}
}

pub struct StructVariant(String, String, BTreeMap<String, Value>);
impl SerializeStructVariant for StructVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()> {
        self.2.insert(key.to_string(), value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {
        Ok(Value::named(&self.0, Value::named(&self.1, Value::Object(self.2))))
    }
}
