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

// 3D disc shape
use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};

pub fn create_3d_disc(radius: f32, depth: f32, segments: u32) -> Mesh {
    let angle_increment = 2.0 * std::f32::consts::PI / segments as f32;

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    vertices.push([0.0, 0.0, 0.0]);

    for i in 0..=segments {
        let angle = i as f32 * angle_increment;
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        vertices.push([x, depth, y]);
    }

    let mut indices: Vec<u32> = Vec::new();

    for i in 1..=segments {
        indices.push(0);
        indices.push((i % segments) + 1);
        indices.push(i);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices.clone())
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vertices
            .iter()
            .map(|_| [0.0, 1.0, 0.0])
            .collect::<Vec<[f32; 3]>>(),
    )
    .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}
