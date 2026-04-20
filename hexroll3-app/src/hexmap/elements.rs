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

use crate::battlemaps::BattlemapMaterial;
use crate::clients::model::FetchEntityReason;
use crate::hexmap::curve_tiles::*;
use crate::hexmap::data::*;
use crate::hexmap::tiles::*;
use crate::shared::vtt::HexRevealState;

use bevy::ecs::system::SystemId;
use bevy::platform::collections::hash_map::HashMap;
use bevy::prelude::*;
use hexx::Hex;
use hexx::HexLayout;

// pub const SANDBOX: &str = "3pcYrXdV";
// pub const SANDBOX: &str = "EqnabYIX";
// pub const SANDBOX: &str = "42hmjW6S";
// pub const SANDBOX: &str = "COVCtDBf";

#[derive(States, Debug, Default, Hash, PartialEq, Eq, Clone)]
pub enum HexMapState {
    #[default]
    Active,
    Suspended,
}

#[derive(SubStates, Debug, Default, Hash, PartialEq, Eq, Clone)]
#[source(HexMapState = HexMapState::Active)]
pub enum HexMapToolState {
    #[default]
    Selection,
    DialMenu,
    Edit,
    Draw,
}

#[derive(States, Debug, Default, Hash, PartialEq, Eq, Clone)]
pub enum HexMapSpawnerState {
    #[default]
    Unready,
    Enabled,
    Inhibited,
}

#[derive(States, Debug, Default, Hash, PartialEq, Eq, Clone)]
pub enum VttDataState {
    #[default]
    Unready,
    Loading,
    Available,
}

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct ScaleHudMarker;

#[derive(Debug)]
pub struct PreparedHexTile {
    pub uid: String,
    pub hex_type: TerrainType,
    pub hex_tile_material: Handle<TileMaterial>,
    pub partial_hex_tile_material: Handle<TileMaterial>,
    pub tile_scale: f32,
    pub river_tile: Option<Vec<(i32, Handle<Mesh>)>>,
    pub trail_tile: Option<Vec<(i32, Handle<Mesh>)>>,
    pub feature: HexFeature,
    pub metadata: HexMetadata,
    pub generated: bool,
    pub pool_id: i32,
}

impl PreparedHexTile {
    pub fn is_empty(&self) -> bool {
        match self.feature {
            HexFeature::None | HexFeature::Other => true,
            _ => false,
        }
    }
    pub fn can_source_a_river(&self) -> bool {
        self.hex_type == TerrainType::MountainsHex
            && self.metadata.is_rim
            && self.river_tile.is_none()
    }
    pub fn has_a_river(&self) -> bool {
        self.river_tile.is_some()
    }
    pub fn has_a_trail(&self) -> bool {
        self.trail_tile.is_some()
    }
}

#[derive(Debug, Resource, Default)]
pub struct HexMapCache {
    pub jsons: HashMap<String, String>,
}

impl HexMapCache {
    pub fn invalidate_json(&mut self, key: &str) {
        self.jsons.remove(key);
    }
}

#[derive(Debug)]
pub struct LazySpawn<T> {
    spawned: bool,
    pub value: T,
}

impl<T> LazySpawn<T> {
    pub fn from(value: T) -> Self {
        LazySpawn {
            spawned: false,
            value,
        }
    }
    pub fn set_spawned(&mut self) {
        self.spawned = true;
    }
    pub fn reset(&mut self) {
        self.spawned = false;
    }
    pub fn is_not_spawned(&self) -> bool {
        !self.spawned
    }
    pub fn not_spawned_filter() -> impl FnMut(&mut LazySpawn<T>) -> bool {
        |lazy_spawner| lazy_spawner.is_not_spawned()
    }
}

#[derive(Component, Default, PartialEq, Clone, Debug)]
pub enum HexRevealPattern {
    #[default]
    Flower,
    Single,
}

#[derive(Debug, Resource, Default)]
pub struct HexMapData {
    pub cmin: Hex,
    pub cmax: Hex,
    pub center: Hex,
    pub hexes: HashMap<Hex, PreparedHexTile>,
    // FIXME: are we correctly cleaning up invalidated uids?
    pub coords: HashMap<String, Hex>,
    pub region_labels: Vec<LazySpawn<(String, Vec2, f32)>>,
    pub realm_labels: Vec<LazySpawn<(String, Vec2, f32)>>,
    pub cursor: Option<Vec3>,
    pub selected: Option<Hex>,
    pub generating: bool,
}

impl HexMapData {
    pub fn get_selected_uid(&self) -> Option<String> {
        self.selected
            .and_then(|selected| self.hexes.get(&selected))
            .map(|hex_data| hex_data.uid.clone())
    }
    pub fn get_selected_uid_and_class(&self) -> Option<(String, String)> {
        self.selected
            .and_then(|selected| self.hexes.get(&selected))
            .map(|hex_data| (hex_data.uid.clone(), hex_data.hex_type.as_str().to_string()))
    }
    pub fn get_canonical_pos(&self, hex_uid: &str, offset: Vec2) -> Option<Vec2> {
        return if let Some(hex) = self.coords.get(hex_uid) {
            let layout = y_inverted_hexmap_layout();
            let hex_data = self.hexes.get(hex).unwrap();
            let feature_scale = hex_data.feature.feature_scale();
            let feature_metadata = hex_data.metadata.clone();
            let (angle, offset_b) = feature_metadata.feature_angle_and_offset();
            let mut offset = offset;
            offset = offset + Vec2::new(0.0, offset_b * feature_scale);
            offset = Transform::from_rotation(Quat::from_rotation_y(angle))
                .transform_point(Vec3::new(offset.x, 0.0, offset.y))
                .xz();
            Some(layout.hex_to_world_pos(*hex) + offset / feature_scale)
        } else {
            None
        };
    }
    pub fn center_camera_on_map(&self, ct: &mut Transform, cp: &mut Projection) {
        let layout = y_inverted_hexmap_layout();
        let map_center = layout.hex_to_world_pos(self.center);
        ct.translation.x = map_center.x;
        ct.translation.z = map_center.y;
        if let Projection::Orthographic(proj) = cp {
            proj.scale = 10.0;
        }
    }
}

#[derive(Debug, Component)]
pub struct HexToInvalidateMarker;

#[derive(Debug, Resource)]
pub struct HexEntityCallbacks {
    pub invalidate: SystemId,
}

#[derive(Debug, Resource)]
pub struct HexMapResources {
    pub mesh: Handle<Mesh>,
    pub layer_mesh: Handle<Mesh>,
    pub curved_mesh_tile_set: CurvedMeshTileSet,
    pub river_tile_materials: RiverTileMaterials,
    pub trail_material: Handle<TrailMaterial>,
    //
    pub water_material: Handle<SimpleBackgroundMaterial>,
    pub ocean_material: Handle<BackgroundMaterial>,
    pub underworld_material: Handle<BackgroundMaterial>,
    pub region_labels_material: Handle<StandardMaterial>,
    pub realm_labels_material: Handle<StandardMaterial>,
    pub pins_material: Handle<StandardMaterial>,
    pub dungeon_labels_material: Handle<StandardMaterial>,
    pub token_labels_material: Handle<StandardMaterial>,
    // Selection
    pub selection_mesh: Handle<Mesh>,
    pub selection_visible_material: Handle<StandardMaterial>,
    pub selection_hidden_material: Handle<StandardMaterial>,
    // Mask
    pub hex_mask_material: Handle<StandardMaterial>,
    // Generic Battlemap
    pub battlemap_material: Handle<BattlemapMaterial>,
    // Coordinates Font
    pub coords_font: Handle<Font>,
    // Labels Parent
    pub labels_parent: Entity,
}

#[derive(Component)]
pub struct HexEntity {
    pub hex: Hex,
}

#[derive(Component)]
pub struct HexCoordsForFeature {
    pub hex: Hex,
}

#[derive(Component)]
pub struct DungeonUnderlayer {
    pub hex: Hex,
    pub elevation_change_delay_in_frames: i32,
}

pub const HEX_SIZE: Vec2 = Vec2::splat(120.0);

#[derive(Event)]
pub struct FetchMapFromStorage {
    pub map: HexMapJson,
}

#[derive(Event)]
pub struct RerollHex {
    pub hex_coords: Hex,
    pub hex_uid: String,
    pub class: String,
}

#[derive(Event, Clone)]
pub struct AppendSandboxEntity {
    pub hex_coords: Option<Hex>,
    pub hex_uid: String,
    pub attr: String,
    pub what: String,
    pub send_coords: bool,
}

#[derive(Component)]
pub struct HexUid {
    pub uid: String,
}

#[derive(Component)]
pub struct HexMask(pub Hex);

#[derive(Component)]
pub struct RevealedOceanHex(pub Hex);

#[derive(Component)]
pub struct SelectionEntity;

#[derive(Component)]
pub struct MapLabels;

#[derive(Event, Clone)]
pub struct FetchEntityFromStorage {
    pub uid: String,
    pub anchor: Option<String>,
    pub why: FetchEntityReason,
}

#[derive(Event)]
pub struct RevealHex {
    pub is_ocean: bool,
    pub hex_coords: Hex,
    pub reveal_state: Option<HexRevealState>,
}

#[derive(Resource, Default)]
pub struct MapVisibilityController {
    pub scale: f32,
    pub rect: Rect,
}

impl MapVisibilityController {
    pub fn is_cave_decorations_visible(&self) -> bool {
        self.scale < 0.06
    }
    pub fn are_dungeons_and_settlements_visible(&self) -> bool {
        self.scale < 1.2
    }
    pub fn are_dungeons_hidden(&self) -> bool {
        self.scale > 1.2
    }
    pub fn are_battlemaps_visible(&self) -> bool {
        self.scale < 0.25
    }
}

pub fn hexmap_layout() -> HexLayout {
    HexLayout::new(hexx::HexOrientation::Flat).with_scale(HEX_SIZE)
}

pub fn y_inverted_hexmap_layout() -> HexLayout {
    let mut layout = HexLayout::new(hexx::HexOrientation::Flat).with_scale(HEX_SIZE);
    layout.invert_y();
    layout
}

pub fn hexx_to_hexroll_coords(hex_coords: &Hex) -> (i32, i32) {
    let send_coords = hex_coords.to_doubled_coordinates(hexx::DoubledHexMode::DoubledHeight);
    let y = -send_coords[1];
    let x = (send_coords[0] - (send_coords[0].abs() % 2)) / 2;
    (x, y)
}

pub fn hexroll_coords_to_string(x: i32, y: i32) -> String {
    match (x, y) {
        (0, 0) => "BASE".to_string(),
        _ => {
            let mut result = String::new();
            if x > 0 {
                result.push_str(&format!("E{}", x));
            } else if x < 0 {
                result.push_str(&format!("W{}", x.abs()));
            }
            if y < 0 {
                result.push_str(&format!("N{}", y.abs()));
            } else if y > 0 {
                result.push_str(&format!("S{}", y));
            }
            result
        }
    }
}
