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

use std::time::Duration;

use crate::{
    hexmap::elements::MainCamera,
    shared::{
        AppState,
        tweens::{CameraViewportLens, UiNodeLens, UiNodeSizePos},
    },
};

use bevy::{camera::Viewport, prelude::*, window::PrimaryWindow};
use bevy_editor_cam::prelude::EditorCam;

use super::{
    ContentMode, PAGE_HEIGHT_PORTRAIT, PAGE_WIDTH_LANDSCAPE,
    page::{ContentCamera, ContentPage},
};

pub struct ViewportControllerPlugin;
impl Plugin for ViewportControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Live), setup)
            .add_systems(Update, set_camera_viewports)
            .add_systems(
                Update,
                detect_esc_from_split.run_if(in_state(ContentMode::SplitScreen)),
            )
            .add_systems(OnEnter(ContentMode::SplitScreen), on_split_screen)
            .add_systems(OnExit(ContentMode::SplitScreen), on_full_map);
    }
}

fn detect_esc_from_split(
    keyboard: Res<ButtonInput<KeyCode>>,
    camera: Single<&EditorCam>,
    mut next_content_mode: ResMut<NextState<ContentMode>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_content_mode.set(ContentMode::MapOnly);
    }
    match &camera.current_motion {
        bevy_editor_cam::prelude::motion::CurrentMotion::UserControlled {
            anchor: _,
            motion_inputs,
        } => match motion_inputs {
            bevy_editor_cam::prelude::inputs::MotionInputs::Zoom { zoom_inputs: _ } => {
                next_content_mode.set(ContentMode::MapOnly);
            }
            _ => {}
        },
        _ => {}
    };
}

fn get_split_map_viewport(window_size: UVec2) -> (UVec2, UVec2) {
    let is_portrait = window_size.y > window_size.x;
    if is_portrait {
        let pos = (window_size.as_vec2() * Vec2::new(0.0, 0.00)).as_uvec2();
        let size =
            (window_size.as_vec2() * Vec2::new(1.0, 1.0 - PAGE_HEIGHT_PORTRAIT)).as_uvec2();
        (pos, size)
    } else {
        let pos = (window_size.as_vec2() * Vec2::new(PAGE_WIDTH_LANDSCAPE, 0.0)).as_uvec2();
        let size =
            (window_size.as_vec2() * Vec2::new(1.0 - PAGE_WIDTH_LANDSCAPE, 1.0)).as_uvec2();
        (pos, size)
    }
}

fn get_split_content_viewport(window_size: UVec2) -> (UVec2, UVec2, UVec2, UVec2) {
    let is_portrait = window_size.y > window_size.x;
    if is_portrait {
        let start_pos = (window_size.as_vec2() * Vec2::new(0.0, 1.0)).as_uvec2();
        let start_size = (window_size.as_vec2() * Vec2::new(1.0, 0.0)).as_uvec2();
        let end_pos =
            (window_size.as_vec2() * Vec2::new(0.0, 1.0 - PAGE_HEIGHT_PORTRAIT)).as_uvec2();
        let end_size =
            (window_size.as_vec2() * Vec2::new(1.0, PAGE_HEIGHT_PORTRAIT)).as_uvec2();
        (start_pos, start_size, end_pos, end_size)
    } else {
        let start_pos = (window_size.as_vec2() * Vec2::new(0.0, 0.0)).as_uvec2();
        let start_size = (window_size.as_vec2() * Vec2::new(0.0, 1.0)).as_uvec2();
        let end_pos = (window_size.as_vec2() * Vec2::new(0.0, 0.0)).as_uvec2();
        let end_size =
            (window_size.as_vec2() * Vec2::new(PAGE_WIDTH_LANDSCAPE, 1.0)).as_uvec2();
        (start_pos, start_size, end_pos, end_size)
    }
}

pub fn get_split_content_metrics(window_size: UVec2) -> (Vec2, Vec2, Vec2, Vec2) {
    let is_portrait = window_size.y > window_size.x;
    if is_portrait {
        let start_pos = window_size.as_vec2() * Vec2::new(0.0, 1.00);
        let start_size = window_size.as_vec2() * Vec2::new(1.0, PAGE_HEIGHT_PORTRAIT);
        let end_pos = window_size.as_vec2() * Vec2::new(0.0, 0.0);
        let end_size = window_size.as_vec2() * Vec2::new(1.0, PAGE_HEIGHT_PORTRAIT);
        (start_pos, start_size, end_pos, end_size)
    } else {
        let start_pos = window_size.as_vec2() * Vec2::new(-PAGE_WIDTH_LANDSCAPE, 0.0);
        let start_size = window_size.as_vec2() * Vec2::new(PAGE_WIDTH_LANDSCAPE, 1.0);
        let end_pos = window_size.as_vec2() * Vec2::new(0.0, 0.0);
        let end_size = window_size.as_vec2() * Vec2::new(PAGE_WIDTH_LANDSCAPE, 1.0);
        (start_pos, start_size, end_pos, end_size)
    }
}

fn setup(
    window: Single<&Window, With<PrimaryWindow>>,
    mut main_camera: Single<&mut Camera, With<MainCamera>>,
) {
    let window_size = window.physical_size();
    let pos = UVec2::ZERO;
    let size = window_size;
    main_camera.viewport = Some(Viewport {
        physical_position: pos,
        physical_size: size,
        ..default()
    });
}

fn set_camera_viewports(
    windows: Query<&Window>,
    mut resize_events: MessageReader<bevy::window::WindowResized>,
    mut main_camera: Single<
        &mut Camera,
        (
            With<MainCamera>,
            Without<ContentCamera>,
            Without<ContentPage>,
        ),
    >,
    mut content_camera: Single<
        &mut Camera,
        (
            With<ContentCamera>,
            Without<MainCamera>,
            Without<ContentPage>,
        ),
    >,
    mut content_page: Single<
        &mut Node,
        (
            With<ContentPage>,
            Without<ContentCamera>,
            Without<MainCamera>,
        ),
    >,
    content_mode: Res<State<ContentMode>>,
) {
    for resize_event in resize_events.read() {
        let window = windows.get(resize_event.window).unwrap();
        let window_size = window.physical_size();
        if window_size.x < 128 || window_size.y < 128 {
            return;
        }
        if *content_mode == ContentMode::SplitScreen {
            let (main_pos, main_size) = get_split_map_viewport(window_size);
            let (_, _, content_viewport_pos, content_viewport_size) =
                get_split_content_viewport(window_size);
            let (_, _, content_page_pos, content_page_size) =
                get_split_content_metrics(window_size);
            main_camera.viewport = Some(Viewport {
                physical_position: main_pos,
                physical_size: main_size,
                ..default()
            });
            content_camera.viewport = Some(Viewport {
                physical_position: content_viewport_pos,
                physical_size: content_viewport_size,
                ..default()
            });
            content_page.left = Val::Px(content_page_pos.x);
            content_page.top = Val::Px(content_page_pos.y);
            content_page.width = Val::Px(content_page_size.x);
            content_page.height = Val::Px(content_page_size.y);
        } else {
            content_camera.is_active = false;
            let pos = UVec2::ZERO;
            let size = window_size;
            main_camera.viewport = Some(Viewport {
                physical_position: pos,
                physical_size: size,
                ..default()
            });
        }
    }
}

fn on_full_map(
    window: Single<&Window>,
    mut commands: Commands,
    mut resizables: ParamSet<(
        Single<(Entity, &mut Camera), With<MainCamera>>,
        Single<(Entity, &mut Camera), With<ContentCamera>>,
        Single<Entity, With<ContentPage>>,
    )>,
) {
    let window_size = window.physical_size();
    let (start_pos_node, start_size_node, end_pos_node, end_size_node) =
        get_split_content_metrics(window_size);
    let (start_pos_viewport, start_size_viewport, end_pos_viewport, end_size_viewport) =
        get_split_content_viewport(window_size);
    let (tween_map_viewport, tween_page_viewport, tween_page_node) = {
        let pos = UVec2::ZERO;
        let size = window_size;
        (
            bevy_tweening::Tween::new(
                EaseFunction::QuarticOut,
                Duration::from_millis(200),
                CameraViewportLens {
                    size_start: resizables.p0().1.viewport.as_ref().unwrap().physical_size,
                    size_end: size,
                    pos_start: resizables
                        .p0()
                        .1
                        .viewport
                        .as_ref()
                        .unwrap()
                        .physical_position,
                    pos_end: pos,
                },
            ),
            bevy_tweening::Tween::new(
                EaseFunction::QuarticOut,
                Duration::from_millis(200),
                CameraViewportLens {
                    size_start: end_size_viewport,
                    size_end: start_size_viewport,
                    pos_start: end_pos_viewport,
                    pos_end: start_pos_viewport,
                },
            ),
            bevy_tweening::Tween::new(
                EaseFunction::QuarticOut,
                Duration::from_millis(200),
                UiNodeLens {
                    start: UiNodeSizePos {
                        left: end_pos_node.x,
                        top: end_pos_node.y,
                        width: end_size_node.x,
                        height: end_size_node.y,
                    },
                    end: UiNodeSizePos {
                        left: start_pos_node.x,
                        top: start_pos_node.y,
                        width: start_size_node.x,
                        height: start_size_node.y,
                    },
                },
            ),
        )
    };

    commands
        .entity(resizables.p0().0)
        .insert(bevy_tweening::Animator::new(tween_map_viewport));
    commands
        .entity(resizables.p1().0)
        .insert(bevy_tweening::Animator::new(tween_page_viewport));
    commands
        .entity(*resizables.p2())
        .insert(bevy_tweening::Animator::new(tween_page_node));
}

fn on_split_screen(
    window: Single<&Window>,
    mut commands: Commands,
    mut resizables: ParamSet<(
        Single<(Entity, &mut Camera), With<MainCamera>>,
        Single<(Entity, &mut Camera), With<ContentCamera>>,
        Single<Entity, With<ContentPage>>,
    )>,
) {
    let window_size = window.physical_size();
    let (start_pos_node, start_size_node, end_pos_node, end_size_node) =
        get_split_content_metrics(window_size);
    let (start_pos_viewport, start_size_viewport, end_pos_viewport, end_size_viewport) =
        get_split_content_viewport(window_size);
    let (tween_map_viewport, tween_page_viewport, tween_page_node) = {
        let (pos, size) = get_split_map_viewport(window_size);
        (
            bevy_tweening::Tween::new(
                EaseFunction::QuarticOut,
                Duration::from_millis(200),
                CameraViewportLens {
                    size_start: window_size,
                    size_end: size,
                    pos_start: UVec2::ZERO,
                    pos_end: pos,
                },
            ),
            bevy_tweening::Tween::new(
                EaseFunction::QuarticOut,
                Duration::from_millis(200),
                CameraViewportLens {
                    size_start: start_size_viewport,
                    size_end: end_size_viewport,
                    pos_start: start_pos_viewport,
                    pos_end: end_pos_viewport,
                },
            ),
            bevy_tweening::Tween::new(
                EaseFunction::QuarticOut,
                Duration::from_millis(200),
                UiNodeLens {
                    start: UiNodeSizePos {
                        left: start_pos_node.x,
                        top: start_pos_node.y,
                        width: start_size_node.x,
                        height: start_size_node.y,
                    },
                    end: UiNodeSizePos {
                        left: end_pos_node.x,
                        top: end_pos_node.y,
                        width: end_size_node.x,
                        height: end_size_node.y,
                    },
                },
            ),
        )
    };

    commands
        .entity(resizables.p0().0)
        .insert(bevy_tweening::Animator::new(tween_map_viewport));
    commands
        .entity(resizables.p1().0)
        .insert(bevy_tweening::Animator::new(tween_page_viewport));
    commands
        .entity(*resizables.p2())
        .insert(bevy_tweening::Animator::new(tween_page_node));
}
