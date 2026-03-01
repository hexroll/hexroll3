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

use serde::{Deserialize, Serialize};

use bevy::prelude::*;

use crate::{
    battlemaps::{
        BattleMapConstructs, BattlemapFeatureUtils, SpawnCaveMap, SpawnCityMap,
        SpawnDungeonMap, SpawnVillageMap,
    },
    content::{
        ContentPageModel, EntityRenderingCompleted, ScrollToAnchor, context::ContentContext,
    },
    hexmap::elements::HexMapData,
    shared::{
        camera::GimbalshotCameraMovement,
        settings::{AppSettings, CONFIG_DIR},
        vtt::VttData,
    },
    tokens::Token,
};

use super::model::{FetchEntityReason, SearchResponse};

pub struct ApiControllerPlugin;

impl Plugin for ApiControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_render_entity_completed);
    }
}

#[derive(Event)]
pub struct RenderEntityContent {
    pub uid: String,
    pub data: ContentPageModel,
    pub anchor: Option<String>,
    pub why: FetchEntityReason,
}

pub fn on_render_entity_completed(
    trigger: On<EntityRenderingCompleted>,
    mut commands: Commands,
    map: Res<HexMapData>,
    mut content_stuff: ResMut<ContentContext>,
) {
    if let Some(coords) = &trigger.map_coords {
        content_stuff.current_hex_uid = Some(coords.hex.clone());
        commands.trigger(GimbalshotCameraMovement {
            coords: coords.clone(),
        });
    } else {
        if let Some(coord) = map.coords.get(&trigger.uid) {
            content_stuff.current_hex_uid = Some(trigger.uid.clone());
            if let Some(entity) = map.hexes.get(coord) {
                commands.trigger(GimbalshotCameraMovement {
                    coords: crate::shared::camera::MapCoords {
                        hex: entity.uid.clone(),
                        x: 0.0,
                        y: 0.0,
                        zoom: 4,
                    },
                });
            }
        }
    }

    if let Some(anchor) = &trigger.anchor {
        commands.trigger(ScrollToAnchor {
            anchor: anchor.clone(),
        });
    }
}

pub fn on_ingest_battlemap_data(
    key: &(String, Entity),
    data: (BattleMapConstructs, String),
    commands: &mut Commands,
    cache: &mut crate::hexmap::elements::HexMapCache,
) {
    if let Ok(mut entity) = commands.get_entity(key.1) {
        entity.mark_battlemap_has_valid_state();
    }
    match data.0 {
        BattleMapConstructs::Dungeon(dungeon_map_constructs) => {
            cache.jsons.insert(key.0.clone(), data.1);
            commands.trigger(SpawnDungeonMap {
                hex: key.1,
                data: dungeon_map_constructs,
            })
        }
        BattleMapConstructs::Cave(cave_map_constructs) => {
            cache.jsons.insert(key.0.clone(), data.1);
            commands.trigger(SpawnCaveMap {
                hex: key.1,
                data: cave_map_constructs,
            })
        }
        BattleMapConstructs::City(city_map_constructs) => commands.trigger(SpawnCityMap {
            hex: key.1,
            hex_uid: key.0.clone(),
            data: city_map_constructs,
        }),
        BattleMapConstructs::Village(village_map_constructs) => {
            commands.trigger(SpawnVillageMap {
                hex: key.1,
                hex_uid: key.0.clone(),
                data: village_map_constructs,
            })
        }
        _ => {}
    }
}

#[derive(Event)]
pub struct ShowSearchResults {
    pub search_response: SearchResponse,
}

impl ShowSearchResults {
    pub fn from_response(search_response: SearchResponse) -> Self {
        Self { search_response }
    }
}

#[derive(Serialize, Deserialize)]
pub struct TokenState {
    pub token: Token,
    pub transform: Transform,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ClientState {
    pub settings: AppSettings,
    pub vtt: VttData,
    pub tokens: Vec<TokenState>,
}

impl ClientState {
    pub fn path(sandbox: &str) -> PathBuf {
        let config_dir = dirs::config_dir().expect("Unable to get config dir");
        let config_path = config_dir
            .join(CONFIG_DIR)
            .join(format!("vtt_{}.json", sandbox));
        config_path
    }
}

#[derive(Resource, PartialEq)]
pub enum VttStateApiController {
    Unloaded,
    Idle,
    Staged(Timer),
    Inhibited,
}
