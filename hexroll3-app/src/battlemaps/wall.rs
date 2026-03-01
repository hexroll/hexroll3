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

#![allow(dead_code)]
use core::f32;

use bevy::asset::RenderAssetUsages;
use bevy::math::{Dir3, Quat, Vec2, Vec3, primitives::Plane3d};
use bevy::mesh::{Indices, Mesh, MeshBuilder, PrimitiveTopology};

/// WallMeshBuilder constructs a wall segment for dungeon rooms
/// and corrider. Wall segments are planes designed to be both
/// irregular and able to seamlessly interconnect with each other.
///
/// Wall profile from the top:
///        ____________________
///       |  _       ___       |
///       |_/ \_/\__/   \_/\/\_|
///
/// Wall profile from the side:
///
///        ______
///        ||
///        ||
///        ||
///   
/// Note that the roof-like segment at the top of the wall is designed
/// to hide the extruding part of the floor that overflow under the
/// irregular shape of the wall.
///
#[derive(Clone, Copy, Debug, Default)]
pub struct WallMeshBuilder {
    pub plane: Plane3d,
    pub subdivisions: u32,
    pub internal_corners: (bool, bool),
    pub aligned_edges: (bool, bool),
}

impl WallMeshBuilder {
    #[inline]
    pub fn new(
        normal: Dir3,
        size: Vec2,
        internal_corners: (bool, bool),
        aligned_edges: (bool, bool),
    ) -> Self {
        Self {
            plane: Plane3d {
                normal,
                half_size: size / 2.0,
            },
            subdivisions: 0,
            internal_corners,
            aligned_edges,
        }
    }
}

const BUFFER_DEPTH: f32 = 0.25;

use rand::Rng;
impl MeshBuilder for WallMeshBuilder {
    fn build(&self) -> Mesh {
        // w_multiplier can be random, or set to 2 for a more uniform look
        let w_multiplier = rand::thread_rng().gen_range(0..=2);
        let w = self.plane.half_size.x as u32 * w_multiplier;
        let subdivs = rand::thread_rng().gen_range(w + 1..=w * 2 + 1);
        let z_vertex_count = self.subdivisions + 2;
        let x_vertex_count = subdivs + 4;
        let num_vertices = (z_vertex_count * x_vertex_count * 2) as usize;
        let num_indices = ((z_vertex_count - 1) * (x_vertex_count - 1) * 6 * 2) as usize;

        let mut positions: Vec<Vec3> = Vec::with_capacity(num_vertices);
        let mut normals: Vec<[f32; 3]> = Vec::with_capacity(num_vertices);
        let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(num_vertices);
        let mut indices: Vec<u32> = Vec::with_capacity(num_indices);

        let rotation = match self.plane.normal {
            Dir3::Z => Quat::from_rotation_x(f32::consts::PI / 2.0),
            Dir3::NEG_Z => Quat::from_rotation_x(f32::consts::PI / -2.0),
            Dir3::X => {
                Quat::from_rotation_x(f32::consts::PI / 2.0)
                    * Quat::from_rotation_z(f32::consts::PI / -2.0)
            }
            Dir3::NEG_X => {
                Quat::from_rotation_x(f32::consts::PI / 2.0)
                    * Quat::from_rotation_z(f32::consts::PI / 2.0)
            }
            _ => Quat::from_rotation_y(0.0),
        };

        let size = self.plane.half_size * 2.0;

        // Build the irregular shape of the wall, excluding the edges
        let mut offsets = Vec::with_capacity(x_vertex_count as usize);
        for x in 0..x_vertex_count {
            let is_middle =
                x != 0 && x != x_vertex_count - 1 && x != 1 && x != x_vertex_count - 2;
            let offset = if is_middle {
                // TODO: Experiment with -0.025..=0.025 up to -0.075..=0.075
                rand::thread_rng().gen_range(-0.05..=0.05)
            } else {
                0.0
            };
            offsets.push(offset);
        }

        // This adds a first set of quad positions, normals and uvs
        for z in 0..z_vertex_count {
            for x in 0..x_vertex_count {
                let atx = if x == 1 {
                    -0.5 * size.x + BUFFER_DEPTH / 2.0
                } else if x == x_vertex_count - 2 {
                    0.5 * size.x - BUFFER_DEPTH / 2.0
                } else {
                    let tx = x as f32 / (x_vertex_count - 1) as f32;
                    (-0.5 + tx) * size.x
                };
                let tz = z as f32 / (z_vertex_count - 1) as f32;
                let pos = rotation * Vec3::new(atx, offsets[x as usize], (-0.5 + tz) * size.y);
                positions.push(pos);
                normals.push(self.plane.normal.to_array());
                uvs.push([atx / size.x, tz]);
            }
        }

        for x in 0..x_vertex_count {
            // The buffer x offset is mostly evenly
            // distributed based on subdivs, except for the edges
            // where we have the fixed connector offset (currently 0.25 / 2.0)
            let buffer_front_x = if x == 1 {
                -0.5 * size.x + BUFFER_DEPTH / 2.0
            } else if x == x_vertex_count - 2 {
                0.5 * size.x - BUFFER_DEPTH / 2.0
            } else {
                let tx = x as f32 / (x_vertex_count - 1) as f32;
                (-0.5 + tx) * size.x
            };
            let wall_top_z = if self.plane.normal == Dir3::NEG_Z {
                0.5
            } else {
                -0.5
            } * size.y;

            let pos = rotation * Vec3::new(buffer_front_x, offsets[x as usize], wall_top_z);
            positions.push(pos);
            normals.push(self.plane.normal.to_array());
            uvs.push([0.0, 0.0]);

            // Dealing with internal corners that should have the back of the
            // inverted buffer connect outside the boundries of the wall:
            let buffer_rear_x = if x == 0 && self.internal_corners.0 {
                -0.5 * size.x - BUFFER_DEPTH
            } else if x == x_vertex_count - 1 && self.internal_corners.1 {
                0.5 * size.x + BUFFER_DEPTH
            } else {
                buffer_front_x
            };

            // The buffer depth is set to -0.25 unless we are on the
            // edges and the edge is not an aligned_edge, meaning,
            // it is not continuing another interfacing wall .
            let depth = if ((x == 0 && !self.aligned_edges.0)
                || (x == x_vertex_count - 1 && !self.aligned_edges.1))
                && buffer_rear_x == buffer_front_x
            {
                -0.0001
            } else {
                -BUFFER_DEPTH
            };

            let pos = rotation * Vec3::new(buffer_rear_x, depth, wall_top_z);

            positions.push(pos);
            normals.push(Dir3::NEG_Y.to_array());
            uvs.push([0.1, 0.1]);
        }

        // This adds the triangle list indices for the first set of quads
        for z in 0..z_vertex_count - 1 {
            for x in 0..x_vertex_count - 1 {
                let quad = z * x_vertex_count + x;
                indices.push(quad + x_vertex_count + 1);
                indices.push(quad + 1);
                indices.push(quad + x_vertex_count);
                indices.push(quad);
                indices.push(quad + x_vertex_count);
                indices.push(quad + 1);
            }
        }

        for x in 0..x_vertex_count - 1 {
            let quad = z_vertex_count * x_vertex_count + 2 * x;
            indices.push(quad + 1);
            indices.push(quad + 2);
            indices.push(quad);
            indices.push(quad + 3);
            indices.push(quad + 2);
            indices.push(quad + 1);
        }

        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_indices(Indices::U32(indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    }
}
