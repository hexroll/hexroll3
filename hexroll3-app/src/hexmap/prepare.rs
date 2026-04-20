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

// Hexmap Data Staging
//                   |
//                   V
// +-------+     +-------+     +-------+
// | setup | --> | stage | --> | spawn |
// +-------+     +-------+     +-------+
//     ^             ^             ^
//     |             |             |
// [Startup]    [Map Loaded]   [Each Frame]
//
use bevy::{platform::collections::HashSet, prelude::*};

use bevy::platform::collections::hash_map::HashMap;

use hexx::{DoubledHexMode, Hex};

use crate::{
    clients::{
        controller::{PostMapLoadedOp, PostMapLoadedOpPrefix},
        model::{FetchEntityReason, SandboxMode},
    },
    hexmap::{curve_tiles::CurvedMeshTileSet, data::*, elements::*, tiles::*},
    shared::{geometry::*, labels::MapLabel, poly::polylabel},
    tokens::Token,
};

/// Prepares the hex map data for rendering within a separate task.
///
/// # Parameters
///
/// * `map`: A copy of the `HexMapJson` structure containing the hex map data.
/// * `curved_mesh_tile_set`: A copy of the `RiverTileSet` structure with river tile information.
/// * `tiles`: A copy of the `HexMapTileMaterials` structure with hex map tile materials.
///
/// # Returns
///
/// An instance of `HexMapData` representing the prepared hex map data.
pub fn prepare_hex_map_data(
    map: HexMapJson,
    curved_mesh_tile_set: CurvedMeshTileSet,
    tiles: HexMapTileMaterials,
    scale_calculator: (f32, f32),
) -> HexMapData {
    let layout = hexmap_layout();

    let preprocessed_hexes = map
        .map
        .iter()
        .map(|h| {
            let y = -h.y;
            let x = h.x * 2 + (h.y.abs() % 2);
            let coords = Hex::from_doubled_coordinates([x, y], DoubledHexMode::DoubledHeight);
            (coords, h.clone())
        })
        .collect();

    let mut regions: HashMap<String, Vec<Hex>> = HashMap::new();

    let mut realms: HashMap<String, Vec<Hex>> = HashMap::new();

    let mut coords_by_uid: HashMap<String, Hex> = HashMap::new();

    let mut hex_bounds_min = Hex::new(10000, 10000);
    let mut hex_bounds_max = Hex::new(-10000, -10000);

    let hexes = map
        .map
        .iter()
        .map(|h| -> (Hex, PreparedHexTile) {
            let y = -h.y;
            let x = h.x * 2 + (h.y.abs() % 2);
            let coords = Hex::from_doubled_coordinates([x, y], DoubledHexMode::DoubledHeight);

            // NOTE: We set the map bounds for the initial camera frame while considering
            // only land hexes
            if h.hex_type != TerrainType::OceanHex {
                if coords.x < hex_bounds_min.x {
                    hex_bounds_min.x = coords.x
                }
                if coords.x > hex_bounds_max.x {
                    hex_bounds_max.x = coords.x
                }
                if coords.y < hex_bounds_min.y {
                    hex_bounds_min.y = coords.y
                }
                if coords.y > hex_bounds_max.y {
                    hex_bounds_max.y = coords.y
                }
            }

            if let Some(region) = h.region.as_ref() {
                regions.entry(region.to_string()).or_default().push(coords);
            }

            if let Some(realm) = h.realm.as_ref() {
                realms.entry(realm.to_string()).or_default().push(coords);
            }

            let get_curved_tile = |a: i32, b: i32| {
                let (a, mut b) = (a, b);
                if a > b {
                    b += 6;
                }
                let diff = (a - b).abs();
                let tile = match diff {
                    5 => curved_mesh_tile_set.north_to_north_west.clone(),
                    4 => curved_mesh_tile_set.north_to_south_west.clone(),
                    3 => curved_mesh_tile_set.north_to_south.clone(),
                    2 => curved_mesh_tile_set.north_to_south_east.clone(),
                    1 => curved_mesh_tile_set.north_to_north_east.clone(),
                    _ => unreachable!(),
                };
                (a, tile)
            };

            let get_curved_mesh_tile_stack = |indices: &Option<Vec<u8>>| {
                if let Some(trails) = indices {
                    let mut ret = Vec::new();

                    if trails.len() == 1 {
                        ret.push((trails[0] as i32, curved_mesh_tile_set.to_north.clone()));
                    }
                    if trails.len() > 1 {
                        ret.push(get_curved_tile(trails[0] as i32, trails[1] as i32));
                    }
                    if trails.len() == 3 {
                        ret.push((trails[2] as i32, curved_mesh_tile_set.to_north.clone()));
                    }
                    if trails.len() == 4 {
                        ret.push(get_curved_tile(trails[2] as i32, trails[3] as i32));
                    }
                    if ret.is_empty() { None } else { Some(ret) }
                } else {
                    None
                }
            };

            let river_tile = get_curved_mesh_tile_stack(&h.rivers);
            let trail_tile = get_curved_mesh_tile_stack(&h.trails);

            coords_by_uid.insert(h.uuid.clone(), coords);

            let (hex_tile_material, _) =
                get_tile_material(coords, &preprocessed_hexes, &tiles, false);
            let (partial_hex_tile_material, is_rim) =
                get_tile_material(coords, &preprocessed_hexes, &tiles, true);
            (
                coords,
                PreparedHexTile {
                    uid: h.uuid.clone(),
                    generated: true,
                    pool_id: 0,
                    hex_type: h.hex_type.clone(),
                    hex_tile_material,
                    partial_hex_tile_material,
                    tile_scale: if is_rim {
                        scale_calculator.0
                    } else {
                        scale_calculator.1
                    },
                    river_tile,
                    trail_tile,
                    feature: h.feature.clone().unwrap_or(HexFeature::None),
                    metadata: HexMetadata {
                        harbor: h.feature.as_ref().and_then(|feature| {
                            if feature.can_have_a_harbor() {
                                h.harbor.clone()
                            } else {
                                None
                            }
                        }),
                        river_dir: h.river_dir.unwrap_or(0.0).to_radians(),
                        offset: h
                            .feature
                            .as_ref()
                            .map(|feature| feature.harbor_offset())
                            .unwrap_or(0.0),
                        is_rim,
                    },
                },
            )
        })
        .collect();

    let normalize = |count: &HashMap<String, usize>| -> HashMap<String, f32> {
        let (min, max) = count
            .values()
            .fold((usize::MAX, usize::MIN), |(min, max), &v| {
                (min.min(v), max.max(v))
            });

        let range = (max - min) as f32;
        if range == 0.0 {
            return count.keys().map(|k| (k.clone(), 1.0)).collect();
        }

        count
            .iter()
            .map(|(k, &v)| (k.clone(), (v as f32 - min as f32) / range))
            .collect()
    };

    let (normalized_region_count, normalized_realm_count) = {
        let mut count = (HashMap::new(), HashMap::new());
        for h in &map.map {
            if let Some(region) = h.region.as_ref() {
                *count.0.entry(region.clone()).or_insert(0) += 1;
            }
            if let Some(realm) = h.realm.as_ref() {
                *count.1.entry(realm.clone()).or_insert(0) += 1;
            }
        }
        (normalize(&count.0), normalize(&count.1))
    };

    let region_labels: Vec<LazySpawn<(String, Vec2, f32)>> = regions
        .iter()
        .map(|(k, v)| {
            let polygon_points = make_polygon(&layout, v);

            let j: Vec<Vec<f64>> = polygon_points
                .iter()
                .rev()
                .map(|v| vec![v.x as f64, v.y as f64])
                .collect();
            let test = polylabel(vec![j], 1.0);

            let label_position = Vec2::new(test[0] as f32, test[1] as f32);
            LazySpawn::from((
                map.regions.get(k).unwrap().clone(),
                Vec2::new(label_position.x as f32, label_position.y as f32),
                *normalized_region_count.get(k).unwrap_or(&0.0),
            ))
        })
        .collect();

    let realm_labels: Vec<LazySpawn<(String, Vec2, f32)>> = realms
        .iter()
        .map(|(k, v)| {
            let polygon_points = make_polygon(&layout, &biggest_island(v, &hexes));
            let j: Vec<Vec<f64>> = polygon_points
                .iter()
                .rev()
                .map(|v| vec![v.x as f64, v.y as f64])
                .collect();
            let test = polylabel(vec![j], 1.0);

            let label_position = Vec2::new(test[0] as f32, test[1] as f32);
            LazySpawn::from((
                map.realms.get(k).unwrap().name.clone(),
                Vec2::new(label_position.x as f32, label_position.y as f32),
                *normalized_realm_count.get(k).unwrap_or(&0.0),
            ))
        })
        .collect();
    HexMapData {
        cmin: Hex::ZERO,
        cmax: Hex::ZERO,
        center: hex_bounds_min + (hex_bounds_max - hex_bounds_min) / 2,
        hexes,
        coords: coords_by_uid,
        region_labels,
        realm_labels,
        cursor: None,
        selected: None,
        generating: false,
    }
}

fn get_tile_material(
    hex: Hex,
    map: &HashMap<hexx::Hex, MapHex>,
    tiles: &HexMapTileMaterials,
    partial: bool,
) -> (Handle<TileMaterial>, bool) {
    let use_rim_for_rivers = tiles.use_rim_for_rivers;
    if map.contains_key(&hex) {
        let hex_data = map.get(&hex).unwrap();
        let is_river = hex_data.rivers.is_some();
        let is_rim = {
            let mut ret = false;
            for h in hex.all_neighbors() {
                if let Some(d) = map.get(&h) {
                    if d.hex_type != hex_data.hex_type {
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
        let materials = if is_rim || (is_river && use_rim_for_rivers) {
            &tiles.terrain_rim_materials
        } else {
            &tiles.terrain_materials
        };
        if let Some(feature) = &hex_data.feature {
            if feature == &HexFeature::Other || partial {
                (materials.get(&hex_data.hex_type).unwrap().clone(), is_rim)
            } else {
                (
                    tiles
                        .terrain_feature_materials
                        .get(&hex_data.hex_type)
                        .unwrap()
                        .get(feature)
                        .unwrap()
                        .clone(),
                    is_rim,
                )
            }
        } else {
            (materials.get(&hex_data.hex_type).unwrap().clone(), is_rim)
        }
    } else {
        unreachable!()
    }
}

pub fn post_map_loaded_handler(
    trigger: On<PostMapLoadedOp>,
    mut commands: Commands,
    visible_hexes: Query<Entity, With<HexEntity>>,
    map_data: Res<HexMapData>,
    all_labels: Query<Entity, With<MapLabel>>,
    mut camera: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    mut next_hex_map_updater_state: ResMut<NextState<HexMapSpawnerState>>,
) {
    match trigger.event() {
        PostMapLoadedOp::None => {}
        PostMapLoadedOp::Initialize(_) => {
            visible_hexes
                .iter()
                .for_each(|e| commands.entity(e).despawn());
            all_labels.iter().for_each(|e| commands.entity(e).despawn());
            for (mut ct, mut cp) in camera.iter_mut() {
                map_data.center_camera_on_map(&mut ct, &mut cp);
            }
            next_hex_map_updater_state.set(HexMapSpawnerState::Enabled);
        }
        PostMapLoadedOp::InvalidateVisible => {
            visible_hexes
                .iter()
                .for_each(|e| commands.entity(e).despawn());
            all_labels.iter().for_each(|e| commands.entity(e).despawn());
            next_hex_map_updater_state.set(HexMapSpawnerState::Enabled);
        }
        PostMapLoadedOp::FetchEntity(uid) => {
            // FIXME: despawning all labels might be an overkill here.
            all_labels.iter().for_each(|e| commands.entity(e).despawn());
            commands.trigger(FetchEntityFromStorage {
                uid: uid.to_string(),
                anchor: None,
                why: FetchEntityReason::SandboxLink,
            });
        }
    }
}

pub fn post_map_loaded_handler_prefix(
    trigger: On<PostMapLoadedOpPrefix>,
    mut commands: Commands,
    mut next_hex_map_updater_state: ResMut<NextState<HexMapSpawnerState>>,
    mut next_vtt_data_state: ResMut<NextState<VttDataState>>,
    visible_hexes: Query<Entity, With<HexEntity>>,
    all_labels: Query<Entity, With<MapLabel>>,
    all_tokens: Query<Entity, With<Token>>,
) {
    match &trigger.event().post_map_op {
        PostMapLoadedOp::Initialize(sandbox_mode) => {
            next_hex_map_updater_state.set(HexMapSpawnerState::Inhibited);
            match sandbox_mode {
                SandboxMode::Player => {}
                SandboxMode::Referee => next_vtt_data_state.set(VttDataState::Loading),
            }
            all_tokens.iter().for_each(|e| commands.entity(e).despawn());
            all_labels.iter().for_each(|e| commands.entity(e).despawn());
            visible_hexes
                .iter()
                .for_each(|e| commands.entity(e).despawn());
        }
        _ => {}
    }
}

pub fn group_by_islands(
    hex_list: &Vec<Hex>,
    hexes: &HashMap<Hex, PreparedHexTile>,
) -> Vec<Vec<Hex>> {
    let mut allocated_coords: HashSet<Hex> = HashSet::new();

    let mut islands: Vec<Vec<Hex>> = Vec::new();

    for hex in hex_list.iter() {
        if !allocated_coords.contains(hex) {
            let mut pool_backlog: Vec<Hex> = Vec::new();
            let mut pool_processed: HashSet<Hex> = HashSet::new();
            let mut island: Vec<Hex> = Vec::new();
            pool_backlog.push(*hex);

            while !pool_backlog.is_empty() {
                let current = pool_backlog.pop().unwrap();
                island.push(current);
                allocated_coords.insert(current);
                for neighbor in current.all_neighbors() {
                    if hexes.contains_key(&neighbor)
                        && !pool_processed.contains(&neighbor)
                        && !allocated_coords.contains(&neighbor)
                    {
                        pool_backlog.push(neighbor);
                        pool_processed.insert(neighbor);
                    }
                }
            }
            islands.push(island);
        }
    }
    islands.sort_by_key(|island| island.len());
    islands
}

fn biggest_island(hex_list: &Vec<Hex>, hexes: &HashMap<Hex, PreparedHexTile>) -> Vec<Hex> {
    group_by_islands(hex_list, hexes).last().unwrap().clone()
}
