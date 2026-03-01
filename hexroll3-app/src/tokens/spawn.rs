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

// Spawn/Despawn tokens and any related post-spawn setups
use std::time::Duration;

use bevy::{
    camera::primitives::Aabb, light::NotShadowCaster, mesh::skinning::SkinnedMesh, prelude::*,
    scene::SceneInstanceReady,
};

use avian3d::prelude::*;
use bevy_rich_text3d::{StrokeJoin, Text3d, Text3dStyling, TextAlign};

use crate::{
    battlemaps::DUNGEON_FOG_COLOR,
    hexmap::elements::{HexMapResources, MapVisibilityController},
    shared::{
        gltf::Animations,
        labels::{TokenLabel, TokenLabelOffset, spawn_token_labels},
        layers::{HEIGHT_OF_MAP_PINS, HexrollPhysicsLayer},
        settings::Config,
        spawnq::SpawnQueue,
        vtt::VttData,
        widgets::cursor::TooltipOnHover,
    },
    tokens::{
        TokenMessage, TokenUpdateMessage,
        tokens::{TokenMarker, TokenMobilityActuator},
    },
    vtt::sync::EventContext,
};

use super::{
    DespawnToken, DespawnVisibleTokens, MainTokenEntity, SelectedToken, SpawnToken, Token,
    TokenMeshEntity,
    control::TokenControls,
    tokens::{TokensAssets, Torch},
};

pub struct TokenSpawnPlugin;
impl Plugin for TokenSpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, set_tokens_visibility_by_camera_viewport)
            .add_observer(on_spawn_token)
            .add_observer(on_despawn_token)
            .add_observer(on_despawn_visible_tokens);
    }
}
#[derive(Component)]
struct TokenGltfSceneMarker;

fn on_spawn_token(
    trigger: On<EventContext<SpawnToken>>,
    mut commands: Commands,
    vtt_data: Res<VttData>,
    asset_server: Res<AssetServer>,
    tokens_assets: Res<TokensAssets>,
    map_resources: Res<HexMapResources>,
    mut queue: ResMut<SpawnQueue>,
    app_config: Res<Config>,
) {
    let token = Token {
        token_id: trigger.event.token.token_id,
        token_name: trigger.event.token.token_name.clone(),
        glb_file: trigger.event.token.glb_file.clone(),
        color: trigger.event.token.color,
        light_color: trigger.event.token.light_color,
        label: trigger.event.token.label.clone(),
        light: trigger.event.token.light,
        mobility: trigger.event.token.mobility.clone(),
    };
    let msg = TokenMessage::Update(
        TokenUpdateMessage::from_token(&token).with_transform(trigger.event.transform),
    );
    let m = map_resources.token_labels_material.clone();
    let e = commands
        .spawn((
            Mesh3d(tokens_assets.base_plate.clone()),
            Name::new(format!("Token ({})", trigger.event.token.token_name)),
            Collider::capsule(0.15, 4.0),
            RigidBody::Static,
            ColliderDisabled,
            GravityScale(0.0),
            MaxAngularSpeed(0.0),
            Restitution::new(0.0),
            Friction::new(0.0),
            ColliderDensity(100.0),
            CollidingEntities::default(),
            Mass(0.1),
            SweptCcd {
                mode: SweepMode::Linear,
                include_dynamic: true,
                linear_threshold: 0.0,
                angular_threshold: 0.0,
            },
            LockedAxes::new()
                .lock_rotation_x()
                .lock_rotation_y()
                .lock_rotation_z()
                .lock_translation_y(),
        ))
        .with_children(|c| {
            c.spawn((
                SceneRoot(
                    asset_server.load(format!("{}#Scene0", trigger.event.token.glb_file)),
                ),
                TokenGltfSceneMarker,
                Transform::from_xyz(0.0, 1.0, 0.0),
            ))
            .observe(postprocess_spawned_token);
            if !vtt_data.is_player() {
                let pin_height_entropy_to_prevent_flickering = rand::random::<f32>() * 100.0;
                c.spawn((
                    Text3d::new("©"),
                    // Text3d::new("®"),
                    Text3dStyling {
                        font: "Eczar".into(),
                        size: 256.,
                        align: TextAlign::Center,
                        color: Srgba::new(0.75, 0.05, 0.05, 0.90),
                        stroke: Some(std::num::NonZero::new(2).unwrap()),
                        stroke_color: Srgba::new(0.25, 0.05, 0.05, 1.0),
                        stroke_join: StrokeJoin::Miter,
                        stroke_in_front: true,
                        ..Default::default()
                    },
                    Mesh3d::default(),
                    MeshMaterial3d(map_resources.pins_material.clone()),
                    Transform::default()
                        .with_translation(Vec3::new(
                            0.0,
                            HEIGHT_OF_MAP_PINS
                                + pin_height_entropy_to_prevent_flickering.abs(),
                            0.0,
                        ))
                        .looking_at(Vec3::new(0.0, 0.0, 0.0), Dir3::NEG_Z),
                    TokenMarker,
                    Visibility::Visible,
                    Pickable {
                        should_block_lower: false,
                        is_hoverable: true,
                    },
                ))
                .observe(|mut trigger: On<Pointer<Over>>| trigger.propagate(false))
                .observe(|mut trigger: On<Pointer<Out>>| trigger.propagate(false))
                .tooltip_on_hover(&token.token_name, 1.0);
            }
        })
        .insert(CollisionLayers::new(
            [HexrollPhysicsLayer::Tokens],
            [HexrollPhysicsLayer::Walls, HexrollPhysicsLayer::Tokens],
        ))
        .insert(trigger.event.transform)
        .apply_mobility_on_token(&token.mobility, &vtt_data)
        .insert(token)
        .insert(TokenLabelOffset(0.0))
        .observe_control_handlers()
        .id();

    if !trigger.event.token.label.is_empty() {
        spawn_token_labels(
            &mut queue,
            e,
            trigger.event.token.label.clone(),
            app_config.tokens_config.label_font_scale,
            m,
            trigger.event.transform.translation.x,
            trigger.event.transform.translation.z,
        );
    }

    let t = commands
        .spawn((
            make_torch_light(trigger.event.token.light, trigger.event.token.light_color),
            // NOTE: we set Y to 0.5 to prevent hotspot on the floor
            // but this means we need walls and doors to be high enough
            // so light wont leak to invisible areas.
            Transform::from_xyz(0.0, 0.5, 0.0),
            Torch,
            if trigger.event.token.light > 0.0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ))
        .id();
    commands.entity(e).add_child(t);

    let we_need_to_spawn_a_shadow_mask_point_light = trigger.event.token.light > 0.0
        && trigger.event.token.mobility.is_player_controllable();
    if we_need_to_spawn_a_shadow_mask_point_light {
        let t = commands
            .spawn((
                make_player_light(trigger.event.token.light),
                Transform::from_xyz(0.0, 0.5, 0.0),
                PlayerShadowMaskPointLight::PostSpawnSetup(Timer::from_seconds(
                    0.1,
                    TimerMode::Once,
                )),
            ))
            .id();
        commands.entity(e).add_child(t);
    }

    commands.trigger(EventContext::from(msg).with_source(trigger.source.clone()));
}

#[derive(Component)]
pub enum PlayerShadowMaskPointLight {
    PostSpawnSetup(Timer),
    Despawn(Timer),
}

fn make_torch_light(_light_radius: f32, color: Color) -> PointLight {
    PointLight {
        // NOTE: using grey to prevent a hotspot on the floor
        color,
        shadows_enabled: true,
        #[cfg(feature = "soft_shadows")]
        soft_shadows_enabled: true,
        radius: 0.0,
        shadow_depth_bias: -0.006,
        shadow_normal_bias: 6.0,
        shadow_map_near_z: 0.1,
        intensity: 0.0,
        range: 0.0,
        ..default()
    }
}
pub fn make_player_light(_light_radius: f32) -> PointLight {
    PointLight {
        // NOTE: using grey to prevent a hotspot on the floor
        color: LinearRgba::new(0.0, 0.0, 0.0, 1.0).into(),
        shadows_enabled: true,
        #[cfg(feature = "soft_shadows")]
        soft_shadows_enabled: true,
        radius: 0.0,
        shadow_depth_bias: -0.006,
        shadow_normal_bias: 6.0,
        shadow_map_near_z: 0.1,
        intensity: 1000000.0,
        range: 1000000.0,
        ..default()
    }
}

fn postprocess_spawned_token(
    trigger: On<SceneInstanceReady>,
    mut commands: Commands,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    meshes: Query<(Entity, &SkinnedMesh, &Aabb)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    token_data: Query<&Token>,
    mut animation_players: Query<(Entity, &mut AnimationPlayer)>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    vtt_data: Res<VttData>,
    asset_server: Res<AssetServer>,
) {
    let token_entity = {
        let Ok(parent) = parents.get(trigger.entity) else {
            return;
        };
        parent.0
    };
    for child in children.iter_descendants(trigger.entity) {
        if let Ok((skinned_entity, _skinned_mesh, skinned_aabb)) = meshes.get(child) {
            commands
                .entity(skinned_entity)
                .try_insert(MainTokenEntity(token_entity));
            commands
                .entity(token_entity)
                .try_insert(TokenMeshEntity(skinned_entity))
                .try_insert(TokenLabelOffset(skinned_aabb.half_extents.z));
            commands
                .entity(skinned_entity)
                .insert(NotShadowCaster)
                .insert(MeshMaterial3d(materials.add(StandardMaterial {
                    unlit: if vtt_data.is_player() { false } else { true },
                    emissive: DUNGEON_FOG_COLOR.into(),
                    alpha_mode: AlphaMode::AlphaToCoverage,
                    diffuse_transmission: 1.0, // NOTE: this allows the token to stay above the torch
                    base_color: if let Ok(token_data) = token_data.get(token_entity) {
                        token_data.color
                    } else {
                        DUNGEON_FOG_COLOR
                    },
                    ..default()
                })));
        }

        let Ok(token_data) = token_data.get(token_entity) else {
            return;
        };

        if let Ok((entity, mut player)) = animation_players.get_mut(child) {
            let (graph, node_indices) = AnimationGraph::from_clips([asset_server
                .load(GltfAssetLabel::Animation(0).from_asset(token_data.glb_file.clone()))]);

            let graph_handle = graphs.add(graph);

            let animations = Animations {
                animations: node_indices,
                graph: graph_handle,
            };

            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, animations.animations[0], Duration::ZERO)
                .repeat();
            commands
                .entity(entity)
                .try_insert(AnimationGraphHandle(animations.graph.clone()))
                .try_insert(transitions);
        }
    }
}

fn on_despawn_token(
    trigger: On<DespawnToken>,
    mut commands: Commands,
    tokens: Query<&Token>,
    token_labels: Query<&TokenLabel>,
) {
    if let Ok(token_label) = token_labels.get(trigger.event().token_entity) {
        if let Ok(mut e) = commands.get_entity(token_label.label_entity) {
            e.despawn();
        }
    }
    if let Ok(mut c) = commands.get_entity(trigger.event().token_entity) {
        c.despawn();
    }
    if let Ok(token) = tokens.get(trigger.event().token_entity) {
        commands.trigger(EventContext::from(TokenMessage::Delete(token.token_id)));
    }
}

fn on_despawn_visible_tokens(
    _: On<DespawnVisibleTokens>,
    mut commands: Commands,
    tokens: Query<(Entity, &Token, &GlobalTransform), Without<SelectedToken>>,
    token_labels: Query<&TokenLabel>,
    visibility: Res<MapVisibilityController>,
) {
    for (token_entity, token, token_global_transform) in tokens.iter() {
        if visibility
            .rect
            .contains(token_global_transform.translation().xz())
        {
            if let Ok(token_label) = token_labels.get(token_entity) {
                if let Ok(mut e) = commands.get_entity(token_label.label_entity) {
                    e.despawn();
                }
            }
            if let Ok(mut c) = commands.get_entity(token_entity) {
                c.despawn();
            }
            commands.trigger(EventContext::from(TokenMessage::Delete(token.token_id)));
        }
    }
}

fn set_tokens_visibility_by_camera_viewport(
    mut items: Query<(&mut Visibility, &GlobalTransform), With<Token>>,
    visibility: Res<MapVisibilityController>,
) {
    // Prevent token lights from turning off when slightly out of bounds
    // TODO: Make this part of GlobalSettings
    for (mut v, gt) in items.iter_mut() {
        if visibility
            .rect
            .inflate(10.0)
            .contains(gt.translation().xz())
            && visibility.scale < 0.7
        {
            *v = Visibility::Inherited;
        } else {
            *v = Visibility::Hidden;
        }
    }
}
