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

use std::collections::VecDeque;

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future},
    window::{CursorIcon, PrimaryWindow, SystemCursorIcon},
};
use hexroll3_cartographer::dungeons::map_data_providers;
use hexx::*;
use rand::seq::SliceRandom;
use serde_json::json;

use crate::{
    clients::{
        controller::{PostMapLoadedOp, RequestMapFromBackend},
        standalone::StandaloneSandbox,
    },
    hexmap::{
        data::{HexFeature, HexMetadata, TerrainType},
        editor::{Knob, get_tile_material},
        elements::*,
        tiles::HexMapTileMaterials,
    },
    hud::ShowTransientUserMessage,
    shared::{
        vtt::*,
        widgets::modal::{DiscreteAppState, ModalState},
    },
};

use hexroll3_scroll::generators::*;
use hexroll3_scroll::instance::*;

use super::MapEditor;

fn pick_terrain_to_remove(
    intent: &HashMap<TerrainType, i32>,
    map: &HexMapData,
    rng: &mut impl rand::Rng,
) -> Option<TerrainType> {
    let total_intent: i32 = intent.values().map(|k| k).sum();
    if total_intent == 0 {
        return None;
    }

    let mut region_sets: HashMap<&TerrainType, HashSet<i32>> = HashMap::new();
    for tile in map.hexes.values() {
        if !tile.generated && tile.pool_id != 0 {
            region_sets
                .entry(&tile.hex_type)
                .or_default()
                .insert(tile.pool_id);
        }
    }
    let total_regions: usize = region_sets.values().map(|s| s.len()).sum();
    if total_regions == 0 {
        return None;
    }

    // Find the terrain(s) most over-represented relative to intent.
    let mut best_gap = f32::NEG_INFINITY;
    let mut best_terrains: Vec<&TerrainType> = Vec::new();

    for (terrain, knob) in intent.iter() {
        if *knob == 0 {
            continue;
        }
        let intended = *knob as f32 / total_intent as f32;
        let actual =
            region_sets.get(terrain).map_or(0, |s| s.len()) as f32 / total_regions as f32;
        let gap = actual - intended;
        if gap > best_gap {
            best_gap = gap;
            best_terrains.clear();
            best_terrains.push(terrain);
        } else if (gap - best_gap).abs() < f32::EPSILON {
            best_terrains.push(terrain);
        }
    }

    if best_terrains.is_empty() {
        debug!("Best terrain is empty");
        return None;
    }
    Some((*best_terrains[rng.gen_range(0..best_terrains.len())]).clone())
}

fn staged_map_stays_connected(map: &HexMapData, removed: &HashSet<Hex>) -> bool {
    let remaining: HashSet<Hex> = map
        .hexes
        .iter()
        .filter(|(h, t)| !t.generated && !removed.contains(*h))
        .map(|(h, _)| *h)
        .collect();

    if remaining.is_empty() {
        return true;
    }

    let start = *remaining.iter().next().unwrap();
    let mut visited: HashSet<Hex> = HashSet::new();
    let mut queue: VecDeque<Hex> = VecDeque::new();
    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
        for neighbor in current.all_neighbors() {
            if remaining.contains(&neighbor) && !visited.contains(&neighbor) {
                visited.insert(neighbor);
                queue.push_back(neighbor);
            }
        }
    }

    visited.len() == remaining.len()
}

fn pick_frontier_region_to_remove(
    map: &HexMapData,
    terrain: &TerrainType,
    rng: &mut impl rand::Rng,
) -> Option<i32> {
    let mut pool_to_hexes: HashMap<i32, Vec<Hex>> = HashMap::new();
    for (hex, tile) in map.hexes.iter() {
        if !tile.generated && tile.pool_id != 0 && &tile.hex_type == terrain {
            pool_to_hexes.entry(tile.pool_id).or_default().push(*hex);
        }
    }

    let mut frontier_pools: Vec<i32> = pool_to_hexes
        .iter()
        .filter(|(_, hexes)| {
            hexes
                .iter()
                .any(|h| h.all_neighbors().iter().any(|n| !map.hexes.contains_key(n)))
        })
        .filter(|(_, hexes)| {
            let removed: HashSet<Hex> = hexes.iter().copied().collect();
            staged_map_stays_connected(map, &removed)
        })
        .map(|(pool_id, _)| *pool_id)
        .collect();

    if frontier_pools.is_empty() {
        return pool_to_hexes.keys().copied().max();
    }

    frontier_pools.shuffle(rng);
    Some(frontier_pools[0])
}

fn pick_terrain_by_intent(
    intent: &HashMap<TerrainType, i32>,
    map: &HexMapData,
    rng: &mut impl rand::Rng,
) -> TerrainType {
    let total_intent: i32 = intent.values().map(|k| k).sum();

    let mut region_sets: HashMap<&TerrainType, HashSet<i32>> = HashMap::new();
    for tile in map.hexes.values() {
        if !tile.generated && tile.pool_id != 0 {
            region_sets
                .entry(&tile.hex_type)
                .or_default()
                .insert(tile.pool_id);
        }
    }
    let total_regions: usize = region_sets.values().map(|s| s.len()).sum();

    let mut best_gap = f32::NEG_INFINITY;
    let mut best_terrains: Vec<&TerrainType> = Vec::new();

    for (terrain, knob) in intent.iter() {
        if *knob == 0 {
            continue;
        }
        let intended = *knob as f32 / total_intent as f32;
        let actual = if total_regions == 0 {
            0.0
        } else {
            region_sets.get(terrain).map_or(0, |s| s.len()) as f32 / total_regions as f32
        };
        let gap = intended - actual;
        if gap > best_gap {
            best_gap = gap;
            best_terrains.clear();
            best_terrains.push(terrain);
        } else if (gap - best_gap).abs() < f32::EPSILON {
            best_terrains.push(terrain);
        }
    }

    (*best_terrains[rng.gen_range(0..best_terrains.len())]).clone()
}

pub fn terrain_generator(
    mut map: &mut HexMapData,
    tiles: &Res<HexMapTileMaterials>,
    editor: &mut MapEditor,
    existing_pool_ids: &HashSet<i32>,
) {
    let current_region_count = existing_pool_ids.len() as i32;
    let mut next_pool_id = existing_pool_ids.iter().copied().max().unwrap_or(0) + 1;
    let regions_needed = editor.budget.target - current_region_count;
    for _ in 0..regions_needed {
        if place_next_region(&mut map, tiles, editor, next_pool_id) {
            next_pool_id += 1;
        }
    }
}

fn place_next_region(
    map: &mut HexMapData,
    tiles: &Res<HexMapTileMaterials>,
    editor: &mut MapEditor,
    next_pool_id: i32,
) -> bool {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let terrain = pick_terrain_by_intent(&editor.intent, map, &mut rng);
    let half_range = editor.volume.target.min(3).max(1);
    let count = rng.gen_range(
        editor.volume.target.max(1) - half_range..=editor.volume.target.max(1) + half_range,
    );
    let mut attempts = 0;
    loop {
        if place_region(
            map,
            tiles,
            terrain.clone(),
            count as usize,
            None,
            next_pool_id,
            attempts > 0,
        ) {
            editor.budget.current += 1;
            return true;
        }
        attempts += 1;
        if attempts > 10 {
            warn!("could not place region after 10 attempts, skipping");
            return false;
        }
    }
}

pub fn interactive_terrain_generator(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut map: ResMut<HexMapData>,
    mut vtt_data: ResMut<VttData>,
    tiles: Res<HexMapTileMaterials>,
    mut editor: ResMut<MapEditor>,
    spawned_hexes: Query<(Entity, &HexEntity)>,
) {
    let mut rng = rand::thread_rng();

    let existing_pool_ids: HashSet<i32> = map
        .hexes
        .values()
        .filter(|tile| !tile.generated && tile.pool_id != 0)
        .map(|tile| tile.pool_id)
        .collect();
    let current_region_count = existing_pool_ids.len() as i32;
    let regions_needed = editor.budget.target - current_region_count;

    let intent_is_configured =
        !editor.intent.is_empty() && editor.intent.values().any(|k| *k > 0);

    if regions_needed > 0 && intent_is_configured {
        let next_pool_id = existing_pool_ids.iter().copied().max().unwrap_or(0) + 1;
        if place_next_region(&mut map, &tiles, &mut editor, next_pool_id) {
            vtt_data.invalidate_map = true;
        }
    } else if regions_needed < 0 && intent_is_configured {
        let regions_to_remove = (-regions_needed) as usize;
        let mut removed_any = false;
        for _ in 0..regions_to_remove {
            if let Some(terrain) = pick_terrain_to_remove(&editor.intent, &map, &mut rng) {
                if let Some(pool_id) = pick_frontier_region_to_remove(&map, &terrain, &mut rng)
                {
                    let hexes_to_remove: Vec<Hex> = map
                        .hexes
                        .iter()
                        .filter(|(_, t)| t.pool_id == pool_id && !t.generated)
                        .map(|(h, _)| *h)
                        .collect();
                    for hex in hexes_to_remove {
                        map.hexes.remove(&hex);
                        for (e, c) in spawned_hexes.iter() {
                            if c.hex == hex {
                                commands.entity(e).try_despawn();
                                break;
                            }
                        }
                    }
                    removed_any = true;
                }
            }
            editor.budget.current -= 1;
            break;
        }
        if removed_any {
            vtt_data.invalidate_map = true;
        }
    }

    if keyboard.just_pressed(KeyCode::F1) {
        place_region(
            &mut map,
            &tiles,
            TerrainType::ForestHex,
            10,
            Some(Hex::ZERO),
            1,
            false,
        );
        vtt_data.invalidate_map = true;
    }
}

fn is_a_pool(map: &HexMapData, hex_to_check: Hex, extra_occupied: Option<Hex>) -> bool {
    let mut stack: Vec<Hex> = vec![hex_to_check];
    let mut tested: HashSet<Hex> = HashSet::new();
    let mut counter = 0usize;

    while let Some(empty_hex) = stack.pop() {
        if tested.contains(&empty_hex) {
            continue;
        }
        tested.insert(empty_hex);
        counter += 1;
        if counter > 100 {
            return false;
        }
        // FIXME: This should be changed
        if empty_hex.x > 1000
            || empty_hex.y > 1000
            || empty_hex.x < -1000
            || empty_hex.y < -1000
        {
            return false;
        }
        for neighbor in empty_hex.all_neighbors() {
            let occupied = map.hexes.contains_key(&neighbor)
                || extra_occupied.is_some_and(|e| e == neighbor);
            if !occupied {
                stack.push(neighbor);
            }
        }
    }
    true
}

fn num_of_occupied_neighbors(map: &HexMapData, hex: Hex) -> usize {
    hex.all_neighbors()
        .iter()
        .filter(|n| map.hexes.contains_key(*n))
        .count()
}

fn is_creating_a_hole(map: &HexMapData, empty_hex_to_check: Hex) -> bool {
    for neighbor in empty_hex_to_check.all_neighbors() {
        if map.hexes.contains_key(&neighbor) {
            continue;
        }
        if is_a_pool(map, neighbor, Some(empty_hex_to_check)) {
            return true;
        }
    }
    false
}

fn find_best_coords_to_map_from(map: &HexMapData, override_preference: bool) -> Hex {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();

    let mut coords_vec: Vec<Hex> = map
        .hexes
        .iter()
        .filter(|(_, tile)| !tile.generated)
        .map(|(hex, _)| *hex)
        .collect();
    coords_vec.shuffle(&mut rng);

    if override_preference {
        let mut candidates: Vec<Hex> = Vec::new();
        for coords in &coords_vec {
            for neighbor in coords.all_neighbors() {
                if !map.hexes.contains_key(&neighbor) && !is_creating_a_hole(map, neighbor) {
                    candidates.push(neighbor);
                }
            }
        }
        if let Some(&picked) = candidates.choose(&mut rng) {
            return picked;
        }
        // FIXME: This should be changed
        return Hex::new(map.cmax.x + 5, map.cmin.y + (map.cmax.y - map.cmin.y) / 2);
    }

    let mut dirs: [usize; 6] = [0, 1, 2, 3, 4, 5];
    dirs.shuffle(&mut rng);

    let mut best_so_far: Vec<Hex> = Vec::new();
    let mut backup: Vec<Hex> = Vec::new();
    let mut n_count = 1usize;

    for coords in &coords_vec {
        for &dir_idx in &dirs {
            let test = coords.all_neighbors()[dir_idx];

            if map.hexes.contains_key(&test) {
                continue;
            }
            if is_creating_a_hole(map, test) {
                continue;
            }

            let n_occ = num_of_occupied_neighbors(map, test);

            if n_occ >= n_count && dir_idx == 0 {
                if n_occ > n_count {
                    best_so_far.clear();
                }
                n_count = n_occ;
                best_so_far.push(test);
            }
            if n_occ >= n_count && dir_idx != 0 {
                backup.push(test);
            }
        }
    }

    if best_so_far.is_empty() && backup.is_empty() {
        // FIXME: This should be changed
        return Hex::new(map.cmax.x + 5, map.cmin.y + (map.cmax.y - map.cmin.y) / 2);
    }

    best_so_far.shuffle(&mut rng);
    backup.shuffle(&mut rng);

    if best_so_far.is_empty() {
        backup[0]
    } else {
        best_so_far[0]
    }
}

fn find_next_available_coords(map: &HexMapData, from: Hex) -> Option<Hex> {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    let mut dirs: [usize; 6] = [0, 1, 2, 3, 4, 5];
    dirs.shuffle(&mut rng);

    let mut best_options: Vec<Hex> = Vec::new();
    let mut n_count = 0usize;

    for &dir_idx in &dirs {
        let test = from.all_neighbors()[dir_idx];
        if map.hexes.contains_key(&test) {
            continue;
        }
        if is_creating_a_hole(map, test) {
            continue;
        }
        let n_occ = num_of_occupied_neighbors(map, test);
        if n_occ > n_count {
            best_options.clear();
            best_options.push(test);
            n_count = n_occ;
        } else if n_occ == n_count || rng.gen_range(0..=2) == 2 {
            best_options.push(test);
        }
    }

    if best_options.is_empty() {
        return None;
    }
    Some(best_options[rng.gen_range(0..best_options.len())])
}

fn place_region(
    map: &mut HexMapData,
    tiles: &Res<HexMapTileMaterials>,
    terrain: TerrainType,
    count: usize,
    override_start: Option<Hex>,
    pool_id: i32,
    o: bool,
) -> bool {
    let start = if map.hexes.is_empty() && override_start.is_none() {
        Hex::ZERO
    } else if let Some(pos) = override_start.filter(|p| !map.hexes.contains_key(p)) {
        pos
    } else if map.hexes.values().all(|t| t.generated) && !map.hexes.is_empty() {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let mut frontier: Vec<Hex> = Vec::new();
        for (hex, tile) in map.hexes.iter() {
            if tile.generated {
                for neighbor in hex.all_neighbors() {
                    if !map.hexes.contains_key(&neighbor) {
                        frontier.push(neighbor);
                    }
                }
            }
        }
        frontier.shuffle(&mut rng);
        frontier.into_iter().next().unwrap_or(Hex::ZERO)
    } else {
        find_best_coords_to_map_from(map, o)
    };

    let mut stack: Vec<Hex> = vec![start];
    let mut added: Vec<Hex> = Vec::new();

    for _ in 0..count {
        let mut found_next: Option<Hex> = None;
        while !stack.is_empty() {
            let from = *stack.last().unwrap();
            if let Some(next) = find_next_available_coords(map, from) {
                found_next = Some(next);
                break;
            } else {
                stack.pop();
            }
        }

        let coord = match found_next {
            Some(c) => c,
            None => {
                // warn!(
                //     "place_region: reattempting — ran out of placement slots after {} of {} hexes",
                //     added.len(),
                //     count
                // );
                for c in &added {
                    map.hexes.remove(c);
                }
                return false;
            }
        };

        let tile = PreparedHexTile {
            generated: false,
            pool_id,
            uid: "<uid>".to_string(),
            realm_uid: None,
            region_uid: None,
            hex_tile_material: get_tile_material(coord, terrain.clone(), &map.hexes, tiles),
            partial_hex_tile_material: get_tile_material(
                coord,
                terrain.clone(),
                &map.hexes,
                tiles,
            ),
            hex_type: terrain.clone(),
            tile_scale: 1.0,
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

        map.hexes.insert(coord, tile);
        added.push(coord);
        stack.push(coord);
    }

    true
}

#[derive(Resource)]
pub struct BackgroundMapGenerationTasks {
    pub tasks: Vec<Task<Option<Vec<(String, Hex)>>>>,
}

#[derive(Event)]
pub struct GenerateHexMap;

#[derive(Component)]
pub struct SpinnerText;

#[derive(Component)]
pub struct SpinnerNode;

pub fn generate_hex_map(
    _: On<GenerateHexMap>,
    sandbox: Res<StandaloneSandbox>,
    mut map: ResMut<HexMapData>,
    mut tasks: ResMut<BackgroundMapGenerationTasks>,
    editor: Res<MapEditor>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<DiscreteAppState>>,
    mut sub_state: ResMut<NextState<ModalState>>,
) {
    next_state.set(DiscreteAppState::Modal);
    sub_state.set(ModalState::Spinner);
    map.generating = true;
    commands
        .entity(*window)
        .insert(CursorIcon::System(SystemCursorIcon::Progress));

    commands
        .spawn((
            Name::new("SpinnerMessage"),
            SpinnerNode,
            Node {
                align_self: AlignSelf::Center,
                justify_self: JustifySelf::Center,
                ..default()
            },
            TextSpan::default(),
            TextColor::WHITE,
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextLayout {
                justify: Justify::Center,
                ..default()
            },
            Text::default(),
            ZIndex(3000),
        ))
        .with_child((
            SpinnerText,
            TextSpan::new("..."),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor::WHITE,
        ))
        .with_child((
            TextSpan::new("Press ESC to Cancel"),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor::WHITE,
        ));

    let realm_type = editor.realm_type.clone();
    let instance = sandbox.instance.clone();

    let (regions, num_hexes_to_generate) = partition_hexes_to_regions(&map);

    // detect features to generate
    let features_backlog: HashMap<Hex, HexFeature> = map
        .hexes
        .iter()
        .filter(|v| !v.1.generated && v.1.feature != HexFeature::None)
        .map(|v| (v.0.clone(), v.1.feature.clone()))
        .collect();
    let empties_backlog: HashSet<Hex> = map
        .hexes
        .iter()
        .filter(|v| !v.1.generated && v.1.feature == HexFeature::None)
        .map(|v| v.0.clone())
        .collect();

    let generation_tracker = GenerationWorkload::default();
    let hexes_to_generate = num_hexes_to_generate as i32;
    let mut hexes_generated = 0;
    if let Ok(mut message) = generation_tracker.message.lock() {
        *message = format!(
            "Generated {} out of {} hexes\n",
            hexes_generated, hexes_to_generate,
        );
    }
    commands.insert_resource(generation_tracker.clone());

    let task_pool = AsyncComputeTaskPool::get();
    let task = task_pool.spawn(async move {
        let generation_tracker = generation_tracker.clone();
        let mut ret: Vec<(String, Hex)> = Vec::new();
        let sid = instance.sid().unwrap();
        if instance
            .repo
            .mutate(|tx| {
                let mut rivers: Vec<(String, (i32, i32))> = Vec::new();

                let builder = SandboxBuilder::from_instance(&instance);
                let realm_uid = {
                    let Ok(mut blueprint) = builder.sandbox.blueprint.lock() else {
                        return anyhow::Result::Err(anyhow::anyhow!(
                            "Error trying to lock the sandbox blueprint"
                        ));
                    };
                    blueprint
                        .globals
                        .insert("realm_type".to_string(), json!([realm_type.clone()]));
                    let realm_uids =
                        append(&builder, &mut blueprint, tx, &sid, "realms", None, 1)?;
                    realm_uids.first().unwrap().clone()
                };
                for (terrain, region) in regions.iter() {
                    info!("Creating terrain type {}", terrain.as_region_str());
                    let region_uid = {
                        let Ok(mut blueprint) = builder.sandbox.blueprint.lock() else {
                            return anyhow::Result::Err(anyhow::anyhow!(
                                "Error trying to lock the sandbox blueprint"
                            ));
                        };
                        let region_uids = append(
                            &builder,
                            &mut blueprint,
                            tx,
                            &realm_uid,
                            "regions",
                            Some(terrain.as_region_str()),
                            1,
                        )?;
                        region_uids.first().unwrap().clone()
                    };
                    let hex_uids = {
                        let Ok(mut blueprint) = builder.sandbox.blueprint.lock() else {
                            return anyhow::Result::Err(anyhow::anyhow!(
                                "Error trying to lock the sandbox blueprint"
                            ));
                        };
                        let hex_uids = append(
                            &builder,
                            &mut blueprint,
                            tx,
                            &region_uid,
                            "Hexmap",
                            Some(terrain.as_str()),
                            region.len() as u32,
                        )?;
                        hex_uids
                    };
                    hexes_generated += region.len() as i32;
                    if let Ok(mut message) = generation_tracker.message.lock() {
                        *message = format!(
                            "Generated {} out of {} hexes\n",
                            hexes_generated, hexes_to_generate,
                        );
                    }

                    if let Ok(red_button) = generation_tracker.red_button.lock() {
                        if *red_button {
                            return anyhow::Result::Err(anyhow::anyhow!(
                                "Map generation aborted"
                            ));
                        }
                    }

                    for (hex, hex_uid) in region.iter().zip(hex_uids) {
                        {
                            let (x, y) = hexx_to_hexroll_coords(hex);
                            let patch = json!({
                                "coord_x": x,
                                "coord_y": y,
                                "$coords": {
                                    "x": x,
                                    "y": y
                                }
                            });
                            tx.patch(&hex_uid, &patch)?;
                            if *terrain == TerrainType::MountainsHex {
                                rivers.push((hex_uid.to_string(), (x, y)));
                            }
                        }
                        ret.push((hex_uid, *hex));
                    }
                }
                let mut hex_map = hexroll3_cartographer::hexmap::HexMap::new();
                hex_map.reconstruct_in_transaction(&sid, tx)?;
                hex_map.extend_existing_rivers(tx, &builder.randomizer)?;
                for (hex_uid, (x, y)) in rivers {
                    let coords = hexroll3_cartographer::hexmap::Hex::new(x, y);
                    if hex_map.can_start_river_from(&coords) {
                        if builder.randomizer.in_range(1, 4) == 2 {
                            hex_map.draw_river(tx, &builder.randomizer, coords, hex_uid)?;
                        }
                    }
                }
                hex_map.apply_layout(tx, &builder.randomizer)?;

                // Append features
                for r in ret.iter() {
                    let (xc, yc) = hexx_to_hexroll_coords(&r.1);
                    let display_coords = hexroll_coords_to_string(xc, yc);

                    if let Some(_task) = empties_backlog.get(&r.1) {
                        let Ok(mut blueprint) = builder.sandbox.blueprint.lock() else {
                            return anyhow::Result::Err(anyhow::anyhow!(
                                "Error trying to lock the sandbox blueprint"
                            ));
                        };
                        if let Ok(mut message) = generation_tracker.message.lock() {
                            *message = format!(
                                "Populating hex {} with a random feature\n",
                                display_coords
                            );
                        }
                        let _uids =
                            append(&builder, &mut blueprint, tx, &r.0, "Feature", None, 1)?;
                    }
                    if let Some(task) = features_backlog.get(&r.1) {
                        if let Ok(mut message) = generation_tracker.message.lock() {
                            *message = format!(
                                "Populating hex {} with a {}\n",
                                display_coords,
                                match task {
                                    HexFeature::Dungeon => "Dungeon",
                                    HexFeature::Inn => "Inn",
                                    HexFeature::Residency => "Residency",
                                    HexFeature::City => "City",
                                    HexFeature::Town => "Town",
                                    HexFeature::Village => "Village",
                                    _ => unreachable!(),
                                }
                            );
                        }

                        let Ok(mut blueprint) = builder.sandbox.blueprint.lock() else {
                            return anyhow::Result::Err(anyhow::anyhow!(
                                "Error trying to lock the sandbox blueprint"
                            ));
                        };
                        blueprint.map_data_provider = map_data_providers();
                        tx.invalidate(&r.0)?;
                        let uids = append(
                            &builder,
                            &mut blueprint,
                            tx,
                            &r.0,
                            match task {
                                HexFeature::Dungeon => "Dungeon",
                                HexFeature::Inn => "Inn",
                                HexFeature::Residency => "Residency",
                                HexFeature::City => "Settlement",
                                HexFeature::Town => "Settlement",
                                HexFeature::Village => "Settlement",
                                _ => unreachable!(),
                            },
                            Some(match task {
                                HexFeature::Dungeon => "Dungeon",
                                HexFeature::Inn => "Inn",
                                HexFeature::Residency => "Residency",
                                HexFeature::City => "City",
                                HexFeature::Town => "Town",
                                HexFeature::Village => "Village",
                                _ => unreachable!(),
                            }),
                            1,
                        )?;
                        let Some(uid) = uids.first() else {
                            return Err(anyhow::anyhow!(
                                "Something went wrong with appending"
                            ));
                        };
                        let entity = tx.load(&uid)?;
                        // TODO: This is a duplication of the manual append logic.
                        if let Some(on_roll) = entity.get("$on_roll") {
                            if on_roll == "roll_settlement_map" {
                                let builder = SandboxBuilder::from_instance(&instance);
                                hexroll3_cartographer::watabou::map_settlement(
                                    tx,
                                    &builder.randomizer,
                                    &mut hex_map,
                                    &r.0,
                                )?;
                            }
                        }
                    }
                }
                hex_map.stage_trails(tx)?;

                Ok(())
            })
            .map_err(|err| error!("{}", err.to_string()))
            .is_err()
        {
            return None;
        }
        Some(ret)
    });
    tasks.tasks.push(task);
}

pub fn poll_generation_tasks(
    mut commands: Commands,
    mut tasks: ResMut<BackgroundMapGenerationTasks>,
    mut map: ResMut<HexMapData>,
    mut next_state: ResMut<NextState<DiscreteAppState>>,
    mut next_tool_state: ResMut<NextState<HexMapToolState>>,
    temp: Option<Res<GenerationWorkload>>,
    mut spinner_text: Single<&mut TextSpan, With<SpinnerText>>,
    spinner_node: Single<Entity, With<SpinnerNode>>,
) {
    if let Some(t) = temp {
        if let Ok(message) = t.message.lock() {
            spinner_text.0 = message.to_string();
        }
    }
    let tasks = &mut tasks.tasks;
    tasks.retain_mut(|task| {
        let status = block_on(future::poll_once(task));
        let retain = status.is_none();
        if let Some(batch) = status {
            if let Some(result) = batch {
                for (hex_uid, hex) in result.iter() {
                    {
                        map.coords.insert(hex_uid.clone(), *hex);
                    }
                    {
                        let hex_data = map.hexes.get_mut(hex).unwrap();
                        hex_data.uid = hex_uid.to_string();
                    }
                }
                map.generating = false;
                commands.trigger(RequestMapFromBackend {
                    post_map_loaded_op: PostMapLoadedOp::InvalidateVisible,
                });
                next_tool_state.set(HexMapToolState::Selection);
            } else {
                commands.trigger(ShowTransientUserMessage {
                    text: String::from("The stars were not aligned. Please try saving again."),
                    special: None,
                    keep_alive: None,
                });
                map.generating = false;
            }
            commands.entity(*spinner_node).try_despawn();
            commands.remove_resource::<GenerationWorkload>();
            next_state.set(DiscreteAppState::Normal);
        }
        retain
    });
}

#[derive(Resource, Default, Clone)]
pub struct GenerationWorkload {
    message: std::sync::Arc<std::sync::Mutex<String>>,
    red_button: std::sync::Arc<std::sync::Mutex<bool>>,
}

pub fn detect_abort(
    keyboard: Res<ButtonInput<KeyCode>>,
    progress: Option<ResMut<GenerationWorkload>>,
) {
    if let Some(progress) = progress {
        if keyboard.pressed(KeyCode::Escape) {
            if let Ok(mut holder) = progress.red_button.lock() {
                *holder = true;
            }
        }
    }
}

fn partition_hexes_to_regions(map: &HexMapData) -> (Vec<(TerrainType, Vec<Hex>)>, usize) {
    let mut new_coords: Vec<(Hex, TerrainType)> = Vec::new();
    let mut coord_to_terrain: HashMap<Hex, TerrainType> = HashMap::new();
    let mut coord_to_pool_id: HashMap<Hex, i32> = HashMap::new();
    let mut unallocated_coords: HashSet<Hex> = HashSet::new();
    for (coords, hex) in map.hexes.iter() {
        if hex.uid == "<uid>" {
            new_coords.push((*coords, hex.hex_type.clone()));
            coord_to_terrain.insert(*coords, hex.hex_type.clone());
            coord_to_pool_id.insert(*coords, hex.pool_id);
            unallocated_coords.insert(*coords);
        }
    }

    let mut regions: Vec<(TerrainType, Vec<Hex>)> = Vec::new();

    for (hex, terrain) in new_coords.iter() {
        if unallocated_coords.contains(hex) {
            let mut pool_backlog: VecDeque<Hex> = VecDeque::new();
            let mut pool_processed: HashSet<Hex> = HashSet::new();
            let mut region: Vec<Hex> = Vec::new();
            pool_backlog.push_front(*hex);

            while !pool_backlog.is_empty() {
                let current = pool_backlog.pop_front().unwrap();
                if coord_to_terrain.get(&current).unwrap() == terrain {
                    unallocated_coords.remove(&current);
                    region.push(current);
                    for neighbor in current.all_neighbors() {
                        if unallocated_coords.contains(&neighbor)
                            && !pool_processed.contains(&neighbor)
                        {
                            // Hexes with distinct non-zero pool_ids are forced
                            // into separate regions even if they share a terrain type.
                            let current_pool =
                                coord_to_pool_id.get(&current).copied().unwrap_or(0);
                            let neighbor_pool =
                                coord_to_pool_id.get(&neighbor).copied().unwrap_or(0);
                            let same_pool = current_pool == 0
                                || neighbor_pool == 0
                                || current_pool == neighbor_pool;
                            if !same_pool {
                                continue;
                            }
                            pool_backlog.push_front(neighbor);
                            pool_processed.insert(neighbor);
                        }
                    }
                }
            }
            regions.push((terrain.clone(), region));
        }
    }
    (regions, new_coords.len())
}

pub fn tune_editor_for_realm_type(editor: &mut MapEditor, realm_type: &str) {
    editor.realm_type = format!("RealmType{}", realm_type);
    match realm_type {
        "Lands" => {
            editor.budget.target = 20;
            editor.volume.target = 15;
            editor.intent.insert(TerrainType::JungleHex, 10);
            editor.intent.insert(TerrainType::SwampsHex, 3);
            editor.intent.insert(TerrainType::MountainsHex, 7);
            editor.intent.insert(TerrainType::PlainsHex, 5);
            editor.intent.insert(TerrainType::TundraHex, 0);
            editor.intent.insert(TerrainType::ForestHex, 0);
            editor.intent.insert(TerrainType::DesertHex, 4);
            editor.knobs.insert(HexFeature::Dungeon, Knob::from(20));
            editor.knobs.insert(HexFeature::City, Knob::from(0));
            editor.knobs.insert(HexFeature::Town, Knob::from(2));
            editor.knobs.insert(HexFeature::Village, Knob::from(10));
            editor.knobs.insert(HexFeature::Inn, Knob::from(6));
            editor.knobs.insert(HexFeature::Residency, Knob::from(15));
        }
        "Kingdom" => {
            editor.budget.target = 17;
            editor.volume.target = 12;
            editor.intent.insert(TerrainType::JungleHex, 0);
            editor.intent.insert(TerrainType::SwampsHex, 3);
            editor.intent.insert(TerrainType::MountainsHex, 6);
            editor.intent.insert(TerrainType::PlainsHex, 10);
            editor.intent.insert(TerrainType::TundraHex, 1);
            editor.intent.insert(TerrainType::ForestHex, 7);
            editor.intent.insert(TerrainType::DesertHex, 0);
            editor.knobs.insert(HexFeature::Dungeon, Knob::from(16));
            editor.knobs.insert(HexFeature::City, Knob::from(2));
            editor.knobs.insert(HexFeature::Town, Knob::from(5));
            editor.knobs.insert(HexFeature::Village, Knob::from(10));
            editor.knobs.insert(HexFeature::Inn, Knob::from(6));
            editor.knobs.insert(HexFeature::Residency, Knob::from(10));
        }
        "Empire" => {
            editor.budget.target = 27;
            editor.volume.target = 17;
            editor.intent.insert(TerrainType::JungleHex, 4);
            editor.intent.insert(TerrainType::SwampsHex, 3);
            editor.intent.insert(TerrainType::MountainsHex, 6);
            editor.intent.insert(TerrainType::PlainsHex, 8);
            editor.intent.insert(TerrainType::TundraHex, 2);
            editor.intent.insert(TerrainType::ForestHex, 7);
            editor.intent.insert(TerrainType::DesertHex, 3);
            editor.knobs.insert(HexFeature::Dungeon, Knob::from(23));
            editor.knobs.insert(HexFeature::City, Knob::from(5));
            editor.knobs.insert(HexFeature::Town, Knob::from(10));
            editor.knobs.insert(HexFeature::Village, Knob::from(15));
            editor.knobs.insert(HexFeature::Inn, Knob::from(9));
            editor.knobs.insert(HexFeature::Residency, Knob::from(15));
        }
        "Duchy" => {
            editor.budget.target = 12;
            editor.volume.target = 8;
            editor.intent.insert(TerrainType::JungleHex, 0);
            editor.intent.insert(TerrainType::SwampsHex, 0);
            editor.intent.insert(TerrainType::MountainsHex, 6);
            editor.intent.insert(TerrainType::PlainsHex, 7);
            editor.intent.insert(TerrainType::TundraHex, 2);
            editor.intent.insert(TerrainType::ForestHex, 7);
            editor.intent.insert(TerrainType::DesertHex, 0);
            editor.knobs.insert(HexFeature::Dungeon, Knob::from(10));
            editor.knobs.insert(HexFeature::City, Knob::from(1));
            editor.knobs.insert(HexFeature::Town, Knob::from(3));
            editor.knobs.insert(HexFeature::Village, Knob::from(10));
            editor.knobs.insert(HexFeature::Inn, Knob::from(3));
            editor.knobs.insert(HexFeature::Residency, Knob::from(5));
        }
        _ => {}
    }
}
