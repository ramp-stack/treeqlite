use rusqlite::{Connection, Transaction, TransactionBehavior, types};
use types::{FromSql, FromSqlResult, FromSqlError};

use serde_ron::value::{Value, Object, Data, Primitive, Number, PrimitiveType, NumberType};
use serde::{Serialize, Deserialize};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{PathBuf, Path};
use std::str::FromStr;

pub mod table;
use table::{VALUE, Table, Schema, Column, Cell};

///TODO:
///1. Tuples should be collapsed under parent name with column names like meta/0 and meta/1
///2. Maybe create Table for unnamed maps under parent such as Room/messages
///3. Optimize look ups with a Parent column?

#[derive(Debug)]
pub enum Error {
    Rusqlite(rusqlite::Error),
    ///Occures when a Table is expecting a Column that was not provided
    MissingColumn(String),
    ///Occures when a Table's Name could not be parsed
    InvalidTableName(String),
    ///Occures when a Column's name could not be parsed
    InvalidColumnName(String),
    ///Occures when a Sqlite Column Affinity could not be parsed
    UnknownColumnAffinity(String),
    ///Occures when the Sqlite Column Affnity dose not match the Rust Type
    InvalidColumnAffinity(rusqlite::types::Type, rusqlite::types::Type),
    ///Occures when the nullablitiy of a Column does not match the Rust Type
    InvalidNullState(bool, bool),
    ///Occures when a Map/Vec/Tuple is not wrapped in a NewType Structure
    MustNewType,
    ///Occures when the provided path is not pathed or formatted correctly
    InvalidPath(PathBuf),
    ///Map keys must be a Primitive that can be Stringified
    StringlessMapKey(Value),
    ///Occures when a different Column than the one expected was found
    InvalidColumn(String, Column, Column),
    ///Occures when a mismatch between input schema and table schema was found
    SchemaMismatch
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "{}", match self {
        Error::Rusqlite(r) => r.to_string(),
        Error::MissingColumn(column) => format!("Missing Column: {column}"),
        Error::InvalidTableName(table) => format!("Could Not Parse Table Name: {table}"),
        Error::InvalidColumnName(column) => format!("Could Not Parse Column Name: {column}"),
        Error::UnknownColumnAffinity(column) => format!("Could Not Parse Column Affinity: {column}"),
        Error::InvalidColumnAffinity(affinity, column) => format!("Columns Affinity Dose Not Match Required Affinity: {affinity} != {column}"),
        Error::InvalidNullState(nullable, column) => format!("Columns Null State Dose Not Match Required State: {nullable} != {column}"),
        Error::MustNewType => "Collection Types such as Map/Vec/Tuples must be wrapped in a NewType Structure".to_string(),
        Error::InvalidPath(path) => format!("Invalid Path: \"{path:?}\". Paths must start with Root and have at least one Component"),
        Error::StringlessMapKey(value) => format!("Map key could not be converted into a String: {value:?}"),
        Error::InvalidColumn(field, a, b) => format!("Column Mismatch: {field}: {a:?} != {b:?}"),
        Error::SchemaMismatch => format!("Schema Mismatch")
    })}
}
impl std::error::Error for Error {}
impl From<rusqlite::Error> for Error {fn from(error: rusqlite::Error) -> Error {Error::Rusqlite(error)}}

pub trait Treeqlite {
    fn serialize<P: AsRef<Path>, S: Serialize>(&mut self, path: P, value: &S) -> Result<(), Error>;
    fn deserialize<P: AsRef<Path>, D: for<'a> Deserialize<'a>>(&mut self, path: P) -> Result<Option<D>, Error>;
}
impl Treeqlite for Connection {
    fn serialize<P: AsRef<Path>, S: Serialize>(&mut self, path: P, value: &S) -> Result<(), Error> {
        let mut access = TableAccess::new(self)?;
        let ron_value = value.serialize(serde_ron::value::Serializer).expect("Ron Serializer Does Not Fail");
        access.insert(path, ron_value)?;
        access.commit()
    }
    fn deserialize<P: AsRef<Path>, D: for<'a> Deserialize<'a>>(&mut self, path: P) -> Result<Option<D>, Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct TableAccess<'a> {
    transaction: Transaction<'a>,
    tables: BTreeMap<String, Table>,
}
impl<'a> TableAccess<'a> {
    pub fn new(connection: &'a mut Connection) -> Result<Self, Error> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        Ok(TableAccess{
            tables: Table::tables(&transaction)?,
            transaction,
        })
    }

    pub fn get(&mut self, name: String, schema: Schema) -> Result<(&mut Table, &Transaction<'a>), Error> {
        if !self.tables.contains_key(&name) {
            self.tables.insert(name.clone(), Table::new(&self.transaction, name.clone(), schema)?);
        }// else {
        //    self.tables.get_mut(&name).unwrap().verify(&self.transaction, schema)?;
        //}
        Ok((self.tables.get_mut(&name).unwrap(), &self.transaction))
    }

    fn process_pending(pending: Pending, processed: &mut Processed) -> (Schema, Cells) {
        match pending {
            Pending::Collection(column, cells) => (Schema::Map(column), cells),
            Pending::Tuple(cells) => (Schema::Struct(
                cells.iter().enumerate().map(|(i, v)| (i.to_string(), v.to_column())).collect()
            ), cells.into_iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect()),
            Pending::Cell(Cell::Table(name)) => {
                processed.remove(&name, &PathBuf::new())
            },
            Pending::Cell(cell) => (Schema::NewType(cell.to_column()), BTreeMap::from([(VALUE.to_string(), cell)]))
        }
    }

    fn process_data(data: Data) -> Result<((Schema, Cells), Processed), Error> {
        let map = match data {
            Data::NewType(value) => {
                let (mut pending, mut processed) = Self::process_value(*value)?;
                let (schema, cells) = Self::process_pending(pending, &mut processed);
                return Ok(((schema, cells), processed));
            },
            Data::Struct(map) => map,
            Data::Tuple(values) => values.into_iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect(),
            Data::Unit => BTreeMap::new()
        };

        let mut processed = Processed::default();
        let cells = map.into_iter().map(|(k, v)| {
            let (pending, p) = Self::process_value(v)?;
            processed.extend(&k, p);
            println!("pending: {:?}", pending);
            Ok((k, pending.cell()?))
        }).collect::<Result<BTreeMap<_, _>, Error>>()?;
        Ok(((Schema::Struct(cells.iter().map(|(k, v)| (k.clone(), v.to_column())).collect()), cells), processed))
    }

    fn process_object(object: Object) -> Result<(Pending, Processed), Error> {
        let (name, variant, data) = match object {
            Object::Variant(name, variant, data) => (name, Some(variant), data),
            Object::Struct(name, data) => (name, None, data),
            Object::Option(Some(v)) => {
                let (pending, mut processed) = Self::process_value(*v)?;
                return Ok((Pending::Cell(Cell::Some(Box::new(pending.cell()?))), processed));
            },
            Object::Option(None) => {return Ok((Pending::Cell(Cell::None), Processed::default()));}
        };
        let ((schema, cells), mut processed) = Self::process_data(data)?;
        let pending = Pending::Cell(Cell::Table(name.clone()));
        processed.insert(name, if let Some(v) = variant {Schema::Variant(v, Box::new(schema))} else {schema}, PathBuf::new(), cells);
        Ok((pending, processed))
    }

    fn process_value(value: Value) -> Result<(Pending, Processed), Error> {
        let mut processed = Processed::default();
        match value {
            Value::Object(object) => Self::process_object(object),
            Value::Tuple(tuple) => {
                Ok((Pending::Tuple(tuple.into_iter().enumerate().map(|(i, v)| {
                    let (pending, p) = Self::process_value(v)?;
                    processed.extend(&i.to_string(), p);
                    pending.cell()
                }).collect::<Result<Vec<_>, Error>>()?), processed))
            },
            Value::Vec(vec) => {
                let vec = vec.into_iter().enumerate().map(|(i, v)| {
                    let (pending, p) = Self::process_value(v)?;
                    processed.extend(&i.to_string(), p);
                    Ok((i.to_string(), pending.cell()?))
                }).collect::<Result<BTreeMap<_, _>, Error>>()?;
                Ok((Pending::Collection(vec.first_key_value().map(|(_, v)| v.to_column()).unwrap_or(Column::Null), vec), processed))
            },
            Value::Map(map) => {
                let map = map.into_iter().map(|(k, v)|
                    if let Value::Primitive(s) = k {
                        let s = s.to_string();
                        let (pending, p) = Self::process_value(v)?;
                        processed.extend(&s, p);
                        Ok((s, pending.cell()?))
                    } else {Err(Error::StringlessMapKey(k))}
                ).collect::<Result<BTreeMap<_, _>, Error>>()?;
                Ok((Pending::Collection(map.first_key_value().map(|(_, v)| v.to_column()).unwrap_or(Column::Null), map), processed))
            }
            Value::Primitive(primitive) => Ok((Pending::Cell(Cell::Primitive(primitive)), processed)),
        }
    }

    ///TODO: Prefix path with structure name /Room/my_room instead of just /my_room
    ///Room may not be globally unique so for a distributed system use a hash of the Room Schema
    pub fn insert<P: AsRef<Path>>(&mut self, offset: P, value: Value) -> Result<(), Error> {
        //
        println!("data: {value:?}");
        //Interprates interface into records/rows
        let x = Self::process_value(value)?;
        println!("x: {x:#?}");
        let mut parent = offset.as_ref().components();
        if parent.clone().count() < 2 {return Err(Error::InvalidPath(offset.as_ref().to_path_buf()));}
        let parent = if parent.clone().count() < 3 {None} else {parent.next_back(); Some(parent.as_path().to_path_buf())};
        println!("parent: {:?}", parent);
        match x.0 {
            Pending::Cell(Cell::Table(name)) => {
                if let Some(parent) = parent {
                    //Find parent and update type if needed
                    todo!()
                }
                x.1.0.into_iter().try_for_each(|(name, (schema, cellss))| {
                    let (table, tx) = self.get(name, schema)?;
                    cellss.into_iter().try_for_each(|(id, cells)|
                        table.insert(tx, &offset.as_ref().join(id), cells)
                    )
                })
            },
            y => {
                //Partial Object
                panic!("y: {y:?}");
            }
        }
      
    }


    pub fn get<P: AsRef<Path>>(&mut self, offset: P) -> Result<Option<Value>, Error> {
        todo!()
    }

    pub fn delete<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        //TODO: could be primitive field
        self.tables.values().try_for_each(|table| table.delete(&self.transaction, path.as_ref()))
    }

    pub fn commit(self) -> Result<(), Error> {
        Ok(self.transaction.commit()?)
    }
}

type Cells = BTreeMap<String, Cell>;

#[derive(Default, Debug)]
struct Processed(BTreeMap<String, (Schema, BTreeMap<PathBuf, Cells>)>);
impl Processed {
    pub fn insert(&mut self, name: String, schema: Schema, path: PathBuf, cells: Cells) {
        let inserts = &mut self.0.entry(name).or_insert((schema, BTreeMap::new())).1;
        inserts.insert(path, cells);
    }

    pub fn remove(&mut self, name: &str, path: &PathBuf) -> (Schema, Cells) {
        let (schema, cellss) = self.0.get_mut(name).unwrap();
        (schema.clone(), cellss.remove(path).unwrap())
    }

    pub fn extend(&mut self, path: &str, other: Processed) {
        let prefix = PathBuf::from(path);
        other.0.into_iter().for_each(|(name, (schema, cellss))| {
            let inserts = &mut self.0.entry(name).or_insert((schema, BTreeMap::new())).1;
            cellss.into_iter().for_each(|(path, cells)| {
                inserts.insert(prefix.join(path), cells);
            })
        })
    }
}

#[derive(Debug)]
enum Pending {
    Collection(Column, BTreeMap<String, Cell>),
    Tuple(Vec<Cell>),
    Cell(Cell)
}

impl Pending {
    fn cell(self) -> Result<Cell, Error> {match self {
        Pending::Cell(cell) => Ok(cell),
        _ => Err(Error::MustNewType)
    }}
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;
    use serde::{Serialize, Deserialize};
    use serde_ron::value::Serializer;
    use crate::{Treeqlite, TableAccess};

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Ord, PartialOrd, Eq)]
    pub struct Tesing2 {
        valu: u32
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    pub struct Tesing{
        hello: Option<Tesing2>
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    pub enum TestVariant {
        Alpha(u8),
        Beta(Tesing),
        Charlie(BTreeMap<Tesing2, Tesing>),
        Delta(Option<Box<Self>>)
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    pub struct Room {
        bob: Option<u32>,
        author: String,
        meta: Meta,
        messages: Messages,
        server_message: Option<Message>
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    pub struct Review(Option<Option<u32>>);

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    pub struct Messages(BTreeMap<String, Message>);

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    pub struct Message {
        meta: Option<Meta>,
        author: String,
        body: String,
        reviewed: Review,
        comments: Messages,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    pub struct Meta(u8, u32);

    #[test]
    fn test() {
        let mut con = rusqlite::Connection::open("test.db").unwrap();

      //let beta = TestVariant::Beta(Tesing{hello: None});
      //let beta2 = TestVariant::Beta(Tesing{hello: Some(Tesing2{valu: 9445})});
      //let charlie = TestVariant::Charlie(BTreeMap::new());
      //let charlie2 = TestVariant::Charlie(BTreeMap::from([
      //    (Tesing2{valu: 4785}, Tesing{hello: None}),
      //    (Tesing2{valu: 82874}, Tesing{hello: Some(Tesing2{valu: 48757})})
      //]));
      //let delta_none = TestVariant::Delta(None);
      //let delta_alpha = TestVariant::Delta(Some(Box::new(alpha.clone())));
      //let delta_beta = TestVariant::Delta(Some(Box::new(beta2.clone())));
      //let delta_charlie = TestVariant::Delta(Some(Box::new(charlie2.clone())));
      //let delta_delta_charlie = TestVariant::Delta(Some(Box::new(delta_charlie.clone())));

      //let alpha = TestVariant::Alpha(29);
      //let mut access = TableAccess::new(&mut con).unwrap();
      //let ron_value = alpha.serialize(Serializer).unwrap();
      //access.insert("/bravo", ron_value).unwrap();
      //access.commit().unwrap();

        let message1 = Message{
            meta: Some(Meta(29, 14858585)),
            author: "bob".to_string(),
            body: "First Message".to_string(),
            reviewed: Review(Some(None)),
            comments: Messages(BTreeMap::from([]))
        };

        let message2 = Message{
            meta: Some(Meta(2, 14855)),
            author: "alice".to_string(),
            body: "Second Message".to_string(),
            reviewed: Review(Some(Some(834834))),
            comments: Messages(BTreeMap::from([("First".to_string(), message1.clone())]))
        };

        let message3 = Message{
            meta: Some(Meta(9, 8585)),
            author: "charlie".to_string(),
            body: "Third Message".to_string(),
            reviewed: Review(None),
            comments: Messages(BTreeMap::from([("Previous".to_string(), message2.clone())]))
        };

        let room = Room{
            bob: None,
            author: "bob".to_string(),
            meta: Meta(0, 2),
            messages: Messages(BTreeMap::from([
                ("0".to_string(), message1),
                ("1".to_string(), message2),
            ])),
            server_message: Some(message3) 
        };

        let review = Review(Some(Some(384585)));

        con.serialize("/charlie", &room).unwrap();


        panic!("SUCCESS");
    }
}
