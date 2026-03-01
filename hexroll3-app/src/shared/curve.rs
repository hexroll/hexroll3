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

use bevy::prelude::*;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;

use lyon::math::*;
use lyon::tessellation::*;

pub fn create_curve_mesh(from: &hexx::Vec2, to: &hexx::Vec2, width: f32, length: f32) -> Mesh {
    return create_curve_with_path_func(build_curve_path, from, to, width, length);
}

pub struct CurveCoords {
    pub from: Vec2,
    pub to: Vec2,
}

pub fn get_hex_curve_coords(
    layout: &hexx::HexLayout,
    from: hexx::EdgeDirection,
    to: hexx::EdgeDirection,
) -> CurveCoords {
    let a1 = layout.vertex_coordinates(hexx::GridVertex {
        origin: hexx::Hex::new(0, 0),
        direction: from.vertex_ccw(),
    });
    let a2 = layout.vertex_coordinates(hexx::GridVertex {
        origin: hexx::Hex::new(0, 0),
        direction: from.vertex_cw(),
    });

    let b1 = layout.vertex_coordinates(hexx::GridVertex {
        origin: hexx::Hex::new(0, 0),
        direction: to.vertex_cw(),
    });
    let b2 = layout.vertex_coordinates(hexx::GridVertex {
        origin: hexx::Hex::new(0, 0),
        direction: to.vertex_ccw(),
    });
    const GAP_RATIO: f32 = 1.00;
    let aa = hexx::Vec2::new((a1.x + a2.x) / 2.0, (a1.y + a2.y) / 2.0) * GAP_RATIO;
    let bb = hexx::Vec2::new((b1.x + b2.x) / 2.0, (b1.y + b2.y) / 2.0) * GAP_RATIO;

    CurveCoords { from: aa, to: bb }
}

pub fn get_hex_midstart_coords(
    layout: &hexx::HexLayout,
    to: hexx::EdgeDirection,
) -> CurveCoords {
    let b1 = layout.vertex_coordinates(hexx::GridVertex {
        origin: hexx::Hex::new(0, 0),
        direction: to.vertex_cw(),
    });
    let b2 = layout.vertex_coordinates(hexx::GridVertex {
        origin: hexx::Hex::new(0, 0),
        direction: to.vertex_ccw(),
    });
    const GAP_RATIO: f32 = 1.00;
    let bb = hexx::Vec2::new((b1.x + b2.x) / 2.0, (b1.y + b2.y) / 2.0) * GAP_RATIO;

    CurveCoords {
        from: Vec2::new(0.0, 5.0),
        to: bb,
    }
}

fn build_curve_path(from: Point, to: Point) -> lyon::path::Path {
    let mut builder = path::Path::builder();
    builder.begin(from);
    builder.line_to(from / 2.0);
    builder.quadratic_bezier_to(point(0.0, 0.0), to / 2.0);
    builder.line_to(to);
    builder.end(false);
    builder.build()
}

struct Vertex {
    position: Point,
    advancement: f32,
    side: f32,
}

fn tessellate_curve(path: &lyon::path::Path, width: f32) -> VertexBuffers<Vertex, u32> {
    let mut buffer = VertexBuffers::new();
    let mut tessellator = StrokeTessellator::new();

    let options = StrokeOptions::default()
        .with_tolerance(0.1)
        .with_line_width(width);

    tessellator
        .tessellate_path(
            path,
            &options,
            &mut BuffersBuilder::new(&mut buffer, |vertex: StrokeVertex| Vertex {
                position: vertex.position(),
                advancement: vertex.advancement(),
                side: if vertex.side() == Side::Negative {
                    0.0
                } else {
                    1.0
                },
            }),
        )
        .unwrap();

    buffer
}

fn create_curve_with_path_func(
    path_func: fn(Point, Point) -> lyon::path::Path,
    from: &hexx::Vec2,
    to: &hexx::Vec2,
    width: f32,
    length: f32,
) -> Mesh {
    let path = path_func(point(from.x, from.y), point(to.x, to.y));
    create_bezier_curve_mesh(path, width, length)
}

pub fn create_bezier_curve_mesh(path: lyon::path::Path, width: f32, u_factor: f32) -> Mesh {
    let buffer = tessellate_curve(&path, width);

    use lyon::algorithms::length::approximate_length;

    let curve_length = approximate_length(path.iter(), 0.1);

    let uvs = buffer
        .vertices
        .iter()
        .map(|vertex| {
            let j = vertex.advancement / curve_length * u_factor + 2.5;
            let u = j;
            let v = vertex.side;
            [u, v]
        })
        .collect::<Vec<[f32; 2]>>();

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        buffer
            .vertices
            .iter()
            .map(|v| [v.position.x, 0.0, v.position.y])
            .collect::<Vec<[f32; 3]>>(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        buffer
            .vertices
            .iter()
            .map(|_| [0.0, 1.0, 0.0])
            .collect::<Vec<[f32; 3]>>(),
    )
    .with_inserted_indices(bevy::mesh::Indices::U32(buffer.indices))
}
