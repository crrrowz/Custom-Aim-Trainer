//! Local persistence: one SQLite file holding settings, assessments, drill levels and the plan.

use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("stored data could not be read: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS assessment (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 day INTEGER NOT NULL,
                 body TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS practice (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 day INTEGER NOT NULL,
                 body TEXT NOT NULL
             );",
        )?;
        Ok(Self { conn })
    }

    /// Settings, the plan and drill levels are single JSON values under a key.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let text: Option<String> = self
            .conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(text.map(|t| serde_json::from_str(&t)).transpose()?)
    }

    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.conn.execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    fn insert<T: Serialize>(&self, table: &str, day: u32, body: &T) -> Result<()> {
        self.conn.execute(
            &format!("INSERT INTO {table} (day, body) VALUES (?1, ?2)"),
            params![day, serde_json::to_string(body)?],
        )?;
        Ok(())
    }

    fn list<T: DeserializeOwned>(&self, table: &str) -> Result<Vec<T>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT body FROM {table} ORDER BY id"))?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(serde_json::from_str(&row?)?);
        }
        Ok(out)
    }

    pub fn add_assessment<T: Serialize>(&self, day: u32, body: &T) -> Result<()> {
        self.insert("assessment", day, body)
    }

    /// Oldest first.
    pub fn assessments<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        self.list("assessment")
    }

    pub fn add_practice<T: Serialize>(&self, day: u32, body: &T) -> Result<()> {
        self.insert("practice", day, body)
    }

    pub fn practice<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        self.list("practice")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_and_overwrite() {
        let s = Store::in_memory().unwrap();
        assert_eq!(s.get::<u32>("a").unwrap(), None);
        s.set("a", &1_u32).unwrap();
        s.set("a", &2_u32).unwrap();
        assert_eq!(s.get::<u32>("a").unwrap(), Some(2));
    }

    #[test]
    fn assessments_keep_their_order() {
        let s = Store::in_memory().unwrap();
        s.add_assessment(1, &"first").unwrap();
        s.add_assessment(2, &"second").unwrap();
        assert_eq!(s.assessments::<String>().unwrap(), vec!["first", "second"]);
    }
}
