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

use bevy::{
    app::{App, Plugin, Update},
    color::Color,
    ecs::{component::Component, event::Event, resource::Resource},
    state::state::States,
};
use header::submit_editable_title;

pub mod context;

mod clipboard;
mod demidom;
mod header;
mod page;
mod spoiler;
mod tts;
mod viewport;

// These are the window width factors that the content page will use
// when the window is in portrait or landscape proportions:
const PAGE_HEIGHT_PORTRAIT: f32 = 0.6;
const PAGE_WIDTH_LANDSCAPE: f32 = 0.4;

use crate::{clients::model::FetchEntityReason, shared::camera::MapCoords};

pub struct ContentPlugin;
impl Plugin for ContentPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ContentDarkMode::default())
            .add_systems(Update, submit_editable_title)
            .add_plugins((viewport::ViewportControllerPlugin, page::PageRendererPlugin));
    }
}

pub use page::ContentPageModel;

#[derive(Event)]
// Triggered when the page renderer is done spawning all content elements
pub struct EntityRenderingCompleted {
    pub uid: String,
    pub anchor: Option<String>,
    pub map_coords: Option<MapCoords>,
    pub fetch_reason: FetchEntityReason,
}

#[derive(Clone, Debug)]
pub struct EditableAttributeParams {
    pub attr_name: String,
    pub attr_entity: Option<String>,
    pub is_a_map_label: bool,
    pub in_settlement: Option<String>,
}

#[derive(Event, Clone)]
pub struct RenameSandboxEntity {
    pub entity_uid: String,
    pub value: String,
    pub params: EditableAttributeParams,
}

#[derive(Event)]
pub struct ScrollToAnchor {
    pub anchor: String,
}

#[derive(Component)]
pub struct ThemeBackgroundColor(pub Color);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, States)]
pub enum ContentMode {
    #[default]
    MapOnly,
    SplitScreen,
}

#[derive(Resource, PartialEq, Default)]
pub enum ContentDarkMode {
    #[default]
    Off,
    On,
}

impl ContentDarkMode {
    pub fn toggle(&mut self) {
        match self {
            ContentDarkMode::Off => *self = ContentDarkMode::On,
            ContentDarkMode::On => *self = ContentDarkMode::Off,
        }
    }
}

#[derive(Component)]
pub struct NpcAnchor(String);
