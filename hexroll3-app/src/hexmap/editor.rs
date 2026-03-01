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

use bevy::{platform::collections::HashMap, prelude::*};
use hexx::*;

use crate::{
    hexmap::data::{HexFeature, HexMetadata, TerrainType},
    hexmap::daynight::HexMapTime,
    hexmap::elements::*,
    hexmap::spawn::spawn_tile,
    hexmap::tiles::{HexMapTileMaterials, TileMaterial},
    shared::vtt::*,
};

pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MapEditor {
            pen: TerrainType::ForestHex,
        })
        .add_systems(Update, set_pen)
        .add_systems(Update, draw_tiles.run_if(in_state(HexMapToolState::Edit)));
    }
}

#[derive(Resource)]
pub struct MapEditor {
    pub pen: TerrainType,
}

pub fn draw_tiles(
    mut commands: Commands,
    mut map: ResMut<HexMapData>,
    mut vtt_data: ResMut<VttData>,
    assets: Res<HexMapResources>,
    tiles: Res<HexMapTileMaterials>,
    click: Res<ButtonInput<MouseButton>>,
    map_parent: Single<Entity, With<HexMapTime>>,
    spawned_hexes: Query<(Entity, &HexEntity)>,
    editor: ResMut<MapEditor>,
) {
    let layout = hexmap_layout();

    if let Some(coord) = map.selected {
        if click.pressed(MouseButton::Right) && !map.hexes.contains_key(&coord) {
            let new_hex = PreparedHexTile {
                uid: "<uid>".to_string(),
                hex_type: editor.pen.clone(),
                hex_tile_material: get_tile_material2(
                    coord,
                    editor.pen.clone(),
                    &map.hexes,
                    &tiles,
                ),
                partial_hex_tile_material: get_tile_material2(
                    coord,
                    editor.pen.clone(),
                    &map.hexes,
                    &tiles,
                ),
                tile_scale: 0.5,
                river_tile: None,
                trail_tile: None,
                feature: HexFeature::None,
                metadata: HexMetadata {
                    harbor: None,
                    river_dir: 0.0,
                    offset: 0.0,
                    is_rim: true,
                },
            };
            map.hexes.insert(coord, new_hex);
            draw_tile_and_refresh_neighbors(
                &mut commands,
                &layout,
                coord,
                *map_parent,
                &tiles,
                &assets,
                &mut map,
                &mut vtt_data,
                &spawned_hexes,
            );
        }
    }
}

fn draw_tile_and_refresh_neighbors(
    commands: &mut Commands,
    layout: &HexLayout,
    hex: Hex,
    map_parent: Entity,
    tiles: &Res<HexMapTileMaterials>,
    map_resources: &Res<HexMapResources>,
    map_data: &mut ResMut<HexMapData>,
    vtt_data: &mut ResMut<VttData>,
    spawned_hexes: &Query<(Entity, &HexEntity)>,
) {
    spawn_tile(
        commands,
        layout,
        hex,
        map_parent,
        tiles,
        map_resources,
        map_data,
        vtt_data,
    );
    for neighbor in hex.all_neighbors() {
        //
        if map_data.hexes.contains_key(&neighbor) {
            //
            for (e, h) in spawned_hexes.iter() {
                if h.hex == neighbor {
                    commands.entity(e).despawn();
                    let terrain_type = map_data.hexes.get(&neighbor).unwrap().hex_type.clone();
                    map_data.hexes.get_mut(&neighbor).unwrap().hex_tile_material =
                        get_tile_material2(neighbor, terrain_type, &map_data.hexes, tiles);
                    spawn_tile(
                        commands,
                        layout,
                        neighbor,
                        map_parent,
                        tiles,
                        map_resources,
                        map_data,
                        vtt_data,
                    );
                    break;
                }
            }
        }
    }
}

fn get_tile_material2(
    hex: Hex,
    terrain: TerrainType,
    map: &HashMap<hexx::Hex, PreparedHexTile>,
    tiles: &Res<HexMapTileMaterials>,
) -> Handle<TileMaterial> {
    // The prototype map is one big forest with a single dungeon in the middle:
    let is_rim = {
        let mut ret = false;
        for h in hex.all_neighbors() {
            if let Some(d) = map.get(&h) {
                if d.hex_type != terrain {
                    ret = true;
                    break;
                }
            } else {
                ret = true;
                break;
            }
        }
        ret
    };
    let materials = if is_rim {
        &tiles.terrain_rim_materials
    } else {
        &tiles.terrain_materials
    };
    if let Some(hex_data) = map.get(&hex) {
        let feature = &hex_data.feature;
        if feature == &HexFeature::Other || feature == &HexFeature::None {
            materials.get(&terrain).unwrap().clone()
        } else {
            tiles
                .terrain_feature_materials
                .get(&hex_data.hex_type)
                .unwrap()
                .get(feature)
                .unwrap()
                .clone()
        }
    } else {
        materials.get(&terrain).unwrap().clone()
    }
}

fn set_pen(mut editor: ResMut<MapEditor>, keyboard: Res<ButtonInput<KeyCode>>) {
    if keyboard.just_pressed(KeyCode::F1) {
        editor.pen = TerrainType::PlainsHex;
    }
    if keyboard.just_pressed(KeyCode::F2) {
        editor.pen = TerrainType::SwampsHex;
    }
    if keyboard.just_pressed(KeyCode::F3) {
        editor.pen = TerrainType::MountainsHex;
    }
}
