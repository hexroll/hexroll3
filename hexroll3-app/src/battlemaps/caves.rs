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

// Cave maps - using hexroll's cave map generator dataset
use bevy::prelude::*;

use avian3d::prelude::{ColliderConstructor, CollisionLayers, RigidBody};
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::Deserialize;

use crate::{
    audio::{DungeonAudioSample, PlayDungeonSound},
    clients::model::{BackendUid, FetchEntityReason},
    hexmap::{
        BackgroundMaterial,
        elements::{
            FetchEntityFromStorage, HexCoordsForFeature, HexMapResources,
            MapVisibilityController,
        },
        update_hex_map_tiles,
    },
    shared::{
        dragging::DraggingMotionDetector,
        geometry::{make_filled_mesh_from_outline, make_mesh_from_outline},
        labels::spawn_area_labels,
        layers::{HEIGHT_OF_BATTLEMAP_ON_FEATURE, HexrollPhysicsLayer},
        spawnq::SpawnQueue,
        vtt::{HexRevealState, VttData},
        widgets::cursor::PointerOnHover,
    },
};

use super::{
    BattlemapFeatureUtils, DUNGEON_FOG_COLOR,
    battlemaps::{BattlemapMaterial, BattlemapMaterialControls, DEFAULT_BATTLEMAP_COLOR},
};

pub struct CavesPlugin;
impl Plugin for CavesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_observer(on_spawn_cave_map)
            .add_systems(Update, cave_maps_decorations.after(update_hex_map_tiles));
    }
}

/// SpawnCaveMap is triggered after a backend client finished loading a preparing
/// a cave.
/// Orchestrating caves and other battlemaps spawning is done via the BattlemapsPlugin.
#[derive(Event)]
pub struct SpawnCaveMap {
    pub hex: Entity,
    pub data: CaveMapConstructs,
}

/// Cave map constructs required to spawn a cave map
/// Should be prepared in a separate task (async)
pub struct CaveMapConstructs {
    caverns: Vec<CavernMeshes>,
    map: CaveMap,
    all_points: Vec<CavePoint>,
}

pub struct CavernMeshes {
    uid: String,
    wall: Mesh,
    outline: Mesh,
    floor: Mesh,
}

impl CaveMapConstructs {
    pub fn from(json: String, uid: BackendUid) -> Self {
        // FIXME: cleanup this expect
        let map: CaveMap = serde_json::from_str(&json).expect("Failed to parse JSON");
        let mut all_points: Vec<CavePoint> = Vec::new();
        let mut rng = StdRng::seed_from_u64(uid.as_u64_hash());
        let caverns = map
            .caverns
            .iter()
            .map(|c| {
                let a: Vec<CavePoint> = c
                    .polygon
                    .iter()
                    .map(|p| CavePoint {
                        coords: Vec2::new(p.x as f32, p.y as f32),
                        owners: p.c,
                    })
                    .collect();

                let subd_poly = subdivide_polygon(&mut rng, smoother_polygon(a));
                let final_poly = noisify_polygon(&mut rng, subd_poly);

                let output: Vec<lyon::math::Point> = final_poly
                    .iter()
                    .map(|p| lyon::math::Point::new(p.coords.x, p.coords.y))
                    .collect();
                let cavern_meshes = CavernMeshes {
                    uid: c.uuid.clone(),
                    wall: generate_polygon_wall(&c.uuid, &final_poly, map.caverns.as_slice()),
                    outline: make_mesh_from_outline(&output, 5.0),
                    floor: make_filled_mesh_from_outline(&output),
                };
                all_points.extend(final_poly);
                cavern_meshes
            })
            .collect();

        CaveMapConstructs {
            caverns,
            map,
            all_points,
        }
    }
}

#[derive(Resource)]
struct CaveMapResources {
    pub wall_material: Handle<StandardMaterial>,
    pub background_material: Handle<BackgroundMaterial>,
    pub floor_material: Handle<BattlemapMaterial>,
    pub rubble_sphere: Handle<Mesh>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials2: ResMut<Assets<BattlemapMaterial>>,
    mut materials3: ResMut<Assets<BackgroundMaterial>>,
) {
    let wall_material = materials.add(StandardMaterial {
        base_color: DUNGEON_FOG_COLOR,
        reflectance: 0.0,
        perceptual_roughness: 1.0,
        unlit: true,
        ..Default::default()
    });
    let background_material = materials3.add(BackgroundMaterial {
        base_color: DUNGEON_FOG_COLOR.into(),
        layer_color: DUNGEON_FOG_COLOR.into(),
    });
    let floor_material = materials2.add(BattlemapMaterial {
        controls: BattlemapMaterialControls {
            zoom_factor: 0.0,
            grid_mix: 1.0,
            blend: 1.0,
            scale: 1.0,
        },
        color: Vec4::from(DEFAULT_BATTLEMAP_COLOR),
        offset: Vec4::new(0.017, 0.015, 0.015, 0.0),
    });
    let rubble_sphere = meshes.add(Sphere::new(1.0));

    commands.insert_resource(CaveMapResources {
        wall_material,
        background_material,
        floor_material,
        rubble_sphere,
    });
}

#[derive(Component)]
struct CaveMapPolygon {
    polygon: Vec<CavePoint>,
}

#[derive(Component)]
struct CaveMapIsDecoratedMarker;

fn cave_maps_decorations(
    mut commands: Commands,
    maps_to_decorate: Query<(Entity, &CaveMapPolygon), Without<CaveMapIsDecoratedMarker>>,
    cave_map_resources: Res<CaveMapResources>,
    visibility_controller: Res<MapVisibilityController>,
) {
    if visibility_controller.is_cave_decorations_visible() {
        for (entity, polygon) in maps_to_decorate.iter() {
            if let Ok(mut entity_commands) = commands.get_entity(entity) {
                entity_commands.try_insert(CaveMapIsDecoratedMarker);
                entity_commands.with_children(|commands| {
                    let mut index = 0;
                    for point in &polygon.polygon {
                        let mut positions = Vec::new();
                        positions.push(VALS[index]);
                        index = (index + 1) % VALS.len();
                        positions.push(VALS[index]);
                        index = (index + 1) % VALS.len();
                        positions.push(VALS[index]);
                        index = (index + 1) % VALS.len();
                        positions.push(VALS[index]);
                        index = (index + 1) % VALS.len();

                        if point.owners == 1 && (point.coords.x as i32).abs() % 3 == 1 {
                            for pos in &positions {
                                commands.spawn((
                                    Mesh3d(cave_map_resources.rubble_sphere.clone()),
                                    MeshMaterial3d(cave_map_resources.wall_material.clone()),
                                    Transform::from_xyz(
                                        point.coords.x + pos.0 as f32,
                                        0.0,
                                        point.coords.y + pos.1 as f32,
                                    )
                                    .with_scale(Vec3::splat(pos.2)),
                                ));
                            }
                        }
                    }
                });
            }
        }
    }
}

fn on_spawn_cave_map(
    trigger: On<SpawnCaveMap>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    map_resources: Res<HexMapResources>,
    cave_map_resources: Res<CaveMapResources>,
    mut battlemap_materials: ResMut<Assets<BattlemapMaterial>>,
    vtt_data: Res<VttData>,
    mut q: ResMut<SpawnQueue>,
    coords: Query<&HexCoordsForFeature>,
) {
    let is_revealed = if let Ok(coords) = coords.get(trigger.event().hex) {
        vtt_data.revealed.get(&coords.hex) == Some(&HexRevealState::Full)
    } else {
        return;
    };
    if is_revealed {
        commands.trigger(PlayDungeonSound {
            hex_entity: trigger.hex,
            sample: DungeonAudioSample::Cave,
        });
    }

    let is_player = vtt_data.is_player();
    battlemap_materials
        .get_mut(&cave_map_resources.floor_material)
        .unwrap()
        .controls
        .grid_mix = if is_player { 0.0 } else { 1.0 };
    let parent_node = commands
        .spawn_empty()
        .insert(Name::new("CaveMap"))
        .insert(Visibility::default())
        .insert(
            Transform::from_xyz(0.0, HEIGHT_OF_BATTLEMAP_ON_FEATURE - 5.0, 0.0)
                .with_scale(Vec3::splat(0.10)),
        )
        .with_children(|commands| {
            commands.spawn((
                Mesh3d(map_resources.mesh.clone()),
                MeshMaterial3d(cave_map_resources.background_material.clone()),
                Transform::from_xyz(0.0, -10.0, 0.0).with_scale(Vec3::splat(10.0)),
            ));
            for c in trigger.event().data.caverns.iter() {
                commands
                    .spawn((
                        Mesh3d(meshes.add(c.wall.clone())),
                        MeshMaterial3d(cave_map_resources.wall_material.clone()),
                        CollisionLayers::new(
                            [HexrollPhysicsLayer::Walls],
                            [HexrollPhysicsLayer::Tokens],
                        ),
                    ))
                    .insert_if(
                        (RigidBody::Static, ColliderConstructor::TrimeshFromMesh),
                        || is_revealed,
                    );
                //
                commands.spawn((
                    Mesh3d(meshes.add(c.outline.clone())),
                    MeshMaterial3d(cave_map_resources.wall_material.clone()),
                    Transform::from_xyz(0.0, -0.5, 0.0),
                ));
                //
                let mut area_floor = commands.spawn((
                    Mesh3d(meshes.add(c.floor.clone())),
                    MeshMaterial3d(cave_map_resources.floor_material.clone()),
                    AreaUid(c.uid.clone()),
                ));
                if !is_player {
                    area_floor
                        .observe(
                            |trigger: On<Pointer<Click>>,
                             camera_motion_state: Res<DraggingMotionDetector>,
                             mut commands: Commands,
                             token_gizmo: Query<&super::BattlemapSelection>,
                             area_uids: Query<&AreaUid>| {
                                if !token_gizmo.is_empty() {
                                    return;
                                }
                                if trigger.button != PointerButton::Primary {
                                    return;
                                }
                                if camera_motion_state.motion_detected() {
                                    return;
                                }
                                if let Ok(uid) = area_uids.get(trigger.entity) {
                                    commands.trigger(FetchEntityFromStorage {
                                        uid: uid.0.clone(),
                                        anchor: None,
                                        why: FetchEntityReason::SandboxLink,
                                    });
                                }
                            },
                        )
                        .pointer_on_hover();
                }
            }
        })
        .insert(CaveMapPolygon {
            polygon: trigger.event().data.all_points.clone(),
        })
        .id();

    if let Ok(mut entity) = commands.get_entity(trigger.event().hex) {
        entity.mark_battlemap_as_ready();
        entity.add_child(parent_node);
    } else {
        commands.entity(parent_node).despawn();
    }

    if is_revealed {
        for cavern in trigger.event().data.map.caverns.iter() {
            let m = map_resources.dungeon_labels_material.clone();
            let l = cavern.n.clone();
            let polygon = &cavern.polygon;
            let j: Vec<Vec<f64>> = polygon
                .iter()
                .map(|point| vec![point.x as f64, point.y as f64])
                .collect();

            let test = crate::shared::poly::polylabel(vec![j], 1.0);
            let x = test[0] as f32;
            let y = test[1] as f32;

            spawn_area_labels(&mut q, parent_node, is_player, l.to_string(), 0.06, m, x, y);
        }
    }
}

#[derive(Component)]
struct AreaUid(String);

const VALS: [(i32, i32, f32); 12] = [
    (-1, 0, 0.3),
    (-1, 1, 0.6),
    (1, 2, 0.2),
    (0, 1, 0.4),
    (0, 0, 1.0),
    (1, 2, 0.2),
    (-1, -1, 0.5),
    (0, 0, 0.8),
    (2, 1, 0.2),
    (0, 0, 0.6),
    (0, 0, 1.3),
    (1, 0, 0.9),
];

#[derive(Deserialize, Debug, Clone)]
struct Cavern {
    polygon: Vec<Point>,
    n: usize,
    uuid: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Point {
    x: i32,
    y: i32,
    c: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CaveMap {
    caverns: Vec<Cavern>,
}

fn subdivide_polygon(rng: &mut StdRng, polygon: Vec<CavePoint>) -> Vec<CavePoint> {
    let mut subdivided = Vec::new();
    for i in 0..polygon.len() {
        let current = &polygon[i];
        let next = &polygon[(i + 1) % polygon.len()];

        if next.owners > 1 && current.owners > 1 {
            subdivided.push(CavePoint {
                coords: current.coords,
                owners: current.owners,
            });
            continue;
        }

        subdivided.push(CavePoint {
            coords: current.coords,
            owners: current.owners,
        });

        let tgt = rng.gen_range(1..=3);

        for j in 1..tgt {
            let fraction = j as f32 / tgt as f32;
            let new_coords = current.coords * (1.0 - fraction) + next.coords * fraction;
            subdivided.push(CavePoint {
                coords: new_coords,
                owners: 1,
            });
        }
    }
    subdivided
}

#[derive(Clone)]
struct CavePoint {
    coords: Vec2,
    owners: i32,
}

fn smoother_polygon(polygon: Vec<CavePoint>) -> Vec<CavePoint> {
    polygon
        .iter()
        .enumerate()
        .map(|(i, point)| {
            if point.owners == 1 {
                let prev_index = if i == 0 { polygon.len() - 1 } else { i - 1 };
                let next_index = if i == polygon.len() - 1 { 0 } else { i + 1 };
                let prev_point = &polygon[prev_index];
                let next_point = &polygon[next_index];

                let smoothed_coords = Vec2 {
                    x: (prev_point.coords.x + point.coords.x + next_point.coords.x) / 3.0,
                    y: (prev_point.coords.y + point.coords.y + next_point.coords.y) / 3.0,
                };

                CavePoint {
                    coords: smoothed_coords,
                    owners: point.owners,
                }
            } else {
                CavePoint {
                    coords: point.coords,
                    owners: point.owners,
                }
            }
        })
        .collect()
}
fn noisify_polygon(rng: &mut StdRng, polygon: Vec<CavePoint>) -> Vec<CavePoint> {
    let mut result = Vec::new();

    for i in 0..polygon.len() {
        let prev = if i == 0 { polygon.len() - 1 } else { i - 1 };
        let next = if i == polygon.len() - 1 { 0 } else { i + 1 };

        let prev_point = polygon[prev].coords;
        let curr_point = polygon[i].coords;
        let next_point = polygon[next].coords;

        let edge1 = curr_point - prev_point;
        let edge2 = next_point - curr_point;

        let normal = Vec2::new(-edge1.y, edge1.x).normalize()
            + Vec2::new(-edge2.y, edge2.x).normalize();

        let mut noisy_point = curr_point;

        if polygon[i].owners == 1 {
            // let noise = rng.gen_range(-0.75..0.75);
            let noise = rng.gen_range(-0.5..0.5);
            noisy_point += normal * noise;
        }

        result.push(CavePoint {
            coords: noisy_point,
            owners: polygon[i].owners,
        });
    }

    result
}

fn generate_polygon_wall(my_uid: &str, polygon: &Vec<CavePoint>, other: &[Cavern]) -> Mesh {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    let mut normals = Vec::new();

    for i in 0..polygon.len() {
        let next_i = (i + 1) % polygon.len();
        let current = &polygon[i];
        let next = &polygon[next_i];

        // Properly ensure we do not create walls for edges connected to other
        // areas
        let mut current_other_uid = String::new();
        let mut next_other_uid = String::new();
        if current.owners > 1 && next.owners > 1 {
            // Dealing with the special case of an edge connected to two different areas
            // but may still require a wall:
            //
            //     ___  this  ____
            //   /    \__V__/
            //        /     \
            //
            if current.owners == 2 && next.owners == 2 {
                for c in other {
                    if c.uuid == my_uid {
                        continue;
                    }
                    for p in c.polygon.iter() {
                        if p.x == current.coords.x as i32 && p.y == current.coords.y as i32 {
                            current_other_uid = c.uuid.to_string();
                        }
                        if p.x == next.coords.x as i32 && p.y == next.coords.y as i32 {
                            next_other_uid = c.uuid.to_string();
                        }
                    }
                }
                if next_other_uid == current_other_uid
                    || next_other_uid.is_empty()
                    || current_other_uid.is_empty()
                {
                    continue;
                }
            } else {
                continue;
            }
        }

        let current_pos = [current.coords.x, 0.0, current.coords.y];
        let next_pos = [next.coords.x, 0.0, next.coords.y];
        let current_top = [current.coords.x, 20.0, current.coords.y];
        let next_top = [next.coords.x, 20.0, next.coords.y];

        let start_index = positions.len() as u32;
        positions.extend_from_slice(&[current_pos, next_pos, current_top, next_top]);

        // Create front facing face
        indices.extend_from_slice(&[
            start_index,
            start_index + 1,
            start_index + 2,
            start_index + 2,
            start_index + 1,
            start_index + 3,
        ]);
        // Create back facing face so shadows work correctly
        indices.extend_from_slice(&[
            start_index,
            start_index + 2,
            start_index + 1,
            start_index + 2,
            start_index + 3,
            start_index + 1,
        ]);
        let v2 = Vec3::new(current.coords.x, 0.0, current.coords.y);
        let v1 = Vec3::new(next.coords.x, 0.0, next.coords.y);
        let edge_vector = v2 - v1;
        let up_vector = Vec3::new(0.0, 1.0, 0.0);

        let normal = up_vector.cross(edge_vector).normalize() * -1.0;

        normals.extend_from_slice(&[normal, normal, normal, normal])
    }

    Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}
