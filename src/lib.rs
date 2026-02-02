use rusqlite::{Connection, Transaction, TransactionBehavior, types};
use types::{FromSql, FromSqlResult, FromSqlError};

use serde::{Serialize, Deserialize};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{PathBuf, Path};
use std::str::FromStr;

pub mod value;
use value::{Value};

pub mod table;
use table::{VALUE, Table, Column, Affinity};

pub mod session;
use session::Session;

///TODO:
///1. Tuples should be collapsed under parent name with column names like meta/0 and meta/1
///2. Maybe create Table for unnamed maps under parent such as Room/messages
///3. Optimize look ups with a Parent column?

#[derive(Debug)]
pub enum Error {
    Rusqlite(rusqlite::Error),
    ///Only Named Values can be inserted at root
    InvalidInsert,
    TableMissing,
    
    ///Occures when a mismatch between input schema and table schema was found
    SchemaMismatch
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {match self {
        Error::Rusqlite(r) => write!(f, "{r}"),
        e => write!(f, "{e:?}")
    }}
}
impl std::error::Error for Error {}
impl From<rusqlite::Error> for Error {fn from(error: rusqlite::Error) -> Error {Error::Rusqlite(error)}}

pub trait Treeqlite {
    fn insert<P: AsRef<Path>>(&mut self, path: P, value: Value) -> Result<(), Error>;
}
impl Treeqlite for Connection {
    fn insert<P: AsRef<Path>>(&mut self, path: P, value: Value) -> Result<(), Error> {
        let mut s = Session::new(self)?;
        s.insert(path, value)?;
        s.commit()
        
      //if path.as_ref().components().len() < 2 {Err(Error::InvalidInsert)?}
      //println!("value: {:#?}", value);
      //let mut tables = Table::tables(self)?;
      //if tables.get("Root").is_none() {
      //    tables.insert("Root".to_string(), Table::new(self, "Root", BTreeMap::from([("Table".to_string(), Column::Cell(Affinity::Text))]))?);
      //}
      ////let (name, value) = value.get_name();
      //let parent = self.find(&tables, None, path[..1])?;

      ////root.query(first
      //println!("tables: {:#?}", tables);
        //1. Read root table entry for path[0]
        
        //1.1 If it dose not exists create its entry and skip to step 5
        //2. Traverse structure to find path[..1], hopping Tables as needed
        //2.1 When a table name is a variant then we need to find the variant containing value
        //2.2 Or Read the tables to find the possible parent tables and do a query on that sub set of tables
        //3. Verify or ReType Parent
        //4. Traverse to path
        //5. Set value getting back sub values at sub paths
        //6. Set sub values/paths without verifying parent
    }

  //fn get<P: AsRef<Path>>(&mut self, path: P) -> Result<Option<Value>, Error> {
  //    //1. Read table structure from path[0]
  //    //2. Traverse structure to find path, hopping Tables as needed
  //    //3. Read the whole structure from that point on
  //    todo!()
  //}

    //fn query<P: AsRef<Path>>(&mut self, path: P, query: Query) -> Result<BTreeMap<PathBuf, Value>, Error>;
}



























#[cfg(test)]
mod test {
    use std::collections::BTreeMap;
    use serde::{Serialize, Deserialize};
    use crate::{Treeqlite, value::{Value, Serializer}};

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
        meta: (u8, u32),
        messages: BTreeMap<String, BTreeMap<String, Message>>,//Messages,
        server_message: Option<Message>,
        tags: RTags
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    pub struct RTags(Option<Tags>);

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    pub struct Tags(BTreeMap<String, BTreeMap<String, Option<String>>>);

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
            meta: (0, 2),
            messages: BTreeMap::from([("test".to_string(), BTreeMap::from([
                ("0".to_string(), message1),
                ("1".to_string(), message2),
            ]))]),
            server_message: Some(message3),
            tags: RTags(Some(Tags(BTreeMap::from([("one".to_string(), BTreeMap::from([("two".to_string(), None)]))]))))
                //Should Equal RTags("one: two: Null")
            //tags: RTags(Some(Tags(BTreeMap::from([("one".to_string(), BTreeMap::from([("two".to_string(), Some("three".to_string()))]))]))))
            //tags: Tags(BTreeMap::from([("one".to_string(), BTreeMap::from([("two".to_string(), "three".to_string())]))]))
        };

        let review = Review(Some(Some(384585)));

        let ser = room.serialize(Serializer).unwrap();

        con.insert("/charlie", ser).unwrap();


        panic!("SUCCESS");
    }
}
