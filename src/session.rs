use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::table::{Table, Column, Affinity};
use super::{Value, Error};

use std::path::{PathBuf, Path};
use std::collections::BTreeMap;

pub struct Session<'a> {
    tables: BTreeMap<String, Table>,
    transaction: Transaction<'a>,
}

impl<'a> Session<'a> {
    pub fn new(connection: &'a mut Connection) -> Result<Self, Error> {
        let mut transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        let mut tables = Table::tables(&transaction)?;
        if tables.get("Root").is_none() {tables.insert("Root".to_string(),
            Table::new(&transaction, "Root", BTreeMap::from([("Table".to_string(), Column::Cell(Affinity::Text))]))?
        );}

        Ok(Session{
            tables,
            transaction
        })
    }

    //Path could end in a field if and only if value is a point
    pub fn insert<P: AsRef<Path>>(&mut self, path: P, value: Value) -> Result<(), Error> {
        let mut components = path.as_ref().components();
        println!("value: {:#?}", value);
        let mut tables = Table::tables(&self.transaction)?;
        
        let id = components.next_back().ok_or(Error::InvalidInsert)?;
        //let (name, value) = value.get_name();
        let parent = self.find(&tables, None, components.as_path())?;

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
        panic!("fininshed")
    }

    pub fn query<P: AsRef<Path>>(&mut self, path: P) -> Result<Value, Error> {

    }

    ///Returns a list of the possible tables your path could be in
    fn find<P: AsRef<Path>>(&mut self, table: Option<String>, path: P) -> Result<Vec<PathBuf>, Error> {
        let mut components = path.as_ref().components();
        let table: Vec<String> = if let Some(table) = table {

        } else {
            if components.next() == Some(Component::RootDir) {Err(Error::InvalidInsert)?}
            let first = components.next().ok_or(Error::InvalidInsert)?;
            println!("first: {:?}", first);
            if let Some(r) = self.tables.get(&"Root").unwrap().query(&self.transaction, first.as_path().join("Table"))?.first_entry() {
                vec![r]
            } else {return Ok(vec![]);}
        };


        if let Some(last) = components.next_back() {
            let path = components.join(last);
            for (path, table) in results {
                self.find(Some(table), 
            }
        }

        /// /Rooms/*/colored/messages

        for (path, table) in results {
            
        }


        panic!("test: {t:?}");
    }

    fn insert_unchecked<P: AsRef<Path>>(&mut self, path: P, value: Value) -> Result<(), Error> {
        Ok(())
    }

    pub fn commit(self) -> Result<(), Error> {
        self.transaction.commit();
        Ok(())
    }
}

impl Query for String {
    fn query() -> Vec<PathBuf> {vec![PathBuf::new()]}
}

impl<T: Query> Query for BTreeMap<String, T> {
    fn query() -> Vec<PathBuf> {T::query().into_iter().map(|q| PathBuf::from("[^/]*"))}
}

pub struct Room {
    author: String,
    messages: BTreeMap<String, Message>
}

impl Query for Room {
    fn query() -> Vec<PathBuf> {
        let mut queries = BTreeMap::<String, Message>::query().into_iter().map(|q| PathBuf::from("messages").join(q)).collect();
        queries.extend(String::query().into_iter().map(|q| PathBuf::from("author").join(q)));
        queries
    }
}

pub struct Message {
    author: String,
    body: String
}

impl Query for Message {
    fn query() -> PathBuf {
        let mut queries = String::query().into_iter().map(|q| PathBuf::from("body").join(q)).collect();
        queries.extend(String::query().into_iter().map(|q| PathBuf::from("author").join(q)));
        queries
    }
}

Room::query() =
vec![
    "author",
    "messages/*/body"
    "messages/*/author"
]
