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

use hexx::Hex;
use serde::{Deserialize, Serialize};

use crate::{
    battlemaps::DoorData,
    clients::controller::{PostMapLoadedOp, RequestMapFromBackend},
    hexmap::elements::HexEntity,
    shared::{
        settings::UserSettings,
        snapshot::FreezeScreenSnapshot,
        vtt::{HexRevealState, VttData},
    },
};
use avian3d::prelude::ColliderDisabled;
use bevy::{platform::collections::HashSet, prelude::*};

use super::{
    HexMapJson, LoadHexmapTheme, ToggleDayNight, daynight::DayNight, elements::HexMapData,
};

#[derive(Serialize, Deserialize, Event, Clone)]
pub struct ChunkedMap {
    pub chunk: String,
    pub part: usize,
    pub total: usize,
    /// Hash of the full original content (same value on every chunk of the same stream).
    pub hash: u64,
}

#[derive(Serialize, Deserialize, Event, Clone)]
pub enum MapMessageCacheType {
    ChunkedHexMap(ChunkedMap),
    ChunkedBattleMap(String, ChunkedMap),
}

#[derive(Serialize, Deserialize, Event, Clone)]
pub enum MapMessage {
    Cache(MapMessageCacheType),
    HexStateChange(HexState),
    DoorStateChange(DoorState),
    OpenedDoors(HashSet<String>),
    ReloadMap(Option<String>),
    ChangeTheme(String),
    SwitchDayNight(DayNight),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HexState {
    pub coords: Hex,
    pub is_ocean: bool,
    pub state: Option<HexRevealState>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DoorState {
    pub door_uid: String,
    pub is_open: bool,
}

pub fn on_map_message(
    trigger: On<MapMessage>,
    mut commands: Commands,
    mut vtt_data: ResMut<VttData>,
    hexes: Query<(Entity, &HexEntity)>,
    mut doors: Query<(Entity, &DoorData)>,
    mut map_data: ResMut<HexMapData>,
    mut cache: ResMut<crate::hexmap::elements::HexMapCache>,
    visible_hexes: Query<(Entity, &HexEntity), With<HexEntity>>,
    user_settings: Res<UserSettings>,
    children: Query<&Children>,
) {
    match trigger.event() {
        MapMessage::Cache(cache_type) => match cache_type {
            MapMessageCacheType::ChunkedHexMap(chunk_data) => {
                debug!("Player receiving cached map chunk");
                if chunk_data.part == 1 {
                    vtt_data.buffer = "".to_string();
                }
                vtt_data.buffer.push_str(&chunk_data.chunk);

                if chunk_data.part == chunk_data.total {
                    debug!("Player chunked map is complete");
                    if let Ok(map_data) = serde_json::from_str::<HexMapJson>(&vtt_data.buffer)
                    {
                        vtt_data.cache = Some(map_data);

                        // NOTE: This ensures players, who previously received a revealed
                        // hex sync for a materialized ocean, will now actually see the correct
                        // entity on their map.
                        vtt_data.prune_duplicate_oceans();

                        commands.trigger(RequestMapFromBackend {
                            post_map_loaded_op: PostMapLoadedOp::InvalidateVisible(None),
                        });
                    }
                }
            }
            MapMessageCacheType::ChunkedBattleMap(key, chunk_data) => {
                if cache.hashes.get(key) == Some(&chunk_data.hash) {
                    // We already have this exact content cached — ignore the entire stream.
                } else {
                    debug!("Player receiving cached battlemap map chunk");
                    let buffer_key = format!("{}_buffer", key);
                    if chunk_data.part == 1 {
                        cache.jsons.insert(buffer_key.clone(), "".to_string());
                    }

                    if let Some(buf) = cache.jsons.get_mut(&buffer_key) {
                        buf.push_str(&chunk_data.chunk);
                    }

                    if chunk_data.part == chunk_data.total {
                        debug!("Player chunked battle map is complete");
                        if let Some(buffer) = cache.jsons.remove(&buffer_key) {
                            cache.jsons.insert(key.clone(), buffer);
                            cache.hashes.insert(key.clone(), chunk_data.hash);
                        }
                        visible_hexes.iter().for_each(|(_, h)| {
                            if let Some(revealed_hex) = map_data.hexes.get(&h.hex) {
                                if revealed_hex.uid.as_str() == key {
                                    map_data.force_refresh.push(h.hex);
                                }
                            }
                        });
                        commands.trigger(FreezeScreenSnapshot);
                        vtt_data.invalidate_map = true;
                    }
                }
            }
        },
        MapMessage::HexStateChange(hex_message) => {
            if let Some(reveal_state) = hex_message.state {
                if !hex_message.is_ocean {
                    vtt_data.revealed.insert(hex_message.coords, reveal_state);
                } else {
                    vtt_data.revealed_ocean.insert(hex_message.coords);
                }
                // NOTE: When VTT peers receive HexStateChange message, they
                // must verify the existance of an entity for that hex before
                // appending the coords to force_refresh. Otherwise, a double-
                // spawn will occur when the hex is revealed for the first time,
                // and gets the first spawn from not being on the map, and the
                // second spawn from the force_refresh request.
                // (An alternative implementation would be to add a
                // ForceRefresh component to an Entity)
                for (_, hex) in hexes.iter() {
                    if hex.hex == hex_message.coords {
                        map_data.force_refresh.push(hex_message.coords);
                    }
                }
            } else {
                vtt_data.revealed.remove(&hex_message.coords);
                vtt_data.revealed_ocean.remove(&hex_message.coords);
                // NOTE: When unrevealing a hex, we must despawn it here
                // since the only other way hexes get despawned is when they
                // leave the viewport (in spawn.rs)
                for (entity, hex) in hexes.iter() {
                    if hex.hex == hex_message.coords {
                        commands.entity(entity).try_despawn();
                    }
                }
            }
            vtt_data.invalidate_map = true;
        }
        MapMessage::DoorStateChange(door_message) => {
            if door_message.is_open {
                vtt_data.open_doors.insert(door_message.door_uid.clone());
            } else {
                vtt_data.open_doors.remove(&door_message.door_uid);
            }
            for (e, d) in doors.iter_mut() {
                if door_message.door_uid == d.door_uid {
                    if door_message.is_open {
                        commands
                            .entity(e)
                            .insert(Visibility::Hidden)
                            .insert(ColliderDisabled);
                        children.iter_descendants(e).for_each(|e| {
                            commands
                                .entity(e)
                                .insert(ColliderDisabled)
                                .insert(Visibility::Hidden);
                        });
                    } else {
                        commands
                            .entity(e)
                            .insert(Visibility::Inherited)
                            .remove::<ColliderDisabled>();
                        children.iter_descendants(e).for_each(|e| {
                            commands
                                .entity(e)
                                .remove::<ColliderDisabled>()
                                .insert(Visibility::Inherited);
                        });
                    }
                }
            }
        }
        MapMessage::OpenedDoors(open_doors) => {
            vtt_data.open_doors = open_doors.clone();
            for (e, d) in doors.iter_mut() {
                if vtt_data.open_doors.contains(&d.door_uid) {
                    commands
                        .entity(e)
                        .try_insert(Visibility::Hidden)
                        .try_insert(ColliderDisabled);
                } else {
                    commands
                        .entity(e)
                        .try_insert(Visibility::Inherited)
                        .try_remove::<ColliderDisabled>();
                }
            }
        }
        MapMessage::ReloadMap(hex_uid) => {
            if !user_settings.local.unwrap_or(false) {
                commands.trigger(FreezeScreenSnapshot);
                if let Some(uid) = hex_uid {
                    commands.trigger(RequestMapFromBackend {
                        post_map_loaded_op: PostMapLoadedOp::None,
                    });
                    let to_refresh = map_data.coords.get(uid).unwrap().clone();
                    map_data.force_refresh.push(to_refresh);
                    vtt_data.invalidate_map = true;
                } else {
                    // NOTE: This ensures players, who previously received a revealed
                    // hex sync for a materialized ocean, will now actually see the correct
                    // entity on their map.
                    vtt_data.prune_duplicate_oceans();
                    commands.trigger(RequestMapFromBackend {
                        post_map_loaded_op: PostMapLoadedOp::InvalidateVisible(None),
                    });
                }
            }
            if let Some(hex_uid) = hex_uid {
                cache.invalidate_json(&hex_uid);
            }
        }
        MapMessage::ChangeTheme(theme) => {
            commands.trigger(LoadHexmapTheme {
                theme: theme.clone(),
            });
        }
        MapMessage::SwitchDayNight(day_night) => {
            commands.trigger(ToggleDayNight {
                value: day_night.clone(),
            });
        }
    }
}
