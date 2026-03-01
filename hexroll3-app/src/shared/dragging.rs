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

// Detects camera drag or intent to drag.
//
// This is primarly used to inhibit hex selection when the user
// is intending to, or currently dragging the camera.
use bevy::prelude::*;

use crate::hexmap::elements::HexMapToolState;

pub struct DraggingDetectorPlugin;
impl Plugin for DraggingDetectorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DraggingMotionDetector::Pending)
            .add_systems(Update, reset_dragging_detector)
            .add_systems(
                Update,
                update_dragging_detector.before(reset_dragging_detector),
            );
    }
}

#[derive(Resource, Default, PartialEq, Clone)]
pub enum DraggingMotionDetector {
    #[default]
    Pending,
    PotentialMovement(Option<Vec2>),
    MovementRecorded,
}

impl DraggingMotionDetector {
    pub fn motion_detected(&self) -> bool {
        *self == DraggingMotionDetector::MovementRecorded
    }

    pub fn set_detected(&mut self) {
        *self = DraggingMotionDetector::MovementRecorded
    }
}

pub fn reset_dragging_detector(
    click: Res<ButtonInput<MouseButton>>,
    mut camera_motion_state: ResMut<DraggingMotionDetector>,
) {
    if click.just_released(MouseButton::Left) {
        *camera_motion_state = DraggingMotionDetector::Pending;
    }
}

pub fn update_dragging_detector(
    mut camera_motion_state: ResMut<DraggingMotionDetector>,
    click: Res<ButtonInput<MouseButton>>,
    map_state: Res<State<HexMapToolState>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    if let Ok(window) = windows.single() {
        if click.just_pressed(MouseButton::Left) {
            *camera_motion_state =
                DraggingMotionDetector::PotentialMovement(window.cursor_position());
        }
        match *camera_motion_state {
            DraggingMotionDetector::PotentialMovement(maybe_anchor) => {
                if let Some(previous_anchor) = maybe_anchor {
                    if let Some(pos) = window.cursor_position() {
                        if pos != previous_anchor {
                            *camera_motion_state = DraggingMotionDetector::MovementRecorded
                        }
                    } else {
                        *camera_motion_state = DraggingMotionDetector::MovementRecorded
                    }
                }
            }
            _ => (),
        }
        if *map_state == HexMapToolState::DialMenu {
            *camera_motion_state = DraggingMotionDetector::MovementRecorded;
        }
    }
}
