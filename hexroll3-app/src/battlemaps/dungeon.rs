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

// Dungeon maps - using hexroll's dungeon map generator dataset
use std::f32;

use serde::Deserialize;

use bevy::prelude::*;
use bevy::{light::NotShadowCaster, mesh::CylinderMeshBuilder};

use avian3d::prelude::{
    Collider, ColliderConstructor, ColliderDisabled, CollisionLayers, RigidBody,
};

use crate::{
    audio::{DungeonAudioSample, PlayDungeonSound},
    clients::model::FetchEntityReason,
    hexmap::{
        BackgroundMaterial,
        elements::{FetchEntityFromStorage, HexCoordsForFeature, HexMapResources},
    },
    shared::{
        dragging::DraggingMotionDetector,
        geometry::make_filled_mesh_from_path,
        labels::spawn_area_labels,
        layers::{HEIGHT_OF_BATTLEMAP_ON_FEATURE, HexrollPhysicsLayer},
        spawnq::{MustBeChildOf, SpawnQueue},
        svg::svg_to_path,
        vtt::{HexRevealState, VttData},
        widgets::cursor::PointerOnHover,
    },
};

use super::BattlemapFeatureUtils;
use super::{
    DUNGEON_FOG_COLOR, DoorData,
    battlemaps::{
        BattlemapMaterial, BattlemapMaterialControls, DEFAULT_BATTLEMAP_COLOR,
        PlayerBattlemapEntity,
    },
    doors::*,
    wall::WallMeshBuilder,
};

pub struct DungeonsPlugin;
impl Plugin for DungeonsPlugin {
    fn build(&self, app: &mut App) {
        app
            // -->
            .add_systems(Startup, setup)
            .add_observer(on_spawn_dungeon_map)
            // <--
        ;
    }
}

/// SpawnDungeonMap is triggered after a backend client finished loading a preparing
/// a dungeon.
/// Orchestrating dungeons and other battlemaps spawning is done via the BattlemapsPlugin.
#[derive(Event)]
pub struct SpawnDungeonMap {
    pub hex: Entity,
    pub data: DungeonMapConstructs,
}

const WALLS_Y_SCALE_FOR_LIGHTING: f32 = 5.0;
const SECRET_DOOR_SVG: &str = include_str!("../../assets/svg/secret_door.svg");
const CORNER_SVG: &str = include_str!("../../assets/svg/corner.svg");

#[derive(Resource)]
struct DungeonMapAssets {
    pillar_mesh: Handle<Mesh>,
    dirt_mesh: Handle<Mesh>,
    corner_mesh: Handle<Mesh>,
    door_frame_mesh: Handle<Mesh>,
    door_mesh: Handle<Mesh>,
    secret_door_mesh: Handle<Mesh>,
    secret_door_walls_mesh: Handle<Mesh>,
    wall_material: Handle<StandardMaterial>,
    dirt_material: Handle<StandardMaterial>,
    highlight_material: Handle<StandardMaterial>,
    toned_black: Handle<StandardMaterial>,
    background_material: Handle<BackgroundMaterial>,
    battlemap_material: Handle<BattlemapMaterial>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut battlemap_materials: ResMut<Assets<BattlemapMaterial>>,
    mut background_materials: ResMut<Assets<BackgroundMaterial>>,
) {
    let pillar_mesh = meshes.add(CylinderMeshBuilder::new(0.25, 6.0, 16));
    let door_frame_mesh = meshes.add(create_door_frame_mesh());
    let door_mesh = meshes.add(Cuboid::new(0.5, 1.0, 0.2));
    let secret_door_mesh =
        meshes.add(make_filled_mesh_from_path(svg_to_path(SECRET_DOOR_SVG)));
    let secret_door_walls_mesh = meshes.add(Cuboid::new(1.0, 1.0, 0.2));
    let dirt_mesh = meshes.add(make_filled_mesh_from_path(svg_to_path(CORNER_SVG)));
    let size = 0.15;
    let corner_mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(size)));

    let wall_material = standard_materials.add(StandardMaterial {
        base_color: DUNGEON_FOG_COLOR,
        unlit: true,
        cull_mode: None,
        ..default()
    });

    let dirt_material = standard_materials.add(StandardMaterial {
        base_color: DUNGEON_FOG_COLOR.with_alpha(0.5),
        unlit: true,
        cull_mode: None,
        alpha_mode: AlphaMode::AlphaToCoverage,
        ..default()
    });

    let highlight_material = standard_materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.0, 0.0),
        emissive: LinearRgba::new(1.0, 0.0, 0.0, 1.0),
        cull_mode: None,
        ..default()
    });
    let toned_black = wall_material.clone();
    let background_material = background_materials.add(BackgroundMaterial {
        base_color: DUNGEON_FOG_COLOR.into(),
        layer_color: DUNGEON_FOG_COLOR.into(),
    });
    let battlemap_material = battlemap_materials.add(BattlemapMaterial {
        controls: BattlemapMaterialControls {
            zoom_factor: 1.0,
            grid_mix: 1.0,
            blend: 1.0,
            scale: 1.0,
        },
        color: Vec4::from(DEFAULT_BATTLEMAP_COLOR),
        offset: Vec4::new(0.017, 0.015, 0.015, 0.0),
    });
    commands.insert_resource(DungeonMapAssets {
        pillar_mesh,
        dirt_mesh,
        corner_mesh,
        door_frame_mesh,
        door_mesh,
        secret_door_mesh,
        secret_door_walls_mesh,
        wall_material,
        dirt_material,
        highlight_material,
        toned_black,
        background_material,
        battlemap_material,
    });
}

fn on_spawn_dungeon_map(
    trigger: On<SpawnDungeonMap>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut q: ResMut<SpawnQueue>,
    map_assets: Res<HexMapResources>,
    dungeon_assets: Res<DungeonMapAssets>,
    mut battlemap_materials: ResMut<Assets<BattlemapMaterial>>,
    vtt_data: Res<VttData>,
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
            sample: DungeonAudioSample::Dungeon,
        });
    }
    let y_scale_for_lighting = if is_revealed {
        WALLS_Y_SCALE_FOR_LIGHTING
    } else {
        1.0
    };

    let mut parent_node = commands.spawn_empty();
    let parent_node_id = parent_node.id();
    let is_player = vtt_data.mode.is_player();
    let player_entity_visibility = if is_player {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    battlemap_materials
        .get_mut(&dungeon_assets.battlemap_material)
        .unwrap()
        .controls
        .grid_mix = if is_player { 0.0 } else { 1.0 };
    parent_node
        .insert(Name::new("DungeonMap"))
        .insert(Visibility::Hidden)
        .insert(Transform::from_xyz(
            0.0,
            HEIGHT_OF_BATTLEMAP_ON_FEATURE - 5.0,
            0.0,
        ))
        .with_children(|commands| {
            commands.spawn((
                Mesh3d(map_assets.mesh.clone()),
                MeshMaterial3d(dungeon_assets.background_material.clone()),
                Transform::from_xyz(0.0, -1.0, 0.0),
            ));

            // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
            // WALL PANELS (with colliders)
            //
            for p in &trigger.event().data.panels {
                let panel_mesh = meshes.add(p.mesh.clone());
                let wall_material = dungeon_assets.wall_material.clone();
                let panel_collider = p.collider.clone();
                let panel_x = p.panel.x;
                let panel_y = p.panel.y;
                q.queue_children(parent_node_id, move |pid, commands| {
                    if commands.get_entity(pid).is_ok() {
                        commands
                            .spawn((
                                DungeonWall,
                                Mesh3d(panel_mesh),
                                MeshMaterial3d(wall_material),
                                Transform::from_xyz(panel_x, 0.0, panel_y)
                                    .with_scale(Vec3::new(1.0, y_scale_for_lighting, 1.0)),
                                avian3d::prelude::ColliderDensity(2000.0),
                                avian3d::prelude::CollisionMargin(0.1),
                            ))
                            .insert(CollisionLayers::new(
                                [HexrollPhysicsLayer::Walls],
                                [HexrollPhysicsLayer::Tokens],
                            ))
                            .insert(MustBeChildOf)
                            .insert(ChildOf(pid))
                            .insert_if((RigidBody::Static, panel_collider), || is_revealed);
                    }
                });
            }

            // if !is_revealed {
            //     return;
            // }

            for prepared_area in &trigger.event().data.areas {
                let area = &prepared_area.area;
                let x = (area.x as f32) + (area.w as f32) / 2.0;
                let y = (area.y as f32) + (area.h as f32) / 2.0;
                let wt = area.w as f32 / 2.0;
                let ht = area.h as f32 / 2.0;

                let m = map_assets.dungeon_labels_material.clone();
                let l = area.n.to_string();

                // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
                // AREA LABELS
                //
                if area.t == 1 && is_revealed {
                    spawn_area_labels(
                        &mut q,
                        parent_node_id,
                        is_player,
                        l.to_string(),
                        0.0075,
                        m,
                        x,
                        y,
                    );
                }

                // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
                // PILLARS
                //
                if is_revealed {
                    let pillar_mesh = &dungeon_assets.pillar_mesh;
                    let wall_material = &dungeon_assets.wall_material;
                    let pillar_collider = Collider::cylinder(0.2, 1.0);
                    for pillar_pos in prepared_area.pillars.iter() {
                        commands.spawn((
                            Mesh3d(pillar_mesh.clone()),
                            MeshMaterial3d(wall_material.clone()),
                            Transform::from_translation(*pillar_pos),
                            CollisionLayers::new(
                                [HexrollPhysicsLayer::Walls],
                                [HexrollPhysicsLayer::Tokens],
                            ),
                            RigidBody::Static,
                            pillar_collider.clone(),
                        ));
                        // .insert_if((RigidBody::Static, pillar_collider.clone()), || {
                        //     is_revealed
                        // });
                    }
                }

                // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
                // CORNERS DIRT
                //
                let not_a_corridor_or_a_small_chamber = ht > 0.5 && wt > 0.5;
                if not_a_corridor_or_a_small_chamber {
                    let dirt_mesh = &dungeon_assets.dirt_mesh;
                    let dirt_material = &dungeon_assets.dirt_material;
                    if !prepared_area.corners.0 {
                        commands.spawn((
                            Mesh3d(dirt_mesh.clone()),
                            MeshMaterial3d(dirt_material.clone()),
                            NotShadowCaster,
                            Transform::from_xyz(x - wt, 0.0, y - ht)
                                .with_scale(Vec3::splat(1.0)),
                        ));
                    }
                    if !prepared_area.corners.1 {
                        commands.spawn((
                            Mesh3d(dirt_mesh.clone()),
                            MeshMaterial3d(dirt_material.clone()),
                            NotShadowCaster,
                            Transform::from_xyz(x + wt, 0.0, y - ht)
                                .with_scale(Vec3::splat(1.0))
                                .with_rotation(Quat::from_rotation_y(f32::consts::PI / -2.0)),
                        ));
                    }
                    if !prepared_area.corners.2 {
                        commands.spawn((
                            Mesh3d(dirt_mesh.clone()),
                            MeshMaterial3d(dirt_material.clone()),
                            NotShadowCaster,
                            Transform::from_xyz(x + wt, 0.0, y + ht)
                                .with_scale(Vec3::splat(1.0))
                                .with_rotation(Quat::from_rotation_y(f32::consts::PI)),
                        ));
                    }
                    if !prepared_area.corners.3 {
                        commands.spawn((
                            Mesh3d(dirt_mesh.clone()),
                            MeshMaterial3d(dirt_material.clone()),
                            NotShadowCaster,
                            Transform::from_xyz(x - wt, 0.0, y + ht)
                                .with_scale(Vec3::splat(1.0))
                                .with_rotation(Quat::from_rotation_y(f32::consts::PI / 2.0)),
                        ));
                    }
                }

                // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
                // DOORS AND PORTALS (with colliders)
                //
                if let Some(portals) = &area.portals {
                    let filtered_portlas: Vec<_> = if let Some(passages) = &area.passages {
                        portals
                            .iter()
                            .filter(|a| !passages.iter().any(|b| a.x == b.x && a.y == b.y))
                            .collect()
                    } else {
                        portals.iter().collect()
                    };

                    for p in filtered_portlas {
                        let is_door_open = vtt_data.open_doors.contains(&p.uuid);
                        let door_frame_mesh = dungeon_assets.door_frame_mesh.clone();
                        let wall_material = dungeon_assets.wall_material.clone();
                        let door_mesh = dungeon_assets.door_mesh.clone();
                        let secret_door_mesh = dungeon_assets.secret_door_mesh.clone();
                        let secret_door_walls_mesh =
                            dungeon_assets.secret_door_walls_mesh.clone();
                        let highlight_material = dungeon_assets.highlight_material.clone();

                        let px = (p.x as f32) + 0.5;
                        let py = (p.y as f32) + 0.5;
                        let p = p.clone();
                        q.queue_children(parent_node_id, move |pid, commands| {
                            if commands.get_entity(pid).is_ok() {
                                commands
                                    .spawn_empty()
                                    .insert(MustBeChildOf)
                                    .insert(ChildOf(pid))
                                    .insert(Visibility::Inherited)
                                    .insert(Transform::from_xyz(px, 0.0, py).with_rotation(
                                        Quat::from_rotation_y(match p.wall.as_str() {
                                            "W" => f32::consts::PI / 2.0,
                                            "E" => f32::consts::PI / -2.0,
                                            "S" => f32::consts::PI,
                                            _ => 0.0,
                                        }),
                                    ))
                                    .with_children(|c| {
                                        let mut door_frame = c.spawn((
                                            DungeonWall,
                                            Mesh3d(door_frame_mesh),
                                            MeshMaterial3d(wall_material.clone()),
                                            Transform::from_xyz(-0.5, 0.0, 0.25).with_scale(
                                                Vec3::new(1.0, y_scale_for_lighting, 1.0),
                                            ),
                                            Pickable {
                                                should_block_lower: true,
                                                is_hoverable: true,
                                            },
                                        ));
                                        if !is_player {
                                            door_frame
                                                .observe(update_material_on(
                                                    highlight_material.clone(),
                                                ))
                                                .observe(update_material_out(
                                                    wall_material.clone(),
                                                ))
                                                .observe(close_door(p.uuid.clone()));
                                        }
                                        if is_revealed {
                                            door_frame.with_children(|c| {
                                                c.spawn((
                                                    Collider::cuboid(0.5, 1.0, 0.5),
                                                    Transform::from_xyz(0.0, 0.0, 0.0)
                                                        .with_scale(Vec3::new(
                                                            1.0,
                                                            y_scale_for_lighting,
                                                            1.0,
                                                        )),
                                                    CollisionLayers::new(
                                                        [HexrollPhysicsLayer::Walls],
                                                        [HexrollPhysicsLayer::Tokens],
                                                    ),
                                                    RigidBody::Static,
                                                ));
                                                c.spawn((
                                                    Collider::cuboid(0.5, 1.0, 0.5),
                                                    Transform::from_xyz(1.0, 0.0, 0.0),
                                                    CollisionLayers::new(
                                                        [HexrollPhysicsLayer::Walls],
                                                        [HexrollPhysicsLayer::Tokens],
                                                    ),
                                                    RigidBody::Static,
                                                ));
                                            });
                                        }
                                        c.spawn_empty()
                                            .insert(Visibility::Inherited)
                                            .insert(
                                                Transform::from_xyz(-0.25, 0.0, 0.25)
                                                    .with_rotation(Quat::from_rotation_y(0.0)),
                                            )
                                            .with_children(|c| {
                                                let mut door = c.spawn((
                                                    DungeonWall,
                                                    Name::new("DungeonDoor"),
                                                    Mesh3d(if p.type_ == 0 {
                                                        door_mesh
                                                    } else {
                                                        secret_door_mesh
                                                    }),
                                                    MeshMaterial3d(wall_material.clone()),
                                                    Transform::from_xyz(0.25, 0.0, 0.0)
                                                        .with_scale(Vec3::new(
                                                            1.0,
                                                            y_scale_for_lighting,
                                                            1.0,
                                                        )),
                                                    DoorData {
                                                        door_uid: p.uuid.clone(),
                                                    },
                                                    CollisionLayers::new(
                                                        [HexrollPhysicsLayer::Walls],
                                                        [HexrollPhysicsLayer::Tokens],
                                                    ),
                                                    Pickable {
                                                        should_block_lower: true,
                                                        is_hoverable: true,
                                                    },
                                                ));
                                                door.insert_if(
                                                    (
                                                        RigidBody::Static,
                                                        Collider::cuboid(0.5, 1.0, 0.5),
                                                    ),
                                                    || is_revealed,
                                                );
                                                if is_door_open {
                                                    door.insert(Visibility::Hidden);
                                                    door.insert(ColliderDisabled);
                                                }

                                                door.with_children(|c| {
                                                    if p.type_ == 2 {
                                                        c.spawn_empty()
                                                            .insert(player_entity_visibility)
                                                            .insert(PlayerBattlemapEntity)
                                                            .insert(
                                                                Transform::from_xyz(
                                                                    -0.25, -2.0, -0.65,
                                                                )
                                                                .with_rotation(
                                                                    Quat::from_rotation_y(0.0),
                                                                )
                                                                .with_scale(Vec3::new(
                                                                    1.2, 1.0, 1.0,
                                                                )),
                                                            )
                                                            .with_children(|c| {
                                                                c.spawn((
                                                                    DungeonWall,
                                                                    Mesh3d(
                                                                        secret_door_walls_mesh
                                                                            .clone(),
                                                                    ),
                                                                    MeshMaterial3d(
                                                                        wall_material.clone(),
                                                                    ),
                                                                    Transform::from_xyz(
                                                                        0.25, 0.0, 0.0,
                                                                    )
                                                                    .with_scale(Vec3::new(
                                                                        1.0,
                                                                        y_scale_for_lighting,
                                                                        1.0,
                                                                    )),
                                                                ));
                                                            });
                                                        c.spawn_empty()
                                                            .insert(player_entity_visibility)
                                                            .insert(PlayerBattlemapEntity)
                                                            .insert(
                                                                Transform::from_xyz(
                                                                    -0.25, -2.0, 0.15,
                                                                )
                                                                .with_rotation(
                                                                    Quat::from_rotation_y(0.0),
                                                                ),
                                                            )
                                                            .with_children(|c| {
                                                                c.spawn((
                                                                    DungeonWall,
                                                                    Mesh3d(
                                                                        secret_door_walls_mesh
                                                                            .clone(),
                                                                    ),
                                                                    MeshMaterial3d(
                                                                        wall_material.clone(),
                                                                    ),
                                                                    Transform::from_xyz(
                                                                        0.25, 0.0, 0.0,
                                                                    )
                                                                    .with_scale(Vec3::new(
                                                                        1.0,
                                                                        y_scale_for_lighting,
                                                                        1.0,
                                                                    )),
                                                                ));
                                                            });
                                                    }
                                                });
                                                if !is_player {
                                                    door.observe(update_material_on(
                                                        highlight_material,
                                                    ))
                                                    .observe(update_material_out(
                                                        wall_material.clone(),
                                                    ))
                                                    .observe(open_door(p.uuid.clone()));
                                                }
                                            });
                                    });
                            }
                        });
                    }
                }

                // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
                // CORNER JOINTS FIX (masks mesh connections)
                //
                if is_revealed {
                    let corners = &prepared_area.corners;
                    let size = 0.15;
                    if !corners.1 {
                        commands.spawn((
                            Mesh3d(dungeon_assets.corner_mesh.clone()),
                            MeshMaterial3d(dungeon_assets.toned_black.clone()),
                            Transform::from_xyz(x + wt + size, 1.0, y - ht - size),
                        ));
                    }
                    if !corners.3 {
                        commands.spawn((
                            Mesh3d(dungeon_assets.corner_mesh.clone()),
                            MeshMaterial3d(dungeon_assets.toned_black.clone()),
                            Transform::from_xyz(x - wt - size, 1.0, y + ht + size),
                        ));
                    }
                    if !corners.2 {
                        commands.spawn((
                            Mesh3d(dungeon_assets.corner_mesh.clone()),
                            MeshMaterial3d(dungeon_assets.toned_black.clone()),
                            Transform::from_xyz(x + wt + size, 1.0, y + ht + size),
                        ));
                    }
                    if !corners.0 {
                        commands.spawn((
                            Mesh3d(dungeon_assets.corner_mesh.clone()),
                            MeshMaterial3d(dungeon_assets.toned_black.clone()),
                            Transform::from_xyz(x - wt - size, 1.0, y - ht - size),
                        ));
                    }
                }

                // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
                // AREA FLOOR
                //
                let mut area_floor = commands.spawn((
                    Mesh3d(meshes.add(prepared_area.mesh.clone())),
                    MeshMaterial3d(dungeon_assets.battlemap_material.clone()),
                    Transform::from_xyz(x, 0.0, y),
                    AreaUid(prepared_area.area.uuid.clone()),
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

                q.queue_children(parent_node_id, move |pid, commands| {
                    commands.entity(pid).try_insert(Visibility::default());
                });
            }
        });
    if let Ok(mut entity) = commands.get_entity(trigger.event().hex) {
        entity.mark_battlemap_as_ready();
        entity.add_child(parent_node_id);
    } else {
        commands.entity(parent_node_id).despawn();
    }
}

#[derive(Component)]
struct AreaUid(String);

#[derive(Component)]
struct DungeonWall;

#[derive(Debug, Clone)]
struct Panel {
    x: f32,
    y: f32,
    w: f32,
    normal: Dir3,
    internal_corners: (bool, bool),
    aligned_edges: (bool, bool),
}

struct PreparedArea {
    area: Area,
    mesh: Mesh,
    corners: (bool, bool, bool, bool),
    pillars: Vec<Vec3>,
}

struct PreparedPanel {
    panel: Panel,
    mesh: Mesh,
    collider: Collider,
}

pub struct DungeonMapConstructs {
    areas: Vec<PreparedArea>,
    panels: Vec<PreparedPanel>,
}

impl DungeonMapConstructs {
    pub fn from(json: String) -> Self {
        let map: DungeonMap = serde_json::from_str(&json).expect("Failed to parse JSON");

        let mut panels: Vec<Panel> = Vec::new();

        let mut prepared_areas: Vec<PreparedArea> = Vec::new();
        let mut prepared_panels: Vec<PreparedPanel> = Vec::new();
        for area in &map.areas {
            let xt = (area.x as f32) / 2.0;
            let yt = (area.y as f32) / 2.0;

            let wt = area.w as f32 / 2.0;
            let ht = area.h as f32 / 2.0;

            let mut pillars: Vec<Vec3> = Vec::new();
            let has_pillars = (wt > 4.0 || ht > 4.0) && (wt > 2.0 && ht > 2.0);
            if has_pillars {
                let x = (area.x as f32) + (area.w as f32) / 2.0;
                let y = (area.y as f32) + (area.h as f32) / 2.0;
                if wt < ht {
                    let mut y_offset = y - ht + 1.0;
                    while y_offset < (y + ht) {
                        pillars.push(Vec3::new(x - wt / 1.75, 0.0, y_offset));
                        pillars.push(Vec3::new(x + wt / 1.75, 0.0, y_offset));
                        y_offset += 1.0;
                    }
                }
                if wt > ht {
                    let mut x_offset = x - wt + 1.0;
                    while x_offset < (x + wt) {
                        pillars.push(Vec3::new(x_offset, 0.0, y - ht / 1.75));
                        pillars.push(Vec3::new(x_offset, 0.0, y + ht / 1.75));
                        x_offset += 1.0;
                    }
                }
            }

            let corners = get_shared_corners(area, &map.areas);
            let matches = get_aligned_corners(area, &map.areas);

            add_panels(area, &mut panels, &map.areas, corners, matches);

            let buffer = 0.3;
            let half_buffer = buffer / 4.0;
            let pp = Plane3d::default()
                .mesh()
                .size(area.w as f32 + buffer, area.h as f32 + buffer)
                .build()
                .with_inserted_attribute(
                    Mesh::ATTRIBUTE_UV_0,
                    vec![
                        [xt - half_buffer, yt - half_buffer],
                        [xt + wt + half_buffer, yt - half_buffer],
                        [xt - half_buffer, yt + ht + half_buffer],
                        [xt + wt + half_buffer, yt + ht + half_buffer],
                    ],
                );
            prepared_areas.push(PreparedArea {
                area: area.clone(),
                mesh: pp,
                corners,
                pillars,
            });
        }
        for p in panels {
            if p.w * 2.0 < 1.0 {
                continue;
            }
            let mesh = WallMeshBuilder::new(
                p.normal,
                Vec2::new(p.w, 1.0),
                p.internal_corners.to_owned(),
                p.aligned_edges.to_owned(),
            )
            .build();
            let collider = Collider::try_from_constructor(
                ColliderConstructor::ConvexHullFromMesh,
                Some(&mesh),
            )
            .unwrap();

            prepared_panels.push(PreparedPanel {
                mesh,
                collider,
                panel: p.clone(),
            });
        }

        DungeonMapConstructs {
            areas: prepared_areas,
            panels: prepared_panels,
        }
    }
}

fn get_aligned_corners(area: &Area, areas: &[Area]) -> (bool, bool, bool, bool) {
    let top_left_neighbors = [(area.x - 1, area.y), (area.x, area.y)];
    let top_right_neighbors = [
        (area.x + area.w, area.y),
        (area.x + area.w - 1, area.y + area.h), //?
    ];
    let bottom_left_neighbors = [(area.x - 1, area.y + area.h), (area.x, area.y + area.h)];
    let bottom_right_neighbors = [
        (area.x + area.w, area.y + area.h),
        (area.x + area.w - 1, area.y + area.h), //?
    ];

    let mut neighbors = (false, false, false, false);

    for other in areas {
        if other != area {
            if top_left_neighbors
                .iter()
                .any(|&pos| other.is_top_right(pos) || other.is_bottom_left(pos))
            {
                neighbors.0 = true;
            }
            if top_right_neighbors
                .iter()
                .any(|&pos| other.is_top_left(pos) || other.is_bottom_right(pos))
            {
                neighbors.1 = true;
            }
            if bottom_right_neighbors
                .iter()
                .any(|&pos| other.is_bottom_left(pos) || other.is_top_right(pos))
            {
                neighbors.2 = true;
            }
            if bottom_left_neighbors
                .iter()
                .any(|&pos| other.is_bottom_right(pos) || other.is_top_left(pos))
            {
                neighbors.3 = true;
            }
        }
    }

    neighbors
}

fn get_shared_corners(area: &Area, areas: &[Area]) -> (bool, bool, bool, bool) {
    let top_left_neighbors = [
        (area.x - 1, area.y),
        (area.x, area.y - 1),
        (area.x - 1, area.y - 1),
    ];
    let top_right_neighbors = [
        (area.x + area.w, area.y),
        (area.x + area.w - 1, area.y - 1),
        (area.x + area.w, area.y - 1),
    ];
    let bottom_left_neighbors = [
        (area.x - 1, area.y + area.h - 1),
        (area.x, area.y + area.h),
        (area.x - 1, area.y + area.h),
    ];
    let bottom_right_neighbors = [
        (area.x + area.w, area.y + area.h - 1),
        (area.x + area.w - 1, area.y + area.h),
        (area.x + area.w, area.y + area.h),
    ];

    let mut neighbors = (false, false, false, false);

    for other in areas {
        if other != area {
            if top_left_neighbors.iter().any(|&pos| other.contains(pos)) {
                neighbors.0 = true;
            }
            if top_right_neighbors.iter().any(|&pos| other.contains(pos)) {
                neighbors.1 = true;
            }
            if bottom_right_neighbors
                .iter()
                .any(|&pos| other.contains(pos))
            {
                neighbors.2 = true;
            }
            if bottom_left_neighbors.iter().any(|&pos| other.contains(pos)) {
                neighbors.3 = true;
            }
        }
    }

    neighbors
}

impl PartialEq for Area {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.w == other.w && self.h == other.h
    }
}

fn find_portals(area_to_check: &Area, areas: &[Area]) -> Vec<IVec2> {
    let mut ret: Vec<IVec2> = Vec::new();

    for a in areas {
        if area_to_check != a {
            for p in area_to_check.outer_bounds() {
                if a.inner_bounds().contains(&p) {
                    ret.push(p);
                }
            }
        }
    }
    ret
}

fn add_panels(
    area: &Area,
    panels: &mut Vec<Panel>,
    areas: &[Area],
    internal_corners: (bool, bool, bool, bool),
    aligned_edges: (bool, bool, bool, bool),
) {
    let mut portals: Vec<IVec2> = find_portals(area, areas);

    if let Some(area_passages) = &area.passages {
        let output: Vec<IVec2> = area_passages.iter().map(|p| IVec2::new(p.x, p.y)).collect();
        portals.extend(output);
    }

    let x = (area.x as f32) + (area.w as f32) / 2.0;
    let y = (area.y as f32) + (area.h as f32) / 2.0;

    let w = area.w as f32 / 2.0;
    let h = area.h as f32 / 2.0;

    for &(dy, norm, my) in &[(area.h, Dir3::NEG_Z, 1.0), (-1, Dir3::Z, -1.0)] {
        let mut portals: Vec<IVec2> = (0..area.w)
            .map(|panel_x| IVec2::new(area.x + panel_x, area.y + dy))
            .filter(|portal| portals.contains(portal))
            .collect();
        if portals.is_empty() || portals.last().unwrap().x != area.x + area.w - 1 {
            portals.push(IVec2::new(area.x + area.w, area.y - 1));
        }

        let mut last_split_idx: i32 = -1;
        for (i, portal) in portals.iter().enumerate() {
            let left_is_internal_corner = (i == 0 && norm == Dir3::Z && !internal_corners.0)
                || (i == 0 && norm == Dir3::NEG_Z && !internal_corners.3);
            let right_is_internal_corner =
                (i == portals.len() - 1 && norm == Dir3::Z && !internal_corners.1)
                    || (i == portals.len() - 1 && norm == Dir3::NEG_Z && !internal_corners.2);
            let left_is_aligned_edge = (i == 0 && norm == Dir3::Z && aligned_edges.0)
                || (i == 0 && norm == Dir3::NEG_Z && aligned_edges.3);
            let right_is_aligned_edge =
                (i == portals.len() - 1 && norm == Dir3::Z && aligned_edges.1)
                    || (i == portals.len() - 1 && norm == Dir3::NEG_Z && aligned_edges.2);
            let split_idx = portal.x - area.x;
            panels.push(Panel {
                y: y + (h * my),
                x: area.x as f32
                    + (last_split_idx + 1) as f32
                    + ((split_idx - last_split_idx - 1) as f32 / 2.0),
                w: (split_idx - last_split_idx - 1) as f32,
                normal: norm,
                internal_corners: (left_is_internal_corner, right_is_internal_corner),
                aligned_edges: (left_is_aligned_edge, right_is_aligned_edge),
            });
            last_split_idx = split_idx;
        }
    }

    for &(dx, norm, mx) in &[(area.w, Dir3::NEG_X, 1.0), (-1, Dir3::X, -1.0)] {
        let mut portals: Vec<IVec2> = (0..area.h)
            .map(|panel_y| IVec2::new(area.x + dx, area.y + panel_y))
            .filter(|portal| portals.contains(portal))
            .collect();
        if portals.is_empty() || portals.last().unwrap().y != area.y + area.h - 1 {
            portals.push(IVec2::new(area.x - 1, area.y + area.h));
        }

        let mut last_split_idx: i32 = -1;
        for (i, portal) in portals.iter().enumerate() {
            let top_is_internal_corner =
                (i == portals.len() - 1 && norm == Dir3::X && !internal_corners.3)
                    || (i == 0 && norm == Dir3::NEG_X && !internal_corners.1);
            let bottom_is_internal_corner = (i == 0 && norm == Dir3::X && !internal_corners.0)
                || (i == portals.len() - 1 && norm == Dir3::NEG_X && !internal_corners.2);
            let top_is_aligned_edge =
                (i == portals.len() - 1 && norm == Dir3::X && aligned_edges.3)
                    || (i == 0 && norm == Dir3::NEG_X && aligned_edges.1);
            let bottom_is_aligned_edge = (i == 0 && norm == Dir3::X && aligned_edges.0)
                || (i == portals.len() - 1 && norm == Dir3::NEG_X && aligned_edges.2);
            let split_idx = portal.y - area.y;
            panels.push(Panel {
                x: x + (w * mx),
                y: area.y as f32
                    + (last_split_idx + 1) as f32
                    + ((split_idx - last_split_idx - 1) as f32 / 2.0),
                w: (split_idx - last_split_idx - 1) as f32,
                normal: norm,
                internal_corners: (top_is_internal_corner, bottom_is_internal_corner),
                aligned_edges: (top_is_aligned_edge, bottom_is_aligned_edge),
            });
            last_split_idx = split_idx;
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // Adding lint exception for unused field
pub struct DungeonMap {
    areas: Vec<Area>,
    portals: Vec<Portal>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
struct Area {
    h: i32,
    n: i32,
    t: i32,
    w: i32,
    x: i32,
    y: i32,
    uuid: String,
    #[serde(default)]
    d: Option<Position>,
    #[serde(default)]
    portals: Option<Vec<AreaPortal>>,
    #[serde(default)]
    passages: Option<Vec<AreaPassage>>,
}

impl Area {
    fn outer_bounds(&self) -> Vec<IVec2> {
        let mut points = Vec::new();
        for i in self.x - 1..self.x + self.w + 1 {
            points.push(IVec2::new(i, self.y - 1));
            points.push(IVec2::new(i, self.y + self.h));
        }
        for j in self.y - 1..self.y + self.h + 1 {
            points.push(IVec2::new(self.x - 1, j));
            points.push(IVec2::new(self.x + self.w, j));
        }
        points.dedup();
        points
    }
    fn inner_bounds(&self) -> Vec<IVec2> {
        let mut points = Vec::new();
        for i in self.x..self.x + self.w {
            points.push(IVec2::new(i, self.y));
            points.push(IVec2::new(i, self.y + self.h - 1));
        }
        for j in self.y..self.y + self.h {
            points.push(IVec2::new(self.x, j));
            points.push(IVec2::new(self.x + self.w - 1, j));
        }
        points.dedup();
        points
    }
    fn contains(&self, point: (i32, i32)) -> bool {
        let (x, y) = point;
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    fn is_top_left(&self, point: (i32, i32)) -> bool {
        let (x, y) = point;
        x == self.x && y == self.y
    }

    fn is_top_right(&self, point: (i32, i32)) -> bool {
        let (x, y) = point;
        x == self.x + self.w && y == self.y
    }

    fn is_bottom_left(&self, point: (i32, i32)) -> bool {
        let (x, y) = point;
        x == self.x && y == self.y + self.h
    }

    fn is_bottom_right(&self, point: (i32, i32)) -> bool {
        let (x, y) = point;
        x == self.x + self.w && y == self.y + self.h
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
struct Position {
    x: i32,
    y: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
struct Portal {
    #[serde(rename = "type")]
    type_: u32,
    wall: String,
    x: i32,
    y: i32,
    #[serde(default)]
    uuid: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct AreaPortal {
    #[serde(rename = "type")]
    type_: u32,
    wall: String,
    x: i32,
    y: i32,
    uuid: String,
}

use std::hash::{Hash, Hasher};

impl Hash for AreaPortal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
    }
}

impl PartialEq for AreaPortal {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl Eq for AreaPortal {}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
struct AreaPassage {
    #[serde(rename = "type")]
    type_: u32,
    wall: String,
    x: i32,
    y: i32,
}

impl PartialEq for AreaPassage {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl Eq for AreaPassage {}

impl Hash for AreaPassage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
    }
}
