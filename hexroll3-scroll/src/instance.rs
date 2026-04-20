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
use std::cell::RefCell;
use std::{collections::HashMap, path::PathBuf};

use anyhow::{Result, anyhow};
use caith::RollResultType;
use minijinja::Environment;
use rand::{
    Rng, distributions::Alphanumeric, rngs::ThreadRng, seq::SliceRandom,
    thread_rng,
};
use serde_json::Value;

use crate::{
    generators::roll,
    parser::{parse_buffer, parse_file},
    renderer_env::prepare_renderer,
    repository::*,
    semantics::*,
};

/// SandboxBuilder is a wrapper for sandbox instances, providing the
/// additional facilities required to generate content.
pub struct SandboxBuilder<'a> {
    pub sandbox: &'a SandboxInstance,
    pub randomizer: Randomizer,
    pub templating_env: Environment<'a>,
}

impl<'a> SandboxBuilder<'a> {
    pub fn from_instance(instance: &'a SandboxInstance) -> Self {
        let mut env = Environment::new();
        prepare_renderer(&mut env, instance);
        SandboxBuilder {
            sandbox: instance,
            randomizer: Randomizer::new(),
            templating_env: env,
        }
    }
}

/// SandboxBlueprint stores the generation model read from the scroll
/// files as well as any extenions stored and dynamically loaded from
/// the sandbox repo.
pub struct SandboxBlueprint {
    pub classes: HashMap<String, Class>,
    pub globals: HashMap<String, serde_json::Value>,
    pub map_data_provider: fn(
        &SandboxBuilder,
        &mut SandboxBlueprint,
        &mut ReadWriteTransaction,
        &str,
    ) -> Result<Option<(String, Value)>>,
}

impl SandboxBlueprint {
    pub fn new() -> Self {
        SandboxBlueprint {
            classes: HashMap::new(),
            globals: HashMap::new(),
            map_data_provider: |_, _, _, _| Ok(None),
        }
    }
    pub fn parse_buffer(&mut self, buffer: &str) -> &mut Self {
        parse_buffer(self, buffer, None, None).unwrap();
        self
    }
}

/// SandboxInstance holds all the data needed to read and render
/// generated content as well as the sandbox blueprint
#[derive(Clone)]
pub struct SandboxInstance {
    pub sid: Option<String>,
    pub repo: Repository,
    pub blueprint: std::sync::Arc<std::sync::Mutex<SandboxBlueprint>>,
}

impl SandboxInstance {
    pub fn new() -> Self {
        SandboxInstance {
            sid: None,
            repo: Repository::new(),
            // blueprint has to allow internal mutability to support dynamically
            // generated model elements such as dungeon maps.
            blueprint: std::sync::Arc::new(std::sync::Mutex::new(
                SandboxBlueprint::new(),
            )),
        }
    }

    pub fn with_scroll(
        &mut self,
        scroll_filepath: PathBuf,
    ) -> Result<&mut Self> {
        parse_file(&mut self.blueprint.lock().unwrap(), scroll_filepath)?;
        Ok(self)
    }

    pub fn open(&mut self, filepath: &str) -> Result<&mut Self> {
        self.repo.open(filepath)?;
        let root = self.repo.inspect(|tx| tx.load("root"))?;
        if let Some(sid) = root.value.as_str() {
            self.sid = Some(sid.to_string());
            Ok(self)
        } else {
            Err(anyhow!("Unable to find root entity in {}", filepath))
        }
    }

    pub fn create(&mut self, filepath: &str) -> Result<&mut Self> {
        self.repo.create(filepath)?;

        if let Ok(sid) = self.repo.mutate(|tx| {
            let mut builder = SandboxBuilder::from_instance(self);
            let mut blueprint = builder.sandbox.blueprint.lock().unwrap();
            // NOTE: Precreate a root placeholder so that collection in the following
            // roll call will not fail, and later set the uid for root in a following
            // `store` call.
            tx.store("root", &serde_json::Value::Null)?;
            let ret =
                roll(&mut builder, &mut blueprint, tx, "main", "root", None);
            tx.store("root", &serde_json::json!(ret.as_ref().unwrap()))?;
            tx.store("rerolls", &serde_json::json!({"entities":[]}))?;
            ret
        }) {
            self.sid = Some(sid.to_string());
            Ok(self)
        } else {
            Err(anyhow!(
                "Was unable to create a new sandbox in {}",
                filepath
            ))
        }
    }

    pub fn sid(&self) -> Option<String> {
        self.sid.clone()
    }

    pub fn parse_buffer(
        &self,
        blueprint: &mut SandboxBlueprint,
        buffer: &str,
    ) -> &Self {
        parse_buffer(blueprint, buffer, None, None).unwrap();
        self
    }

    pub fn resolve_class(
        &self,
        blueprint: &mut SandboxBlueprint,
        class_name: &str,
    ) -> Option<Class> {
        {
            // NOTE: This is the optimistic path, where the class
            // was loaded as part of the scroll files.
            if blueprint.classes.contains_key(class_name) {
                return blueprint.classes.get(class_name).cloned();
            }
        }
        // NOTE: This is the exception, where the class was dynamically
        // generated and stored in the repo.
        if let Ok(stored_class) =
            self.repo.inspect(|tx| tx.retrieve(class_name))
        {
            self.parse_buffer(blueprint, stored_class.value.as_str().unwrap());
        }
        return blueprint.classes.get(class_name).cloned();
    }
}

impl Default for SandboxInstance {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Randomizer {
    rng: RefCell<ThreadRng>, // Use RefCell for interior mutability
}

impl Randomizer {
    pub fn new() -> Self {
        Randomizer {
            rng: RefCell::new(thread_rng()),
        }
    }

    pub fn choose<'a, T>(&self, v: &'a Vec<T>) -> &'a T {
        match v.choose(&mut *self.rng.borrow_mut()) {
            Some(item) => item,
            None => panic!("List is empty"),
        }
    }

    pub fn uid(&self) -> String {
        let mut rng = self.rng.borrow_mut();
        (0..8).map(|_| rng.sample(Alphanumeric) as char).collect()
    }

    pub fn in_range(&self, min: i32, max: i32) -> i32 {
        let mut rng = self.rng.borrow_mut();
        rng.gen_range(min..max + 1)
    }

    pub fn shuffle<T>(&self, collection: &mut [T]) {
        let mut rng = self.rng.borrow_mut();
        collection.shuffle(&mut *rng);
    }

    pub fn u64(&self) -> u64 {
        let mut rng = self.rng.borrow_mut();
        rng.r#gen::<u64>()
    }

    pub fn dice(&self, roll: &str) -> Option<i32> {
        let mut rng = self.rng.borrow_mut();
        let roller = caith::Roller::new(roll).unwrap();
        if let RollResultType::Single(value) =
            roller.roll_with(&mut *rng).unwrap().get_result()
        {
            Some(value.get_total() as i32)
        } else {
            None
        }
    }
}

impl Default for Randomizer {
    fn default() -> Self {
        Self::new()
    }
}
