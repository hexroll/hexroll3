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
use hexx::*;
use rand::seq::SliceRandom;
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
            cursor::PointerExclusivityIsPreferred,
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
        })
        .add_systems(OnEnter(HexMapToolState::Edit), create_drawing_hud)
        .add_systems(OnExit(HexMapToolState::Edit), destroy_drawing_hud)
        .add_systems(Update, extend_seeds.run_if(in_state(HexMapToolState::Edit)))
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
}

#[derive(Resource)]
pub struct MapEditor {
    pub pen: PenType,
    pub terrain: TerrainType,
    pub realm_type: String,
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
                    .hover_effect()
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
                    .hover_effect()
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
                    "Eraser",
                    HudButtonTool(PenType::Pencil),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-pencil-256.ktx2",
                ))
                .toggle_tool::<HudButtonTool>()
                .hover_effect()
                .observe(
                    |_: On<Pointer<Click>>,
                     mut editor: ResMut<MapEditor>,
                     terrain_hud: Single<Entity, With<DrawingHudTerrain>>,
                     mut commands: Commands| {
                        editor.pen = PenType::Pencil;
                        commands
                            .entity(*terrain_hud)
                            .try_insert(Visibility::Inherited);
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "Eraser",
                    HudButtonTool(PenType::Brush),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-brush.ktx2",
                ))
                .toggle_tool::<HudButtonTool>()
                .hover_effect()
                .observe(
                    |_: On<Pointer<Click>>,
                     mut editor: ResMut<MapEditor>,
                     terrain_hud: Single<Entity, With<DrawingHudTerrain>>,
                     mut commands: Commands| {
                        editor.pen = PenType::Brush;
                        commands
                            .entity(*terrain_hud)
                            .try_insert(Visibility::Inherited);
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
                .hover_effect()
                .observe(
                    |_: On<Pointer<Click>>,
                     mut editor: ResMut<MapEditor>,
                     terrain_hud: Single<Entity, With<DrawingHudTerrain>>,
                     mut commands: Commands| {
                        editor.pen = PenType::Eraser;
                        commands.entity(*terrain_hud).try_insert(Visibility::Hidden);
                    },
                );
            });
            c.spawn((
                Name::new("DrawingHudTerrain"),
                DrawingHudTerrain,
                Node {
                    top: Val::Px(20.0),
                    justify_self: JustifySelf::Center,
                    ..default()
                },
                if editor.pen == PenType::Eraser {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
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
                .hover_effect()
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
                .hover_effect()
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
                .hover_effect()
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
                .hover_effect()
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
                .hover_effect()
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
                .hover_effect()
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
                .hover_effect()
                .observe(
                    |_: On<Pointer<Click>>, mut editor: ResMut<MapEditor>| {
                        editor.terrain = TerrainType::TundraHex;
                    },
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
struct HudButtonAction;

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
    hexes_to_generate: i32,
    hexes_generated: std::sync::Arc<std::sync::Mutex<i32>>,
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

    let mut generation_tracker = GenerationWorkload::default();
    generation_tracker.hexes_to_generate = num_hexes_to_generate as i32;
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
                    if let Ok(mut hexes_generated) = generation_tracker.hexes_generated.lock()
                    {
                        *hexes_generated += region.len() as i32;
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
                Ok(())
            })
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
        if let Ok(v) = t.hexes_generated.lock() {
            // let hex_area = 18.0 * 3.0_f32.sqrt();
            spinner_text.0 = format!(
                // "Generated {} out of {} hexes ({:.2} Square Miles)\n",
                "Generated {} out of {} hexes\n",
                *v,
                t.hexes_to_generate,
                // *v as f32 * hex_area
            );
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
