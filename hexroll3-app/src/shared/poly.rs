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

// Hexroll's version of https://github.com/mapbox/polylabel
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub fn polylabel(polygon: Vec<Vec<Vec<f64>>>, precision: f64) -> Vec<f64> {
    let precision = if precision != 0.0 { precision } else { 1.0 };

    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );

    for p in &polygon[0] {
        min_x = min_x.min(p[0]);
        min_y = min_y.min(p[1]);
        max_x = max_x.max(p[0]);
        max_y = max_y.max(p[1]);
    }

    let width = max_x - min_x;
    let height = max_y - min_y;
    let cell_size = width.min(height);
    let mut h = cell_size / 2.0;

    if cell_size == 0.0 {
        return vec![min_x, min_y, 0.0];
    }

    let mut cell_queue = BinaryHeap::new();

    for x in (min_x as i64..max_x as i64)
        .map(|x| x as f64)
        .step_by(cell_size as usize)
    {
        for y in (min_y as i64..max_y as i64)
            .map(|y| y as f64)
            .step_by(cell_size as usize)
        {
            cell_queue.push(Cell::new(x + h, y + h, h, &polygon));
        }
    }

    let mut best_cell = get_centroid_cell(&polygon);
    let bbox_cell = Cell::new(min_x + width / 2.0, min_y + height / 2.0, 0.0, &polygon);
    if bbox_cell.d > best_cell.d {
        best_cell = bbox_cell;
    }

    let mut _num_probes = cell_queue.len();

    while let Some(cell) = cell_queue.pop() {
        if cell.d > best_cell.d {
            best_cell = cell.clone();
        }

        if cell.max - best_cell.d <= precision {
            continue;
        }

        h = cell.h / 2.0;
        cell_queue.push(Cell::new(cell.x - h, cell.y - h, h, &polygon));
        cell_queue.push(Cell::new(cell.x + h, cell.y - h, h, &polygon));
        cell_queue.push(Cell::new(cell.x - h, cell.y + h, h, &polygon));
        cell_queue.push(Cell::new(cell.x + h, cell.y + h, h, &polygon));
        _num_probes += 4;
    }

    vec![best_cell.x, best_cell.y, best_cell.d]
}

#[derive(PartialEq, Clone)]
struct Cell {
    x: f64,
    y: f64,
    h: f64,
    d: f64,
    max: f64,
}

impl Eq for Cell {}

impl Ord for Cell {
    fn cmp(&self, other: &Self) -> Ordering {
        other.max.partial_cmp(&self.max).unwrap()
    }
}

impl PartialOrd for Cell {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Cell {
    fn new(x: f64, y: f64, h: f64, polygon: &Vec<Vec<Vec<f64>>>) -> Self {
        let d = point_to_polygon_dist(x, y, polygon);
        Cell {
            x,
            y,
            h,
            d,
            max: d + h * 2f64.sqrt(),
        }
    }
}

fn point_to_polygon_dist(x: f64, y: f64, polygon: &Vec<Vec<Vec<f64>>>) -> f64 {
    let mut inside = false;
    let mut min_dist_sq = f64::INFINITY;

    for ring in polygon {
        let len = ring.len();
        for (i, a) in ring.iter().enumerate() {
            let b = &ring[(i + len - 1) % len];
            if (a[1] > y) != (b[1] > y)
                && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0]
            {
                inside = !inside;
            }
            min_dist_sq = min_dist_sq.min(get_seg_dist_sq(x, y, a, b));
        }
    }

    if min_dist_sq == 0.0 {
        0.0
    } else {
        (if inside { 1.0 } else { -1.0 }) * min_dist_sq.sqrt()
    }
}

fn get_seg_dist_sq(px: f64, py: f64, a: &[f64], b: &[f64]) -> f64 {
    let (x, y, dx, dy) = (a[0], a[1], b[0] - a[0], b[1] - a[1]);
    let t = if dx != 0.0 || dy != 0.0 {
        ((px - x) * dx + (py - y) * dy) / (dx * dx + dy * dy)
    } else {
        0.0
    };
    let (x, y) = if t > 1.0 {
        (b[0], b[1])
    } else if t > 0.0 {
        (x + dx * t, y + dy * t)
    } else {
        (x, y)
    };
    let dx = px - x;
    let dy = py - y;
    dx * dx + dy * dy
}

fn get_centroid_cell(polygon: &Vec<Vec<Vec<f64>>>) -> Cell {
    let points = &polygon[0];
    let mut area = 0.0;
    let (mut x, mut y) = (0.0, 0.0);

    for (i, a) in points.iter().enumerate() {
        let b = &points[(i + points.len() - 1) % points.len()];
        let f = a[0] * b[1] - b[0] * a[1];
        x += (a[0] + b[0]) * f;
        y += (a[1] + b[1]) * f;
        area += f * 3.0;
    }

    if area == 0.0 {
        Cell::new(points[0][0], points[0][1], 0.0, polygon)
    } else {
        Cell::new(x / area, y / area, 0.0, polygon)
    }
}
