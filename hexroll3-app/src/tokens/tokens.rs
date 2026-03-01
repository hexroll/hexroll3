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

use serde::{Deserialize, Serialize};

use bevy::prelude::*;

use bevy_editor_cam::prelude::EditorCam;

use avian3d::prelude::*;

use crate::{
    battlemaps::DUNGEON_FOG_COLOR,
    hexmap::elements::{HexMapResources, HexMapState, MainCamera, MapVisibilityController},
    shared::{
        disc::create_3d_disc, input::InputMode, layers::HEIGHT_OF_TOKENS, vtt::VttData,
        widgets::buttons::ToggleResourceWrapper,
    },
};

use super::{
    BattlemapsSnapping, SpawnTokenFromLibrary, TOKEN_MAP_PINS_CLOSEST_VISIBILITY_ZOOM,
    TOKEN_MAP_PINS_OPAQUE_VISIBILITY_ZOOM, TOKEN_MAP_PINS_SCALE,
    TOKEN_VISIBILITY_FRUSTRUM_BUFFER, TOKEN_VISIBILITY_ZOOM_THRESHOLD,
    control::TokenControlPlugin,
    initiative::*,
    library::{TokenTemplate, TokensLibraryPlugin},
    spawn::TokenSpawnPlugin,
    sync::on_token_message,
};

pub struct Tokens;
impl Plugin for Tokens {
    fn build(&self, app: &mut App) {
        app.insert_resource(SubstepCount(24))
            .insert_resource(ToggleResourceWrapper {
                value: BattlemapsSnapping::default(),
            })
            .add_plugins(TokenSpawnPlugin)
            .add_plugins(TokenControlPlugin)
            .add_plugins(TokenInitiativePlugin)
            .add_plugins(TokensLibraryPlugin)
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                spawn_tokens.run_if(not(in_state(HexMapState::Suspended))),
            )
            .add_systems(Update, set_tokens_visibility_by_camera_viewport)
            .add_systems(
                Update,
                clear_camera_state.run_if(not(in_state(HexMapState::Suspended))),
            )
            .add_observer(on_token_message);
    }
}

#[derive(Component, Clone, Serialize, Deserialize, Debug)]
pub struct Token {
    pub token_id: u32,
    pub token_name: String,
    pub glb_file: String,
    pub color: Color,
    pub light_color: Color,
    pub label: String,
    pub light: f32,
    pub mobility: TokenMobility,
}

#[derive(Component)]
pub struct TokenIsLocked;

#[derive(Component, Clone, Serialize, Deserialize, Debug)]
pub enum TokenMobility {
    Unrestricted(bool),
    RefereeOnly(bool),
}

impl TokenMobility {
    pub fn is_player_controllable(&self) -> bool {
        match self {
            TokenMobility::Unrestricted(_) => true,
            TokenMobility::RefereeOnly(_) => false,
        }
    }
    pub fn is_locked_now(&self) -> bool {
        match self {
            TokenMobility::Unrestricted(locked) => *locked,
            TokenMobility::RefereeOnly(locked) => *locked,
        }
    }
}

pub trait TokenMobilityActuator {
    fn apply_mobility_on_token(
        &mut self,
        mobility: &TokenMobility,
        vtt_data: &VttData,
    ) -> &mut Self;
}

impl TokenMobilityActuator for EntityCommands<'_> {
    fn apply_mobility_on_token(
        &mut self,
        mobility: &TokenMobility,
        vtt_data: &VttData,
    ) -> &mut Self {
        if mobility.is_locked_now()
            || (!mobility.is_player_controllable() && vtt_data.is_player())
        {
            self.try_insert(TokenIsLocked);
        } else {
            self.try_remove::<TokenIsLocked>();
        }
        self
    }
}

impl Token {
    pub fn duplicate(&self) -> Self {
        Self {
            token_id: rand::random::<u32>(),
            token_name: self.token_name.clone(),
            glb_file: self.glb_file.clone(),
            color: self.color,
            light_color: self.light_color,
            label: self.label.clone(),
            light: self.light,
            mobility: self.mobility.clone(),
        }
    }
    pub fn from_template(token_template: &TokenTemplate) -> Self {
        let token_name = token_template.token_name[..1].to_ascii_uppercase()
            + &token_template.token_name[1..];
        Token {
            token_name,
            glb_file: format!("tokens/{}.glb", token_template.token_name),
            token_id: rand::random::<u32>(),
            light: token_template.light_radius,
            color: DUNGEON_FOG_COLOR,
            light_color: LinearRgba::new(0.6, 0.6, 0.6, 1.0).into(),
            label: String::new(),
            mobility: if token_template.player_enabled {
                TokenMobility::Unrestricted(false)
            } else {
                TokenMobility::RefereeOnly(false)
            },
        }
    }
}

#[derive(Component)]
pub struct ActiveToken;

#[derive(Component)]
pub struct Torch;

#[derive(Component)]
pub struct TokenMarker;

#[derive(Resource)]
pub struct TokensAssets {
    pub base_plate: Handle<Mesh>,
    pub torch_bubble: Handle<Mesh>,
}

fn clear_camera_state(
    mut panorbit: Single<&mut EditorCam>,
    targets: Query<Entity, With<ActiveToken>>,
) {
    if targets.is_empty() {
        panorbit.enabled_motion.pan = true;
        panorbit.enabled_motion.zoom = true;
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(TokensAssets {
        base_plate: meshes.add(create_3d_disc(0.25, 0.0, 10)),
        torch_bubble: meshes.add(Sphere::new(1.0)),
    });
}

fn spawn_tokens(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    input_mode: Res<InputMode>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    if let Ok(window) = windows.single() {
        if let Ok((camera, cam_transform)) = cameras.single() {
            let Some(ray) = window
                .cursor_position()
                .and_then(|p| camera.viewport_to_world(cam_transform, p).ok())
            else {
                return;
            };
            let Some(distance) =
                ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Dir3::Y))
            else {
                return;
            };
            let point = ray.origin + ray.direction * distance;
            if keyboard.just_pressed(KeyCode::KeyT)
                && !keyboard.pressed(KeyCode::ControlLeft)
                && input_mode.keyboard_available()
            {
                commands.trigger(SpawnTokenFromLibrary {
                    pos: Vec3::new(point.x, HEIGHT_OF_TOKENS, point.z),
                });
            }
        }
    }
}

fn set_tokens_visibility_by_camera_viewport(
    mut items: Query<
        (&mut Visibility, &GlobalTransform, &Transform),
        (With<Token>, Without<TokenMarker>),
    >,
    visibility: Res<MapVisibilityController>,
    mut markers: Query<(&mut Visibility, &mut Transform, &ChildOf), With<TokenMarker>>,
    map_resources: Res<HexMapResources>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Prevent token lights from turning off when slightly out of bounds
    for (mut v, gt, _) in items.iter_mut() {
        let token_should_be_visible_given_zoom_and_frustrum = {
            visibility
                .rect
                .inflate(TOKEN_VISIBILITY_FRUSTRUM_BUFFER)
                .contains(gt.translation().xz())
                && visibility.scale < TOKEN_VISIBILITY_ZOOM_THRESHOLD
        };
        if token_should_be_visible_given_zoom_and_frustrum {
            *v = Visibility::Inherited;
        } else {
            *v = Visibility::Hidden;
        }
    }

    if let Some(mat) = materials.get_mut(&map_resources.pins_material) {
        let token_pins_alpha = if visibility.scale < TOKEN_MAP_PINS_OPAQUE_VISIBILITY_ZOOM
            && visibility.scale > TOKEN_MAP_PINS_CLOSEST_VISIBILITY_ZOOM
        {
            (visibility.scale - TOKEN_MAP_PINS_CLOSEST_VISIBILITY_ZOOM)
                / (TOKEN_MAP_PINS_OPAQUE_VISIBILITY_ZOOM
                    - TOKEN_MAP_PINS_CLOSEST_VISIBILITY_ZOOM)
        } else {
            1.0
        };
        mat.base_color.set_alpha(token_pins_alpha);
    }

    for (mut v, mut marker_transform, child_of) in markers.iter_mut() {
        if let Ok((_, _, parent_transform)) = items.get(child_of.0) {
            let pin_scale_neutral_to_zoom_level =
                Vec3::splat(visibility.scale) * 0.5 * TOKEN_MAP_PINS_SCALE;
            let pin_scale_also_neutral_to_token_scale =
                pin_scale_neutral_to_zoom_level * (1.0 / parent_transform.scale.x);
            marker_transform.scale = pin_scale_also_neutral_to_token_scale;
            marker_transform.rotation = parent_transform.rotation.inverse();
            *marker_transform =
                marker_transform.looking_at(Vec3::new(0.0, 0.0, 0.0), Dir3::NEG_Z);
            marker_transform.rotate_y(-parent_transform.rotation.to_euler(EulerRot::YXZ).0);
        }
        if visibility.scale > TOKEN_MAP_PINS_CLOSEST_VISIBILITY_ZOOM {
            *v = Visibility::Visible;
        } else {
            *v = Visibility::Hidden;
        }
    }
}
