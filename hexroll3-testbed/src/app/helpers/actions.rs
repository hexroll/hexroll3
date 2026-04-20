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
use std::{path::PathBuf, str::FromStr};

use anyhow::Result;

use hexroll3_scroll::{
    generators::{append, reroll, unroll},
    instance::{SandboxBuilder, SandboxInstance},
    renderer::{render_entity, render_entity_html},
};

use crate::app::{HexrollTestbedApp, helpers::config::ConfigSandboxState};

impl HexrollTestbedApp {
    fn refresh_raw_json(&mut self) {
        self.current_url = "".to_string();
        if let Some(instance) = &self.instance {
            if let Ok(j) = instance
                .repo
                .inspect(|tx| Ok(tx.load(&self.uid_to_show)?.value))
            {
                self.current_entity.uid = self.uid_to_show.clone();
                self.current_entity.json_stored = j.clone();
            } else {
                //
            }
        }
    }
    pub fn navigate(&mut self, uid: &str, push_to_history: bool) {
        if uid != self.current_entity.uid && !self.uid_to_show.is_empty() {
            self.uid_to_show = uid.to_string();
            self.current_entity.json_rendered = serde_json::Value::Null;
            self.refresh_raw_json();
            self.prepare_demidom();
            self.config
                [self.instance.as_ref().unwrap().sid.as_ref().unwrap()] =
                ConfigSandboxState {
                    last_uid: uid.to_owned(),
                };
            self.save_settings()
                .map_err(|e| {
                    log::warn!("Unable to save configuration. Error is {:#}", e)
                })
                .ok();
            if push_to_history {
                self.history.push_front(uid.to_owned());
            }
        }
    }

    pub fn open_existing_sandbox_dialog(&mut self) {
        self.file.select_file();
    }

    pub fn roll_new_sandbox_dialog(&mut self, _ui: &mut egui::Ui) {
        self.file.config_mut().labels.save_button = "ROLL".to_string();
        self.file.config_mut().labels.title_save_file =
            "Roll a new sandbox".to_string();
        self.file.save_file();
    }

    pub fn open_existing_sandbox(&mut self, filepath: &str) -> Result<()> {
        let mut instance = SandboxInstance::new();
        if let Some(root_uid) = instance
            .with_scroll(PathBuf::from_str(&self.config.main_scroll_filepath)?)?
            .open(filepath)?
            .sid()
        {
            self.instance = Some(instance);
            let temp = self
                .config
                .sandbox_state
                .get(self.instance.as_ref().unwrap().sid.as_ref().unwrap())
                .map(|entry| entry.last_uid.clone())
                .unwrap_or_default();
            if !temp.is_empty() {
                self.navigate(&temp, true);
            } else {
                self.navigate(&root_uid, true);
            }
            self.update_mru_list(filepath);
            Ok(())
        } else {
            unreachable!()
        }
    }

    pub fn roll_new_sandbox(&mut self, filepath: &str) -> Result<()> {
        let mut instance = SandboxInstance::new();
        if let Some(root_uid) = instance
            .with_scroll(PathBuf::from_str(&self.config.main_scroll_filepath)?)?
            .create(filepath)?
            .sid()
        {
            self.instance = Some(instance);
            self.navigate(&root_uid, true);
            self.update_mru_list(filepath);
            Ok(())
        } else {
            unreachable!()
        }
    }

    pub fn test_scroll_filepath(scroll_filepath: &str) -> bool {
        let mut test_instance = SandboxInstance::new();
        let scroll_path = match PathBuf::from_str(scroll_filepath) {
            Ok(path) => path,
            Err(e) => {
                log::error!("Failed to create PathBuf from string. {:#}", e);
                return false;
            }
        };

        test_instance
            .with_scroll(scroll_path)
            .map_err(|e| {
                log::error!("Scrolls test failed. {:#}", e);
            })
            .is_ok()
    }

    pub fn unroll(&mut self, uid: &str) {
        if let Some(instance) = &self.instance {
            match instance.repo.mutate(|tx| {
                let builder = SandboxBuilder::from_instance(instance);
                let mut blueprint = builder.sandbox.blueprint.lock().unwrap();
                unroll(&builder, &mut blueprint, tx, uid, None)
            }) {
                Ok(_) => {
                    self.prepare_demidom();
                    self.refresh_raw_json();
                }
                Err(e) => {
                    log::error!("Error in unroll: {:?}", e);
                }
            }
        }
    }

    pub fn reroll(&mut self, uid: &str) {
        if let Some(instance) = &self.instance {
            match instance.repo.mutate(|tx| {
                let builder = SandboxBuilder::from_instance(instance);
                let mut blueprint = builder.sandbox.blueprint.lock().unwrap();
                reroll(&builder, &mut blueprint, tx, uid, None)
            }) {
                Ok(_) => {
                    self.prepare_demidom();
                    self.refresh_raw_json();
                }
                Err(e) => {
                    log::error!("Error in reroll: {:?}", e);
                }
            }
        }
    }

    pub fn append(&mut self, parent_uid: &str, attr_name: &str) {
        if let Some(instance) = &self.instance {
            if let Ok(_savepoint) = instance.repo.savepoint() {}
            match instance.repo.mutate(|tx| {
                let builder = SandboxBuilder::from_instance(instance);
                let mut blueprint = builder.sandbox.blueprint.lock().unwrap();
                append(
                    &builder,
                    &mut blueprint,
                    tx,
                    parent_uid,
                    attr_name,
                    None,
                    1,
                )?;
                Ok(())
            }) {
                Ok(()) => {
                    self.prepare_demidom();
                    self.refresh_raw_json();
                }
                Err(e) => {
                    log::error!("Error when appending: {:?}", e);
                }
            }
        }
    }

    pub fn load_rendered_json(&mut self) {
        log::trace!("Loading rendered json for {}", self.current_entity.uid);
        if let Some(instance) = &self.instance {
            let mut blueprint = instance.blueprint.lock().unwrap();
            if let Ok(result) = instance.repo.inspect(|tx| {
                let e = tx.load(&self.current_entity.uid)?;
                let rendered_json = render_entity(
                    instance,
                    &mut blueprint,
                    tx,
                    &e.value,
                    true,
                )?;
                Ok(rendered_json)
            }) {
                self.current_entity.json_rendered = result;
            }
        }
    }

    pub fn prepare_demidom(&mut self) {
        if let Some(instance) = &self.instance {
            log::info!("Rendering HTML for {}", self.current_entity.uid);
            let result = instance.repo.inspect(|tx| {
                let mut blueprint = instance.blueprint.lock().unwrap();
                log::trace!("fetching entity {}", self.current_entity.uid);
                let e = tx.load(&self.current_entity.uid)?;
                log::trace!("generating HTML for {}", self.current_entity.uid);
                let (header_html, body_html) =
                    render_entity_html(instance, &mut blueprint, tx, &e.value)?;
                log::trace!(
                    "generating HTML for {} is DONE",
                    self.current_entity.uid
                );
                Ok((body_html, header_html))
            });
            match result {
                Ok((html_content, header_content)) => {
                    let html_to_parse = format!(
                        "<html>{}\n{}</htmL>",
                        header_content, html_content
                    );
                    log::trace!("parsing HTML for {}", self.current_entity.uid);
                    self.parse_entity_html(&html_to_parse);
                    log::trace!("parsing DONE for {}", self.current_entity.uid);
                    self.current_entity.html_source = html_to_parse;
                }
                Err(e) => {
                    let html_to_parse = format!("<html>Error: {:?}</htmL>", e);
                    self.parse_entity_html(&html_to_parse);
                    self.current_entity.html_source = html_to_parse;
                    log::error!("Error when rendering HTML: {:?}", e);
                }
            }
        }
    }
}
