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
    ecs::relationship::RelatedSpawnerCommands,
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use hexx::*;
use rand::seq::{SliceRandom, index::sample};

use crate::{
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
            cursor::PointerExclusivityIsPreferred, knob::GeneratorKnob, link::ContentHoverLink,
        },
    },
};

use super::{
    HexmapTheme,
    builder::{
        BackgroundMapGenerationTasks, GenerateHexMap, GenerationWorkload, detect_abort,
        generate_hex_map, poll_generation_tasks, tester,
    },
};

fn enter_edit_mode_setup(
    mut vtt_data: ResMut<VttData>,
    map: Res<HexMapData>,
    mut masks: Query<(&HexMask, &mut Visibility)>,
    mut editor: ResMut<MapEditor>,
) {
    editor.budget.target = 0;
    editor.budget.current = 0;
    vtt_data.edit_mode = true;
    for (mask, mut vis) in masks.iter_mut() {
        if map.hexes.get(&mask.0).map_or(false, |t| t.generated) {
            *vis = Visibility::Visible;
        }
    }
}

fn exit_edit_mode_teardown(
    mut vtt_data: ResMut<VttData>,
    mut masks: Query<&mut Visibility, With<HexMask>>,
) {
    vtt_data.edit_mode = false;
    for mut vis in masks.iter_mut() {
        *vis = Visibility::Hidden;
    }
}

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
            budget: Knob::default(),
            volume: Knob::default(),
            intent: HashMap::new(),
        })
        .add_systems(
            OnEnter(HexMapToolState::Edit),
            (enter_edit_mode_setup, create_drawing_hud),
        )
        .add_systems(
            OnExit(HexMapToolState::Edit),
            (destroy_drawing_hud, exit_edit_mode_teardown),
        )
        .add_systems(Update, tester.run_if(in_state(HexMapToolState::Edit)))
        .add_systems(Update, add_features.run_if(in_state(HexMapToolState::Edit)))
        .add_observer(on_add_features)
        .add_observer(on_del_features)
        .add_systems(
            Update,
            (detect_abort, poll_generation_tasks)
                // .run_if(in_state(HexMapToolState::Edit))
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
    TerrainMaker,
}

impl PenType {
    fn show_terrain_bar(&self) -> bool {
        *self == PenType::Pencil || *self == PenType::Brush
    }

    fn show_feature_bar(&self) -> bool {
        *self == PenType::FeaturePen
    }

    fn show_terrain_knobs_bar(&self) -> bool {
        *self == PenType::TerrainMaker
    }
}

#[derive(Default)]
pub struct Knob {
    pub target: i32,
    pub current: i32,
}

#[derive(Resource)]
pub struct MapEditor {
    pub pen: PenType,
    pub terrain: TerrainType,
    pub realm_type: String,
    pub knobs: HashMap<HexFeature, Knob>,
    pub selected_feature: HexFeature,
    pub budget: Knob,
    pub volume: Knob,
    pub intent: HashMap<TerrainType, i32>,
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
fn get_region_volume_for_realm_type(realm_type: &str) -> i32 {
    if realm_type == "RealmTypeKingdom" {
        return 12;
    }
    if realm_type == "RealmTypeEmpire" {
        return 16;
    }
    if realm_type == "RealmTypeLands" {
        return 14;
    }
    if realm_type == "RealmTypeDuchy" {
        return 8;
    }
    return 10;
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
            PenType::TerrainMaker => {}
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

pub fn get_tile_material(
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
                is_hoverable: false,
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
                     features_knobs_hud: Single<Entity, With<FeaturesHudKnobs>>,
                     terrain_knobs_hud: Single<Entity, With<TerrainHudKnobs>>,
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
                        commands
                            .entity(*features_knobs_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                        commands
                            .entity(*terrain_knobs_hud)
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
                     features_knobs_hud: Single<Entity, With<FeaturesHudKnobs>>,
                     terrain_knobs_hud: Single<Entity, With<TerrainHudKnobs>>,
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
                        commands
                            .entity(*features_knobs_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                        commands
                            .entity(*terrain_knobs_hud)
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
                     features_knobs_hud: Single<Entity, With<FeaturesHudKnobs>>,
                     terrain_knobs_hud: Single<Entity, With<TerrainHudKnobs>>,
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
                        commands
                            .entity(*features_knobs_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                        commands
                            .entity(*terrain_knobs_hud)
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
                     features_knobs_hud: Single<Entity, With<FeaturesHudKnobs>>,
                     terrain_knobs_hud: Single<Entity, With<TerrainHudKnobs>>,
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
                        commands
                            .entity(*features_knobs_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                        commands
                            .entity(*terrain_knobs_hud)
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
                     features_knobs_hud: Single<Entity, With<FeaturesHudKnobs>>,
                     terrain_knobs_hud: Single<Entity, With<TerrainHudKnobs>>,
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
                        commands
                            .entity(*features_knobs_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::Flex);
                        commands
                            .entity(*terrain_knobs_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                    },
                );
                c.spawn(make_hud_button_bundle(
                    "TerrainMaker",
                    HudButtonTool(PenType::TerrainMaker),
                    &editor,
                ))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-realm.ktx2",
                ))
                .toggle_tool::<HudButtonTool>()
                .hover_effect_ex(true)
                .observe(
                    |_: On<Pointer<Click>>,
                     mut editor: ResMut<MapEditor>,
                     terrain_hud: Single<Entity, With<DrawingHudTerrain>>,
                     feature_hud: Single<Entity, With<DrawingHudFeatures>>,
                     features_knobs_hud: Single<Entity, With<FeaturesHudKnobs>>,
                     terrain_knobs_hud: Single<Entity, With<TerrainHudKnobs>>,
                     mut commands: Commands| {
                        editor.pen = PenType::TerrainMaker;
                        commands
                            .entity(*terrain_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                        commands
                            .entity(*feature_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                        commands
                            .entity(*features_knobs_hud)
                            .entry::<Node>()
                            .and_modify(|mut n| n.display = Display::None);
                        commands
                            .entity(*terrain_knobs_hud)
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
                Name::new("TerrainHudKnobs"),
                TerrainHudKnobs,
                Node {
                    top: Val::Px(20.0),
                    justify_self: JustifySelf::Center,
                    display: if editor.pen.show_terrain_knobs_bar() {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
            ))
            .with_children(|c| {
                spawn_terrain_knobs(c, &asset_server, &editor);
                spawn_volume_knobs(c, &asset_server, &editor);
            });
            c.spawn((
                Name::new("FeaturesHudKnobs"),
                FeaturesHudKnobs,
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
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
            ))
            .with_children(|c| {
                spawn_feature_knobs(c, &asset_server, &editor);
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

#[derive(Component)]
struct FeaturesHudKnobs;

#[derive(Component)]
struct TerrainHudKnobs;

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

pub fn spawn_terrain_knobs(
    c: &mut RelatedSpawnerCommands<'_, ChildOf>,
    asset_server: &AssetServer,
    editor: &MapEditor,
) {
    c.spawn_empty().spawn_knob_ex::<MapEditor>(
        false,
        |editor, value| {
            let knob = editor
                .intent
                .entry(TerrainType::ForestHex.clone())
                .or_insert(0);
            *knob = value;
        },
        |editor| *editor.intent.get(&TerrainType::ForestHex).unwrap_or(&0) as f32,
        editor,
        0.9,
        "icons/icon-forest.ktx2",
        "Forests Probability",
        &asset_server,
        64.0,
        Color::srgb_u8(110, 153, 68),
    );
    c.spawn_empty().spawn_knob_ex::<MapEditor>(
        false,
        |editor, value| {
            let knob = editor
                .intent
                .entry(TerrainType::MountainsHex.clone())
                .or_insert(0);
            *knob = value;
        },
        |editor| *editor.intent.get(&TerrainType::MountainsHex).unwrap_or(&0) as f32,
        editor,
        0.9,
        "icons/icon-mountains.ktx2",
        "Mountains Probability",
        &asset_server,
        64.0,
        Color::srgb_u8(153, 126, 68),
    );
    c.spawn_empty().spawn_knob_ex::<MapEditor>(
        false,
        |editor, value| {
            let knob = editor
                .intent
                .entry(TerrainType::PlainsHex.clone())
                .or_insert(0);
            *knob = value;
        },
        |editor| *editor.intent.get(&TerrainType::PlainsHex).unwrap_or(&0) as f32,
        editor,
        0.9,
        "icons/icon-plains.ktx2",
        "Grasslands Probability",
        &asset_server,
        64.0,
        Color::srgb_u8(173, 199, 112),
    );
    c.spawn_empty().spawn_knob_ex::<MapEditor>(
        false,
        |editor, value| {
            let knob = editor
                .intent
                .entry(TerrainType::SwampsHex.clone())
                .or_insert(0);
            *knob = value;
        },
        |editor| *editor.intent.get(&TerrainType::SwampsHex).unwrap_or(&0) as f32,
        editor,
        0.9,
        "icons/icon-swamps.ktx2",
        "Wetlands/Swamps Probability",
        &asset_server,
        64.0,
        Color::srgb_u8(199, 180, 112),
    );
    c.spawn_empty().spawn_knob_ex::<MapEditor>(
        false,
        |editor, value| {
            let knob = editor
                .intent
                .entry(TerrainType::DesertHex.clone())
                .or_insert(0);
            *knob = value;
        },
        |editor| *editor.intent.get(&TerrainType::DesertHex).unwrap_or(&0) as f32,
        editor,
        0.9,
        "icons/icon-desert.ktx2",
        "Deserts Probability",
        &asset_server,
        64.0,
        Color::srgb_u8(226, 207, 183),
    );
    c.spawn_empty().spawn_knob_ex::<MapEditor>(
        false,
        |editor, value| {
            let knob = editor
                .intent
                .entry(TerrainType::JungleHex.clone())
                .or_insert(0);
            *knob = value;
        },
        |editor| *editor.intent.get(&TerrainType::JungleHex).unwrap_or(&0) as f32,
        editor,
        0.9,
        "icons/icon-jungle.ktx2",
        "Jungles Probability",
        &asset_server,
        64.0,
        Color::srgb_u8(199, 230, 250),
    );
    c.spawn_empty().spawn_knob_ex::<MapEditor>(
        false,
        |editor, value| {
            let knob = editor
                .intent
                .entry(TerrainType::TundraHex.clone())
                .or_insert(0);
            *knob = value;
        },
        |editor| *editor.intent.get(&TerrainType::TundraHex).unwrap_or(&0) as f32,
        editor,
        0.9,
        "icons/icon-tundra.ktx2",
        "Tundras Probability",
        &asset_server,
        64.0,
        Color::srgb_u8(106, 190, 112),
    );
}

pub fn spawn_volume_knobs(
    c: &mut RelatedSpawnerCommands<'_, ChildOf>,
    asset_server: &AssetServer,
    editor: &MapEditor,
) {
    c.spawn_empty().spawn_knob::<MapEditor>(
        false,
        |editor, value| {
            editor.volume.target = value;
        },
        |editor| editor.volume.target as f32,
        editor,
        1.0,
        "icons/icon-region.ktx2",
        "Region Scale",
        &asset_server,
        80.0,
    );
    c.spawn_empty().spawn_knob::<MapEditor>(
        true,
        |editor, value| {
            editor.budget.target = value;
        },
        |editor| editor.budget.target as f32,
        editor,
        0.5,
        "icons/icon-realm.ktx2",
        "Realm Scale",
        &asset_server,
        96.0,
    );
}

pub fn spawn_feature_knobs(
    c: &mut RelatedSpawnerCommands<'_, ChildOf>,
    asset_server: &AssetServer,
    editor: &MapEditor,
) {
    c.spawn_empty().spawn_knob::<MapEditor>(
        true,
        |editor, value| {
            let knob = editor
                .knobs
                .entry(HexFeature::Dungeon.clone())
                .or_insert(Knob::default());
            knob.target = value;
        },
        |editor| {
            editor
                .knobs
                .get(&HexFeature::Dungeon)
                .map_or(0, |k| k.target) as f32
        },
        editor,
        get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::Dungeon),
        "icons/icon-dungeon.ktx2",
        "Dungeons",
        &asset_server,
        64.0,
    );
    c.spawn_empty().spawn_knob::<MapEditor>(
        true,
        |editor, value| {
            let knob = editor
                .knobs
                .entry(HexFeature::City.clone())
                .or_insert(Knob::default());
            knob.target = value;
        },
        |editor| editor.knobs.get(&HexFeature::City).map_or(0, |k| k.target) as f32,
        editor,
        get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::City),
        "icons/icon-city.ktx2",
        "Cities",
        &asset_server,
        64.0,
    );
    c.spawn_empty().spawn_knob::<MapEditor>(
        true,
        |editor, value| {
            let knob = editor
                .knobs
                .entry(HexFeature::Town.clone())
                .or_insert(Knob::default());
            knob.target = value;
        },
        |editor| editor.knobs.get(&HexFeature::Town).map_or(0, |k| k.target) as f32,
        editor,
        get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::Town),
        "icons/icon-town.ktx2",
        "Towns",
        &asset_server,
        64.0,
    );
    c.spawn_empty().spawn_knob::<MapEditor>(
        true,
        |editor, value| {
            let knob = editor
                .knobs
                .entry(HexFeature::Village.clone())
                .or_insert(Knob::default());
            knob.target = value;
        },
        |editor| {
            editor
                .knobs
                .get(&HexFeature::Village)
                .map_or(0, |k| k.target) as f32
        },
        editor,
        get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::Village),
        "icons/icon-village.ktx2",
        "Villages",
        &asset_server,
        64.0,
    );
    c.spawn_empty().spawn_knob::<MapEditor>(
        true,
        |editor, value| {
            let knob = editor
                .knobs
                .entry(HexFeature::Inn.clone())
                .or_insert(Knob::default());
            knob.target = value;
        },
        |editor| editor.knobs.get(&HexFeature::Inn).map_or(0, |k| k.target) as f32,
        editor,
        get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::Inn),
        "icons/icon-inn.ktx2",
        "Inns (outside settlements)",
        &asset_server,
        64.0,
    );
    c.spawn_empty().spawn_knob::<MapEditor>(
        true,
        |editor, value| {
            let knob = editor
                .knobs
                .entry(HexFeature::Residency.clone())
                .or_insert(Knob::default());
            knob.target = value;
        },
        |editor| {
            editor
                .knobs
                .get(&HexFeature::Residency)
                .map_or(0, |k| k.target) as f32
        },
        editor,
        get_feature_ratio_for_realm_type(&editor.realm_type, HexFeature::Residency),
        "icons/icon-dwelling.ktx2",
        "Dwellings (outside settlements)",
        &asset_server,
        64.0,
    );
}
