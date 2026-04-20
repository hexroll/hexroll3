/*
// Copyright (C) 2020-2025 Pen, Dice & Paper
//
// This program is dual-licensed under the following terms:
//
// Option 1: (Non-Commercial) GNU Affero General Public License (AGPL)
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.
//
// Option 2: Commercial License
// For commercial use, you are required to obtain a separate commercial
// license. Please contact ithai at pendicepaper.com
// for more information about commercial licensing terms.
*/
use anyhow::{Result, anyhow};
use json_value_merge::Merge;
use redb::{ReadableDatabase, ReadableTable, Savepoint};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct Repository {
    pub db: Option<Arc<Mutex<redb::Database>>>,
}

impl Repository {
    pub fn new() -> Self {
        Repository { db: None }
    }

    pub fn create(&mut self, filename: &str) -> Result<&mut Self> {
        self.db = Some(Arc::new(Mutex::new(redb::Database::create(filename)?)));
        Ok(self)
    }

    pub fn open(&mut self, filename: &str) -> Result<&mut Self> {
        self.db = Some(Arc::new(Mutex::new(redb::Database::open(filename)?)));
        Ok(self)
    }

    pub fn load(&self, uid: &str) -> Result<serde_json::Value> {
        let tx = self
            .db
            .as_ref()
            .ok_or_else(|| anyhow!("Database reference is missing"))?
            .lock()
            .map_err(|_| anyhow!("Failed to acquire lock"))?
            .begin_read()
            .map_err(|_| anyhow!("Failed to begin read transaction"))?;

        const TABLE: redb::TableDefinition<String, JsonValue> =
            redb::TableDefinition::new("sandbox");

        let table = tx
            .open_table(TABLE)
            .map_err(|_| anyhow!("Failed to open table"))?;

        match table.get(uid.to_string()) {
            Ok(Some(ret)) => Ok(ret.value().value),
            Ok(None) => Err(anyhow!("No entry found for uid: {}", uid)),
            Err(_) => Err(anyhow!("Error retrieving entry for uid: {}", uid)),
        }
    }

    pub fn mutate<F, R>(&self, mut f: F) -> Result<R>
    where
        F: FnMut(&mut ReadWriteTransaction) -> Result<R>,
    {
        let tx = self
            .db
            .as_ref()
            .ok_or_else(|| anyhow!("Database not initialized"))?
            .lock()
            .map_err(|_| anyhow!("Failed to acquire lock"))?
            .begin_write()
            .map_err(|_| anyhow!("Failed to begin write transaction"))?;
        const TABLE: redb::TableDefinition<String, JsonValue> =
            redb::TableDefinition::new("sandbox");

        let closure_result = {
            let table = tx.open_table(TABLE)?;
            let mut repo_tx = ReadWriteTransaction {
                cache: HashMap::new(),
                table,
            };
            f(&mut repo_tx)?
        };

        tx.commit()
            .map_err(|_| anyhow!("Failed to commit transaction"))?;
        Ok(closure_result)
    }

    pub fn savepoint(&self) -> Result<Savepoint> {
        let tx = self
            .db
            .as_ref()
            .ok_or_else(|| anyhow!("Database not initialized"))?
            .lock()
            .map_err(|_| anyhow!("Failed to acquire lock"))?
            .begin_write()
            .map_err(|_| anyhow!("Failed to begin write transaction"))?;
        Ok(tx.ephemeral_savepoint()?)
    }

    pub fn restore(&self, savepoint: &Savepoint) -> Result<()> {
        let mut tx = self
            .db
            .as_ref()
            .ok_or_else(|| anyhow!("Database not initialized"))?
            .lock()
            .map_err(|_| anyhow!("Failed to acquire lock"))?
            .begin_write()
            .map_err(|_| anyhow!("Failed to begin write transaction"))?;
        tx.restore_savepoint(savepoint)?;
        Ok(tx.commit()?)
    }

    pub fn inspect<F, R>(&self, f: F) -> Result<R>
    where
        F: FnMut(&mut ReadOnlyTransaction) -> Result<R>,
    {
        let f = RefCell::new(f);
        let tx = self
            .db
            .as_ref()
            .ok_or_else(|| anyhow!("Database reference is missing"))?
            .lock()
            .map_err(|_| anyhow!("Failed to acquire database lock"))?
            .begin_read()
            .map_err(|_| anyhow!("Failed to begin read transaction"))?;
        const TABLE: redb::TableDefinition<String, JsonValue> =
            redb::TableDefinition::new("sandbox");
        let closure_result: R;
        {
            let table = tx.open_table(TABLE)?;
            let mut repo_tx = ReadOnlyTransaction {
                cache: HashMap::new(),
                table,
            };
            closure_result = f.borrow_mut()(&mut repo_tx)?;
        }
        Ok(closure_result)
    }
}

impl Default for Repository {
    fn default() -> Self {
        Self::new()
    }
}

pub trait ReadOnlyLoader {
    fn retrieve(&self, uid: &str) -> Result<JsonValue>;
}

pub struct ReadWriteTransaction<'a> {
    cache: HashMap<String, serde_json::Value>,
    pub table: redb::Table<'a, String, JsonValue>,
}

pub struct ReadOnlyTransaction {
    pub cache: HashMap<String, serde_json::Value>,
    pub table: redb::ReadOnlyTable<String, JsonValue>,
}

impl<'a> ReadOnlyLoader for ReadWriteTransaction<'a> {
    fn retrieve(&self, uid: &str) -> Result<JsonValue> {
        if let Some(cached) = self.cache.get(uid) {
            Ok(JsonValue {
                value: cached.clone(),
            })
        } else if let Ok(Some(ret)) = self.table.get(uid.to_string()) {
            Ok(ret.value())
        } else {
            Err(anyhow!("error in loading {}", uid))
        }
    }
}

impl<'a> ReadWriteTransaction<'a> {
    pub fn _has_cache(&mut self, uid: &str) -> bool {
        self.cache.contains_key(uid)
    }
    pub fn create(&mut self, uid: &str) -> Result<&mut serde_json::Value> {
        if !(self.cache.contains_key(uid)) {
            self.cache
                .insert(uid.to_string(), serde_json::json!({"uid": uid}));
        }
        Ok(self.cache.get_mut(uid).unwrap())
    }
    pub fn load(&mut self, uid: &str) -> Result<&mut serde_json::Value> {
        if !(self.cache.contains_key(uid)) {
            self.cache
                .insert(uid.to_string(), self.retrieve(uid)?.value);
        }
        Ok(self.cache.get_mut(uid).unwrap())
    }
    pub fn store(
        &mut self,
        uid: &str,
        value: &serde_json::Value,
    ) -> Result<()> {
        self.table
            .insert(
                uid.to_string(),
                JsonValue {
                    value: value.clone(),
                },
            )
            .map_err(|e| anyhow!(e))?;
        Ok(())
    }
    pub fn save(&mut self, uid: &str) -> Result<()> {
        if let Some(e) = self.cache.get(uid) {
            self.table
                .insert(uid.to_string(), &JsonValue { value: e.clone() })
                .map_err(|e| anyhow!(e))?;
            Ok(())
        } else {
            Err(anyhow!("Entity not found in cache"))
        }
    }
    pub fn patch(
        &mut self,
        uid: &str,
        attrs: &serde_json::Value,
    ) -> Result<()> {
        let entry = self
            .cache
            .get(uid)
            .ok_or_else(|| anyhow!("Entity not found in cache"))?;

        let mut value = entry.clone();
        value.merge(attrs);

        self.table
            .insert(
                uid.to_string(),
                &JsonValue {
                    value: value.clone(),
                },
            )
            .map_err(|e| anyhow!(e))?;

        self.cache.insert(uid.to_string(), value);

        Ok(())
    }
    pub fn remove(&mut self, uid: &str) -> Result<()> {
        self.table.remove(uid.to_string())?;
        if self.cache.contains_key(uid) {
            self.cache.remove(uid);
        }
        Ok(())
    }
    pub fn emplace_and_save(
        &mut self,
        uid: &str,
        v: serde_json::Value,
    ) -> Result<()> {
        self.cache.insert(uid.to_string(), v);
        self.save(uid)
    }
}

impl ReadOnlyLoader for ReadOnlyTransaction {
    fn retrieve(&self, uid: &str) -> Result<JsonValue> {
        if let Some(cached) = self.cache.get(uid) {
            Ok(JsonValue {
                value: cached.clone(),
            })
        } else if let Ok(Some(ret)) = self.table.get(uid.to_string()) {
            Ok(ret.value())
        } else {
            Err(anyhow!("error in loading {}", uid))
        }
    }
}

impl ReadOnlyTransaction {
    pub fn fetch(&mut self, uid: &str) -> Result<&mut serde_json::Value> {
        if !(self.cache.contains_key(uid)) {
            self.cache
                .insert(uid.to_string(), self.retrieve(uid)?.value);
        }
        Ok(self.cache.get_mut(uid).unwrap())
    }
    pub fn load(&self, uid: &str) -> Result<JsonValue> {
        if let Some(cached) = self.cache.get(uid) {
            Ok(JsonValue {
                value: cached.clone(),
            })
        } else if let Ok(Some(ret)) = self.table.get(uid.to_string()) {
            Ok(ret.value())
        } else {
            Err(anyhow!("error in loading {}", uid))
        }
    }
}

pub trait Entity {
    fn values(&self) -> &serde_json::Value;
    fn values_mut(&mut self) -> &mut serde_json::Value;
    fn is_missing(&self, attr: &str) -> bool {
        !self.values().as_object().unwrap().contains_key(attr)
    }
    fn clear(&mut self, attr: &str) {
        self.values_mut().as_object_mut().unwrap().swap_remove(attr);
    }
}

impl Entity for serde_json::Value {
    fn values(&self) -> &serde_json::Value {
        self
    }
    fn values_mut(&mut self) -> &mut serde_json::Value {
        self
    }
}

fn json_bytes<T>(structure: T) -> Vec<u8>
where
    T: serde::Serialize,
{
    let mut bytes: Vec<u8> = Vec::new();
    ciborium::into_writer(&structure, &mut bytes).unwrap();
    #[cfg(feature = "zstd")]
    {
        zstd::encode_all(bytes.as_slice(), 0).unwrap()
    }
    #[cfg(not(feature = "zstd"))]
    {
        bytes
    }
}

fn bytes_json(bytes: &[u8]) -> serde_json::Value {
    #[cfg(feature = "zstd")]
    let cbor = {
        let bytes = zstd::decode_all(bytes).unwrap();
        ciborium::from_reader(bytes.as_slice()).unwrap();
    };

    #[cfg(not(feature = "zstd"))]
    let cbor: ciborium::Value = ciborium::from_reader(bytes).unwrap();

    serde_json::to_value(cbor).unwrap()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonValue {
    pub value: serde_json::Value,
}

impl redb::Value for JsonValue {
    type SelfType<'a>
        = JsonValue
    where
        Self: 'a;
    type AsBytes<'a>
        = Vec<u8>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> JsonValue
    where
        Self: 'a,
    {
        JsonValue {
            value: bytes_json(data),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        json_bytes(&value.value)
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("test::JsonValue")
    }
}
