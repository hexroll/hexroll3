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

use std::path::PathBuf;

pub mod controller;
pub mod http;
pub mod model;
pub mod standalone;

use anyhow::Result;
use bevy::{ecs::event::Event, log::info};
use hexroll3_scroll::instance::SandboxInstance;

use crate::shared::settings::UserSettings;

pub fn main_scroll_path() -> PathBuf {
    UserSettings::assets_path()
        .join("scrolls")
        .join("main.scroll")
}

pub fn roll_new_sandbox(id: &str) -> Result<String> {
    let mut instance = SandboxInstance::new();
    let filepath = UserSettings::sandbox_path(id);
    let scroll_path = main_scroll_path();
    if let Some(root_uid) = instance
        .with_scroll(scroll_path)?
        .create(filepath.to_str().unwrap())?
        .sid()
    {
        info!("Sid is {}", root_uid);
        Ok(root_uid)
    } else {
        unreachable!()
    }
}

#[derive(Event)]
pub struct RemoteBackendEvent<T: Event>(T);

#[derive(Event)]
pub struct StandaloneBackendEvent<T: Event>(T);
