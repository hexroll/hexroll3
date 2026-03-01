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

use core::f32;

use bevy::{camera::visibility::RenderLayers, prelude::*};
use hexx::EdgeDirection;

use curve::*;

use super::tiles::{RiverMaterial, TrailMaterial};
use crate::shared::curve;
use crate::shared::layers::{self, RENDER_LAYER_MAP_LOD_HIGH};

pub fn spawn_river_tile(
    commands: &mut EntityCommands,
    orientation: i32,
    curved_mesh_tile: &Handle<Mesh>,
    river_tile_materials: &RiverTileMaterials,
) -> Entity {
    commands
        .with_child((
            Name::new("RiverTile"),
            Mesh3d(curved_mesh_tile.clone()),
            MeshMaterial3d(river_tile_materials.river_tile_material.clone()),
            RenderLayers::layer(RENDER_LAYER_MAP_LOD_HIGH),
            Transform::from_xyz(0.0, 0.05, 0.0).with_rotation(Quat::from_rotation_y(
                (f32::consts::PI / 3.0) * (orientation as f32) * -1.0,
            )),
            Pickable {
                should_block_lower: false,
                is_hoverable: false,
            },
        ))
        .id()
}

pub fn spawn_trail_tile(
    commands: &mut ChildSpawnerCommands,
    orientation: i32,
    curved_mesh_tile: &Handle<Mesh>,
    river_tile_materials: &Handle<TrailMaterial>,
) -> Entity {
    commands
        .spawn((
            Name::new("TrailTile"),
            Mesh3d(curved_mesh_tile.clone()),
            MeshMaterial3d(river_tile_materials.clone()),
            RenderLayers::layer(RENDER_LAYER_MAP_LOD_HIGH),
            Transform::from_xyz(0.0, layers::HEIGHT_OF_TOP_MOST_LAYERED_TILE + 150.0, 0.0)
                .with_rotation(Quat::from_rotation_y(
                    (f32::consts::PI / 3.0) * (orientation as f32) * -1.0,
                )),
            Pickable {
                should_block_lower: false,
                is_hoverable: false,
            },
        ))
        .id()
}

#[derive(Debug, Clone)]
pub struct CurvedMeshTileSet {
    pub north_to_north_east: Handle<Mesh>,
    pub north_to_south_east: Handle<Mesh>,
    pub north_to_south: Handle<Mesh>,
    pub north_to_south_west: Handle<Mesh>,
    pub north_to_north_west: Handle<Mesh>,
    pub to_north: Handle<Mesh>,
}

#[derive(Debug)]
pub struct RiverTileMaterials {
    pub river_tile_material: Handle<RiverMaterial>,
    pub river_battlemap_material: Handle<RiverMaterial>,
}

pub fn build_curved_mesh_tile_set(
    layout: &hexx::HexLayout,
    meshes: &mut ResMut<Assets<Mesh>>,
) -> CurvedMeshTileSet {
    CurvedMeshTileSet {
        north_to_north_east: build_curved_mesh_tile(
            &get_hex_curve_coords(
                layout,
                EdgeDirection::FLAT_NORTH,
                EdgeDirection::FLAT_NORTH_EAST,
            ),
            meshes,
            1.0,
        ),
        north_to_south_east: build_curved_mesh_tile(
            &get_hex_curve_coords(
                layout,
                EdgeDirection::FLAT_NORTH,
                EdgeDirection::FLAT_SOUTH_EAST,
            ),
            meshes,
            1.0,
        ),
        north_to_south: build_curved_mesh_tile(
            &get_hex_curve_coords(
                layout,
                EdgeDirection::FLAT_NORTH,
                EdgeDirection::FLAT_SOUTH,
            ),
            meshes,
            1.0,
        ),
        north_to_south_west: build_curved_mesh_tile(
            &get_hex_curve_coords(
                layout,
                EdgeDirection::FLAT_NORTH,
                EdgeDirection::FLAT_SOUTH_WEST,
            ),
            meshes,
            1.0,
        ),
        north_to_north_west: build_curved_mesh_tile(
            &get_hex_curve_coords(
                layout,
                EdgeDirection::FLAT_NORTH,
                EdgeDirection::FLAT_NORTH_WEST,
            ),
            meshes,
            1.0,
        ),
        to_north: build_curved_mesh_tile(
            &get_hex_midstart_coords(layout, EdgeDirection::FLAT_NORTH),
            meshes,
            0.5,
        ),
    }
}

fn build_curved_mesh_tile(
    curve_coords: &CurveCoords,
    meshes: &mut ResMut<Assets<Mesh>>,
    length: f32,
) -> Handle<Mesh> {
    meshes.add(create_curve_mesh(
        &curve_coords.from,
        &curve_coords.to,
        16.0,
        length,
    ))
}
