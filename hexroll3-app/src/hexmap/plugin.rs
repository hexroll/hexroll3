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

use bevy::{
    app::{App, Plugin, Startup, Update},
    prelude::AppExtStates,
};

use crate::shared::AppState;
use crate::shared::input::InputMode;
use crate::shared::vtt::PlayerPreview;
use crate::shared::widgets::buttons::ToggleEventWrapper;
use crate::shared::widgets::cursor::PointerExclusivityIsPreferred;
use crate::tokens::MainTokenEntity;
use crate::{
    battlemaps::BattlemapsPlugin,
    shared::{
        camera::CameraPlugin,
        dragging::DraggingDetectorPlugin,
        effects::EffectsPlugin,
        labels::{LabelsPlugin, MapLabel},
        vtt::{HexMapMode, VttData},
        widgets::dial::DialPlugin,
    },
};

use super::elements::{HexMapSpawnerState, VttDataState};
use super::prepare::post_map_loaded_handler_prefix;
use super::revealing::VttHexRevealer;
use super::spawn::OceanMarker;
use super::sync::on_map_message;
use super::themes::{TileSetThemes, TileSetThemesAssetLoader};
use super::{
    daynight::{DayNightPlugin, HexMapTime},
    editor::MapEditorPlugin,
    elements::{
        HexMapCache, HexMapState, HexMapToolState, HexMask, MainCamera,
        MapVisibilityController, ScaleHudMarker,
    },
    fader::hex_map_zoom_fader,
    grid::GridPlugin,
    hex_dial::HexDialPlugin,
    prepare::post_map_loaded_handler,
    selecting::SelectingPlugin,
    setup::setup,
    spawn::{control_lod_feature_visibility, update_hex_map_tiles},
    tiles::TilesPlugin,
};

pub struct HexMap;

impl Plugin for HexMap {
    fn build(&self, app: &mut App) {
        app.init_asset::<TileSetThemes>()
            .init_asset_loader::<TileSetThemesAssetLoader>()
            .insert_state(HexMapState::Active)
            .insert_state(HexMapToolState::Selection)
            .insert_state(HexMapSpawnerState::default())
            .insert_state(VttDataState::default())
            .insert_resource(HexMapCache::default())
            .insert_resource(MapVisibilityController::default())
            .insert_resource(super::spawn::TileSpawnQueues::default())
            .add_systems(
                PostUpdate,
                super::spawn::spawn_tile_from_queue.run_if(
                    in_state(HexMapSpawnerState::Enabled)
                        .and(in_state(VttDataState::Available)),
                ),
            )
            .add_systems(
                PostUpdate,
                super::spawn::despawn_tile_from_queue.run_if(
                    in_state(HexMapSpawnerState::Enabled)
                        .and(in_state(VttDataState::Available)),
                ),
            )
            .add_plugins(bevy_mod_billboard::plugin::BillboardPlugin)
            .add_plugins(TilesPlugin)
            .add_plugins(GridPlugin)
            .add_plugins(DialPlugin)
            .add_plugins(HexDialPlugin)
            .add_plugins(DayNightPlugin)
            .add_plugins(LabelsPlugin)
            .add_plugins(SelectingPlugin)
            .add_plugins(MapEditorPlugin)
            .add_plugins(BattlemapsPlugin)
            .add_plugins(CameraPlugin)
            .add_plugins(DraggingDetectorPlugin)
            .add_plugins(EffectsPlugin)
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                update_hex_map_tiles.run_if(
                    in_state(HexMapSpawnerState::Enabled)
                        .and(in_state(VttDataState::Available)),
                ),
            )
            .add_systems(Update, update_scale_hud_text)
            .add_systems(Update, update_visibility_controller)
            .add_systems(Update, map_state_enforcer)
            .add_systems(
                Update,
                react_to_vtt_hotkeys
                    .before(update_hex_map_tiles)
                    .run_if(not(in_state(HexMapState::Suspended))),
            )
            .add_systems(Update, control_lod_feature_visibility)
            .add_systems(Update, hex_map_zoom_fader.run_if(in_state(AppState::Live)))
            .add_observer(post_map_loaded_handler_prefix)
            .add_observer(post_map_loaded_handler)
            .add_observer(on_toggle_reveal_mode)
            .add_observer(on_map_message)
            .add_observer(on_player_preview_toggle);
    }
}

pub fn on_player_preview_toggle(
    trigger: On<ToggleEventWrapper<PlayerPreview>>,
    mut vtt_data: ResMut<VttData>,
    mut labels: Query<&mut Visibility, With<MapLabel>>,
    map_parent: Single<Entity, With<HexMapTime>>,
    mut commands: Commands,
    token_mesh_materials: Query<&MeshMaterial3d<StandardMaterial>, With<MainTokenEntity>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let unlit = if trigger.event().value == PlayerPreview::On {
        vtt_data.mode = HexMapMode::RefereeAsPlayer;
        commands.entity(*map_parent).despawn_related::<Children>();
        for mut v in labels.iter_mut() {
            *v = Visibility::Hidden;
        }
        false
    } else {
        commands.entity(*map_parent).despawn_related::<Children>();
        for mut v in labels.iter_mut() {
            *v = Visibility::Inherited;
        }
        vtt_data.mode = HexMapMode::RefereeViewing;
        true
    };
    for token_material in token_mesh_materials.iter() {
        if let Some(mat) = materials.get_mut(&token_material.0) {
            mat.unlit = unlit;
        }
    }
    vtt_data.invalidate_map = true;
}

pub fn react_to_vtt_hotkeys(
    mut commands: Commands,
    vtt_data: Res<VttData>,
    key: Res<ButtonInput<KeyCode>>,
    masks: Query<(Entity, &HexMask)>,
    input_mode: Res<InputMode>,
) {
    // NOTE: this is a temporary measure to ensure this part of the system
    // will not be invoked when we have no sandbox visible.
    // It will however also cause masking to fail when there's only
    // uninstantiated ocean hexes visible.
    if !masks.is_empty() && input_mode.keyboard_available() {
        if key.just_pressed(KeyCode::Space) {
            commands.trigger(ToggleEventWrapper {
                value: RefereeRevealing::On,
            });
        }
        if key.just_released(KeyCode::Space) {
            commands.trigger(ToggleEventWrapper {
                value: RefereeRevealing::Off,
            });
        }
    }
    if key.just_pressed(KeyCode::F8) {
        if vtt_data.mode == HexMapMode::RefereeViewing {
            commands.trigger(ToggleEventWrapper {
                value: PlayerPreview::On,
            });
        }
        if vtt_data.mode == HexMapMode::RefereeAsPlayer {
            commands.trigger(ToggleEventWrapper {
                value: PlayerPreview::Off,
            });
        }
    }
}

pub fn update_visibility_controller(
    mut visibility_controller: ResMut<MapVisibilityController>,
    cam: Single<(&Projection, &Transform), With<MainCamera>>,
) {
    let (proj, transform) = *cam;
    if let Projection::Orthographic(proj) = proj {
        visibility_controller.scale = proj.scale;
        visibility_controller.rect = Rect::new(
            transform.translation.x + proj.area.min.x,
            transform.translation.z + proj.area.min.y,
            transform.translation.x + proj.area.max.x,
            transform.translation.z + proj.area.max.y,
        );
    }
}

pub fn update_scale_hud_text(
    mut hud: Single<&mut Text, With<ScaleHudMarker>>,
    proj: Single<&Projection, With<MainCamera>>,
) {
    if let Projection::Orthographic(proj) = *proj {
        hud.0 = proj.scale.to_string();
    }
}

fn map_state_enforcer(
    curr_state: Res<State<HexMapState>>,
    mut next_state: ResMut<NextState<HexMapState>>,
    pointer_exclusivity_requestors: Query<&PointerExclusivityIsPreferred>,
) {
    let any_pointer_exclusivity_requestor_exists = !pointer_exclusivity_requestors.is_empty();
    match **curr_state {
        HexMapState::Active => {
            if any_pointer_exclusivity_requestor_exists {
                next_state.set(HexMapState::Suspended);
            }
        }
        HexMapState::Suspended => {
            if !any_pointer_exclusivity_requestor_exists {
                next_state.set(HexMapState::Active);
            }
        }
    }
}

fn on_toggle_reveal_mode(
    trigger: On<ToggleEventWrapper<RefereeRevealing>>,
    mut commands: Commands,
    mut masks: Query<(Entity, &HexMask)>,
    mut labels: Query<&mut Visibility, (With<MapLabel>, Without<OceanMarker>)>,
    mut ocean_hexes: Query<&mut Visibility, (Without<MapLabel>, With<OceanMarker>)>,
    mut vtt_data: ResMut<VttData>,
) {
    if vtt_data.mode == HexMapMode::Player {
        return;
    }
    if trigger.value == RefereeRevealing::On {
        vtt_data.mode = HexMapMode::RefereeRevealing;
        for (e, HexMask(hex)) in masks.iter_mut() {
            commands
                .entity(e)
                .try_insert(vtt_data.get_reveal_state_components(hex));
        }
        for mut v in labels.iter_mut() {
            *v = Visibility::Hidden;
        }
        for mut v in ocean_hexes.iter_mut() {
            *v = Visibility::Inherited;
        }
    }
    if trigger.value == RefereeRevealing::Off {
        vtt_data.mode = HexMapMode::RefereeViewing;
        for (e, HexMask(hex)) in masks.iter_mut() {
            commands
                .entity(e)
                .try_insert(vtt_data.get_reveal_state_components(hex));
            for mut v in labels.iter_mut() {
                *v = Visibility::Inherited;
            }
            for mut v in ocean_hexes.iter_mut() {
                *v = Visibility::Hidden;
            }
        }
    }
}

#[derive(Component, Default, PartialEq, Clone)]
pub enum RefereeRevealing {
    #[default]
    Off,
    On,
}
