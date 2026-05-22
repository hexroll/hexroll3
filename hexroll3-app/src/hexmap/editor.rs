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
    window::{CursorGrabMode, CursorIcon, CursorOptions, PrimaryWindow, SystemCursorIcon},
};
use hexroll3_cartographer::dungeons::map_data_providers;
use hexx::*;
use rand::seq::{SliceRandom, index::sample};
use serde_json::json;

use crate::{
    clients::{
        controller::{PostMapLoadedOp, RequestMapFromBackend},
        standalone::StandaloneSandbox,
    },
    content::{ContentMode, ThemeBackgroundColor},
    hexmap::{
        data::{HexFeature, HexMetadata, TerrainType},
        daynight::HexMapTime,
        elements::*,
        spawn::spawn_tile,
        tiles::{HexMapTileMaterials, TileMaterial},
    },
    shared::{
        vtt::*,
        widgets::{
            cursor::{PointerExclusivityIsPreferred, PointerOnHover, TooltipOnHover},
            link::ContentHoverLink,
            modal::{DiscreteAppState, ModalState},
        },
    },
};

use hexroll3_scroll::generators::*;
use hexroll3_scroll::instance::*;

use super::HexmapTheme;

pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BackgroundMapGenerationTasks { tasks: Vec::new() });
        app.insert_resource(MapEditor {
            pen: PenType::Brush,
            terrain: TerrainType::ForestHex,
            realm_type: "RealmTypeKingdom".to_string(),
            knobs: HashMap::new(),
            selected_feature: HexFeature::Dungeon,
        })
        .add_systems(OnEnter(HexMapToolState::Edit), create_drawing_hud)
        .add_systems(OnExit(HexMapToolState::Edit), destroy_drawing_hud)
        .add_systems(Update, extend_seeds.run_if(in_state(HexMapToolState::Edit)))
        .add_systems(Update, add_features.run_if(in_state(HexMapToolState::Edit)))
        .add_observer(on_add_features)
        .add_observer(on_del_features)
        .add_systems(
            Update,
            (detect_abort, poll_generation_tasks)
                .run_if(in_state(HexMapToolState::Edit))
                .run_if(resource_exists::<GenerationWorkload>),
        )
        .add_systems(Update, draw_tiles.run_if(in_state(HexMapToolState::Edit)))
        .add_systems(
            Update,
            refresh_neighbors
                .run_if(in_state(HexMapToolState::Edit))
                .after(draw_tiles),
        )
        .add_observer(generate_hex_map);
    }
}

#[derive(Clone, PartialEq)]
pub enum PenType {
    Pencil,
    Brush,
    Eraser,
    Broom,
    FeaturePen,
}

impl PenType {
    fn show_terrain_bar(&self) -> bool {
        *self == PenType::Pencil || *self == PenType::Brush
    }

    fn show_feature_bar(&self) -> bool {
        *self == PenType::FeaturePen
    }
}

#[derive(Default)]
pub struct Knob {
    target: i32,
    current: i32,
}

#[derive(Resource)]
pub struct MapEditor {
    pub pen: PenType,
    pub terrain: TerrainType,
    pub realm_type: String,
    pub knobs: HashMap<HexFeature, Knob>,
    pub selected_feature: HexFeature,
}

fn get_feature_ratio_for_realm_type(realm_type: &str, feature_type: HexFeature) -> f32 {
    if realm_type == "RealmTypeKingdom" {
        return match feature_type {
            HexFeature::Dungeon => 0.5,
            HexFeature::Residency => 0.4,
            HexFeature::Village => 0.3,
            HexFeature::Inn => 0.2,
            HexFeature::Town => 0.2,
            HexFeature::City => 0.1,
            _ => 0.0,
        };
    }
    if realm_type == "RealmTypeEmpire" {
        return match feature_type {
            HexFeature::Dungeon => 0.8,
            HexFeature::Residency => 0.8,
            HexFeature::Village => 0.5,
            HexFeature::Inn => 0.3,
            HexFeature::Town => 0.3,
            HexFeature::City => 0.2,
            _ => 0.0,
        };
    }
    if realm_type == "RealmTypeLands" {
        return match feature_type {
            HexFeature::Dungeon => 0.8,
            HexFeature::Residency => 0.3,
            HexFeature::Village => 0.5,
            HexFeature::Inn => 0.2,
            HexFeature::Town => 0.1,
            HexFeature::City => 0.05,
            _ => 0.0,
        };
    }
    if realm_type == "RealmTypeDuchy" {
        return match feature_type {
            HexFeature::Dungeon => 0.5,
            HexFeature::Residency => 0.4,
            HexFeature::Village => 0.4,
            HexFeature::Inn => 0.2,
            HexFeature::Town => 0.1,
            HexFeature::City => 0.05,
            _ => 0.0,
        };
    }
    return 0.0;
}

pub fn random_neighboring_hexes(coords: Hex) -> Vec<Hex> {
    const TARGET: usize = 3;

    let mut rng = rand::thread_rng();

    let mut candidates: Vec<Hex> = coords.all_neighbors().to_vec();
    candidates.push(coords);
    candidates.shuffle(&mut rng);

    candidates.into_iter().take(TARGET).collect::<Vec<_>>()
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
    theme: Res<HexmapTheme>,
) {
    let scale_calculator = theme.tile_scale_values();
    let layout = y_inverted_hexmap_layout();
    let mut paint_coords =
        |map: &mut ResMut<HexMapData>, mut coords: Vec<Hex>, terrain_type: &TerrainType| {
            for coord in coords.clone().iter() {
                for coord_to_test in coord.all_neighbors() {
                    let creating_hole =
                        coord_to_test
                            .all_neighbors()
                            .to_vec()
                            .iter()
                            .all(|neighbor| {
                                map.hexes.contains_key(neighbor) || coords.contains(neighbor)
                            });
                    if creating_hole {
                        coords.push(coord_to_test);
                    }
                }
            }
            for coord in coords {
                if !map.hexes.contains_key(&coord) {
                    let new_hex = PreparedHexTile {
                        generated: false,
                        pool_id: 0,
                        uid: "<uid>".to_string(),
                        realm_uid: None,
                        region_uid: None,
                        hex_type: terrain_type.clone(),
                        hex_tile_material: get_tile_material(
                            coord,
                            terrain_type.clone(),
                            &map.hexes,
                            &tiles,
                        ),
                        partial_hex_tile_material: get_tile_material(
                            coord,
                            terrain_type.clone(),
                            &map.hexes,
                            &tiles,
                        ),
                        tile_scale: scale_calculator.0,
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
                        map,
                        &mut vtt_data,
                    );
                }
            }
        };

    if let Some(coord) = map.selected {
        match &editor.pen {
            PenType::Pencil => {
                if click.pressed(MouseButton::Right) && !map.hexes.contains_key(&coord) {
                    paint_coords(&mut map, vec![coord], &editor.terrain);
                }
            }
            PenType::Brush => {
                if click.pressed(MouseButton::Right) && !map.hexes.contains_key(&coord) {
                    paint_coords(&mut map, random_neighboring_hexes(coord), &editor.terrain);
                }
            }
            PenType::Eraser => {
                if click.pressed(MouseButton::Right) && map.hexes.contains_key(&coord) {
                    if let Some(uid) = map.get_selected_uid()
                        && uid == "<uid>"
                    {
                        map.hexes.remove(&coord);
                        for (e, c) in spawned_hexes.iter() {
                            if c.hex == coord {
                                commands.entity(e).try_despawn();
                            }
                        }
                    }
                }
            }
            PenType::Broom => {
                if click.pressed(MouseButton::Right) && map.hexes.contains_key(&coord) {
                    if let Some(uid) = map.get_selected_uid()
                        && uid == "<uid>"
                    {
                        if let Some(data) = map.hexes.get_mut(&coord) {
                            data.feature = HexFeature::None;
                        }
                        let material = if let Some(data) = map.hexes.get(&coord) {
                            get_tile_material(coord, data.hex_type.clone(), &map.hexes, &tiles)
                        } else {
                            return;
                        };
                        if let Some(data) = map.hexes.get_mut(&coord) {
                            data.hex_tile_material = material;
                        }
                        for (e, c) in spawned_hexes.iter() {
                            if c.hex == coord {
                                commands.entity(e).try_despawn();
                            }
                        }
                    }
                    vtt_data.invalidate_map = true;
                }
            }
            PenType::FeaturePen => {
                if click.pressed(MouseButton::Right) && map.hexes.contains_key(&coord) {
                    if let Some(uid) = map.get_selected_uid()
                        && uid == "<uid>"
                    {
                        if let Some(data) = map.hexes.get_mut(&coord) {
                            data.feature = editor.selected_feature.clone();
                        }
                        let material = if let Some(data) = map.hexes.get(&coord) {
                            get_tile_material(coord, data.hex_type.clone(), &map.hexes, &tiles)
                        } else {
                            return;
                        };
                        if let Some(data) = map.hexes.get_mut(&coord) {
                            data.hex_tile_material = material;
                        }
                        for (e, c) in spawned_hexes.iter() {
                            if c.hex == coord {
                                commands.entity(e).try_despawn();
                            }
                        }
                        vtt_data.invalidate_map = true;
                    }
                }
            }
        }
    }
}

#[derive(Event)]
struct AddFeature(HexFeature);
#[derive(Event)]
struct DelFeature(HexFeature);

fn on_add_features(
    trigger: On<AddFeature>,
    mut commands: Commands,
    to_extend: Query<(Entity, &HexEntity), With<TempHex>>,
    mut map: ResMut<HexMapData>,
    mut vtt_data: ResMut<VttData>,
    tiles: Res<HexMapTileMaterials>,
    mut editor: ResMut<MapEditor>,
) {
    let range = to_extend.count();
    let pick_count = 1usize.min(range);
    if pick_count == 0 {
        return;
    }
    let indices: Vec<usize> = sample(&mut rand::thread_rng(), range, pick_count).into_vec();

    let mut retrying_next = false;
    for (i, (e, hex_coords)) in to_extend.iter().enumerate() {
        if indices.contains(&i) || retrying_next {
            if let Some(data) = map.hexes.get_mut(&hex_coords.hex) {
                if data.feature == HexFeature::None {
                    data.feature = trigger.0.clone();
                    retrying_next = false;
                } else {
                    retrying_next = true;
                    continue;
                }
            }
            let material = if let Some(data) = map.hexes.get(&hex_coords.hex) {
                get_tile_material(hex_coords.hex, data.hex_type.clone(), &map.hexes, &tiles)
            } else {
                continue;
            };
            if let Some(data) = map.hexes.get_mut(&hex_coords.hex) {
                data.hex_tile_material = material;
                commands.entity(e).try_despawn();
            }
        }
    }
    vtt_data.invalidate_map = true;
    editor.knobs.get_mut(&trigger.0).unwrap().current += 1;
}

fn on_del_features(
    trigger: On<DelFeature>,
    mut commands: Commands,
    to_extend: Query<(Entity, &HexEntity), With<TempHex>>,
    mut map: ResMut<HexMapData>,
    mut vtt_data: ResMut<VttData>,
    tiles: Res<HexMapTileMaterials>,
    mut editor: ResMut<MapEditor>,
) {
    let mut to_extend_vec: Vec<_> = to_extend.iter().collect();
    to_extend_vec.shuffle(&mut rand::thread_rng());
    for (e, hex_coords) in to_extend_vec.iter() {
        if let Some(data) = map.hexes.get_mut(&hex_coords.hex) {
            if data.feature == trigger.0 {
                data.feature = HexFeature::None;
            } else {
                continue;
            }
        }
        let material = if let Some(data) = map.hexes.get(&hex_coords.hex) {
            get_tile_material(hex_coords.hex, data.hex_type.clone(), &map.hexes, &tiles)
        } else {
            continue;
        };
        if let Some(data) = map.hexes.get_mut(&hex_coords.hex) {
            data.hex_tile_material = material;
            commands.entity(*e).try_despawn();
            debug!(" despawing entity hex");
        }
        break;
    }
    vtt_data.invalidate_map = true;
    editor.knobs.get_mut(&trigger.0).unwrap().current -= 1;
}

fn add_features(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    editor: Res<MapEditor>,
) {
    if keyboard.just_pressed(KeyCode::F2) {}
    if keyboard.just_pressed(KeyCode::F3) {}

    for (feature, knob) in editor.knobs.iter() {
        let diff = knob.target - knob.current;
        if diff > 0 {
            commands.trigger(AddFeature(feature.clone()));
        }
        if diff < 0 {
            commands.trigger(DelFeature(feature.clone()));
        }
    }
}

fn extend_seeds(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    to_extend: Query<&HexEntity, With<TempHex>>,
    mut map: ResMut<HexMapData>,
    mut vtt_data: ResMut<VttData>,
    assets: Res<HexMapResources>,
    tiles: Res<HexMapTileMaterials>,
    map_parent: Single<Entity, With<HexMapTime>>,
    theme: Res<HexmapTheme>,
    mut inc: Local<u32>,
) {
    if keyboard.pressed(KeyCode::F1) {
        *inc += 1;
        let mut hexes_to_extend = Vec::new();
        for seed_hex in to_extend {
            let free_hexes: Vec<Hex> = seed_hex
                .hex
                .all_neighbors()
                .iter()
                .filter_map(|neighbor| {
                    if !map.hexes.contains_key(neighbor) {
                        Some(neighbor.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let count_of_free_hexes = free_hexes.len();

            use rand::Rng;

            let limit = rand::thread_rng().gen_range(20..=100);

            if count_of_free_hexes > 0 {
                let pool_size = pool_size(&mut map, &seed_hex.hex);
                if pool_size < limit {
                    hexes_to_extend.push((
                        count_of_free_hexes,
                        free_hexes,
                        map.hexes.get(&seed_hex.hex).unwrap().hex_type.clone(),
                        map.hexes.get(&seed_hex.hex).unwrap().pool_id,
                    ));
                }
            }
        }
        hexes_to_extend.sort_by(|a, b| b.0.cmp(&a.0));

        let scale_calculator = theme.tile_scale_values();
        let layout = y_inverted_hexmap_layout();
        let mut budget = 1000000;
        for (_, free_hexes, terrain_type, pool_id) in hexes_to_extend {
            budget -= 1;
            if budget == 0 {
                break;
            }

            let n = rand::random::<usize>() % (free_hexes.len() + 1);
            let mut rng = rand::thread_rng();
            let mut shuffled_hexes = free_hexes.clone();
            shuffled_hexes.shuffle(&mut rng);
            let random_items = &shuffled_hexes[..n];

            for coord in random_items {
                if !map.hexes.contains_key(coord) {
                    let do_not_place = coord.all_neighbors().to_vec().iter().any(|neighbor| {
                        if let Some(tester) = map.hexes.get(neighbor) {
                            if tester.hex_type == terrain_type && tester.pool_id != pool_id {
                                return true;
                            }
                        }
                        return false;
                    });

                    if do_not_place {
                        continue;
                    }

                    let new_hex = PreparedHexTile {
                        generated: false,
                        pool_id,
                        uid: "<uid>".to_string(),
                        realm_uid: None,
                        region_uid: None,
                        hex_type: terrain_type.clone(),
                        hex_tile_material: get_tile_material(
                            *coord,
                            terrain_type.clone(),
                            &map.hexes,
                            &tiles,
                        ),
                        partial_hex_tile_material: get_tile_material(
                            *coord,
                            terrain_type.clone(),
                            &map.hexes,
                            &tiles,
                        ),
                        tile_scale: scale_calculator.0,
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

                    map.hexes.insert(*coord, new_hex);
                    draw_tile_and_refresh_neighbors(
                        &mut commands,
                        &layout,
                        *coord,
                        *map_parent,
                        &tiles,
                        &assets,
                        &mut map,
                        &mut vtt_data,
                    );
                }
            }
        }
    } else {
        *inc = inc.saturating_sub(10);
    }
}

#[derive(Component)]
struct HexBrush;

#[derive(Component)]
pub struct TempHex;

fn draw_tile_and_refresh_neighbors(
    commands: &mut Commands,
    layout: &HexLayout,
    hex: Hex,
    map_parent: Entity,
    tiles: &Res<HexMapTileMaterials>,
    map_resources: &Res<HexMapResources>,
    map_data: &mut ResMut<HexMapData>,
    vtt_data: &mut ResMut<VttData>,
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
        HexBrush,
    );
}

fn refresh_neighbors(
    mut commands: Commands,
    mut map_data: ResMut<HexMapData>,
    vtt_data: ResMut<VttData>,
    map_resources: Res<HexMapResources>,
    tiles: Res<HexMapTileMaterials>,
    map_parent: Single<Entity, With<HexMapTime>>,
    spawned_hexes: Query<(Entity, &HexEntity)>,
    brush_hexes: Query<(Entity, &HexEntity, &HexBrush)>,
) {
    let layout = y_inverted_hexmap_layout();
    let mut backlog: Vec<(Entity, Hex)> = Vec::new();
    let mut backlog_set: HashSet<Hex> = HashSet::new();
    for (ee, hh, _) in brush_hexes {
        commands.entity(ee).remove::<HexBrush>();
        commands.entity(ee).insert(TempHex);
        for neighbor in hh.hex.all_neighbors() {
            if backlog_set.contains(&neighbor) {
                continue;
            }
            if map_data.hexes.contains_key(&neighbor) {
                for (e, h) in spawned_hexes.iter() {
                    if h.hex == neighbor {
                        backlog.push((e, neighbor));
                        backlog_set.insert(neighbor);
                        break;
                    }
                }
            }
        }
    }
    for (e, neighbor) in backlog {
        let hex_data = map_data.hexes.get(&neighbor).unwrap();
        if hex_data.generated {
            continue;
        }
        commands.entity(e).despawn();
        let terrain_type = hex_data.hex_type.clone();
        map_data.hexes.get_mut(&neighbor).unwrap().hex_tile_material =
            get_tile_material(neighbor, terrain_type, &map_data.hexes, &tiles);
        spawn_tile(
            &mut commands,
            &layout,
            neighbor,
            *map_parent,
            &tiles,
            &map_resources,
            &map_data,
            &vtt_data,
            TempHex,
        );
    }
}

fn get_tile_material(
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

#[derive(Component)]
struct DrawingHud;

#[derive(Component)]
struct DrawingHudTerrain;

fn create_drawing_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut next_content_mode: ResMut<NextState<ContentMode>>,
    editor: Res<MapEditor>,
) {
    next_content_mode.set(ContentMode::MapOnly);

    commands
        .spawn((
            DrawingHud,
            Name::new("DrawingHud"),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Px(10.0),
                justify_self: JustifySelf::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Pickable {
                should_block_lower: true,
                ..default()
            },
        ))
        .observe(|trigger: On<Pointer<Over>>, mut commands: Commands| {
            commands
                .entity(trigger.entity)
                .try_insert(PointerExclusivityIsPreferred);
        })
        .observe(|trigger: On<Pointer<Out>>, mut commands: Commands| {
            commands
                .entity(trigger.entity)
                .try_remove::<PointerExclusivityIsPreferred>();
        })
        .with_children(|c| {
            c.spawn((
                Name::new("DrawingHudSave"),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(20.0),
                    left: Val::Px(20.0),
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn(make_hud_button_bundle("Save", HudButtonAction, &editor))
                    .with_child(make_hud_button_image_bundle(
                        &asset_server,
                        "icons/icon-save.ktx2",
                    ))
                    .hover_effect_ex(true)
                    .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                        commands.trigger(GenerateHexMap);
                    });
            });
            c.spawn((
                Name::new("DrawingHudDiscard"),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(20.0),
                    right: Val::Px(20.0),
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn(make_hud_button_bundle("Discard", HudButtonAction, &editor))
                    .with_child(make_hud_button_image_bundle(
                        &asset_server,
                        "icons/icon-trash.ktx2",
                    ))
                    .hover_effect_ex(true)
                    .observe(|_: On<Pointer<Click>>,
                        mut commands: Commands,
                        to_discard: Query<(Entity, &HexEntity), With<TempHex>>,
                        mut map: ResMut<HexMapData>,
                        mut next_tool_state: ResMut<NextState<HexMapToolState>> | {
                            for (e, hex) in to_discard {
                                commands.entity(e).try_despawn();
                                map.hexes.remove(&hex.hex);
                            }
                            map.hexes.retain(|_, v|{
                                v.uid != "<uid>"
                            });
                            next_tool_state.set(HexMapToolState::Selection);
                    });
            });

            c.spawn((
                Name::new("DrawingHudTools"),
                Node {
                    top: Val::Px(20.0),
                    justify_self: JustifySelf::Center,
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn(make_hud_button_bundle(
                    "Pencil",
                    HudButtonTool(PenType::Pencil),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-pencil-256.ktx2",
                ))
                .toggle_tool::<HudButtonTool>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>,
                     mut editor: ResMut<MapEditor>,
                     terrain_hud: Single<Entity, With<DrawingHudTerrain>>,
                     feature_hud: Single<Entity, With<DrawingHudFeatures>>,
                     mut commands: Commands| {
                        editor.pen = PenType::Pencil;
                        commands
                            .entity(*terrain_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::Flex);
                        commands
                            .entity(*feature_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "Brush",
                    HudButtonTool(PenType::Brush),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-brush.ktx2",
                ))
                .toggle_tool::<HudButtonTool>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>,
                     mut editor: ResMut<MapEditor>,
                     terrain_hud: Single<Entity, With<DrawingHudTerrain>>,
                     feature_hud: Single<Entity, With<DrawingHudFeatures>>,
                     mut commands: Commands| {
                        editor.pen = PenType::Brush;
                        commands
                            .entity(*terrain_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::Flex);
                        commands
                            .entity(*feature_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "Eraser",
                    HudButtonTool(PenType::Eraser),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-eraser.ktx2",
                ))
                .toggle_tool::<HudButtonTool>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>,
                     mut editor: ResMut<MapEditor>,
                     terrain_hud: Single<Entity, With<DrawingHudTerrain>>,
                     feature_hud: Single<Entity, With<DrawingHudFeatures>>,
                     mut commands: Commands| {
                        editor.pen = PenType::Eraser;
                        commands
                            .entity(*terrain_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                        commands
                            .entity(*feature_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "Broom",
                    HudButtonTool(PenType::Broom),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-broom.ktx2",
                ))
                .toggle_tool::<HudButtonTool>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>,
                     mut editor: ResMut<MapEditor>,
                     terrain_hud: Single<Entity, With<DrawingHudTerrain>>,
                     feature_hud: Single<Entity, With<DrawingHudFeatures>>,
                     mut commands: Commands| {
                        editor.pen = PenType::Broom;
                        commands
                            .entity(*terrain_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                        commands
                            .entity(*feature_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "FeaturePen",
                    HudButtonTool(PenType::FeaturePen),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-feature.ktx2",
                ))
                .toggle_tool::<HudButtonTool>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>,
                     mut editor: ResMut<MapEditor>,
                     terrain_hud: Single<Entity, With<DrawingHudTerrain>>,
                     feature_hud: Single<Entity, With<DrawingHudFeatures>>,
                     mut commands: Commands| {
                        editor.pen = PenType::FeaturePen;
                        commands
                            .entity(*terrain_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                        commands
                            .entity(*feature_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::Flex);
                    },
                );
            });
            c.spawn((
                Name::new("DrawingHudTerrain"),
                DrawingHudTerrain,
                Node {
                    top: Val::Px(20.0),
                    justify_self: JustifySelf::Center,
                    display: if editor.pen.show_terrain_bar() {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn(make_hud_button_bundle(
                    "ForestHexToll",
                    HudButtonTerrain(TerrainType::ForestHex),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-forest.ktx2",
                ))
                .toggle_tool::<HudButtonTerrain>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.terrain = TerrainType::ForestHex;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "MountainsHexTool",
                    HudButtonTerrain(TerrainType::MountainsHex),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-mountains.ktx2",
                ))
                .toggle_tool::<HudButtonTerrain>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.terrain = TerrainType::MountainsHex;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "PlainsHexTool",
                    HudButtonTerrain(TerrainType::PlainsHex),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-plains.ktx2",
                ))
                .toggle_tool::<HudButtonTerrain>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.terrain = TerrainType::PlainsHex;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "DesertHexTool",
                    HudButtonTerrain(TerrainType::DesertHex),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-desert.ktx2",
                ))
                .toggle_tool::<HudButtonTerrain>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.terrain = TerrainType::DesertHex;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "SwampsHexTool",
                    HudButtonTerrain(TerrainType::SwampsHex),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-swamps.ktx2",
                ))
                .toggle_tool::<HudButtonTerrain>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.terrain = TerrainType::SwampsHex;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "JungleHexTool",
                    HudButtonTerrain(TerrainType::JungleHex),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-jungle.ktx2",
                ))
                .toggle_tool::<HudButtonTerrain>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.terrain = TerrainType::JungleHex;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "TundraHexTool",
                    HudButtonTerrain(TerrainType::TundraHex),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-tundra.ktx2",
                ))
                .toggle_tool::<HudButtonTerrain>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.terrain = TerrainType::TundraHex;
                    },
                );
            });
            c.spawn((
                Name::new("DrawingHudFeatures"),
                DrawingHudFeatures,
                Node {
                    top: Val::Px(20.0),
                    justify_self: JustifySelf::Center,
                    display: if editor.pen.show_feature_bar() {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn(make_hud_button_bundle(
                    "DungeonFeatureTool",
                    HudButtonFeature(HexFeature::Dungeon),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-dungeon.ktx2",
                ))
                .toggle_tool::<HudButtonFeature>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.selected_feature = HexFeature::Dungeon;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "CityFeatureTool",
                    HudButtonFeature(HexFeature::City),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-city.ktx2",
                ))
                .toggle_tool::<HudButtonFeature>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.selected_feature = HexFeature::City;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "TownFeatureTool",
                    HudButtonFeature(HexFeature::Town),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-town.ktx2",
                ))
                .toggle_tool::<HudButtonFeature>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.selected_feature = HexFeature::Town;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "VillageFeatureTool",
                    HudButtonFeature(HexFeature::Village),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-village.ktx2",
                ))
                .toggle_tool::<HudButtonFeature>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.selected_feature = HexFeature::Village;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "InnFeatureTool",
                    HudButtonFeature(HexFeature::Inn),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-inn.ktx2",
                ))
                .toggle_tool::<HudButtonFeature>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.selected_feature = HexFeature::Inn;
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "ResidencyFeatureTool",
                    HudButtonFeature(HexFeature::Residency),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-dwelling.ktx2",
                ))
                .toggle_tool::<HudButtonFeature>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.selected_feature = HexFeature::Residency;
                    },
                );
            });
            c.spawn((
                Name::new("DrawingHudKnobs"),
                Node {
                    top: Val::Px(20.0),
                    justify_self: JustifySelf::Center,
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn_empty().spawn_knob(
                    HexFeature::Dungeon,
                    get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::Dungeon),
                    "icons/icon-dungeon.ktx2",
                    "",
                    &asset_server,
                );
                c.spawn_empty().spawn_knob(
                    HexFeature::City,
                    get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::City),
                    "icons/icon-city.ktx2",
                    "",
                    &asset_server,
                );
                c.spawn_empty().spawn_knob(
                    HexFeature::Town,
                    get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::Town),
                    "icons/icon-town.ktx2",
                    "",
                    &asset_server,
                );
                c.spawn_empty().spawn_knob(
                    HexFeature::Village,
                    get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::Village),
                    "icons/icon-village.ktx2",
                    "",
                    &asset_server,
                );
                c.spawn_empty().spawn_knob(
                    HexFeature::Inn,
                    get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::Inn),
                    "icons/icon-inn.ktx2",
                    "",
                    &asset_server,
                );
                c.spawn_empty().spawn_knob(
                    HexFeature::Residency,
                    get_feature_ratio_for_realm_type(
                        &editor.realm_type,
                        HexFeature::Residency,
                    ),
                    "icons/icon-dwelling.ktx2",
                    "",
                    &asset_server,
                );
            });
        });
}

fn destroy_drawing_hud(mut commands: Commands, hud: Query<Entity, With<DrawingHud>>) {
    hud.iter().for_each(|e| commands.entity(e).try_despawn());
}

#[derive(Component)]
struct HudButtonTool(PenType);

#[derive(Component)]
struct HudButtonTerrain(TerrainType);

#[derive(Component)]
struct HudButtonFeature(HexFeature);

#[derive(Component)]
struct HudButtonAction;

#[derive(Component)]
struct DrawingHudFeatures;

trait HudButton {
    fn border() -> f32;
    fn active(&self, editor: &MapEditor) -> bool;
}

impl HudButton for HudButtonAction {
    fn border() -> f32 {
        0.0
    }
    fn active(&self, _: &MapEditor) -> bool {
        false
    }
}

impl HudButton for HudButtonTerrain {
    fn border() -> f32 {
        4.0
    }
    fn active(&self, editor: &MapEditor) -> bool {
        editor.terrain == self.0
    }
}

impl HudButton for HudButtonTool {
    fn border() -> f32 {
        4.0
    }
    fn active(&self, editor: &MapEditor) -> bool {
        editor.pen == self.0
    }
}

impl HudButton for HudButtonFeature {
    fn border() -> f32 {
        4.0
    }
    fn active(&self, editor: &MapEditor) -> bool {
        editor.selected_feature == self.0
    }
}

fn make_hud_button_bundle<T>(name: &str, component: T, editor: &MapEditor) -> impl Bundle
where
    T: Component + HudButton,
{
    (
        Name::new(name.to_string()),
        Node {
            width: Val::Px(48.0),
            height: Val::Px(48.0),
            margin: UiRect::right(Val::Px(10.0)),
            border: UiRect::all(Val::Px(T::border())),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BorderRadius::all(Val::Px(4.0)),
        if component.active(editor) {
            BorderColor::all(Color::WHITE)
        } else {
            BorderColor::all(Color::srgb_u8(20, 20, 20))
        },
        BackgroundColor(Color::srgb_u8(20, 20, 20)),
        ThemeBackgroundColor(Color::srgb_u8(20, 20, 20)),
        Pickable {
            should_block_lower: true,
            ..default()
        },
        component,
    )
}

fn make_hud_button_image_bundle(
    asset_server: &Res<AssetServer>,
    image_asset_name: &str,
) -> impl Bundle {
    (
        Node {
            width: Val::Px(48.0),
            height: Val::Px(48.0),
            padding: UiRect::all(Val::Px(10.0)),
            align_self: AlignSelf::Center,
            ..default()
        },
        Pickable {
            should_block_lower: true,
            ..default()
        },
        ImageNode {
            image: asset_server.load(image_asset_name.to_string()),
            ..default()
        },
    )
}

#[derive(Resource)]
struct BackgroundMapGenerationTasks {
    tasks: Vec<Task<Option<Vec<(String, Hex)>>>>,
}

#[derive(Event)]
struct GenerateHexMap;

fn pool_size(map: &mut HexMapData, coord_to_check: &Hex) -> u32 {
    let (terrain, mut pool_id) = match map.hexes.get(coord_to_check) {
        Some(hex) => (&hex.hex_type.clone(), hex.pool_id),
        None => return 0,
    };

    if pool_id == 0 {
        pool_id = coord_to_check.x * 1000 + coord_to_check.y;
    }

    let mut pool_backlog: VecDeque<Hex> = VecDeque::new();
    let mut pool_processed: HashSet<Hex> = HashSet::new();
    let mut count = 0;

    pool_backlog.push_front(*coord_to_check);

    while !pool_backlog.is_empty() {
        let current = pool_backlog.pop_front().unwrap();

        if map
            .hexes
            .get(&current)
            .map_or(false, |hex| &hex.hex_type == terrain)
        {
            count += 1;
            map.hexes.get_mut(&current).unwrap().pool_id = pool_id;
            pool_processed.insert(current);
            for neighbor in current.all_neighbors() {
                if !pool_processed.contains(&neighbor) && map.hexes.get(&neighbor).is_some() {
                    pool_backlog.push_front(neighbor);
                }
                pool_processed.insert(neighbor);
            }
        }
    }
    count
}

fn partition_hexes_to_regions(map: &HexMapData) -> (Vec<(TerrainType, Vec<Hex>)>, usize) {
    let mut new_coords: Vec<(Hex, TerrainType)> = Vec::new();
    let mut coord_to_terrain: HashMap<Hex, TerrainType> = HashMap::new();
    let mut unallocated_coords: HashSet<Hex> = HashSet::new();
    for (coords, hex) in map.hexes.iter() {
        if hex.uid == "<uid>" {
            new_coords.push((*coords, hex.hex_type.clone()));
            coord_to_terrain.insert(*coords, hex.hex_type.clone());
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

#[derive(Resource, Default, Clone)]
struct GenerationWorkload {
    message: std::sync::Arc<std::sync::Mutex<String>>,
    red_button: std::sync::Arc<std::sync::Mutex<bool>>,
}

fn detect_abort(
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

#[derive(Component)]
struct SpinnerText;

#[derive(Component)]
struct SpinnerNode;

fn generate_hex_map(
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

fn poll_generation_tasks(
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
                map.generating = false;
            }
            commands.entity(*spinner_node).try_despawn();
            commands.remove_resource::<GenerationWorkload>();
            next_state.set(DiscreteAppState::Normal);
        }
        retain
    });
}

pub trait ToggleTool {
    fn toggle_tool<T>(&mut self) -> &mut Self
    where
        T: Component;
}

impl ToggleTool for EntityCommands<'_> {
    fn toggle_tool<T>(&mut self) -> &mut Self
    where
        T: Component,
    {
        self.observe(
            |trigger: On<Pointer<Click>>,
             mut commands: Commands,
             buttons: Query<Entity, With<T>>| {
                for button in buttons {
                    commands
                        .entity(button)
                        .try_insert(BorderColor::all(Color::srgb_u8(20, 20, 20)));
                }
                commands
                    .entity(trigger.entity)
                    .try_insert(BorderColor::all(Color::WHITE));
            },
        );
        self
    }
}

#[derive(Component)]
struct KnobRing;

#[derive(Component)]
struct KnobGauge;

#[derive(Component)]
struct KnobNotch;

trait GeneratorKnob {
    fn spawn_knob(
        &mut self,
        feature: HexFeature,
        fraction: f32,
        icon: &str,
        tooltip: &str,
        asset_server: &Res<AssetServer>,
    ) -> &mut Self;
}
impl GeneratorKnob for EntityCommands<'_> {
    fn spawn_knob(
        &mut self,
        feature: HexFeature,
        fraction: f32,
        icon: &str,
        tooltip: &str,
        asset_server: &Res<AssetServer>,
    ) -> &mut Self {
        self.insert((
            Name::new("Knob"),
            Node {
                width: Val::Px(64.0),
                height: Val::Px(64.0),
                margin: UiRect::right(Val::Px(10.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderRadius::all(Val::Px(32.0)),
            BackgroundColor(Color::srgb_u8(20, 20, 20)),
            ThemeBackgroundColor(Color::srgb_u8(20, 20, 20)),
            Pickable {
                should_block_lower: true,
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(64.0),
                    height: Val::Px(64.0),
                    margin: UiRect::all(Val::Px(-1.0)),
                    ..default()
                },
                KnobRing,
                BorderRadius::all(Val::Px(32.0)),
                UiTransform::from_rotation(Rot2::degrees(-135.0)),
                Pickable {
                    should_block_lower: false,
                    ..default()
                },
            ))
            .with_child((
                KnobGauge,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(64.0),
                    height: Val::Px(64.0),
                    border: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BorderColor::all(Color::WHITE.with_alpha(0.0)),
                BorderRadius::all(Val::Px(32.0)),
                UiTransform::from_rotation(Rot2::degrees(135.0)),
                Pickable {
                    should_block_lower: false,
                    ..default()
                },
            ))
            .with_child((
                KnobNotch,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(32.0),
                    width: Val::Px(5.0),
                    height: Val::Px(12.0),
                    ..default()
                },
                BorderRadius::all(Val::Px(2.0)),
                BackgroundColor(Color::WHITE),
                Pickable {
                    should_block_lower: false,
                    ..default()
                },
            ));
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(64.0),
                    height: Val::Px(64.0),
                    border: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                BorderColor {
                    bottom: Color::srgb_u8(20, 20, 20),
                    ..default()
                },
                BorderRadius::all(Val::Px(32.0)),
                Pickable {
                    should_block_lower: false,
                    ..default()
                },
            ));
        })
        .with_child(make_hud_button_image_bundle(&asset_server, icon))
        .tooltip_on_hover(tooltip, 30.0)
        .toggle_tool::<HudButtonTool>()
        .observe(
            move |trigger: On<Pointer<Drag>>,
                  mut knob_ring_transforms: Query<&mut UiTransform, With<KnobRing>>,
                  mut knob_gauge_borders: Query<
                &mut BorderColor,
                (With<KnobGauge>, Without<KnobRing>),
            >,
                  mut knob_notch_nodes: Query<
                &mut Node,
                (With<KnobNotch>, Without<KnobRing>, Without<KnobGauge>),
            >,
                  mut editor: ResMut<MapEditor>,
                  children: Query<&Children>,
                  time: Res<Time>| {
                let d = (trigger.delta.x + -trigger.delta.y) * time.delta_secs() * 30.0;

                let mut exponential_normalized = 0.0;
                let mut degs = 0.0;

                children
                    .iter_descendants(trigger.entity)
                    .for_each(|entity| {
                        if let Ok(mut tx) = knob_ring_transforms.get_mut(entity) {
                            let current = tx.rotation.as_degrees();
                            let update = (current + d).clamp(-135.0, 135.0);
                            tx.rotation = Rot2::degrees(update);
                            let knob = editor
                                .knobs
                                .entry(feature.clone())
                                .or_insert(Knob::default());

                            degs = update + 135.0;
                            let base = (update + 135.0) / 10.0;
                            let exponential = exponential_graph_value(base);
                            exponential_normalized = base / 27.0;
                            knob.target = (exponential * fraction) as i32;
                        }
                    });
                children
                    .iter_descendants(trigger.entity)
                    .for_each(|entity| {
                        if let Ok(mut border_color) = knob_gauge_borders.get_mut(entity) {
                            border_color.bottom.set_alpha((degs > 1.0) as u8 as f32);
                            border_color.right.set_alpha((degs > 90.0) as u8 as f32);
                            border_color.top.set_alpha((degs > 180.0) as u8 as f32);
                            border_color.left.set_alpha((degs > 270.0) as u8 as f32);
                        }
                    });
                children
                    .iter_descendants(trigger.entity)
                    .for_each(|entity| {
                        if let Ok(mut node) = knob_notch_nodes.get_mut(entity) {
                            let offset = -6.0 * (degs / 360.0);
                            node.left = Val::Px(31.0 + offset);
                        }
                    });
            },
        )
        .custom_pointer_on_hover(SystemCursorIcon::EwResize)
        .observe(
            |trigger: On<Pointer<DragStart>>,
             mut commands: Commands,
             window: Single<(Entity, &Window), With<PrimaryWindow>>| {
                let current_pos: Vec2 = window.1.cursor_position().unwrap();
                commands
                    .entity(trigger.entity)
                    .try_insert(PointerExclusivityIsPreferred)
                    .try_insert(GrabbedMousePosition(current_pos));
                commands.entity(window.0).insert(CursorOptions {
                    visible: false,
                    grab_mode: CursorGrabMode::None,
                    hit_test: true,
                });
            },
        )
        .observe(
            |trigger: On<Pointer<DragEnd>>,
             mut commands: Commands,
             pos: Query<&GrabbedMousePosition>,
             mut window: Single<(Entity, &mut Window), With<PrimaryWindow>>| {
                if let Ok(pos) = pos.get(trigger.entity) {
                    window.1.set_cursor_position(Some(pos.0));
                }
                commands
                    .entity(trigger.entity)
                    .try_remove::<PointerExclusivityIsPreferred>();
                commands.entity(window.0).insert(CursorOptions {
                    visible: true,
                    grab_mode: CursorGrabMode::None,
                    hit_test: true,
                });
            },
        )
    }
}

#[derive(Component)]
struct GrabbedMousePosition(Vec2);

pub fn exponential_graph_value(x: f32) -> f32 {
    let x = x.clamp(0.0, 27.0);
    x.powi(2) / 10.0
}
