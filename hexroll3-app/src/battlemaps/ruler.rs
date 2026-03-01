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

use crate::{
    hexmap::elements::HexMapResources,
    shared::{
        labels::{AreaNumbersMarker, RulerLabelMarker},
        widgets::buttons::ToggleResourceWrapper,
    },
};
use bevy_vector_shapes::{
    prelude::ShapePainter,
    shapes::{DiscPainter, LinePainter, ShapeAlphaMode, ThicknessType},
};

#[derive(Component)]
pub struct RulerDragData {
    start: Vec3,
    stop: Vec3,
    corners: Vec<Vec3>,
}

impl RulerDragData {
    pub fn from_start_pos(start: Vec3) -> Self {
        RulerDragData {
            start,
            stop: start,
            corners: Vec::new(),
        }
    }
    pub fn move_to(&mut self, pos: Vec3) {
        self.stop = pos;
    }

    pub fn add_corner(&mut self, pos: Vec3) {
        self.corners.push(pos);
    }
}

#[derive(Component)]
pub struct RulerLabel(i32);

pub fn draw_ruler(
    mut commands: Commands,
    mut drag_data: Query<&mut RulerDragData>,
    mut painter: ShapePainter,
    labels: Query<(Entity, &Transform, &RulerLabel)>,
    area_labels: Query<Entity, With<AreaNumbersMarker>>,
    map_resources: Res<HexMapResources>,
    keyboard: Res<ButtonInput<KeyCode>>,
    ruler_mode: Res<ToggleResourceWrapper<BattlemapsRuler>>,
) {
    if ruler_mode.value == BattlemapsRuler::Off {
        return;
    }
    if labels.is_empty() && !drag_data.is_empty() {
        for e in area_labels.iter() {
            commands.entity(e).despawn();
        }
        spawn_ruler_label(&mut commands, &map_resources, 1);
        spawn_ruler_label(&mut commands, &map_resources, 2);
        spawn_ruler_label(&mut commands, &map_resources, 3);
    }

    if !labels.is_empty() && drag_data.is_empty() {
        despawn_labels(&mut commands, &labels);
    }

    for mut d in drag_data.iter_mut() {
        let (start, end) = (d.start, d.stop);
        if keyboard.just_pressed(KeyCode::KeyC) {
            d.add_corner(end);
        }

        let label_transform = calc_label_transform(start, end);

        setup_painter(&mut painter);
        draw_circle(&mut painter, start);

        let (distance_so_far, next) = draw_corners(&mut painter, start, &d.corners);

        draw_line(&mut painter, next, end);
        let distance = distance_so_far + next.xz().distance(end.xz()) * 10.0;

        update_labels(&mut commands, &labels, distance, label_transform);
    }
}

fn spawn_ruler_label(
    commands: &mut Commands,
    map_resources: &Res<HexMapResources>,
    digits: i32,
) {
    commands.spawn((
        RulerLabelMarker {
            size: 128.,
            scale: 0.003,
        },
        RulerLabel(digits),
        bevy_rich_text3d::Text3d::new("XXX ft"),
        bevy_rich_text3d::Text3dStyling {
            size: 128. / 12.0,
            font: "Eczar".into(),
            align: bevy_rich_text3d::TextAlign::Center,
            color: Srgba::new(0.2, 0.2, 0.2, 1.0),
            ..default()
        },
        Mesh3d::default(),
        MeshMaterial3d(map_resources.token_labels_material.clone()),
    ));
}

fn despawn_labels(commands: &mut Commands, labels: &Query<(Entity, &Transform, &RulerLabel)>) {
    for (e, _, _) in labels.iter() {
        commands.entity(e).despawn();
    }
}

fn calc_label_transform(start: Vec3, end: Vec3) -> Transform {
    let px = start + (end - start) / 2.0;
    Transform::default()
        .with_translation(px.with_y(10.0))
        .looking_at(Vec3::new(px.x, 0.0, px.z), Dir3::NEG_Z)
}

fn setup_painter(painter: &mut ShapePainter) {
    painter.set_3d();
    painter.color = Color::BLACK.with_alpha(0.3);
    painter.thickness = 3.0;
    painter.thickness_type = ThicknessType::Pixels;
    painter.origin = Some(Vec3::Y * 10.0);
    painter.alpha_mode = ShapeAlphaMode::Blend;
    painter.rotate_x(-std::f32::consts::PI / 2.0);
}

fn draw_circle(painter: &mut ShapePainter, position: Vec3) {
    painter.set_translation(position);
    painter.hollow = true;
    painter.circle(0.2);
}

fn draw_corners(painter: &mut ShapePainter, mut next: Vec3, corners: &[Vec3]) -> (f32, Vec3) {
    let mut distance = 0.0;
    for c in corners.iter() {
        distance += next.xz().distance(c.xz()) * 10.0;
        draw_line(painter, next, c.clone());
        next = c.clone();
        draw_circle(painter, next);
    }
    (distance, next)
}

fn draw_line(painter: &mut ShapePainter, from: Vec3, to: Vec3) {
    painter.set_translation(Vec3::ZERO);
    painter.hollow = false;
    painter.line(Vec3::new(from.x, -from.z, 2.0), Vec3::new(to.x, -to.z, 2.0));
}

fn update_labels(
    commands: &mut Commands,
    labels: &Query<(Entity, &Transform, &RulerLabel)>,
    distance: f32,
    mut tx: Transform,
) {
    for (j, lt, rl) in labels.iter() {
        tx.scale = lt.scale;
        commands.entity(j).insert(tx);

        let feet = distance.floor() as i32;
        let digits = feet.to_string().len() as i32;

        let template = format!("{} ft", feet);
        if digits == rl.0 {
            commands
                .entity(j)
                .insert(Visibility::Inherited)
                .insert(bevy_rich_text3d::Text3d::new(template));
        } else {
            commands.entity(j).insert(Visibility::Hidden);
        }
    }
}

#[derive(Component, Default, PartialEq, Clone)]
pub enum BattlemapsRuler {
    #[default]
    On,
    Off,
}
