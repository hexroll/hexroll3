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

use std::collections::BTreeMap;

// City and town maps - based on Watabou's datasets
use bevy::{platform::collections::HashMap, prelude::*};

use crate::{
    hexmap::elements::{HexMapData, HexMapResources},
    shared::{
        curve::create_bezier_curve_mesh,
        disc::create_3d_disc,
        geometry::{
            make_filled_mesh_from_outline, make_filled_mesh_from_path, make_mesh_from_outline,
            make_mesh_from_outline2,
        },
        layers::HEIGHT_OFFSET_OF_BATTLEMAP_CONTENT,
        widgets::cursor::{PointerOnHover, TooltipOnHover},
    },
};

use super::{BattlemapFeatureUtils, helpers::*, settlement::*};

use hexroll3_cartographer::watabou::json::*;

pub struct CityPlugin;

impl Plugin for CityPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_spawn_city_map);
    }
}

#[derive(Event)]
pub struct SpawnCityMap {
    pub hex: Entity,
    pub hex_uid: String,
    pub data: CityMapConstructs,
}

fn on_spawn_city_map(
    trigger: On<SpawnCityMap>,
    mut commands: Commands,
    settlement_map_resources: Res<SettlementMapResources>,
    hex_map_resources: Res<HexMapResources>,
    mut meshes: ResMut<Assets<Mesh>>,
    map: Res<HexMapData>,
    gltfs: Res<Assets<Gltf>>,
) {
    let city_constructs = &trigger.event().data;
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

    let tower = meshes.add(create_3d_disc(7.5, 0.0, 10));

    let parent_node = commands
        .spawn_empty()
        .insert(Name::new("CityMap"))
        .insert(Visibility::default())
        .insert(
            Transform::from_xyz(tx.x, tx.y, tx.z)
                .with_rotation(Quat::from_rotation_y(angle))
                .with_scale(Vec3::new(0.10, 0.20, 0.10)),
        )
        .with_children(|mut commands| {
            for (building_mesh, uid) in city_constructs.buildings.iter() {
                let mut building = commands.spawn((
                    Mesh3d(meshes.add(building_mesh.clone())),
                    MeshMaterial3d(if uid.is_some() {
                        settlement_map_resources.building_highlight_material.clone()
                    } else {
                        settlement_map_resources.building_material.clone()
                    }),
                    Transform::from_xyz(0.0, -1.1, 0.0),
                ));
                if let Some(uid) = uid {
                    let cuid = uid.clone();
                    building.pointer_on_hover();

                    if let Some(label) = city_constructs.poi_labels.get(uid) {
                        building.tooltip_on_hover(label, 0.1);
                    }
                    building.observe(move |_: On<Pointer<Click>>, mut commands: Commands| {
                        commands.trigger(crate::hexmap::elements::FetchEntityFromStorage {
                            uid: cuid.clone(),
                            anchor: None,
                            why: crate::clients::model::FetchEntityReason::SandboxLink,
                        });
                    });
                }
            }
            for building_outline in city_constructs.building_outlines.iter() {
                commands.spawn((
                    Mesh3d(meshes.add(building_outline.clone())),
                    MeshMaterial3d(settlement_map_resources.building_outline_material.clone()),
                    Transform::from_xyz(0.0, -1.0, 0.0),
                ));
            }
            if let Some(river) = &city_constructs.river {
                commands.spawn((
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
            for water_mesh in city_constructs.water.iter() {
                commands.spawn((
                    Mesh3d(meshes.add(water_mesh.clone())),
                    MeshMaterial3d(hex_map_resources.water_material.clone()),
                    Transform::from_xyz(0.0, -9.9, 0.0),
                ));
            }
            for wall_mesh in city_constructs.walls.iter() {
                commands.spawn((
                    Mesh3d(meshes.add(wall_mesh.clone())),
                    MeshMaterial3d(settlement_map_resources.building_material.clone()),
                    Transform::from_xyz(0.0, -1.00, 0.0),
                ));
            }
            for tower_mesh in city_constructs.towers.iter() {
                commands.spawn((
                    Mesh3d(tower.clone()),
                    MeshMaterial3d(settlement_map_resources.building_material.clone()),
                    Transform::from_xyz(tower_mesh.x, -0.95, tower_mesh.y),
                ));
            }
            city_constructs.base.spawn(
                &mut commands,
                &settlement_map_resources,
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

pub struct CityMapConstructs {
    base: SettlementMapConstructs,
    buildings: Vec<(Mesh, Option<String>)>,
    building_outlines: Vec<Mesh>,
    river: Option<Mesh>,
    water: Vec<Mesh>,
    walls: Vec<Mesh>,
    towers: Vec<Vec2>,
    poi_labels: HashMap<String, String>,
}

impl CityMapConstructs {
    fn split_segments_by_intersections(
        segments: &mut [Vec<lyon::math::Point>],
        line: &[lyon::math::Point],
        gap: f32,
    ) -> Vec<Vec<lyon::math::Point>> {
        let mut new_segments: Vec<Vec<lyon::math::Point>> = Vec::new();
        for segment in segments.iter_mut() {
            if segment.len() < 2 {
                continue;
            }
            let mut new_segment: Vec<lyon::math::Point> = Vec::new();
            segment.push(*segment.last().unwrap());
            let mut last_point: Option<&lyon::math::Point> = None;
            for p in segment.windows(2) {
                if line.contains(&p[0]) {
                    if let Some(last_point) = last_point {
                        let direction1 = (*last_point - p[0]).normalize();
                        let shortened_p0 = p[0] + direction1 * (gap / 2.0);
                        new_segment.push(shortened_p0);
                        new_segments.push(new_segment);
                        new_segment = Vec::new();
                    }
                    let direction1 = (p[1] - p[0]).normalize();
                    let shortened_p0 = p[0] + direction1 * (gap / 2.0);
                    new_segment.push(shortened_p0);
                } else {
                    new_segment.push(p[0]);
                }
                last_point = Some(&p[0]);
            }
            new_segments.push(new_segment);
        }
        new_segments
    }

    pub fn from(json: String) -> Self {
        let map: SettlementJson = serde_json::from_str(&json).expect("Failed to parse JSON");
        let mut river = None;
        let mut water = Vec::new();
        let mut walls = Vec::new();
        let mut towers: Vec<bevy::math::Vec2> = Vec::new();

        let mut buildings: Vec<(Mesh, Option<String>)> = Vec::new();
        let mut building_outlines: Vec<Mesh> = Vec::new();
        let mut offset = 0.0;

        let mut base = SettlementMapConstructs::default();

        let mut water_points: Vec<lyon::math::Point> = Vec::new();
        let mut wall_points: Vec<(Vec<lyon::math::Point>, f32)> = Vec::new();
        let mut river_points: Vec<lyon::math::Point> = Vec::new();
        let mut river_width: f32 = 0.0;
        let mut roads: Vec<(Vec<lyon::math::Point>, f32)> = Vec::new();

        for f in map.map_data.features.iter() {
            if let Feature::Feature {
                id: _,
                roadWidth: _,
                towerRadius: _,
                wallThickness: _,
                generator: _,
                coast_dir,
                has_river: _,
                rotate_river: _,
                version: _,
            } = f
            {
                if coast_dir.is_some() {
                    offset = 420.0;
                }
            }

            if let Some(r) = base.detect_roads(f, 1020.0, offset) {
                roads = r;
            }
            base.detect_trees(f);
            base.detect_fields(f, 1020.0, offset);
            base.detect_squares(f);

            if let Feature::GeometryCollection { id, geometries } = f {
                if id == "walls" {
                    for g in geometries.iter() {
                        if let Geometry::Polygon { width, coordinates } = g {
                            wall_points.push((
                                coordinates
                                    .iter()
                                    .next()
                                    .unwrap()
                                    .iter()
                                    .map(|c| lyon::math::Point::new(c[0] as f32, c[1] as f32))
                                    .collect(),
                                width.unwrap() as f32,
                            ));
                        }
                    }
                }
            }
            if let Feature::MultiPolygon { id, coordinates } = f {
                if id == "water" {
                    for c in coordinates.iter() {
                        water_points = c
                            .points
                            .iter()
                            .map(|p| lyon::math::Point::new(p[0] as f32, p[1] as f32))
                            .collect();
                        if let Some(water_mesh) = create_water_mesh(water_points.clone()) {
                            water.push(water_mesh);
                        }
                    }
                }
                if id == "buildings" {
                    let proxy: BTreeMap<i32, String> = map
                        .poi
                        .iter()
                        .filter_map(|v| {
                            if let Some(building_index) = v.building {
                                Some((building_index, v.uuid.clone()))
                            } else {
                                None
                            }
                        })
                        .collect();
                    for (i, c) in coordinates.iter().enumerate() {
                        if !c
                            .points
                            .iter()
                            .all(|p| is_point_inside_hex(p.as_slice(), 1020.0, offset))
                        {
                            continue;
                        }
                        let polygon: Vec<lyon::math::Point> = c
                            .points
                            .iter()
                            .map(|p| lyon::math::Point::new(p[0] as f32, p[1] as f32))
                            .collect();
                        let uid = proxy.get(&(i as i32)).cloned().or_else(|| c.uid.clone());
                        buildings.push((make_filled_mesh_from_outline(&polygon), uid));
                        building_outlines.push(make_mesh_from_outline(&polygon, 2.0));
                    }
                }
            }
            if let Feature::GeometryCollection { id, geometries } = f {
                if id == "rivers" {
                    for g in geometries.iter() {
                        if let Geometry::LineString { width, coordinates } = g {
                            if let Some(width) = width {
                                river_points = coordinates
                                    .iter()
                                    .map(|p| lyon::math::Point::new(p[0] as f32, p[1] as f32))
                                    .collect();
                                river_width = *width as f32;
                                river = create_river_mesh(
                                    remove_points_outside_of_hex(
                                        extend_line_to_endpoints_by_radius_and_angles(
                                            river_points.clone(),
                                        ),
                                        1020.0,
                                        offset,
                                    ),
                                    *width as f32,
                                );
                            }
                        }
                    }
                }
            }
        }

        for (mut wall, width) in wall_points {
            let mut segments: Vec<Vec<lyon::math::Point>> = vec![Vec::new()];
            let mut hits = 0;
            wall.push(*wall.first().unwrap());
            wall.push(*wall.first().unwrap());
            for p in wall.windows(2) {
                if water_points.contains(&p[0]) && water_points.contains(&p[1]) {
                    hits += 1;
                    if hits >= 2 {
                        if !segments.last().unwrap().is_empty() {
                            segments.push(Vec::new())
                        };
                        continue;
                    }
                }
                segments.last_mut().unwrap().push(p[0]);
            }

            segments = CityMapConstructs::split_segments_by_intersections(
                &mut segments,
                &river_points,
                river_width * 1.2,
            );

            for (road, width) in roads.iter() {
                segments = CityMapConstructs::split_segments_by_intersections(
                    &mut segments,
                    &road,
                    width * 2.6,
                );
            }

            for segment in segments.iter() {
                towers.extend(
                    segment
                        .iter()
                        .map(|p| Vec2::new(p.x, p.y))
                        .collect::<Vec<Vec2>>(),
                );
                walls.push(make_mesh_from_outline2(
                    &remove_points_outside_of_hex(segment.to_vec(), 1020.0, offset),
                    width,
                ));
            }
        }

        let poi_labels: HashMap<String, String> = map
            .poi
            .iter()
            .map(|value| (value.uuid.clone(), value.title.clone()))
            .collect();

        CityMapConstructs {
            base,
            buildings,
            building_outlines,
            walls,
            towers,
            river,
            water,
            poi_labels,
        }
    }
}

fn create_river_mesh(points: Vec<lyon::math::Point>, width: f32) -> Option<Mesh> {
    if points.is_empty() {
        return None;
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
    builder.end(false);
    let path = builder.build();

    Some(create_bezier_curve_mesh(path, width, 2.0))
}

fn create_water_mesh(mut points: Vec<lyon::math::Point>) -> Option<Mesh> {
    points.retain(|point| {
        point.x >= -600.0 && point.x <= 600.0 && point.y >= -600.0 && point.y <= 600.0
    });
    if points.len() < 3 {
        return None;
    }
    points.first_mut().unwrap().y = -600.0;
    let mut cp = points.last().unwrap().clone();
    cp.y = -600.0;
    points.push(cp);
    points.push(cp);
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
