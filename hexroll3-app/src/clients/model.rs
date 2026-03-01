/*
// Copyright (C) 2020-2026 Pen, Dice & Paper
//
// This program is dual-licensed under the following terms:
//
// Option 1: GNU Affero General Public License (AGPL)
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

use std::hash::{Hash, Hasher};

use bevy::ecs::event::Event;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct BackendUid(String);

impl From<String> for BackendUid {
    fn from(value: String) -> Self {
        BackendUid(value)
    }
}

impl Into<String> for BackendUid {
    fn into(self) -> String {
        self.0.clone()
    }
}

impl BackendUid {
    pub fn as_u64_hash(&self) -> u64 {
        let mut hasher = std::hash::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct SearchResultItem {
    pub value: String,
    pub details: String,
    pub uuid: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub icon: String,
    pub anchor: String,
}

#[derive(Event, Clone, PartialEq)]
pub enum FetchEntityReason {
    SandboxLink,
    History,
    Refresh,
}

#[derive(Event)]
pub struct RerollEntity {
    pub uid: String,
    pub class_override: String,
    pub is_map_reload_needed: bool,
}

impl RerollEntity {
    pub fn from_uid(uid: String) -> Self {
        Self {
            uid,
            class_override: "default".to_string(),
            is_map_reload_needed: true,
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum SandboxMode {
    Player,
    Referee,
}
