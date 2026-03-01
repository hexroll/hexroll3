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

// Hexmap Spawning Behavior ------+
//                                |
//                                V
// +-------+     +-------+     +-------+
// | setup | --> | stage | --> | spawn |
// +-------+     +-------+     +-------+
//     ^             ^             ^
//     |             |             |
// [Startup]    [Map Loaded]   [Each Frame]
//
use std::collections::VecDeque;

use bevy::{
    camera::visibility::RenderLayers,
    platform::collections::{HashMap, HashSet},
    prelude::*,
};

use bevy_editor_cam::prelude::{EditorCam, motion::CurrentMotion};
use bevy_mod_billboard::BillboardText;
use hexx::{Hex, HexLayout, HexOrientation, OffsetHexMode, shapes};

use crate::{
    battlemaps::BattlemapDialProvider,
    hexmap::{
        curve_tiles::*, data::*, daynight::*, elements::*, revealing::VttHexRevealer, tiles::*,
    },
    shared::{
        layers::{
            HEIGHT_OF_FEATURE_ON_LAYERED_TILE, HEIGHT_OF_TILES_BACKGROUND_HEX,
            HEIGHT_OF_TOP_MOST_LAYERED_TILE, RENDER_LAYER_MAP_COORDS_HIRES,
            RENDER_LAYER_MAP_COORDS_LOWRES, RENDER_LAYER_MAP_COORDS_MEDRES,
            RENDER_LAYER_MAP_LOD_HIGH, RENDER_LAYER_MAP_LOD_LOW, RENDER_LAYER_MAP_LOD_MEDIUM,
        },
        settings::AppSettings,
        vtt::{HexMapMode, HexRevealState, VttData},
    },
};

#[derive(Component)]
pub struct ExpensiveHex;

#[inline]
pub fn spawn_tile(
    commands: &mut Commands,
    layout: &HexLayout,
    hex: Hex,
    map_parent: Entity,
    tiles: &Res<HexMapTileMaterials>,
    map_resources: &Res<HexMapResources>,
    map_data: &ResMut<HexMapData>,
    vtt_data: &ResMut<VttData>,
) {
    let tiles_z_offset = tiles.tiles_z_offset;
    let pos = layout.hex_to_world_pos(hex);
    let pos = pos.floor();
    let mut base = commands.spawn_empty();
    base.insert(ChildOf(map_parent));
    let (overlayer, underlayer, is_dungeon) =
        get_tile_background_material(hex, map_data, tiles, map_resources);
    let (xc, yc) = hexx_to_hexroll_coords(hex);
    let display_coords = hexroll_coords_to_string(xc, yc);
    base.insert((
        Name::new(format!("Hex {}", display_coords)),
        HexEntity { hex },
        Mesh3d(map_resources.mesh.clone()),
        MeshMaterial3d(overlayer),
        RenderLayers::layer(RENDER_LAYER_MAP_LOD_LOW),
        Visibility::default(),
        Transform::from_xyz(pos.x, HEIGHT_OF_TILES_BACKGROUND_HEX, pos.y),
        Pickable {
            should_block_lower: false,
            is_hoverable: false,
        },
    ));
    if let Some(hex_data) = map_data.hexes.get(&hex) {
        let terrain_material = if vtt_data.mode.is_player() {
            if vtt_data.revealed.get(&hex) == Some(&HexRevealState::Partial) {
                hex_data.partial_hex_tile_material.clone()
            } else {
                hex_data.hex_tile_material.clone()
            }
        } else {
            hex_data.hex_tile_material.clone()
        };
        let y_index = HEIGHT_OF_TOP_MOST_LAYERED_TILE + (pos.y / 200.0) + (pos.x / 1000.0);
        if let Some(mat) = underlayer {
            if vtt_data.revealed.get(&hex) == Some(&HexRevealState::Full) && is_dungeon {
                base.with_child((
                    DungeonUnderlayer {
                        hex,
                        elevation_change_delay_in_frames: 5,
                    },
                    Mesh3d(map_resources.mesh.clone()),
                    // NOTE: Dungeons and caves get an underworld (dark) background. Other features
                    // get their usual hex background color.
                    MeshMaterial3d(map_resources.underworld_material.clone()),
                    Transform::from_xyz(0.0, -1.0, 0.0), // When revealed and not loaded
                ));
            } else {
                base.with_child((
                    Mesh3d(map_resources.mesh.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(0.0, -15.0, 0.0), // When revealed and loaded or not revealed
                ));
            }
        }

        base.with_child((
            Name::new("HexLayerMesh"),
            bevy::render::batching::NoAutomaticBatching, // NOTE: crucial to prevent flickering
            Mesh3d(map_resources.layer_mesh.clone()),
            MeshMaterial3d(terrain_material),
            RenderLayers::layer(RENDER_LAYER_MAP_LOD_MEDIUM),
            Transform::from_xyz(0.0, y_index, tiles_z_offset).with_scale(Vec3::new(
                hex_data.tile_scale,
                1.0,
                hex_data.tile_scale,
            )),
            Pickable {
                should_block_lower: false,
                is_hoverable: false,
            },
        ))
        .with_children(|mut c| {
            c.spawn_empty()
                .battlemap_dial_provider()
                .insert(Name::new("HexFeature"))
                .insert(Transform::from_xyz(
                    0.0,
                    HEIGHT_OF_FEATURE_ON_LAYERED_TILE,
                    0.0,
                ))
                .insert(RenderLayers::layer(RENDER_LAYER_MAP_LOD_HIGH))
                .insert(HexCoordsForFeature { hex })
                .insert(hex_data.hex_type.clone())
                .insert(hex_data.feature.clone())
                .insert(Visibility::Inherited)
                .insert(HexUid {
                    uid: hex_data.uid.clone(),
                });
            if let Some(tile) = &hex_data.trail_tile {
                for t in tile {
                    spawn_trail_tile(&mut c, t.0, &t.1, &map_resources.trail_material);
                }
            }

            // FIXME: this should actually be toggled for players using the
            // lables toggle
            if vtt_data.mode != HexMapMode::Player {
                c.spawn((
                    Name::new("MapCoordsLowRes"),
                    RenderLayers::layer(RENDER_LAYER_MAP_COORDS_LOWRES),
                    BillboardText::default(),
                    TextLayout::new_with_justify(Justify::Center),
                    Transform::from_translation(Vec3::new(0.0, y_index + 1.0, -64.0))
                        .with_scale(Vec3::splat(0.4 * 3.75 * 2.0)),
                ))
                .with_child((
                    TextSpan::new(display_coords.clone()),
                    TextFont::from(map_resources.coords_font.clone()).with_font_size(8.0),
                    TextColor::from(Color::WHITE.with_alpha(0.7)),
                ));
                c.spawn((
                    Name::new("MapCoordsMedRes"),
                    RenderLayers::layer(RENDER_LAYER_MAP_COORDS_MEDRES),
                    BillboardText::default(),
                    TextLayout::new_with_justify(Justify::Center),
                    Transform::from_translation(Vec3::new(0.0, y_index + 1.0, -64.0))
                        .with_scale(Vec3::splat(0.4 * 3.75)),
                ))
                .with_child((
                    TextSpan::new(display_coords.clone()),
                    TextFont::from(map_resources.coords_font.clone()).with_font_size(16.0),
                    TextColor::from(Color::WHITE.with_alpha(0.9)),
                ));
                c.spawn((
                    Name::new("MapCoordsHiRes"),
                    RenderLayers::layer(RENDER_LAYER_MAP_COORDS_HIRES),
                    BillboardText::default(),
                    TextLayout::new_with_justify(Justify::Center),
                    Transform::from_translation(Vec3::new(0.0, y_index + 1.0, -64.0))
                        .with_scale(Vec3::splat(0.4)),
                ))
                .with_child((
                    TextSpan::new(display_coords),
                    TextFont::from(map_resources.coords_font.clone()).with_font_size(60.0),
                    TextColor::from(Color::WHITE),
                ));
            }
        })
        .with_child((
            Name::new("HexVttMask"),
            HexMask(hex),
            Pickable {
                should_block_lower: false,
                is_hoverable: false,
            },
            Mesh3d(map_resources.mesh.clone()),
            MeshMaterial3d(map_resources.hex_mask_material.clone()),
            RenderLayers::layer(RENDER_LAYER_MAP_LOD_LOW),
            vtt_data.get_reveal_state_components(&hex),
        ));

        if let Some(tiles) = &hex_data.river_tile {
            for tile in tiles {
                spawn_river_tile(
                    &mut base,
                    tile.0,
                    &tile.1,
                    &map_resources.river_tile_materials,
                );
            }
        }
    }
}

pub fn invalidate_hex(
    mut commands: Commands,
    hexes: Query<(Entity, &HexEntity), With<HexToInvalidateMarker>>,
    map_parent: Single<Entity, With<HexMapTime>>,
    tiles: Res<HexMapTileMaterials>,
    map_resources: Res<HexMapResources>,
    map_data: ResMut<HexMapData>,
    vtt_data: ResMut<VttData>,
) {
    let layout = y_inverted_hexmap_layout();

    for (entity, hex) in hexes.iter() {
        commands.entity(entity).despawn();
        spawn_tile(
            &mut commands,
            &layout,
            hex.hex,
            *map_parent,
            &tiles,
            &map_resources,
            &map_data,
            &vtt_data,
        );
    }
}

pub fn update_hex_map_tiles(
    hexes: Query<(Entity, &HexEntity)>,
    cameras: Query<(&GlobalTransform, &Projection), With<MainCamera>>,
    mut map_data: ResMut<HexMapData>,
    mut vtt_data: ResMut<VttData>,
    mut q: ResMut<TileSpawnQueues>,
    expensives: Query<&ExpensiveHex>,

    mut commands: Commands,
    map_parent: Single<Entity, With<HexMapTime>>,
    map_resources: Res<HexMapResources>,
) {
    if let Ok((cam_transform, proj)) = cameras.single() {
        let layout = y_inverted_hexmap_layout();

        if let Projection::Orthographic(proj) = proj {
            let view_size_halved = (proj.area.max - proj.area.min) / 2.0;
            let cmin =
                layout.world_pos_to_hex(cam_transform.translation().xz() - view_size_halved);
            let cmax =
                layout.world_pos_to_hex(cam_transform.translation().xz() + view_size_halved);
            if cmin == map_data.cmin && cmax == map_data.cmax && !vtt_data.invalidate_map {
                return;
            }
            map_data.cmin = cmin;
            map_data.cmax = cmax;
            vtt_data.invalidate_map = false;

            let mut hex_map: HashMap<_, _> = HashMap::new();
            hex_map.reserve(hexes.iter().len());
            for (e, h) in hexes.iter() {
                hex_map.insert(h.hex, e);
            }

            let [cmin_x, cmin_y] =
                cmin.to_offset_coordinates(OffsetHexMode::Even, HexOrientation::Flat);
            let [cmax_x, cmax_y] =
                cmax.to_offset_coordinates(OffsetHexMode::Even, HexOrientation::Flat);

            let ocean_marker_visibility =
                if vtt_data.mode == HexMapMode::RefereeRevealing || vtt_data.is_player() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };

            let buffer = 1;
            shapes::flat_rectangle([
                cmin_x - buffer,
                cmax_x + buffer,
                cmax_y - buffer,
                cmin_y + buffer,
            ])
            .for_each(|hex| {
                if vtt_data.revealed_ocean.contains(&hex) && !hex_map.contains_key(&hex) {
                    spawn_ocean_marker(
                        &mut commands,
                        &layout,
                        hex,
                        *map_parent,
                        &map_resources,
                        ocean_marker_visibility,
                    );
                    return;
                }
                if !map_data.hexes.contains_key(&hex) {
                    hex_map.remove(&hex);
                    return;
                }
                if !hex_map.contains_key(&hex)
                    && (!vtt_data.mode.is_player() || vtt_data.revealed.contains_key(&hex))
                {
                    if !q.spawn_queue.queued(&hex) {
                        q.spawn_queue.queue(hex);
                    }
                }
                hex_map.remove(&hex);
            });
            q.despawn_queue.clear();
            for value in hex_map.values() {
                if !q.despawn_queue.queued(value) && !expensives.contains(*value) {
                    q.despawn_queue.queue(*value);
                }
            }
        }
    }
}

pub struct TileQueue<T: Eq + core::hash::Hash + Copy> {
    pub queue: VecDeque<T>,
    pub set: HashSet<T>,
}

impl<T: Eq + core::hash::Hash + Copy> Default for TileQueue<T> {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            set: Default::default(),
        }
    }
}

impl<T: Eq + core::hash::Hash + Copy> TileQueue<T> {
    pub fn queue(&mut self, hex: T) {
        self.queue.push_back(hex);
        self.set.insert(hex);
    }
    pub fn queued(&mut self, hex: &T) -> bool {
        return self.set.contains(hex);
    }
    pub fn clear(&mut self) {
        self.queue.clear();
        self.set.clear();
    }
}

#[derive(Resource, Default)]
pub struct TileSpawnQueues {
    pub spawn_queue: TileQueue<Hex>,
    pub despawn_queue: TileQueue<Entity>,
}

pub fn spawn_tile_from_queue(
    mut q: ResMut<TileSpawnQueues>,
    mut commands: Commands,
    map_parent: Single<Entity, With<HexMapTime>>,
    tiles: Res<HexMapTileMaterials>,
    map_resources: Res<HexMapResources>,
    map_data: ResMut<HexMapData>,
    vtt_data: ResMut<VttData>,
    map_cam: Single<&mut EditorCam>,
) {
    let batch_size = match &map_cam.current_motion {
        CurrentMotion::UserControlled {
            anchor: _,
            motion_inputs: _,
        } => 30,
        _ => 300,
    };

    let layout = y_inverted_hexmap_layout();
    let l = q.spawn_queue.queue.len();
    let mut backlog: Vec<Hex> = Vec::new();
    q.spawn_queue
        .queue
        .drain(..std::cmp::min(batch_size, l))
        .for_each(|hex| {
            backlog.push(hex.clone());
            spawn_tile(
                &mut commands,
                &layout,
                hex,
                *map_parent,
                &tiles,
                &map_resources,
                &map_data,
                &vtt_data,
            );
        });
    for hex in backlog {
        q.spawn_queue.set.remove(&hex);
    }
}

pub fn despawn_tile_from_queue(
    mut q: ResMut<TileSpawnQueues>,
    mut commands: Commands,
    map_cam: Single<&mut EditorCam>,
) {
    let batch_size = match &map_cam.current_motion {
        CurrentMotion::UserControlled {
            anchor: _,
            motion_inputs: _,
        } => 10,
        _ => 100,
    };
    let l = q.despawn_queue.queue.len();
    let mut backlog: Vec<Entity> = Vec::new();
    q.despawn_queue
        .queue
        .drain(..std::cmp::min(batch_size, l))
        .for_each(|hex| {
            backlog.push(hex.clone());
            commands.entity(hex).try_despawn();
        });
    for hex in backlog {
        q.despawn_queue.set.remove(&hex);
    }
}

fn get_tile_background_material(
    hex: Hex,
    map: &ResMut<HexMapData>,
    tiles: &Res<HexMapTileMaterials>,
    map_resources: &Res<HexMapResources>,
) -> (
    Handle<BackgroundMaterial>,         // Overlayer
    Option<Handle<BackgroundMaterial>>, // Underlayer
    bool,                               // is_dungeon
) {
    if map.hexes.contains_key(&hex) {
        let hex_data = map.hexes.get(&hex).unwrap();
        let materials = &tiles.terrain_background_materials;
        // The overlayer variant of tile materials is becoming transparent
        // as we zoom in to reveal the underlying feature entities.
        (
            materials.get(&hex_data.hex_type).unwrap().overlayer.clone(),
            Some(materials.get(&hex_data.hex_type).unwrap().empty.clone()),
            // Dungeons are always a bit special :) let our caller
            // know.
            hex_data.feature == HexFeature::Dungeon,
        )
    } else {
        (map_resources.ocean_material.clone(), None, false)
    }
}

pub fn control_lod_feature_visibility(
    mut commands: Commands,
    cam: Single<(Entity, &Projection), With<MainCamera>>,
    settings: Res<AppSettings>,
) {
    if let Projection::Orthographic(proj) = cam.1 {
        if proj.scale > 30.0 {
            commands
                .entity(cam.0)
                .insert(RenderLayers::from_layers(&[RENDER_LAYER_MAP_LOD_LOW]));
        } else if proj.scale > 10.0 {
            commands.entity(cam.0).insert(RenderLayers::from_layers(&[
                RENDER_LAYER_MAP_LOD_LOW,
                RENDER_LAYER_MAP_LOD_MEDIUM,
            ]));
        } else if proj.scale > 5.0 {
            commands.entity(cam.0).insert(RenderLayers::from_layers(&[
                RENDER_LAYER_MAP_LOD_LOW,
                RENDER_LAYER_MAP_LOD_MEDIUM,
                RENDER_LAYER_MAP_LOD_HIGH,
            ]));
        } else if proj.scale > 3.0 && settings.labels_mode.hex_coords_visible() {
            commands.entity(cam.0).insert(RenderLayers::from_layers(&[
                RENDER_LAYER_MAP_LOD_LOW,
                RENDER_LAYER_MAP_LOD_MEDIUM,
                RENDER_LAYER_MAP_LOD_HIGH,
                RENDER_LAYER_MAP_COORDS_LOWRES,
            ]));
        } else if proj.scale > 2.0 && settings.labels_mode.hex_coords_visible() {
            commands.entity(cam.0).insert(RenderLayers::from_layers(&[
                RENDER_LAYER_MAP_LOD_LOW,
                RENDER_LAYER_MAP_LOD_MEDIUM,
                RENDER_LAYER_MAP_LOD_HIGH,
                RENDER_LAYER_MAP_COORDS_MEDRES,
            ]));
        } else if proj.scale > 0.2 && settings.labels_mode.hex_coords_visible() {
            commands.entity(cam.0).insert(RenderLayers::from_layers(&[
                RENDER_LAYER_MAP_LOD_LOW,
                RENDER_LAYER_MAP_LOD_MEDIUM,
                RENDER_LAYER_MAP_LOD_HIGH,
                RENDER_LAYER_MAP_COORDS_HIRES,
            ]));
        } else {
            commands.entity(cam.0).insert(RenderLayers::from_layers(&[
                RENDER_LAYER_MAP_LOD_LOW,
                RENDER_LAYER_MAP_LOD_MEDIUM,
                RENDER_LAYER_MAP_LOD_HIGH,
            ]));
        }
    }
}

#[derive(Component)]
pub(crate) struct OceanMarker;

#[inline]
pub fn spawn_ocean_marker(
    commands: &mut Commands,
    layout: &HexLayout,
    hex: Hex,
    map_parent: Entity,
    map_resources: &Res<HexMapResources>,
    visibility: Visibility,
) {
    let pos = layout.hex_to_world_pos(hex);
    let mut base = commands.spawn_empty();
    base.insert(ChildOf(map_parent));
    base.insert((
        OceanMarker,
        Name::new("OceanMarker"),
        HexEntity { hex },
        Mesh3d(map_resources.mesh.clone()),
        MeshMaterial3d(map_resources.water_material.clone()),
        RenderLayers::layer(RENDER_LAYER_MAP_LOD_LOW),
        Transform::from_xyz(pos.x, HEIGHT_OF_TILES_BACKGROUND_HEX, pos.y),
        visibility,
        Pickable {
            should_block_lower: false,
            is_hoverable: false,
        },
    ));
}
