use rusqlite::{Connection, Transaction, TransactionBehavior, types};
use types::{FromSql, FromSqlResult, FromSqlError};

use serde_ron::value::{Value, Object, Data, Primitive, Number, PrimitiveType, NumberType};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{PathBuf, Path};
use std::str::FromStr;

use crate::Error;

///Sqlite TODO:
///1. Enable child maps Room/messages etc Allow nested Maps in type Room | messages: Map<Message>
///2. Allow nested types similar to Variants Map<Map<Message>>
///3. Allow tuples and require children to be flattened into a single row meta: (u8, u8) -> Room | meta/0 | meta/1
///
///Contract Interface:
///1. updated_at fields indicate an object that can be updated independently of its parent
///2. Every object that can be created independently of its parent needs a signature field
///
///Contract:
///1. I want multipart contracts to allow me to selectivly read or create parts of it at time
///1-1. This requires a Path System or NameSpace system to create relationships and location
///1-1-1. The path starts with the name of the Contract to allow inter contract queries
///1-1-2. The contracts are named allowing for replacement rules on existing contract data
///
///2. I want an arbitrary validation function:
///2-1. I want to query for data in other parts of contracts, usually a subset of the entire contract
///2-1-1. I need a schema which is respected by the pathing system to enable sub contract queries
///2-1-1-1. This schema does not need to follow rust types, I could have a Number type instead of u8/16
///2-2. I also want to query for data by revision index or timestamp.
///
///3. I want to enforce an ordered series of events resulting in a timestamp contract system
///
///4. I want names to talk with identities or people
///4-1. I need signatures to verify who or what I am talking to
///
///5. I want privacy enforced by encryption which requires discovery keys
///
///Contract Requiremets:
///1. Nothing can ever be deleted except when switching enum variants which is really an update
///2. My signature scheme uses one depth per contract resulting in pathed chunks

///Summery:
///
///1. Universal contract has no communication or enforcement of timestamps or privacy conciderations
///Discovery / Timestamp / Price
///
///2. Inter contract communication requires a subset of all possible contracts
///
///3. Pathing / Schema / Query chosen for V1 of Air Contracts
///
///4. Names / Signatures are used to identify parties or actors

///Column Name used by Collections and NewType Enum/Structures
pub const VALUE: &str = "VALUE";

const ID: &str = "ID";
const FS: &str = ": ";
const VS: &str = "::";


fn missing(n: &str) -> Error {Error::MissingColumn(n.to_string())}
fn invalid_table(n: &str) -> Error {Error::InvalidTableName(n.to_string())}

trait OptionalExtension<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExtension<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Err(rusqlite::Error::SqliteFailure(_, Some(msg))) if msg.contains("no such table") => Ok(None),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Ok(t) => Ok(Some(t)),
            Err(e) => Err(e)
        }
    }
}

#[derive(Clone, Debug)]
pub struct Table {
    name: String,
    schema: Schema,
}

impl Table {
    pub fn schema(&self) -> &Schema {&self.schema}
    pub fn name(&self) -> String {self.schema.name(&self.name)}

    pub fn from_name(connection: &Connection, name: &str) -> Result<Option<Self>, Error> {
        let cmd = format!("SELECT name, type, \"notnull\" FROM pragma_table_info('{name}')");
        let columns = connection.prepare(&cmd)?.query_map([], |row|
            Ok((row.get(0)?, row.get(1)?, !row.get::<usize, bool>(2)?))
        )?.collect::<Result<Vec<(String, String, bool)>, rusqlite::Error>>().optional()?;
        let columns = columns.map(|c| c.into_iter().map(|(field, sty, nullable)| {
            let sty = match sty.to_uppercase().as_str() {
                "INTEGER" => types::Type::Integer,
                "REAL" => types::Type::Real,
                "TEXT" => types::Type::Text,
                "BLOB" => types::Type::Blob,
                "" => types::Type::Null,
                unknown => {return Err(Error::UnknownColumnAffinity(unknown.to_string()));}
            };
            let (name, ty) = if field == ID {
                (field.clone(), Column::Primitive(false, PrimitiveType::String))
            } else {
                Column::from_str(&field)?
            };
            if nullable != ty.nullable() {return Err(Error::InvalidNullState(nullable, ty.nullable()));}
            if sty != ty.to_type() {return Err(Error::InvalidColumnAffinity(sty, ty.to_type()));}
            Ok((name, ty))
        }).collect::<Result<BTreeMap<String, Column>, Error>>()).transpose()?.filter(|c| !c.is_empty());
        let mut columns = if let Some(c) = columns {c} else {return Ok(None);};
        if columns.remove(ID).is_none() {return Err(missing(ID));}

        let (name, schema) = Schema::from(name, columns)?;
        Ok(Some(Table{name, schema}))
    }

    pub fn new(connection: &Connection, name: String, schema: Schema) -> Result<Self, Error> {
        let table_name = schema.name(&name);
        Ok(match Table::from_name(connection, &table_name)? {
            None => {
                let params = schema.columns().iter().map(|(n, p)| format!("\"{n}{FS}{p}\" {}{}",
                    p.to_type().to_string().to_uppercase(), if p.nullable() {String::new()} else {" NOT NULL".to_string()}
                )).collect::<Vec<_>>().join(", ");
                let params = if params.is_empty() {String::new()} else {format!(", {params}")};
                let cmd = format!("CREATE TABLE IF NOT EXISTS \"{table_name}\"(
                    {ID} TEXT PRIMARY KEY NOT NULL{params}
                )");
                println!("cmd: {:?}", cmd);
                connection.execute(&cmd, [])?;
              //let cmd = format!("CREATE INDEX IF NOT EXISTS \"index_{name}_{PARENT}\" ON \"{name}\" (\"{PARENT}\");");
              //connection.execute(&cmd, [])?;
                Table{name, schema}
            },
            Some(mut table) => {
                table.check_schema(connection, schema.into_columns(), true)?;
                table
            },
        })
    }

    fn check_schema(&mut self, connection: &Connection, mut columns: BTreeMap<String, Column>, exaustive: bool) -> Result<(), Error> {
        let retypes = self.schema.columns().iter().enumerate().flat_map(|(i, (n, p))| match (p, columns.remove(n)) {
            (_, None) if exaustive => Some(Err(missing(n))),
            (Column::Null, Some(new)) if new != Column::Null => if new.nullable() {
                Some(Ok((n.to_string(), new)))
            } else {Some(Err(Error::InvalidColumn(n.to_string(), Column::Null, new)))},
            (old, Some(new)) if **old != new && old.nullable() != new.nullable() =>
                Some(Err(Error::InvalidColumn(n.to_string(), (*old).clone(), new))),
            (old, Some(new)) if **old != new && new != Column::Null =>
                Some(Err(Error::InvalidColumn(n.to_string(), (*old).clone(), new))),
            _ => None
        }).collect::<Result<BTreeMap<String, Column>, Error>>()?;
        if let Some((n, _)) = columns.pop_first() {return Err(Error::MissingColumn(n));}
        retypes.into_iter().try_for_each(|(n, t)| self.retype(connection, n, t))?;
        Ok(())
    }

    fn retype(&mut self, connection: &Connection, n: String, t: Column) -> Result<(), Error> {
        let name = self.name();
        let cmd = format!("ALTER TABLE \"{name}\" DROP COLUMN \"{n}: Null\";");
        println!("cmd: {:?}", cmd);
        connection.execute(&cmd, [])?;
        let cmd = format!("ALTER TABLE \"{name}\" ADD COLUMN \"{n}{FS}{t}\" {} {};",
            t.to_type().to_string().to_uppercase(),
            if t.nullable() {String::new()} else {" NOT NULL".to_string()}
        );
        println!("cmd: {:?}", cmd);
        connection.execute(&cmd, [])?;
        self.schema.retype(n, t);
        
        Ok(())
    }

    pub fn tables(connection: &Connection) -> Result<BTreeMap<String, Self>, Error> {
        let cmd = "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%';";
        let names = connection.prepare(cmd)?.query_map([], |row| row.get::<_, String>(0))?.collect::<Result<Vec<_>, rusqlite::Error>>()?;
        names.into_iter().map(|name| {
            let table = Table::from_name(connection, &name)?.unwrap();
            Ok((name, table))
        }).collect()
    }

    pub fn insert<P: AsRef<Path>>(&mut self, connection: &Connection, path: P, values: BTreeMap<String, Cell>) -> Result<(), Error> {
        println!("values: {:?}", values);
        let columns = match &self.schema {
            Schema::Map(_) => {
                let ty = values.iter().enumerate().try_fold(Column::Null, |acc, (i, (k, v))| {
                    let t = v.to_column();
                    if acc != Column::Null && t != acc && t != Column::Null {Err(Error::InvalidColumn("Map".to_string(), acc.clone(), t))} else {Ok(t)}
                })?;
                BTreeMap::from([(VALUE.to_string(), ty)])
            },
            _ => values.iter().map(|(s, v)| (s.to_string(), v.to_column())).collect()
        };
        self.check_schema(connection, columns, false)?;
        let columns = self.schema.columns();

        let inserts = match &self.schema {
            Schema::Map( t) => {
                values.into_iter().map(|(k, v)| (path.as_ref().join(k), BTreeMap::from([(format!("{VALUE}{FS}{t}"), v)]))).collect()
            },
            _ => vec![(path.as_ref().to_path_buf(), values.into_iter().map(|(n, v)| (format!("{n}{FS}{}", columns.get(&n).unwrap()), v)).collect())]
        };
        println!("inserts: {:?}", inserts);

        for (id, columns) in inserts {
            let ((n, u), v): ((Vec<_>, Vec<_>), Vec<_>) = columns.into_iter().map(|(n, v)| {
                ((format!("\"{n}\""), format!("'{n}'=excluded.'{n}'")), v.to_string())
            }).unzip();

            let name = self.name();
            let (n, v, u) = if !n.is_empty() {(
                format!(", {}", n.join(",")), format!(", {}", v.join(",")), format!("UPDATE SET {}", u.join(","))
            )} else { Default::default()};
            let u = if u.is_empty() {"NOTHING".to_string()} else {u};

            let id = id.to_string_lossy();
            let cmd = format!("INSERT INTO \"{name}\"({ID}{n}) VALUES ('{id}'{v}) ON CONFLICT DO {u};");
            println!("cmd: {:?}", cmd);
            connection.execute(&cmd, [])?;
        }
      
        Ok(())
    }

    pub fn get<P: AsRef<Path>>(&self, connection: &Connection, id: P, columns: Option<BTreeSet<String>>) -> Result<Option<BTreeMap<String, Cell>>, Error> {
        let id = id.as_ref().to_string_lossy().to_string();
        let name = self.name();
        match &self.schema {
            Schema::Map(t) => {
                let col = (*t != Column::Null).then_some(format!("{VALUE}{FS}{t}"));
                let param = col.as_ref().map(|col| format!(", \"{col}\"")).unwrap_or_default();
                let id = if id == "/" {id} else {format!("{id}/")}; 
                let cmd = format!("SELECT {ID}{param} FROM \"{name}\" WHERE {ID} LIKE '{id}%' AND id NOT LIKE '{id}%/%'");
                println!("cmd: {:?}", cmd);
                Ok(connection.prepare(&cmd)?.query_map([], |row| Ok((
                    row.get::<&str, String>(ID)?.strip_prefix(&id).unwrap().to_string(),
                    col.as_ref().map(|col| Cell::from_value(1, t, row.get::<&str, types::Value>(col)?)).unwrap_or(Ok(Cell::None))?
                ))).optional()?.map(|t| t.collect()).transpose()?)
            },
            _ => {
                let schema_columns = self.schema.columns();
                let columns = if let Some(columns) = columns {
                    columns.into_iter().map(|n| {
                        let ty = schema_columns.get(&n).ok_or(Error::MissingColumn(n.to_string()))?;
                        Ok((n, ty))
                    }).collect::<Result<BTreeMap<_, _>, Error>>()?
                } else {schema_columns.iter().flat_map(|(n, ty)|
                    (!matches!(ty.to_type(), types::Type::Null)).then_some((n.to_string(), ty))
                ).collect()};

                let param = columns.iter().flat_map(|(n, t)|
                    (!matches!(t.to_type(), types::Type::Null)).then_some(format!("\"{n}{FS}{t}\""))
                ).collect::<Vec<_>>().join(",");
                let param = if param.is_empty() {param} else {format!(", {param}")};
                let cmd = format!("SELECT {ID}{param} FROM \"{name}\" WHERE {ID} = '{id}'");
                println!("cmd: {:?}", cmd);
              //connection.query_row(&cmd, [], |row| {
              //    row.get::<usize, String>(0)?;
              //    columns.into_iter().enumerate().map(|(i, (name, ty))| match ty.to_type() {
              //        types::Type::Null => Ok((name, None)),
              //        _ => Ok((name, Some((ty, row.get::<usize, types::Value>(i+1)?))))
              //    }).collect::<Result<BTreeMap<String, Option<(Type, types::Value)>>, Error>>()
              //}).optional()?.map(|columns| columns.into_iter().map(|(name, opt)| Ok((name, match opt {
              //    None => Cell::None,
              //    Some((ty, value)) => Cell::from_value(ty, value)
              //}))).collect()).transpose()


                Ok(connection.query_row(&cmd, [], |row| {
                    row.get::<usize, String>(0)?;
                    columns.into_iter().enumerate().map(|(i, (name, ty))| match ty.to_type() {
                        types::Type::Null => Ok((name, Cell::None)),
                        _ => Ok((name, Cell::from_value(i+1, ty, row.get::<usize, types::Value>(i+1)?)?))
                    }).collect::<Result<BTreeMap<String, Cell>, rusqlite::Error>>()
                }).optional()?)
            }
        }
    }

    pub fn delete<P: AsRef<Path>>(&self, connection: &Connection, path: P) -> Result<(), Error> {
        let id = path.as_ref().to_string_lossy();
        let name = self.name();
        let cmd = format!("DELETE FROM \"{name}\" WHERE {ID} LIKE '{id}%'");
        let _ = connection.execute(&cmd, [])?;
        Ok(())
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Column {
    Primitive(bool, PrimitiveType),
    Table(bool, String),
    Null
}

impl Column {
    fn to_type(&self) -> types::Type {match self {
        Self::Primitive(_, p) => match p {
            PrimitiveType::bool => types::Type::Integer,
            PrimitiveType::Number(n) => match n {
                NumberType::i8 | NumberType::i16 | NumberType::i32 | NumberType::i64 |
                NumberType::u8 | NumberType::u16 | NumberType::u32 => types::Type::Integer,
                NumberType::f32 | NumberType::f64 => types::Type::Real,
                NumberType::i128 | NumberType::u64 | NumberType::u128 => types::Type::Blob,
            },
            PrimitiveType::char | PrimitiveType::String => types::Type::Text,
            PrimitiveType::Bytes => types::Type::Blob,
            PrimitiveType::Unit => types::Type::Null
        },
        _ => types::Type::Null,
    }}

    fn nullable(&self) -> bool {match self {
        Self::Primitive(n, _) => *n,
        Self::Table(_, _) => true,
        _ => true
    }}

    fn from_str(s: &str) -> Result<(String, Self), Error> {
        let [field, ty]: [&str; 2] = s.split(FS).collect::<Vec<_>>().try_into().map_err(|_| Error::InvalidColumnName(s.to_string()))?;
        Ok((field.to_string(), if ty == "Null" {Self::Null} else {
            let (nullable, p) = ty.strip_prefix("Option<").and_then(|t| t.strip_suffix(">")).map(|o| (true, o)).unwrap_or((false, ty));
            PrimitiveType::from_str(p).ok().map(|p| Self::Primitive(nullable, p)).unwrap_or(Self::Table(nullable, p.to_string()))
        }))
    }
}

impl std::fmt::Display for Column {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {match self {
        Self::Primitive(nullable, p) => if *nullable {write!(f, "Option<{p}>")} else {write!(f, "{p}")},
        Self::Table(nullable, name) => if *nullable {write!(f, "Option<{name}>")} else {write!(f, "{name}")},
        Self::Null => write!(f, "Null"),
    }}
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Primitive(Primitive),
    Table(String),
    Some(Box<Self>), None
}
impl Cell {
    pub fn some(val: Cell) -> Self {Self::Some(Box::new(val))}

    fn from_value(index: usize, col: &Column, value: types::Value) -> Result<Self, rusqlite::Error> {
        let w = |e| rusqlite::Error::FromSqlConversionFailure(index, types::Type::Null, Box::new(e));
        Ok(match col {
            Column::Table(_, name) => Self::Table(name.to_string()),
            Column::Null => Self::None,
            Column::Primitive(false, _) if value == types::Value::Null => {
                return Err(w(FromSqlError::InvalidType));
            },
            _ if value == types::Value::Null => Self::None,
            Column::Primitive(nullable, primitive) => {
                let v = match primitive {
                    PrimitiveType::Unit => Self::Primitive(Primitive::Unit),
                    PrimitiveType::bool => Self::Primitive(Primitive::bool(bool::column_result((&value).into()).map_err(w)?)),
                    PrimitiveType::Number(number) => Self::Primitive(Primitive::Number(match number {
                        NumberType::i8  => Number::i8(i8::column_result((&value).into()).map_err(w)?),
                        NumberType::i16 => Number::i16(i16::column_result((&value).into()).map_err(w)?),
                        NumberType::i32 => Number::i32(i32::column_result((&value).into()).map_err(w)?),
                        NumberType::i64 => Number::i64(i64::column_result((&value).into()).map_err(w)?),
                        NumberType::u8  => Number::u8(u8::column_result((&value).into()).map_err(w)?),
                        NumberType::u16 => Number::u16(u16::column_result((&value).into()).map_err(w)?),
                        NumberType::u32 => Number::u32(u32::column_result((&value).into()).map_err(w)?),
                        NumberType::f32 => Number::f32(f32::column_result((&value).into()).map_err(w)?),
                        NumberType::f64 => Number::f64(f64::column_result((&value).into()).map_err(w)?),

                        NumberType::i128 => Number::i128(i128::from_le_bytes(<[u8; 16]>::column_result((&value).into()).map_err(w)?)),
                        NumberType::u64 => Number::u64(u64::from_le_bytes(<[u8; 8]>::column_result((&value).into()).map_err(w)?)),
                        NumberType::u128 => Number::u128(u128::from_le_bytes(<[u8; 16]>::column_result((&value).into()).map_err(w)?)),
                    })),
                    PrimitiveType::char => Self::Primitive(Primitive::char(
                        String::column_result((&value).into()).map_err(w)?.chars().next().ok_or(w(FromSqlError::InvalidType))?
                    )),
                    PrimitiveType::String => Self::Primitive(Primitive::String(String::column_result((&value).into()).map_err(w)?)),
                    PrimitiveType::Bytes => Self::Primitive(Primitive::Bytes(Vec::<u8>::column_result((&value).into()).map_err(w)?)),
                };
                if *nullable {Self::Some(Box::new(v))} else {v}
            }
        })
    }

    pub fn to_column(&self) -> Column {match self {
        Self::Table(name) => Column::Table(false, name.to_string()),
        Self::Primitive(p) => Column::Primitive(false, p.to_type()),
        Self::Some(c) => match c.to_column() {
            Column::Primitive(_, p) => Column::Primitive(true, p),
            Column::Table(_, p) => Column::Table(true, p),
            Column::Null => Column::Null
        },
        Self::None => Column::Null
    }}

    fn into_value(self) -> types::Value {
        match self {
            Self::Primitive(p) => match p {
                Primitive::Unit => types::Value::Null,
                Primitive::bool(b) => types::Value::Integer(b as i64),
                Primitive::Number(n) => match n {
                    Number::i8(i) => types::Value::Integer(i as i64),
                    Number::i16(i) => types::Value::Integer(i as i64),
                    Number::i32(i) => types::Value::Integer(i as i64), 
                    Number::i64(i) => types::Value::Integer(i),
                    Number::u8(u) => types::Value::Integer(u as i64),
                    Number::u16(u) => types::Value::Integer(u as i64),
                    Number::u32(u) => types::Value::Integer(u as i64),
                    Number::f32(f) => types::Value::Real(f as f64), 
                    Number::f64(f) => types::Value::Real(f),

                    Number::i128(i) => types::Value::Blob(i.to_le_bytes().to_vec()),
                    Number::u64(u) => types::Value::Blob(u.to_le_bytes().to_vec()),
                    Number::u128(i) => types::Value::Blob(i.to_le_bytes().to_vec()),
                },
                Primitive::char(c) => types::Value::Text(c.to_string()), 
                Primitive::String(s) => types::Value::Text(s),
                Primitive::Bytes(b) => types::Value::Blob(b),
            },
            Self::None | Self::Table(_) => types::Value::Null,
            Self::Some(t) => t.into_value()
        }
    }
}

impl std::fmt::Display for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.clone().into_value() {
            types::Value::Null => write!(f, "NULL"),
            types::Value::Integer(i) => write!(f, "{i}"),
            types::Value::Real(r) => write!(f, "{r}"),
            types::Value::Text(t) => write!(f, "'{t}'"),
            types::Value::Blob(b) => write!(f, "x\"{}\"", hex::encode(b)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Schema {
    Variant(String, Box<Self>),
    Struct(BTreeMap<String, Column>),
    NewType(Column),
    Map(Column)
}

impl Schema {
    fn from(name: &str, mut columns: BTreeMap<String, Column>) -> Result<(String, Self), Error> {
        match name.find(":") {
            Some(i) => {
                let split = name.split_at(i);
                if split.1 == ": Map" {
                    Ok((split.0.to_string(), Schema::Map(columns.remove(VALUE).ok_or(missing(VALUE))?)))
                } else if let Some(variant) = split.1.strip_prefix(VS) {
                    let (variant, t) = Schema::from(variant, columns)?;
                    Ok((split.0.to_string(), Schema::Variant(variant.to_string(), Box::new(t))))
                } else {
                    Err(invalid_table(name))
                }
            },
            None => {
                Ok((name.to_string(), if columns.keys().all(|k| k == VALUE) {
                   Schema::NewType(columns.remove(VALUE).unwrap()) 
                } else {Schema::Struct(columns)}))
            }
        }
    }

    pub fn name(&self, name: &str) -> String {match self {
        Self::Variant(variant, inner) => inner.name(&format!("{name}::{variant}")),
        Self::Map(_) => format!("{name}: Map"),
        _ => name.to_string(),
    }}

    fn into_columns(self) -> BTreeMap<String, Column> {match self {
        Self::Variant(_, inner) => inner.into_columns(),
        Self::Struct(columns) => columns,
        Self::Map(value) | Self::NewType(value) => BTreeMap::from([(VALUE.to_string(), value)]),
    }}

    fn columns(&self) -> BTreeMap<String, &Column> {match self {
        Self::Variant(_, inner) => inner.columns(),
        Self::Struct(columns) => columns.iter().map(|(k, v)| (k.to_string(), v)).collect(), 
        Self::Map(value) | Self::NewType(value) => BTreeMap::from([(VALUE.to_string(), value)]),
    }}

    fn retype(&mut self, n: String, t: Column) {match self {
        Self::Variant(_, inner) => inner.retype(n, t),
        Self::Struct(columns) => {columns.insert(n, t);},
        Self::Map(p) | Self::NewType(p) => {*p = t;}
    }}
}
