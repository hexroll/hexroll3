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
use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::HexrollTestbedApp;

impl HexrollTestbedApp {
    pub fn save_settings(&self) -> std::io::Result<()> {
        let path = get_settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(&self.config)?)?;
        Ok(())
    }

    pub fn update_mru_list(&mut self, filepath: &str) {
        if self.config.recently_opened.len() > 5 {
            self.config
                .recently_opened
                .drain(0..self.config.recently_opened.len() - 5);
        }
        self.config.recently_opened.retain(|p| p != filepath);
        self.config.recently_opened.push(filepath.to_owned());
        self.save_settings()
            .map_err(|e| {
                log::warn!("Unable to save configuration. Error is {:#}", e)
            })
            .ok();
    }
}
fn get_settings_path() -> PathBuf {
    match dirs::config_dir() {
        Some(mut path) => {
            path.push("hexroll3");
            path.push("settings.json");
            path
        }
        None => panic!("Could not determine the configuration directory."),
    }
}

pub fn load_settings() -> std::io::Result<TestbedConfig> {
    let path = get_settings_path();
    log::info!("Configuration file path is {}", path.to_str().unwrap());
    let data = fs::read_to_string(path)?;
    let settings: TestbedConfig = serde_json::from_str(&data)?;
    Ok(settings)
}

#[derive(Serialize, Deserialize, Default)]
pub struct ConfigSandboxState {
    pub last_uid: String,
}
use std::{
    collections::HashMap,
    ops::{Index, IndexMut},
};

#[derive(Serialize, Deserialize)]
pub struct TestbedConfig {
    pub main_scroll_filepath: String,
    pub recently_opened: Vec<String>,
    pub sandbox_state: HashMap<String, ConfigSandboxState>,
    pub zoom_level: f32,
}

impl Default for TestbedConfig {
    fn default() -> Self {
        TestbedConfig {
            main_scroll_filepath: String::new(),
            recently_opened: Vec::new(),
            sandbox_state: HashMap::new(),
            zoom_level: 1.1,
        }
    }
}

impl Index<&str> for TestbedConfig {
    type Output = ConfigSandboxState;

    fn index(&self, index: &str) -> &Self::Output {
        &self.sandbox_state[index]
    }
}

impl IndexMut<&str> for TestbedConfig {
    fn index_mut(&mut self, index: &str) -> &mut Self::Output {
        if !self.sandbox_state.contains_key(index) {
            self.sandbox_state
                .insert(index.to_owned(), ConfigSandboxState::default());
        }
        self.sandbox_state.get_mut(index).expect("Key not found")
    }
}
