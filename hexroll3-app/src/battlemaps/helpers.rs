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

pub fn remove_points_outside_of_hex(
    line: Vec<lyon::math::Point>,
    hex_size: f32,
    offset: f32,
) -> Vec<lyon::math::Point> {
    let mut line = line.clone();
    line.retain(|point| {
        sd_hexagon(
            bevy::math::Vec2::new(point.x / hex_size, (point.y - offset) / hex_size),
            1.0,
        ) < 0.0
    });
    line
}

pub fn is_point_inside_hex(point: &[f64], hex_size: f32, offset: f32) -> bool {
    sd_hexagon(
        bevy::math::Vec2::new(
            point[0] as f32 / hex_size,
            (point[1] as f32 - offset) / hex_size,
        ),
        1.0,
    ) < 0.0
}

pub fn ensure_min_distance(
    points: Vec<lyon::math::Point>,
    min_distance: f32,
) -> Vec<lyon::math::Point> {
    let mut filtered_points = Vec::new();

    if points.is_empty() {
        return filtered_points;
    }

    filtered_points.push(points[0]);

    for i in 1..points.len() - 1 {
        let last_point = filtered_points.last().unwrap();
        let current_point = &points[i];

        let distance = last_point.distance_to(*current_point);
        if distance >= min_distance {
            filtered_points.push(*current_point);
        }
    }

    filtered_points.push(points[points.len() - 1]);
    filtered_points
}

pub fn extend_line_to_endpoints_by_radius_and_angles(
    mut line: Vec<lyon::math::Point>,
    is_estuary: bool,
) -> Vec<lyon::math::Point> {
    line.retain(|point| {
        point.x >= -600.0 && point.x <= 600.0 && point.y >= -600.0 && point.y <= 600.0
    });
    if line.is_empty() {
        return line;
    }
    let mut extended_line = line.clone();
    {
        let last_point = line.last().unwrap();
        let almost_last_point = line[(line.len() * 3) / 4];

        let direction = *last_point - almost_last_point;
        let unit_direction = direction / direction.length();
        let mut distance = 100.0;

        while distance <= 900.0 {
            let after_last_point = *last_point + unit_direction * distance;
            distance += 5.0;
            extended_line.push(after_last_point);
        }
    }

    if line.len() > 1 {
        let first_point = line.first().unwrap();
        let almost_first_point = line[1];
        let direction = *first_point - almost_first_point;
        let unit_direction = direction / direction.length();
        let mut distance = 100.0;

        while distance <= if is_estuary { 200.0 } else { 900.0 } {
            let after_first_point = *first_point + unit_direction * distance;
            distance += 5.0;
            extended_line.insert(0, after_first_point);
        }
    }

    extended_line
}

fn sd_hexagon(p: bevy::math::Vec2, r: f32) -> f32 {
    let k = bevy::math::Vec3::new(-0.866025404, 0.5, 0.577350269);
    let mut z = p.abs();
    z -= 2.0 * f32::min(k.xy().dot(z), 0.0) * k.xy();
    z -= bevy::math::Vec2::new(f32::clamp(z.x, -k.z * r, k.z * r), r);
    return z.length() * z.y.signum();
}

pub struct RectInSpace {
    pub center: Vec2,
    pub dimensions: Vec2,
    pub orientation: f32,
}

pub fn get_width_height_rotation_from_rect_points(points: Vec<[f64; 2]>) -> RectInSpace {
    if points.len() != 4 {
        panic!("Exactly 4 points are required to define a rectangle.");
    }

    let (x1, y1) = (points[0][0], points[0][1]);
    let (x2, y2) = (points[1][0], points[1][1]);
    let (x3, y3) = (points[2][0], points[2][1]);
    let (x4, y4) = (points[3][0], points[3][1]);

    let center = Vec2::new(
        (x1 + x2 + x3 + x4) as f32 / 4.0,
        (y1 + y2 + y3 + y4) as f32 / 4.0,
    );

    let dimensions = Vec2::new(
        ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt() as f32,
        ((x1 - x4).powi(2) + (y1 - y4).powi(2)).sqrt() as f32,
    );

    let rotation = (y2 - y1) / (x2 - x1);
    let orientation = rotation.atan() as f32 * -1.0;

    RectInSpace {
        center,
        dimensions,
        orientation,
    }
}
