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

// Shared settlement map data and behaviors
//
// Anything that can be shared between cities, towns and villages maps should be here.
// This includes the JSON deserialization code, common entities such as trees, and
// common materials to name a few.
use std::time::Duration;

use bevy::prelude::*;
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::Deserialize;

use crate::{
    clients::model::BackendUid,
    shared::{
        geometry::{
            make_filled_mesh_from_outline, make_filled_mesh_from_path,
            make_mesh_from_outline2, polygon_to_smooth_path,
        },
        gltf::*,
    },
};

use super::helpers::remove_points_outside_of_hex;

pub struct SettlementPlugin;

impl Plugin for SettlementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, setup_trees_post_gltf_loaded);
    }
}

#[derive(Resource)]
pub struct SettlementMapResources {
    pub field_material: Handle<StandardMaterial>,
    pub square_material: Handle<StandardMaterial>,
    pub building_material: Handle<StandardMaterial>,
    pub building_outline_material: Handle<StandardMaterial>,
    pub building_highlight_material: Handle<StandardMaterial>,
    pub tree_animation: Option<crate::shared::gltf::Animations>,
    pub tree_gltf: Handle<Gltf>,
}

fn setup_trees_post_gltf_loaded(
    mut commands: Commands,
    map_resources: Res<SettlementMapResources>,
    mut tokens: Query<(Entity, &Name, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    if let Some(tree_animation) = &map_resources.tree_animation {
        for (entity, name, mut player) in &mut tokens {
            if name.as_str() == "Windytree" {
                let mut transitions = AnimationTransitions::new();
                transitions
                    .play(&mut player, tree_animation.animations[0], Duration::ZERO)
                    .repeat();
                commands
                    .entity(entity)
                    .try_insert(AnimationGraphHandle(tree_animation.graph.clone()))
                    .try_insert(transitions);
            }
        }
    }
}

fn on_setup_tree_gltf(
    mut map_resources: ResMut<SettlementMapResources>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    gltfs: Res<Assets<Gltf>>,
) {
    if let Some(gltf) = gltfs.get(&map_resources.tree_gltf) {
        let tree_animation = {
            let (graph, node_indices) = AnimationGraph::from_clips([asset_server
                .load(GltfAssetLabel::Animation(0).from_asset("tokens/windytree.glb"))]);
            let graph_handle = graphs.add(graph);
            crate::shared::gltf::Animations {
                animations: node_indices,
                graph: graph_handle,
            }
        };
        map_resources.tree_animation = Some(tree_animation);
        for m in gltf.materials.iter() {
            if let Some(mat) = materials.get_mut(m) {
                mat.unlit = true;
            }
        }
    }
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let sysid = commands.register_system(on_setup_tree_gltf);
    let tree_gltf: Handle<Gltf> = asset_server.load("tokens/windytree.glb");

    commands.spawn(GltfProcessorJob {
        gltf: tree_gltf.clone(),
        callback: sysid,
    });

    commands.insert_resource(SettlementMapResources {
        field_material: materials.add(StandardMaterial {
            unlit: true,
            base_color: Color::srgba_u8(124, 165, 0, 112),
            alpha_mode: AlphaMode::AlphaToCoverage,
            ..default()
        }),
        square_material: materials.add(StandardMaterial {
            unlit: true,
            base_color: Color::srgba_u8(189, 160, 108, 121),
            alpha_mode: AlphaMode::AlphaToCoverage,
            ..default()
        }),
        building_material: materials.add(StandardMaterial {
            unlit: true,
            base_color: Color::srgb_u8(44, 44, 44),
            ..default()
        }),
        building_outline_material: materials.add(StandardMaterial {
            unlit: true,
            base_color: Color::srgb_u8(0, 0, 0),
            ..default()
        }),
        building_highlight_material: materials.add(StandardMaterial {
            unlit: true,
            base_color: Color::srgb_u8(255, 100, 0),
            ..default()
        }),
        tree_animation: None,
        tree_gltf,
    });
}

#[derive(Default)]
pub struct SettlementMapConstructs {
    pub uid: BackendUid,
    pub roads: Vec<Mesh>,
    pub squares: Vec<Mesh>,
    pub fields: Vec<Mesh>,
    pub trees: Vec<Vec2>,
}

impl SettlementMapConstructs {
    pub fn detect_trees(&mut self, f: &Feature) {
        if let Feature::MultiPoint { id, coordinates } = f {
            if id == "trees" {
                self.trees = coordinates
                    .iter()
                    .map(|c| Vec2::new(c[0] as f32, c[1] as f32))
                    .collect();
            }
        }
    }
    pub fn detect_roads(
        &mut self,
        f: &Feature,
        hex_size: f32,
        offset: f32,
    ) -> Option<Vec<(Vec<lyon::math::Point>, f32)>> {
        if let Feature::GeometryCollection { id, geometries } = f {
            if id == "roads" {
                let mut ret = Vec::new();
                for g in geometries.iter() {
                    if let Geometry::LineString { width, coordinates } = g {
                        let width = width.unwrap() as f32;
                        let points: Vec<lyon::math::Point> = coordinates
                            .iter()
                            .map(|c| lyon::math::Point::new(c[0] as f32, c[1] as f32))
                            .collect();
                        ret.push((points.clone(), width));
                        self.roads.push(make_mesh_from_outline2(
                            &remove_points_outside_of_hex(points, hex_size, offset),
                            width,
                        ));
                    }
                }
                return Some(ret);
            }
        }
        None
    }
    pub fn detect_squares(&mut self, f: &Feature) {
        let factor = 1.0;
        if let Feature::MultiPolygon { id, coordinates } = f {
            if id == "squares" {
                for c in coordinates.iter() {
                    let polygon: Vec<lyon::math::Point> = c
                        .points
                        .iter()
                        .map(|p| {
                            lyon::math::Point::new(p[0] as f32 * factor, p[1] as f32 * factor)
                        })
                        .collect();
                    self.squares.push(make_filled_mesh_from_outline(&polygon));
                }
            }
        }
    }
    pub fn detect_fields(&mut self, f: &Feature) {
        let factor = 1.0;
        if let Feature::MultiPolygon { id, coordinates } = f {
            if id == "fields" {
                for c in coordinates.iter() {
                    let polygon: Vec<lyon::math::Point> = c
                        .points
                        .iter()
                        .map(|p| {
                            lyon::math::Point::new(p[0] as f32 * factor, p[1] as f32 * factor)
                        })
                        .collect();

                    self.fields
                        .push(make_filled_mesh_from_path(polygon_to_smooth_path(&polygon)));
                }
            }
        }
    }

    pub fn spawn(
        &self,
        commands: &mut bevy::ecs::relationship::RelatedSpawnerCommands<'_, ChildOf>,
        map_resources: &Res<SettlementMapResources>,
        meshes: &mut ResMut<Assets<Mesh>>,
        gltfs: &Res<Assets<Gltf>>,
    ) {
        for b in self.roads.iter() {
            commands.spawn((
                Mesh3d(meshes.add(b.clone())),
                MeshMaterial3d(map_resources.square_material.clone()),
                Transform::from_xyz(0.0, -10.29, 0.0),
            ));
        }
        for b in self.squares.iter() {
            commands.spawn((
                Mesh3d(meshes.add(b.clone())),
                MeshMaterial3d(map_resources.square_material.clone()),
                Transform::from_xyz(0.0, -10.29, 0.0),
            ));
        }
        for b in self.fields.iter() {
            commands.spawn((
                Mesh3d(meshes.add(b.clone())),
                MeshMaterial3d(map_resources.field_material.clone()),
                Transform::from_xyz(0.0, -12.29, 0.0),
            ));
        }

        let mut rng = StdRng::seed_from_u64(self.uid.as_u64_hash());
        if let Some(gltf) = gltfs.get(&map_resources.tree_gltf) {
            for tree in self.trees.iter() {
                if rng.gen_range(0..3) == 0 {
                    commands.spawn((
                        Name::new("Windytree"),
                        SceneRoot(gltf.default_scene.as_ref().unwrap().clone()),
                        Transform::from_xyz(tree.x, -5.0, tree.y)
                            .with_scale(Vec3::splat(2.5 + rng.r#gen::<f32>() * 1.5))
                            .with_rotation(Quat::from_rotation_y(
                                rng.r#gen::<f32>() * std::f32::consts::PI,
                            )),
                    ));
                }
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct SettlementJson {
    pub map_data: MapData,
    pub poi: Vec<PointOfInterest>,
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)] // Adding lint exception for unused field
pub struct MapData {
    #[serde(rename = "type")]
    pub map_type: String,
    pub features: Vec<Feature>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[allow(non_snake_case)]
#[allow(clippy::large_enum_variant)]
#[allow(clippy::enum_variant_names)]
#[allow(dead_code)] // Adding lint exception for unused field
pub enum Feature {
    Feature {
        id: String,
        roadWidth: Option<f64>,
        towerRadius: Option<f64>,
        wallThickness: Option<f64>,
        generator: Option<String>,
        coast_dir: Option<i32>,
        has_river: Option<bool>,
        rotate_river: Option<f64>,
        version: Option<String>,
    },
    Polygon {
        id: String,
        coordinates: Vec<Vec<[f64; 2]>>,
    },
    GeometryCollection {
        id: String,
        geometries: Vec<Geometry>,
    },
    MultiPolygon {
        id: String,
        coordinates: Vec<Polygon>,
    },
    MultiPoint {
        id: String,
        coordinates: Vec<[f64; 2]>,
    },
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Polygon {
    pub points: Vec<Vec<f64>>,
    pub uid: Option<String>,
    pub uid1: Option<String>,
    pub uid2: Option<String>,
    pub uid3: Option<String>,
    pub uid4: Option<String>,
    pub uid5: Option<String>,
    pub uid6: Option<String>,
    pub uid7: Option<String>,
    pub uid8: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Geometry {
    LineString {
        width: Option<f64>,
        coordinates: Vec<[f64; 2]>,
    },
    Polygon {
        width: Option<f64>,
        coordinates: Vec<Vec<[f64; 2]>>,
    },
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)] // Adding lint exception for unused field
pub struct PointOfInterest {
    pub coords: Coords,
    pub title: String,
    pub uuid: String,
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)] // Adding lint exception for unused field
pub struct Coords {
    pub x: f64,
    pub y: f64,
}
