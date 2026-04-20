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
    shared::vtt::{HexRevealState, VttData},
};
use avian3d::prelude::ColliderDisabled;
use bevy::{platform::collections::HashSet, prelude::*};

use super::{LoadHexmapTheme, ToggleDayNight, daynight::DayNight};

#[derive(Serialize, Deserialize, Event, Clone)]
pub enum MapMessage {
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
    mut cache: ResMut<crate::hexmap::elements::HexMapCache>,
) {
    match trigger.event() {
        MapMessage::HexStateChange(hex_message) => {
            if let Some(reveal_state) = hex_message.state {
                if !hex_message.is_ocean {
                    vtt_data.revealed.insert(hex_message.coords, reveal_state);
                } else {
                    vtt_data.revealed_ocean.insert(hex_message.coords);
                }
            } else {
                vtt_data.revealed.remove(&hex_message.coords);
                vtt_data.revealed_ocean.remove(&hex_message.coords);
            }
            for (entity, hex) in hexes.iter() {
                if hex.hex == hex_message.coords {
                    commands.entity(entity).try_despawn();
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
                    } else {
                        commands
                            .entity(e)
                            .insert(Visibility::Inherited)
                            .remove::<ColliderDisabled>();
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
            commands.trigger(RequestMapFromBackend {
                post_map_loaded_op: PostMapLoadedOp::InvalidateVisible,
            });
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
