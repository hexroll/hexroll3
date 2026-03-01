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

// A very basic SVG to lyon::path::Path convertor
use std::str::SplitWhitespace;

extern crate lyon;
use lyon::math::*;
use lyon::path::Path;

pub fn svg_to_path(svg_data: &str) -> Path {
    let path_commands = parse_svg_path(svg_data);

    let mut builder = Path::builder();

    let mut current_pos = point(0.0, 0.0);

    for command in path_commands {
        match command {
            PathCommand::MoveTo(dx, dy) => {
                current_pos = point(current_pos.x + dx, current_pos.y + dy);
                builder.begin(current_pos);
            }
            PathCommand::CubicTo(dx1, dy1, dx2, dy2, dx, dy) => {
                let control1 = point(current_pos.x + dx1, current_pos.y + dy1);
                let control2 = point(current_pos.x + dx2, current_pos.y + dy2);
                current_pos = point(current_pos.x + dx, current_pos.y + dy);
                builder.cubic_bezier_to(control1, control2, current_pos);
            }
            PathCommand::LineTo(dx, dy) => {
                current_pos = point(current_pos.x + dx, current_pos.y + dy);
                builder.line_to(current_pos);
            }
            PathCommand::HorizontalLineTo(dx) => {
                current_pos = point(current_pos.x + dx, current_pos.y);
                builder.line_to(current_pos);
            }
            PathCommand::VerticalLineTo(dy) => {
                current_pos = point(current_pos.x, current_pos.y + dy);
                builder.line_to(current_pos);
            }
            PathCommand::ClosePath => {
                builder.close();
            }
        }
    }

    let path = builder.build();
    path
}

#[derive(Debug)]
enum PathCommand {
    MoveTo(f32, f32),
    CubicTo(f32, f32, f32, f32, f32, f32),
    LineTo(f32, f32),
    HorizontalLineTo(f32),
    VerticalLineTo(f32),
    ClosePath,
}

fn parse_svg_path(data: &str) -> Vec<PathCommand> {
    let mut commands = Vec::new();
    let mut tokens_iter = data.split_whitespace();
    let factor = 0.01;

    while let Some(command) = tokens_iter.next() {
        match command {
            "m" => {
                let x = parse_value(&mut tokens_iter) * factor;
                let y = parse_value(&mut tokens_iter) * factor;
                commands.push(PathCommand::MoveTo(x, y));
            }
            "c" => {
                let x1 = parse_value(&mut tokens_iter) * factor;
                let y1 = parse_value(&mut tokens_iter) * factor;
                let x2 = parse_value(&mut tokens_iter) * factor;
                let y2 = parse_value(&mut tokens_iter) * factor;
                let x = parse_value(&mut tokens_iter) * factor;
                let y = parse_value(&mut tokens_iter) * factor;
                commands.push(PathCommand::CubicTo(x1, y1, x2, y2, x, y));
            }
            "l" => {
                let x = parse_value(&mut tokens_iter) * factor;
                let y = parse_value(&mut tokens_iter) * factor;
                commands.push(PathCommand::LineTo(x, y));
            }
            "h" => {
                let dx: f32 = parse_value(&mut tokens_iter) * factor;
                commands.push(PathCommand::HorizontalLineTo(dx));
            }
            "v" => {
                let dy: f32 = parse_value(&mut tokens_iter) * factor;
                commands.push(PathCommand::VerticalLineTo(dy));
            }
            "z" => {
                commands.push(PathCommand::ClosePath);
            }
            _ => (),
        }
    }
    commands
}

fn parse_value(tokens_iter: &mut SplitWhitespace<'_>) -> f32 {
    tokens_iter
        .next()
        .expect("Expected more values in SVG path data")
        .parse()
        .expect("Value is not a float")
}
