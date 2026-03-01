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

use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};
use hexx::{Hex, HexLayout};

pub fn make_polygon(layout: &HexLayout, hex_list: &Vec<Hex>) -> Vec<Vec2> {
    let mut all_edges = Vec::new();
    for h in hex_list {
        let edges: [[Vec2; 2]; 6] = layout.all_edge_coordinates(*h);
        let edges: [[Vec2; 2]; 6] = edges.map(|[start, end]| {
            [
                Vec2::new(start.x, start.y * -1.0),
                Vec2::new(end.x, end.y * -1.0),
            ]
        });
        all_edges.extend_from_slice(&edges);
    }
    const EPSILON: f32 = 2.0;

    let mut unique_edges = Vec::new();
    all_edges.retain(|edge| {
        if let Some(pos) = unique_edges.iter().position(|unique_edge: &[Vec2; 2]| {
            ((unique_edge[0].x - edge[0].x).abs() < EPSILON
                && (unique_edge[0].y - edge[0].y).abs() < EPSILON
                && (unique_edge[1].x - edge[1].x).abs() < EPSILON
                && (unique_edge[1].y - edge[1].y).abs() < EPSILON)
                || ((unique_edge[1].x - edge[0].x).abs() < EPSILON
                    && (unique_edge[1].y - edge[0].y).abs() < EPSILON
                    && (unique_edge[0].x - edge[1].x).abs() < EPSILON
                    && (unique_edge[0].y - edge[1].y).abs() < EPSILON)
        }) {
            unique_edges.remove(pos);
            false
        } else {
            unique_edges.push(*edge);
            true
        }
    });

    let mut sorted_edges: Vec<[Vec2; 2]> = Vec::new();
    let mut next = unique_edges.pop().unwrap();
    sorted_edges.push(next);
    while !unique_edges.is_empty() {
        if let Some(next_index) = unique_edges.iter().position(|e| {
            (e[0].x - next[1].x).abs() < EPSILON && (e[0].y - next[1].y).abs() < EPSILON
        }) {
            next = unique_edges.remove(next_index);
            sorted_edges.push(next);
        } else {
            break;
        }
    }

    let mut flattened_edges: Vec<Vec2> = sorted_edges
        .iter()
        .flat_map(|edge| edge.iter().cloned())
        .collect();

    let eps_squared = EPSILON * EPSILON;
    flattened_edges.dedup_by(|a, b| (*a - *b).length_squared() < eps_squared);
    flattened_edges
}

pub fn make_mesh_from_outline(
    outline: &[lyon::math::Point],
    stroke_width: f32,
) -> bevy::prelude::Mesh {
    use lyon::math::{Point, point};
    use lyon::path::Path;
    use lyon::tessellation::*;

    let mut path_builder = Path::builder();

    if let Some(start_point) = outline.first() {
        path_builder.begin(point(start_point.x, start_point.y));

        for point in &outline[1..] {
            path_builder.line_to(point.clone());
        }

        // if let Some(start_point) = outline.first() {
        //     path_builder.line_to(point(start_point.x, start_point.y));
        // }
        path_builder.end(true);

        path_builder.close();
    }

    let path = path_builder.build();

    let mut geometry: VertexBuffers<Point, u32> = VertexBuffers::new();
    {
        // let mut tessellator = FillTessellator::new();
        let options = StrokeOptions::default()
            .with_tolerance(0.1)
            .with_line_width(stroke_width);
        let mut tessellator = StrokeTessellator::new();
        tessellator
            .tessellate_path(
                &path,
                &options,
                // &FillOptions::default(),
                // &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| vertex.position()),
                &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                    vertex.position()
                }),
            )
            .unwrap();
    }

    // Create a mesh from the tessellated geometry

    let vertices = geometry
        .vertices
        .iter()
        .map(|v| [v.x, 0.0, v.y])
        .collect::<Vec<_>>();
    let indices = geometry
        .indices
        .iter()
        .map(|i| *i as u32)
        .collect::<Vec<_>>();

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vertices
            .iter()
            .map(|v| [v[0], v[1], v[2]])
            .collect::<Vec<[f32; 3]>>(),
    )
    // .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vertices
            .iter()
            .map(|_| [0.0, 1.0, 0.0])
            .collect::<Vec<[f32; 3]>>(),
    )
    .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}

pub fn make_filled_mesh_from_outline(outline: &[lyon::math::Point]) -> bevy::prelude::Mesh {
    use lyon::math::{Point, point};
    use lyon::path::Path;
    use lyon::tessellation::*;

    let mut path_builder = Path::builder();

    if let Some(start_point) = outline.first() {
        path_builder.begin(point(start_point.x, start_point.y));

        for point in &outline[1..] {
            path_builder.line_to(point.clone());
        }
        path_builder.end(true);

        path_builder.close();
    }

    let path = path_builder.build();

    let mut geometry: VertexBuffers<Point, u32> = VertexBuffers::new();
    {
        // let mut tessellator = FillTessellator::new();
        let options = FillOptions::default().with_tolerance(0.1);
        let mut tessellator = FillTessellator::new();
        tessellator
            .tessellate_path(
                &path,
                &options,
                // &FillOptions::default(),
                // &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| vertex.position()),
                &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                    vertex.position()
                }),
            )
            .unwrap();
    }

    // Create a mesh from the tessellated geometry

    let vertices = geometry
        .vertices
        .iter()
        .map(|v| [v.x, 0.0, v.y])
        .collect::<Vec<_>>();

    let min_x = -10.;
    let min_y = -10.;
    let max_x = 10.;
    let max_y = 10.;

    let range_x = max_x - min_x;
    let range_y = max_y - min_y;

    let uvs = geometry
        .vertices
        .iter()
        .map(|v| {
            let u = if range_x != 0.0 {
                (v.x - min_x) / range_x
            } else {
                0.0
            };
            let v = if range_y != 0.0 {
                (v.y - min_y) / range_y
            } else {
                0.0
            };
            [u, v]
        })
        .collect::<Vec<_>>();

    let indices = geometry
        .indices
        .iter()
        .map(|i| *i as u32)
        .collect::<Vec<_>>();

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vertices
            .iter()
            .map(|v| [v[0], v[1], v[2]])
            .collect::<Vec<[f32; 3]>>(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        uvs.iter().map(|v| [v[0], v[1]]).collect::<Vec<[f32; 2]>>(),
    )
    // .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vertices
            .iter()
            .map(|_| [0.0, 1.0, 0.0])
            .collect::<Vec<[f32; 3]>>(),
    )
    .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}
pub fn make_filled_mesh_from_path(path: lyon::path::Path) -> bevy::prelude::Mesh {
    use lyon::math::Point;
    use lyon::tessellation::*;

    let mut geometry: VertexBuffers<Point, u32> = VertexBuffers::new();
    {
        // let mut tessellator = FillTessellator::new();
        let options = FillOptions::default().with_tolerance(0.005);
        let mut tessellator = FillTessellator::new();
        tessellator
            .tessellate_path(
                &path,
                &options,
                // &FillOptions::default(),
                // &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| vertex.position()),
                &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                    vertex.position()
                }),
            )
            .unwrap();
    }

    // Create a mesh from the tessellated geometry

    let vertices = geometry
        .vertices
        .iter()
        .map(|v| [v.x, 0.5, v.y])
        .collect::<Vec<_>>();

    let indices = geometry
        .indices
        .iter()
        .map(|i| *i as u32)
        .collect::<Vec<_>>();

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vertices
            .iter()
            .map(|v| [v[0], v[1], v[2]])
            .collect::<Vec<[f32; 3]>>(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vertices
            .iter()
            .map(|_| [0.0, 1.0, 0.0])
            .collect::<Vec<[f32; 3]>>(),
    )
    .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}

pub fn polygon_to_smooth_path(points: &[lyon::math::Point]) -> lyon::path::Path {
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
    builder.build()
}

pub fn make_mesh_from_outline2(
    outline: &[lyon::math::Point],
    stroke_width: f32,
) -> bevy::prelude::Mesh {
    use lyon::math::{Point, point};
    use lyon::path::Path;
    use lyon::tessellation::*;

    let mut path_builder = Path::builder();

    if let Some(start_point) = outline.first() {
        path_builder.begin(point(start_point.x, start_point.y));

        for point in &outline[1..] {
            path_builder.line_to(point.clone());
        }

        path_builder.end(false);
    }

    let path = path_builder.build();

    let mut geometry: VertexBuffers<Point, u32> = VertexBuffers::new();
    {
        // let mut tessellator = FillTessellator::new();
        let options = StrokeOptions::default()
            .with_tolerance(0.1)
            .with_line_width(stroke_width);
        let mut tessellator = StrokeTessellator::new();
        tessellator
            .tessellate_path(
                &path,
                &options,
                // &FillOptions::default(),
                // &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| vertex.position()),
                &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                    vertex.position()
                }),
            )
            .unwrap();
    }

    // Create a mesh from the tessellated geometry

    let vertices = geometry
        .vertices
        .iter()
        .map(|v| [v.x, 0.0, v.y])
        .collect::<Vec<_>>();
    let indices = geometry
        .indices
        .iter()
        .map(|i| *i as u32)
        .collect::<Vec<_>>();

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vertices
            .iter()
            .map(|v| [v[0], v[1], v[2]])
            .collect::<Vec<[f32; 3]>>(),
    )
    // .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vertices
            .iter()
            .map(|_| [0.0, 1.0, 0.0])
            .collect::<Vec<[f32; 3]>>(),
    )
    .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}
