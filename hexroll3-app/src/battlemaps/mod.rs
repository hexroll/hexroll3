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
    ecs::{entity::Entity, event::Event},
    prelude::Deref,
};

mod battlemap_dial;
mod battlemaps;
mod caves;
mod city;
mod doors;
mod drawing;
mod dungeon;
mod effects;
mod helpers;
mod ruler;
mod settlement;
mod village;
mod wall;

pub use battlemaps::BattlemapsPlugin;

#[derive(Clone)]
pub struct BattlemapRequest {
    pub uid: String,
    pub hex: Entity,
}

#[derive(Event, Deref, Clone)]
// This event is triggered by this module when a dungeon map shoule become visible
pub struct RequestDungeonFromBackend(pub BattlemapRequest);

#[derive(Event, Deref, Clone)]
// This event is triggered by this module when a city or a town map shoule become visible
pub struct RequestCityOrTownFromBackend(pub BattlemapRequest);

#[derive(Event, Deref, Clone)]
// This event is triggered by this module when a village map shoule become visible
pub struct RequestVillageFromBackend(pub BattlemapRequest);

pub use caves::CaveMapConstructs;
pub use city::CityMapConstructs;
pub use dungeon::DungeonMapConstructs;
pub use village::VillageMapConstructs;
pub enum BattleMapConstructs {
    Dungeon(DungeonMapConstructs),
    Cave(CaveMapConstructs),
    City(CityMapConstructs),
    Village(VillageMapConstructs),
    Empty,
}

// Users can trigger these events to spawn battlemaps once they have constructed
// a valid BattleMapConstructs instance.
pub use caves::SpawnCaveMap;
pub use city::SpawnCityMap;
pub use dungeon::SpawnDungeonMap;
pub use village::SpawnVillageMap;

pub trait BattlemapFeatureUtils {
    // Users can call this on a hex feature entity commands to invalidate
    // the residing battlemap:
    //
    // [HEX ENTITY] -> [FEATURE ENTITY] -> [BATTLEMAP]
    //                                          ^
    //                                 this will be invalidated
    //
    fn invalidate_battlemap_in_hex_feature(&mut self);

    fn mark_battlemap_has_valid_state(&mut self);
    fn mark_battlemap_as_ready(&mut self);
    fn reset_battlemap_loading_state(&mut self);
}

pub use battlemaps::PlayerBattlemapEntity;
pub use battlemaps::RefereeBattlemapEntity;

pub use drawing::BattlemapUserDrawing;
pub use drawing::BattlemapUserDrawingInProgress;
pub use drawing::DryeraseDrawingMessage;

pub use battlemap_dial::BattlemapDialProvider;
pub use battlemap_dial::BattlemapSelection;
pub use battlemap_dial::BattlemapSelectionFinalizing;

pub use ruler::BattlemapsRuler;
pub use ruler::RulerDragData;

pub use doors::DoorData;
pub use doors::ToggleDungoenDoor;

pub use effects::BattlemapEffects;
pub use effects::SpawnVfx;
pub use effects::SpawnVfxBroadcast;

pub use battlemaps::BattlemapMaterial;
pub use battlemaps::DUNGEON_FOG_COLOR;
