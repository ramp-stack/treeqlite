use rusqlite::{Connection, Transaction, TransactionBehavior, types::{self, Value}, Error};
use types::{FromSql, FromSqlResult, FromSqlError};
pub use rusqlite::types::Type as Affinity;

use regex::Regex;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{PathBuf, Path, Component};
use std::str::FromStr;

///Column Name used by Collections and NewType Enum/Structures
pub const VALUE: &str = "VALUE";

const ID: &str = "ID";
const FS: &str = ": ";
const VS: &str = "::";

fn stringify_value(val: types::Value) -> String {match val {
    types::Value::Null => "NULL".to_string(),
    types::Value::Integer(i) => format!("{i}"),
    types::Value::Real(r) => format!("{r}"),
    types::Value::Text(t) => format!("'{t}'"),
    types::Value::Blob(b) => format!("x\"{}\"", hex::encode(b)),
}}

fn regex_component(c: Component<'_>) -> String {match &*c.as_os_str().to_string_lossy() {
    "*" => "[^/]*".to_string(),
    "**" => ".*".to_string(),
    o => o.to_string()
}}

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

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Column {
    Cell(Affinity),
    Table(String),
    Null
}
impl Column {
    fn affinity(&self) -> Affinity {match self {
        Self::Cell(a) => a.clone(),
        _ => Affinity::Null
    }}
}
impl std::fmt::Display for Column {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cell(Affinity) => write!(f, ""),
            Self::Table(name) => write!(f, "{FS}{name}"),
            Self::Null => write!(f, "{FS}Null"),
        }
    }
}

pub enum Name {
    Pathed(PathBuf),

}

pub enum Schema {
    Map(Column),
    Struct(String, Vec<String>, BTreeMap<String, Column)
}

#[derive(Clone, Debug)]
pub struct Table {
    name: String,
    columns: BTreeMap<String, Column>
}

impl Table {
    pub fn columns(&self) -> &BTreeMap<String, Column> {&self.columns}
    pub fn name(&self) -> &String {&self.name}

    pub fn tables(connection: &Connection) -> Result<BTreeMap<String, Self>, Error> {
        let cmd = "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%';";
        let names = connection.prepare(cmd)?.query_map([], |row| row.get::<_, String>(0))?.collect::<Result<Vec<_>, rusqlite::Error>>()?;
        names.into_iter().map(|name| {
            let table = Table::from_name(connection, &name)?.unwrap();
            Ok((name, table))
        }).collect()
    }

    pub fn from_name(connection: &Connection, name: &str) -> Result<Option<Self>, Error> {
        let cmd = format!("SELECT name, type, cid FROM pragma_table_info('{name}')");
        let columns = connection.prepare(&cmd)?.query_map([], |row|
            Ok((row.get(0)?, row.get(1)?, row.get::<usize, u32>(2)? as usize))
        )?.collect::<Result<Vec<(String, String, usize)>, Error>>().optional()?;
        let columns = columns.map(|c| c.into_iter().map(|(field, affinity, cid)| {
            let affinity = match affinity.to_uppercase().as_str() {
                "INTEGER" => Affinity::Integer,
                "REAL" => Affinity::Real,
                "TEXT" => Affinity::Text,
                "BLOB" => Affinity::Blob,
                "" => Affinity::Null,
                unknown => {return Err(Error::InvalidColumnType(cid, unknown.to_string(), Affinity::Null));}
            };

            if field == ID && affinity != Affinity::Text {Err(Error::InvalidColumnType(cid, field.to_string(), affinity))?}
            Ok(field.split(FS).collect::<Vec<_>>().try_into().map(|[field, ty]: [&str; 2]| {
                (field.to_string(), if ty == "Null" {Column::Null} else {Column::Table(ty.to_string())})
            }).unwrap_or((field.to_string(), Column::Cell(affinity))))
        }).collect::<Result<BTreeMap<String, Column>, Error>>()).transpose()?.filter(|c| !c.is_empty());
        let mut columns = if let Some(c) = columns {c} else {return Ok(None);};
        columns.remove(ID);
        Ok(Some(Table{name: name.to_string(), columns}))
    }

    pub fn new(connection: &Connection, name: &str, columns: BTreeMap<String, Column>) -> Result<Self, Error> {
        Ok(match Table::from_name(connection, name)? {
            None => {
                let params = columns.iter().map(|(n, c)| format!("\"{n}{c}\" {}",
                    c.affinity().to_string().to_uppercase()
                )).collect::<Vec<_>>().join(", ");
                let params = if params.is_empty() {String::new()} else {format!(", {params}")};
                let cmd = format!("CREATE TABLE IF NOT EXISTS \"{name}\"(
                    {ID} TEXT PRIMARY KEY NOT NULL{params}
                )");
                println!("cmd: {:?}", cmd);
                connection.execute(&cmd, [])?;
                Table{name: name.to_string(), columns}
            },
            Some(mut table) => {
                table.check_schema(connection, columns)?;
                table
            },
        })
    }

    fn check_schema(&mut self, connection: &Connection, mut columns: BTreeMap<String, Column>) -> Result<(), Error> {
        let retypes = self.columns.iter().enumerate().flat_map(|(i, (n, p))| match (p, columns.remove(n)) {
            (Column::Null, Some(new)) if new != Column::Null => Some(Ok((n.to_string(), new))),
            (old, Some(new)) if *old != new && new != Column::Null =>
                Some(Err(Error::InvalidColumnType(i, n.to_string(), new.affinity()))),
            _ => None
        }).collect::<Result<BTreeMap<String, Column>, Error>>()?;
        columns.into_iter().try_for_each(|(n, c)| self.add_column(connection, n, c))?;
        retypes.into_iter().try_for_each(|(n, t)| self.retype(connection, n, t))?;
        Ok(())
    }

    fn add_column(&mut self, connection: &Connection, n: String, c: Column) -> Result<(), Error> {
        let name = self.name();
        let cmd = format!("ALTER TABLE \"{name}\" ADD COLUMN \"{n}{c}\" {};",
            c.affinity().to_string().to_uppercase(),
        );
        println!("cmd: {:?}", cmd);
        connection.execute(&cmd, [])?;
        self.columns.insert(n, c);
        Ok(())
    }

    fn retype(&mut self, connection: &Connection, n: String, c: Column) -> Result<(), Error> {
        let name = self.name();
        let cmd = format!("ALTER TABLE \"{name}\" DROP COLUMN \"{n}: Null\";");
        println!("cmd: {:?}", cmd);
        connection.execute(&cmd, [])?;
        self.add_column(connection, n, c)?;
        Ok(())
    }

  //

  //pub fn insert<P: AsRef<Path>>(&mut self, connection: &Connection, path: P, values: BTreeMap<String, Cell>) -> Result<(), Error> {
  //    println!("values: {:?}", values);
  //    let columns = match &self.schema {
  //        Schema::Map(_) => {
  //            let ty = values.iter().enumerate().try_fold(Column::Null, |acc, (i, (k, v))| {
  //                let t = v.to_column();
  //                if acc != Column::Null && t != acc && t != Column::Null {Err(Error::InvalidColumn("Map".to_string(), acc.clone(), t))} else {Ok(t)}
  //            })?;
  //            BTreeMap::from([(VALUE.to_string(), ty)])
  //        },
  //        _ => values.iter().map(|(s, v)| (s.to_string(), v.to_column())).collect()
  //    };
  //    self.check_schema(connection, columns, false)?;
  //    let columns = self.schema.columns();

  //    let inserts = match &self.schema {
  //        Schema::Map( t) => {
  //            values.into_iter().map(|(k, v)| (path.as_ref().join(k), BTreeMap::from([(format!("{VALUE}{FS}{t}"), v)]))).collect()
  //        },
  //        _ => vec![(path.as_ref().to_path_buf(), values.into_iter().map(|(n, v)| (format!("{n}{FS}{}", columns.get(&n).unwrap()), v)).collect())]
  //    };
  //    println!("inserts: {:?}", inserts);

  //    for (id, columns) in inserts {
  //        let ((n, u), v): ((Vec<_>, Vec<_>), Vec<_>) = columns.into_iter().map(|(n, v)| {
  //            ((format!("\"{n}\""), format!("'{n}'=excluded.'{n}'")), v.to_string())
  //        }).unzip();

  //        let name = self.name();
  //        let (n, v, u) = if !n.is_empty() {(
  //            format!(", {}", n.join(",")), format!(", {}", v.join(",")), format!("UPDATE SET {}", u.join(","))
  //        )} else { Default::default()};
  //        let u = if u.is_empty() {"NOTHING".to_string()} else {u};

  //        let id = id.to_string_lossy();
  //        let cmd = format!("INSERT INTO \"{name}\"({ID}{n}) VALUES ('{id}'{v}) ON CONFLICT DO {u};");
  //        println!("cmd: {:?}", cmd);
  //        connection.execute(&cmd, [])?;
  //    }
  //  
  //    Ok(())
  //}
        

    pub fn query<P: AsRef<Path>>(&self, connection: &Connection, query: P) -> Result<BTreeMap<PathBuf, BTreeMap<String, Value>>, Error> {
        let mut components = query.as_ref().components();
        let regex = components.next_back().and_then(|r| Regex::new(&regex_component(r)).ok())
            .ok_or(Error::InvalidPath(components.as_path().to_path_buf()))?;

        let fields = self.columns.keys().filter(|k| regex.is_match(k)).cloned().collect::<Vec<_>>();
        if components.next() != Some(Component::RootDir) {Err(Error::InvalidPath(components.as_path().to_path_buf()))?}
        let regex = format!("/{}", components.into_iter().map(regex_component).collect::<Vec<_>>().join("/"));
        panic!("fields, id: {:?}, {:?}", fields, regex);
    }

  //pub fn get<P: AsRef<Path>>(&self, connection: &Connection, id: P, columns: Option<BTreeSet<String>>) -> Result<Option<BTreeMap<String, Cell>>, Error> {
  //    let id = id.as_ref().to_string_lossy().to_string();
  //    let name = self.name();
  //    match &self.schema {
  //        Schema::Map(t) => {
  //            let col = (*t != Column::Null).then_some(format!("{VALUE}{FS}{t}"));
  //            let param = col.as_ref().map(|col| format!(", \"{col}\"")).unwrap_or_default();
  //            let id = if id == "/" {id} else {format!("{id}/")}; 
  //            let cmd = format!("SELECT {ID}{param} FROM \"{name}\" WHERE {ID} LIKE '{id}%' AND id NOT LIKE '{id}%/%'");
  //            println!("cmd: {:?}", cmd);
  //            Ok(connection.prepare(&cmd)?.query_map([], |row| Ok((
  //                row.get::<&str, String>(ID)?.strip_prefix(&id).unwrap().to_string(),
  //                col.as_ref().map(|col| Cell::from_value(1, t, row.get::<&str, types::Value>(col)?)).unwrap_or(Ok(Cell::None))?
  //            ))).optional()?.map(|t| t.collect()).transpose()?)
  //        },
  //        _ => {
  //            let schema_columns = self.schema.columns();
  //            let columns = if let Some(columns) = columns {
  //                columns.into_iter().map(|n| {
  //                    let ty = schema_columns.get(&n).ok_or(Error::MissingColumn(n.to_string()))?;
  //                    Ok((n, ty))
  //                }).collect::<Result<BTreeMap<_, _>, Error>>()?
  //            } else {schema_columns.iter().flat_map(|(n, ty)|
  //                (!matches!(ty.to_type(), types::Type::Null)).then_some((n.to_string(), ty))
  //            ).collect()};

  //            let param = columns.iter().flat_map(|(n, t)|
  //                (!matches!(t.to_type(), types::Type::Null)).then_some(format!("\"{n}{FS}{t}\""))
  //            ).collect::<Vec<_>>().join(",");
  //            let param = if param.is_empty() {param} else {format!(", {param}")};
  //            let cmd = format!("SELECT {ID}{param} FROM \"{name}\" WHERE {ID} = '{id}'");
  //            println!("cmd: {:?}", cmd);
  //          //connection.query_row(&cmd, [], |row| {
  //          //    row.get::<usize, String>(0)?;
  //          //    columns.into_iter().enumerate().map(|(i, (name, ty))| match ty.to_type() {
  //          //        types::Type::Null => Ok((name, None)),
  //          //        _ => Ok((name, Some((ty, row.get::<usize, types::Value>(i+1)?))))
  //          //    }).collect::<Result<BTreeMap<String, Option<(Type, types::Value)>>, Error>>()
  //          //}).optional()?.map(|columns| columns.into_iter().map(|(name, opt)| Ok((name, match opt {
  //          //    None => Cell::None,
  //          //    Some((ty, value)) => Cell::from_value(ty, value)
  //          //}))).collect()).transpose()


  //            Ok(connection.query_row(&cmd, [], |row| {
  //                row.get::<usize, String>(0)?;
  //                columns.into_iter().enumerate().map(|(i, (name, ty))| match ty.to_type() {
  //                    types::Type::Null => Ok((name, Cell::None)),
  //                    _ => Ok((name, Cell::from_value(i+1, ty, row.get::<usize, types::Value>(i+1)?)?))
  //                }).collect::<Result<BTreeMap<String, Cell>, rusqlite::Error>>()
  //            }).optional()?)
  //        }
  //    }
  //}

  //pub fn delete<P: AsRef<Path>>(&self, connection: &Connection, path: P) -> Result<(), Error> {
  //    let id = path.as_ref().to_string_lossy();
  //    let name = self.name();
  //    let cmd = format!("DELETE FROM \"{name}\" WHERE {ID} LIKE '{id}%'");
  //    let _ = connection.execute(&cmd, [])?;
  //    Ok(())
  //}
}
