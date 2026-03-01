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

// Village maps - based on Watabou's datasets
use core::f32;

use bevy::prelude::*;
use rand::Rng;

use crate::{
    clients::model::BackendUid,
    hexmap::elements::{HexMapData, HexMapResources},
    shared::{
        curve::create_bezier_curve_mesh,
        geometry::{make_filled_mesh_from_path, make_mesh_from_outline},
        layers::HEIGHT_OFFSET_OF_BATTLEMAP_CONTENT,
        widgets::cursor::PointerOnHover,
    },
};

use super::{BattlemapFeatureUtils, helpers::*, settlement::*};

pub struct VillagePlugin;

impl Plugin for VillagePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_spawn_village_map);
    }
}

#[derive(Event)]
pub struct SpawnVillageMap {
    pub hex: Entity,
    pub hex_uid: String,
    pub data: VillageMapConstructs,
}

fn on_spawn_village_map(
    trigger: On<SpawnVillageMap>,
    mut commands: Commands,
    village_map_resources: Res<SettlementMapResources>,
    hex_map_resources: Res<HexMapResources>,
    mut meshes: ResMut<Assets<Mesh>>,
    map: Res<HexMapData>,
    gltfs: Res<Assets<Gltf>>,
) {
    let (tx, angle, _offset) =
        if let Some((angle, offset)) = map
            .coords
            .get(&trigger.event().hex_uid)
            .and_then(|coords| map.hexes.get(coords))
            .map(|hex_data| hex_data.metadata.feature_angle_and_offset())
        {
            (
                Transform::from_rotation(Quat::from_rotation_y(angle)).transform_point(
                    Vec3::new(0.0, HEIGHT_OFFSET_OF_BATTLEMAP_CONTENT, offset),
                ),
                angle,
                offset,
            )
        } else {
            return;
        };
    let building_cube = meshes.add(Cuboid::default());

    let parent_node = commands
        .spawn_empty()
        .insert(Name::new("VillageMap"))
        .insert(Visibility::default())
        .insert(
            Transform::from_xyz(tx.x, tx.y, tx.z)
                .with_rotation(Quat::from_rotation_y(angle))
                .with_scale(Vec3::new(0.20, 0.20, 0.20)),
        )
        .with_children(|mut commands| {
            for (rect, orientation, uid) in trigger.event().data.buildings.iter() {
                let mut building = commands.spawn((
                    Mesh3d(building_cube.clone()),
                    MeshMaterial3d(if uid.is_some() {
                        village_map_resources.building_highlight_material.clone()
                    } else {
                        village_map_resources.building_material.clone()
                    }),
                    Transform::from_xyz(rect.center().x, 0.1, rect.center().y)
                        .with_rotation(Quat::from_rotation_y(*orientation))
                        .with_scale(Vec3::new(rect.width(), 3.0, rect.height())),
                ));
                if let Some(uid) = uid {
                    let cuid = uid.clone();
                    building.pointer_on_hover();
                    building.observe(move |_: On<Pointer<Click>>, mut commands: Commands| {
                        commands.trigger(crate::hexmap::elements::FetchEntityFromStorage {
                            uid: cuid.clone(),
                            anchor: None,
                            why: crate::clients::model::FetchEntityReason::SandboxLink,
                        });
                    });
                }
            }
            for building_outline in trigger.event().data.building_outlines.iter() {
                commands.spawn((
                    Mesh3d(meshes.add(building_outline.clone())),
                    MeshMaterial3d(village_map_resources.building_outline_material.clone()),
                    Transform::from_xyz(0.0, 1.0, 0.0),
                ));
            }
            if let Some(river) = &trigger.event().data.river {
                commands.spawn((
                    Name::new("VillageRiver"),
                    Mesh3d(meshes.add(river.clone())),
                    MeshMaterial3d(
                        hex_map_resources
                            .river_tile_materials
                            .river_battlemap_material
                            .clone(),
                    ),
                    Transform::from_xyz(0.0, -9.95, 0.0),
                ));
            }
            for water_mesh in trigger.event().data.water.iter() {
                commands.spawn((
                    Name::new("VillageCoast"),
                    Mesh3d(meshes.add(water_mesh.clone())),
                    MeshMaterial3d(hex_map_resources.water_material.clone()),
                    Transform::from_xyz(0.0, -9.9, 0.0),
                ));
            }
            trigger.event().data.base.spawn(
                &mut commands,
                &village_map_resources,
                &mut meshes,
                &gltfs,
            );
        })
        .id();
    if let Ok(mut entity) = commands.get_entity(trigger.event().hex) {
        entity.mark_battlemap_as_ready();
        entity.add_child(parent_node);
    } else {
        commands.entity(parent_node).despawn();
    }
}

pub struct VillageMapConstructs {
    base: SettlementMapConstructs,
    building_outlines: Vec<Mesh>,
    buildings: Vec<(Rect, f32, Option<String>)>,
    river: Option<Mesh>,
    water: Vec<Mesh>,
}

impl VillageMapConstructs {
    pub fn from(uid: BackendUid, json: String) -> Self {
        let map: SettlementJson = serde_json::from_str(&json).expect("Failed to parse JSON");
        let mut river = None;
        let mut water = Vec::new();

        let mut building_outlines: Vec<Mesh> = Vec::new();
        let mut buildings: Vec<(Rect, f32, Option<String>)> = Vec::new();
        let factor = 1.0;
        let mut offset = 0.0;
        let mut tagged_has_river = false;

        let mut base = SettlementMapConstructs::default();
        base.uid = uid;

        for f in map.map_data.features.iter() {
            if let Feature::Feature {
                id: _,
                roadWidth: _,
                towerRadius: _,
                wallThickness: _,
                generator: _,
                coast_dir,
                has_river,
                rotate_river: _,
                version: _,
            } = f
            {
                if coast_dir.is_some() {
                    offset = 420.0;
                }
                if let Some(has_river_val) = has_river {
                    tagged_has_river = *has_river_val;
                }
            }

            base.detect_roads(f, 510.0, offset);
            base.detect_trees(f);
            base.detect_fields(f);
            base.detect_squares(f);

            if let Feature::MultiPolygon { id, coordinates } = f {
                if id == "water" {
                    for c in coordinates.iter() {
                        let polygon: Vec<lyon::math::Point> = c
                            .points
                            .iter()
                            .map(|p| {
                                lyon::math::Point::new(
                                    p[0] as f32 * factor,
                                    p[1] as f32 * factor,
                                )
                            })
                            .collect();
                        match classify_water_body(polygon) {
                            VillageWaterBody::Ocean(points) => {
                                if let Some(water_mesh) = create_ocean_mesh(points) {
                                    water.push(water_mesh);
                                }
                            }
                            VillageWaterBody::River((points, gap)) => {
                                if tagged_has_river {
                                    river = create_river_mesh(
                                        remove_points_outside_of_hex(
                                            extend_line_to_endpoints_by_radius_and_angles(
                                                points,
                                            ),
                                            510.0,
                                            offset,
                                        ),
                                        gap * 0.31,
                                    );
                                }
                            }
                        }
                    }
                }
                if id == "buildings" {
                    for c in coordinates.iter() {
                        let v: Vec<[f64; 2]> = c.points.iter().map(|p| [p[0], p[1]]).collect();
                        let rect_in_space = get_width_height_rotation_from_rect_points(v);

                        buildings.push((
                            Rect::from_center_size(
                                rect_in_space.center,
                                rect_in_space.dimensions,
                            ),
                            rect_in_space.orientation,
                            c.uid.clone(),
                        ));

                        let polygon: Vec<lyon::math::Point> = c
                            .points
                            .iter()
                            .map(|p| {
                                lyon::math::Point::new(
                                    p[0] as f32 * factor,
                                    p[1] as f32 * factor,
                                )
                            })
                            .collect();
                        building_outlines.push(make_mesh_from_outline(&polygon, 1.0));
                    }
                }
            }
        }

        VillageMapConstructs {
            base,
            buildings,
            building_outlines,
            river,
            water,
        }
    }
}

fn noisify_point(
    point: lyon::math::Point,
    magnitude_min: f32,
    magnitude_max: f32,
) -> lyon::math::Point {
    let mut rng = rand::thread_rng();
    let offset_x: f32 = rng.gen_range(magnitude_min..=magnitude_max);
    lyon::math::Point::new(point.x + offset_x, point.y)
}

enum VillageWaterBody {
    Ocean(Vec<lyon::math::Point>),
    River((Vec<lyon::math::Point>, f32)),
}

fn classify_water_body(mut points: Vec<lyon::math::Point>) -> VillageWaterBody {
    points.retain(|point| {
        point.x >= -600.0 && point.x <= 600.0 && point.y >= -600.0 && point.y <= 600.0
    });
    let len = points.len();
    let mut max_gap: f32 = 0.0;
    let mut avg_gap: f32 = 0.0;
    let mut potential_river = Vec::new();
    let mut count: f32 = 0.0;
    for i in 0..(len / 2) {
        count += 1.0;
        let forward_point = &points[i]; // From the start
        let backward_point = &points[len - 1 - i]; // From the end
        potential_river
            .push(((forward_point.to_vector() + backward_point.to_vector()) / 2.0).to_point());
        let d = forward_point.distance_to(*backward_point);
        avg_gap += d;
        if d > max_gap {
            max_gap = d;
        }
    }
    const MAX_AVERAGE_GAP_BETWEEN_POINTS_FOR_A_RIVER: f32 = 120.0;
    if avg_gap / count < MAX_AVERAGE_GAP_BETWEEN_POINTS_FOR_A_RIVER {
        VillageWaterBody::River((potential_river, max_gap))
    } else {
        VillageWaterBody::Ocean(points)
    }
}

fn create_ocean_mesh(mut points: Vec<lyon::math::Point>) -> Option<Mesh> {
    points.retain(|point| {
        point.x >= -600.0 && point.x <= 600.0 && point.y >= -600.0 && point.y <= 600.0
    });
    if points.len() < 3 {
        return None;
    }
    points.remove(0);
    points.remove(0);
    points.remove(0);
    points.remove(0);

    points[0].x = -292.0;
    points[0].y = -223.0;

    let source = points[1];

    {
        let step_x = (points[0].x - source.x) / 4.0;
        let step_y = (points[0].y - source.y) / 4.0;
        for i in 1..=3 {
            let point = lyon::math::Point::new(
                source.x + step_x * i as f32,
                source.y + step_y * i as f32,
            );
            let offset = (3 - i) as f32 * 10.0;
            let mut p = noisify_point(point, -20.0, 0.0);
            p.x -= offset;
            p.y += offset / 3.0;
            points.insert(1, p);
        }
    }

    points.pop();
    points.pop();
    points.pop();
    points.pop();

    let target = lyon::math::Point::new(292.0, -223.0);
    let source = points.last().unwrap().clone();

    {
        let step_x = (target.x - source.x) / 4.0;
        let step_y = (target.y - source.y) / 4.0;
        for i in 0..=3 {
            let point = lyon::math::Point::new(
                source.x + step_x * i as f32,
                source.y + step_y * i as f32,
            );
            let offset = (3 - i) as f32 * 10.0;
            let mut p = noisify_point(point, 0.0, 20.0);
            p.x += offset;
            p.y += offset / 3.0;
            points.push(p);
        }
        points.push(target);
        points.push(target);
    }

    let mut builder = lyon::path::Path::builder();
    let first_point = points.first().unwrap();
    builder.begin(*first_point);
    for window in points.windows(3) {
        if let [_, p2, p3] = window {
            let mid_point = lyon::math::Point::new((p2.x + p3.x) / 2.0, (p2.y + p3.y) / 2.0);
            builder.quadratic_bezier_to(*p2, mid_point);
        }
    }
    builder.close();
    let path = builder.build();
    Some(make_filled_mesh_from_path(path))
}

fn create_river_mesh(points: Vec<lyon::math::Point>, width: f32) -> Option<Mesh> {
    if points.len() < 3 {
        return None;
    }

    let points = ensure_min_distance(points, 30.0);

    let mut builder = lyon::path::Path::builder();
    let first_point = points.first().unwrap();
    builder.begin(*first_point);
    for window in points.windows(3) {
        if let [_, p2, p3] = window {
            let mid_point = lyon::math::Point::new((p2.x + p3.x) / 2.0, (p2.y + p3.y) / 2.0);
            builder.quadratic_bezier_to(*p2, mid_point);
        }
    }
    builder.end(false);
    let path = builder.build();
    Some(create_bezier_curve_mesh(path, width, 1.0))
}
