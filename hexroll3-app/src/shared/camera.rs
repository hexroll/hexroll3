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

// Main map camera.
// Can be smoothly moved by triggering `GimbalshotCameraMovement`.
// User control is suspended and enabled using `HexMapState`.
use std::{
    ops::{DerefMut, Range},
    time::Duration,
};

use bevy::{
    anti_alias::fxaa::Fxaa, core_pipeline::tonemapping::Tonemapping, render::view::Hdr,
};
use bevy::{
    camera::visibility::RenderLayers,
    light::ShadowFilteringMethod,
    post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter},
    prelude::*,
    render::view::{ColorGrading, ColorGradingSection},
};
use bevy_editor_cam::prelude::{
    EditorCam, EnabledMotion, Sensitivity, momentum::Momentum, projections,
    smoothing::Smoothing, zoom::ZoomLimits,
};
use bevy_inspector_egui::bevy_egui::PrimaryEguiContext;
use bevy_tweening::{Tween, lens::TransformPositionLens};
use serde::{Deserialize, Serialize};

use crate::{
    hexmap::elements::{HexMapData, HexMapState, MainCamera},
    hud::ShowTransientUserMessage,
    vtt::sync::detect_camera_control,
};

use super::{
    AppState,
    layers::{
        RENDER_LAYER_MAP_LOD_HIGH, RENDER_LAYER_MAP_LOD_LOW, RENDER_LAYER_MAP_LOD_MEDIUM,
    },
    tweens::ProjectionScaleLens,
};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_editor_cam::DefaultEditorCamPlugins)
            .add_systems(OnExit(AppState::Intro), setup)
            .add_systems(OnEnter(AppState::Live), center_on_map)
            .add_systems(Update, detect_camera_control)
            .add_systems(OnEnter(HexMapState::Suspended), suspend_camera)
            .add_systems(OnExit(HexMapState::Suspended), resume_camera)
            .add_observer(tween_camera);
    }
}

#[derive(Event)]
pub struct GimbalshotCameraMovement {
    pub coords: MapCoords,
}

#[derive(Clone, Debug, Default)]
pub struct MapCoords {
    pub hex: String,
    pub x: f32,
    pub y: f32,
    pub zoom: i32,
}

impl MapCoords {
    pub fn ortho_scale_from_zoom(&self) -> f32 {
        match self.zoom {
            2 => 0.02,
            3 => 0.04,
            7 => 0.07,
            6 => 0.2,
            4 => 1.0,
            0 => 2.0,
            _ => 3.0,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CameraControl {
    pub camera_translation: Vec3,
    pub camera_scale: f32,
}

pub fn camera_callback(
    message: In<CameraControl>,
    mut commands: Commands,
    mut camera: Single<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let (t, p) = camera.deref_mut();

    t.translation = message.0.camera_translation;
    if let Projection::Orthographic(o) = p.as_mut() {
        o.scale = message.0.camera_scale;
    }

    commands.trigger(ShowTransientUserMessage {
        text: String::from("Viewpoint changed by referee"),
        special: None,
        keep_alive: None,
    });
}

#[derive(Component)]
pub struct CameraTweenTarget {
    pub target_hex_uid: Option<String>,
    clearing_system: bevy::ecs::system::SystemId,
}

fn tween_camera(
    trigger: On<GimbalshotCameraMovement>,
    mut commands: Commands,
    hex_map: Res<HexMapData>,
    mut cameras: Query<
        (Entity, &Transform, &Projection, &mut CameraTweenTarget),
        With<MainCamera>,
    >,
) {
    let hex_uid = &trigger.event().coords.hex;
    if let Some(pos) = hex_map.get_canonical_pos(
        hex_uid,
        Vec2::new(trigger.event().coords.x, trigger.event().coords.y),
    ) {
        for (camera_entity, camera_transform, camera_projection, mut camera_tween_target) in
            cameras.iter_mut()
        {
            if let Projection::Orthographic(proj) = camera_projection {
                let new_scale = trigger.event().coords.ortho_scale_from_zoom();
                if camera_transform.translation.xz().distance(pos) > 100.0
                    && trigger.event().coords.zoom != 4
                {
                    camera_tween_target.target_hex_uid = Some(hex_uid.clone());
                    let mid = (pos + camera_transform.translation.xz()) / 2.0;

                    let tween_to_mid = Tween::new(
                        EaseFunction::SineIn,
                        Duration::from_millis(1000),
                        TransformPositionLens {
                            start: camera_transform.translation,
                            end: Vec3::new(mid.x, camera_transform.translation.y, mid.y),
                        },
                    );
                    let tween_to_pos = Tween::new(
                        EaseFunction::SineOut,
                        Duration::from_secs(1),
                        TransformPositionLens {
                            start: Vec3::new(mid.x, camera_transform.translation.y, mid.y),
                            end: Vec3::new(pos.x, camera_transform.translation.y, pos.y),
                        },
                    );
                    let tween_to_overview = Tween::new(
                        EaseFunction::SineInOut,
                        Duration::from_millis(1000),
                        ProjectionScaleLens {
                            start: proj.scale,
                            end: 3.0,
                        },
                    );
                    let tween_to_zoom_level = Tween::new(
                        EaseFunction::SineInOut,
                        Duration::from_millis(1000),
                        ProjectionScaleLens {
                            start: 3.0,
                            end: new_scale,
                        },
                    )
                    .with_completed_system(camera_tween_target.clearing_system);
                    commands
                        .entity(camera_entity)
                        .insert(bevy_tweening::Animator::new(
                            tween_to_mid.then(tween_to_pos),
                        ));
                    commands
                        .entity(camera_entity)
                        .insert(bevy_tweening::Animator::new(
                            tween_to_overview.then(tween_to_zoom_level),
                        ));
                } else {
                    let tween_to_pos = Tween::new(
                        EaseFunction::QuadraticInOut,
                        Duration::from_millis(300),
                        TransformPositionLens {
                            start: camera_transform.translation,
                            end: Vec3::new(pos.x, camera_transform.translation.y, pos.y),
                        },
                    );
                    let tween_to_zoom = Tween::new(
                        EaseFunction::QuadraticInOut,
                        Duration::from_millis(300),
                        ProjectionScaleLens {
                            start: proj.scale,
                            end: new_scale,
                        },
                    );
                    commands
                        .entity(camera_entity)
                        .insert(bevy_tweening::Animator::new(tween_to_pos));
                    commands
                        .entity(camera_entity)
                        .insert(bevy_tweening::Animator::new(tween_to_zoom));
                }
            }
        }
    }
}

fn center_on_map(
    map_data: Res<HexMapData>,
    mut camera: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    for (mut ct, mut cp) in camera.iter_mut() {
        map_data.center_camera_on_map(&mut ct, &mut cp);
    }
}

fn setup(mut commands: Commands, mut ambient_light: ResMut<AmbientLight>) {
    ambient_light.brightness = 0.0;
    let cam_trans = Transform::from_xyz(0.0, 10.0, 0.0).looking_at(Vec3::ZERO, Vec3::NEG_Z);
    let clearing_system = commands.register_system(tweening_clearing_system);

    commands
        .spawn((
            Name::new("MainCamera"),
            MainCamera,
            CameraTweenTarget {
                target_hex_uid: None,
                clearing_system,
            },
            Camera3d::default(),
            Projection::from(OrthographicProjection {
                ..OrthographicProjection::default_3d()
            }),
            Msaa::default(),
            Fxaa::default(),
            Bloom {
                intensity: 0.0,
                low_frequency_boost: 0.7,
                low_frequency_boost_curvature: 0.95,
                high_pass_frequency: 0.4,
                prefilter: BloomPrefilter {
                    threshold: 0.9,
                    threshold_softness: 30.0,
                },
                composite_mode: BloomCompositeMode::Additive,
                max_mip_dimension: 4096,
                scale: Vec2::new(0.004, 0.004),
            },
            EditorCam {
                last_anchor_depth: -cam_trans.translation.length() as f64,
                smoothing: Smoothing {
                    zoom: Duration::from_millis(120),
                    pan: Duration::from_millis(120),
                    ..default()
                },
                sensitivity: Sensitivity {
                    zoom: 0.75,
                    ..default()
                },
                momentum: Momentum {
                    init_pan: Duration::from_millis(100),
                    ..default()
                },
                input_debounce: Duration::from_millis(0),
                enabled_motion: EnabledMotion {
                    pan: true,
                    orbit: false,
                    zoom: true,
                },
                zoom_limits: ZoomLimits {
                    min_size_per_pixel: 0.001,
                    max_size_per_pixel: 40.0,
                    ..Default::default()
                },
                orthographic: projections::OrthographicSettings {
                    near_clip_limits: Range {
                        start: 1.0,
                        end: 500.0,
                    },
                    scale_to_near_clip: 1000000.0,
                    far_clip_multiplier: 1.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            ShadowFilteringMethod::Gaussian,
            Camera {
                order: 0,
                clear_color: ClearColorConfig::Custom(Color::srgb_u8(0, 10, 25)),
                ..default()
            },
            RenderLayers::from_layers(&[
                RENDER_LAYER_MAP_LOD_LOW,
                RENDER_LAYER_MAP_LOD_MEDIUM,
                RENDER_LAYER_MAP_LOD_HIGH,
            ]),
            cam_trans,
            Tonemapping::None,
            ColorGrading {
                shadows: ColorGradingSection {
                    contrast: 0.98,
                    ..default()
                },
                ..default()
            },
        ))
        .insert(Hdr)
        .insert(PrimaryEguiContext)
        .with_child((
            bevy_seedling::spatial::SpatialListener3D::default(),
            Transform::from_xyz(0.0, 0.0, -500.0),
        ));

    #[cfg(feature = "dev")]
    commands.spawn((
        Name::new("ScaleHud"),
        crate::hexmap::elements::ScaleHudMarker,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            bottom: Val::Px(20.0),
            ..default()
        },
        Text::new("scale"),
    ));
}

fn suspend_camera(mut map_cam: Single<&mut EditorCam>) {
    map_cam.enabled_motion.pan = false;
    map_cam.enabled_motion.zoom = false;
}
fn resume_camera(mut map_cam: Single<&mut EditorCam>) {
    map_cam.enabled_motion.pan = true;
    map_cam.enabled_motion.zoom = true;
}

fn tweening_clearing_system(mut camera: Single<&mut CameraTweenTarget>) {
    camera.target_hex_uid = None;
}

pub trait CameraZoomRestrictor {
    fn restrict_camera_zoom(&mut self, max: f64);
    fn release_camera_zoom_restriction(&mut self);
}

impl CameraZoomRestrictor for EditorCam {
    fn restrict_camera_zoom(&mut self, max: f64) {
        self.zoom_limits = ZoomLimits {
            min_size_per_pixel: 0.001,
            max_size_per_pixel: max,
            ..Default::default()
        };
    }
    fn release_camera_zoom_restriction(&mut self) {
        self.zoom_limits = ZoomLimits {
            min_size_per_pixel: 0.001,
            max_size_per_pixel: 40.0,
            ..Default::default()
        };
    }
}
