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

// Hex map entities selection
//
use bevy::prelude::*;
use hexx::Hex;

use crate::{
    clients::model::FetchEntityReason,
    hexmap::{
        elements::{
            AppendSandboxEntity, FetchEntityFromStorage, HexCoordsForFeature, HexMapData,
            HexMapState, HexMask, MainCamera, MapVisibilityController, SelectionEntity,
        },
        revealing::reveal_hex_or_ocean,
    },
    shared::{
        dragging::{DraggingMotionDetector, reset_dragging_detector},
        settings::UserSettings,
        vtt::{HexMapMode, StoreVttState, VttData},
        widgets::buttons::ToggleResourceWrapper,
    },
    tokens::SelectedToken,
};

use bevy_inspector_egui::bevy_egui::input::egui_wants_any_input;

use super::elements::{
    HexEntity, HexMapToolState, HexRevealPattern, y_inverted_hexmap_layout,
};

pub struct SelectingPlugin;

impl Plugin for SelectingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            track_hex_under_cursor.run_if(in_state(HexMapState::Active)),
        )
        .add_systems(
            Update,
            detect_click
                .before(reset_dragging_detector)
                .run_if(not(egui_wants_any_input))
                .run_if(in_state(HexMapState::Active))
                .run_if(
                    in_state(HexMapToolState::Selection), //.or(in_state(HexMapToolState::Edit)),
                ),
        )
        .add_systems(
            Update,
            detect_selected_hex
                .before(detect_click)
                .run_if(not(egui_wants_any_input))
                .run_if(in_state(HexMapState::Active))
                .run_if(
                    in_state(HexMapToolState::Selection).or(in_state(HexMapToolState::Edit)),
                ),
        )
        .insert_resource(ToggleResourceWrapper {
            value: HexRevealPattern::default(),
        });
    }
}

pub enum SelectionRange {
    Realm,
    Region,
    Hex,
    Feature,
}

impl MapVisibilityController {
    pub fn selection_range_mode(&self) -> SelectionRange {
        if self.scale > 20.0 {
            SelectionRange::Realm
        } else if self.scale > 10.0 {
            SelectionRange::Region
        } else if self.scale > 0.5 {
            SelectionRange::Hex
        } else {
            SelectionRange::Feature
        }
    }
}

pub fn track_hex_under_cursor(
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut map: ResMut<HexMapData>,
) {
    if let Ok(window) = windows.single() {
        if let Ok((camera, cam_transform)) = cameras.single() {
            let Some(ray) = window.cursor_position().and_then(|p| {
                if let Some(view) = camera.viewport.as_ref() {
                    if p.x < view.physical_position.x as f32
                        || p.y < view.physical_position.y as f32
                        || p.y > (view.physical_position.y + view.physical_size.y) as f32
                    {
                        map.cursor = None;
                        return None;
                    }
                }
                camera.viewport_to_world(cam_transform, p).ok()
            }) else {
                return;
            };
            let Some(distance) =
                ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Dir3::Y))
            else {
                return;
            };
            let point = ray.origin + ray.direction * distance;
            map.cursor = Some(point);
        }
    }
}

pub fn detect_click(
    mut commands: Commands,
    mut map: ResMut<HexMapData>,
    mut vtt_map: ResMut<VttData>,
    click: Res<ButtonInput<MouseButton>>,
    masks: Query<(Entity, &mut HexMask)>,
    camera_motion_state: Res<DraggingMotionDetector>,
    visibility_controller: Res<MapVisibilityController>,
    features: Query<(Entity, &HexCoordsForFeature)>,
    selected_tokens: Query<&SelectedToken>,
    hexes: Query<(Entity, &HexEntity)>,
    reveal_pattern: Res<ToggleResourceWrapper<HexRevealPattern>>,
    user_settings: Res<UserSettings>,
) {
    if camera_motion_state.motion_detected() {
        return;
    }

    if map.cursor.is_some() {
        if let Some(coord) = map.selected {
            if click.just_released(MouseButton::Left) && selected_tokens.is_empty() {
                match vtt_map.mode {
                    HexMapMode::RefereeViewing => {
                        match visibility_controller.selection_range_mode() {
                            // - - - - - - - - - - - - - - - - - - - - - - - - -
                            // Clicking a feature
                            //
                            // NOTE: This will only navigate to features that do
                            // not use a Bevy `.observe` handler, and instead
                            // use the `HexFeatureMap` enum router.
                            SelectionRange::Feature => {
                                // NOTE: The following logic was used before moving
                                // things to bevy::picking.
                                //
                                // TODO: Before removing, ensure the async observers
                                // setup is working well on dungeon and cave areas as a spawning
                                // optimization.
                                //
                                // for (_e, coords, maybe_feature_map) in features.iter() {
                                //     if coords.hex == coord
                                //         && let Some(feature_map) = maybe_feature_map
                                //         && let Some(cursor_pos) = map.cursor
                                //         && let Some(uid) =
                                //             feature_map.get_uid_in_pos(cursor_pos.xz() - pos)
                                //     {
                                //         commands.trigger(FetchEntityFromStorage {
                                //             uid,
                                //             why: FetchEntityReason::SandboxLink,
                                //         });
                                //     }
                                // }
                            }
                            // - - - - - - - - - - - - - - - - - - - - - - - - -
                            // Clicking a hex
                            _ => {
                                // - - - - - - - - - - - - - - - - - - - - - - -
                                // Clicking a pre-generated hex
                                if let Some(entity) = map.hexes.get(&coord) {
                                    if entity.uid == "<uid>" {
                                        debug!("clicking ungenerated hex");
                                        return;
                                    }
                                    commands.trigger(FetchEntityFromStorage {
                                        uid: entity.uid.clone(),
                                        anchor: None,
                                        why: FetchEntityReason::SandboxLink,
                                    });
                                // - - - - - - - - - - - - - - - - - - - - - - -
                                // Clicking a not-yet-generated ocean hex
                                } else {
                                    if let Some(sandbox_uid) = &user_settings.sandbox {
                                        commands.trigger(AppendSandboxEntity {
                                            hex_coords: Some(coord.clone()),
                                            hex_uid: sandbox_uid.clone(),
                                            attr: "ocean".to_string(),
                                            what: "OceanHex".to_string(),
                                            send_coords: true,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    HexMapMode::RefereeRevealing => {
                        reveal_hex_or_ocean(
                            &mut commands,
                            coord,
                            &mut vtt_map,
                            &mut map,
                            masks,
                            features,
                            hexes,
                            &reveal_pattern.value,
                        );

                        commands.trigger(StoreVttState);
                    }
                    _ => {}
                }
            }
        }
    }
}

pub fn detect_selected_hex(
    mut commands: Commands,
    mut map: ResMut<HexMapData>,
    mut highlighted: Local<Hex>,
    selector: Single<Entity, With<SelectionEntity>>,
    camera_motion_state: Res<DraggingMotionDetector>,
) {
    if camera_motion_state.motion_detected() {
        return;
    }
    let layout = y_inverted_hexmap_layout();

    if let Some(point) = map.cursor {
        let coord = layout.world_pos_to_hex(point.xz());
        let pos = layout.hex_to_world_pos(coord);
        let fract = layout.world_pos_to_fract_hex(point.xz());

        const THRESHOLD: f32 = 0.4;
        if !((fract.x.fract().abs() > THRESHOLD && fract.x.fract().abs() < 1.0 - THRESHOLD)
            || (fract.y.fract().abs() > THRESHOLD && fract.y.fract().abs() < 1.0 - THRESHOLD)
                && highlighted.distance_to(coord) < 1)
        {
            commands
                .entity(*selector)
                .insert(Transform::from_xyz(pos.x, 300.0, pos.y));
            map.selected = Some(coord);

            *highlighted = coord;
        }
    }
}
