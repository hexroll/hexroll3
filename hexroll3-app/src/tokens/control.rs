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

// Token controls (primariliy movement, but also scale, color, etc.)
use bevy::{
    input::mouse::MouseWheel,
    mesh::skinning::SkinnedMesh,
    prelude::*,
    window::{CursorIcon, PrimaryWindow, SystemCursorIcon},
};

use bevy_editor_cam::prelude::EditorCam;

use avian3d::{math::Scalar, prelude::*};

use crate::{
    battlemaps::{BattlemapSelection, BattlemapSelectionFinalizing, RulerDragData},
    hexmap::elements::{
        HexMapResources, HexMapToolState, MainCamera, MapVisibilityController,
    },
    shared::{
        camera::CameraZoomRestrictor,
        dragging::{
            DraggingMotionDetector, reset_dragging_detector, update_dragging_detector,
        },
        input::InputMode,
        labels::{TokenLabel, spawn_token_labels},
        layers::HEIGHT_OF_TOKENS,
        settings::{AppSettings, Config, RulersMode},
        spawnq::SpawnQueue,
        vtt::{HexMapMode, VttData},
        widgets::{buttons::ToggleResourceWrapper, cursor::pointer_world_position},
    },
    tokens::TOKEN_ROTATION_ZOOM_LIMIT,
    vtt::sync::EventContext,
};

use super::{
    DuplicateLastSpawnedToken, MainTokenEntity, SelectedToken, SpawnToken,
    TOKEN_DESELECTION_ZOOM_THRESHOLD, TOKEN_MOVEMENT_ZOOM_LIMIT, TeleportSelectedTokens,
    Token, TokenMeshEntity, TokenMessage, TokenUpdateMessage,
    initiative::*,
    library::TokenLibrary,
    spawn::{PlayerShadowMaskPointLight, make_player_light},
    token_dial::SpawnTokenDial,
    tokens::{ActiveToken, TokenIsLocked, Torch},
};

pub struct TokenControlPlugin;
impl Plugin for TokenControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                rotate_token,
                add_outline_to_tokens,
                remove_outline_from_tokens,
                update_torch_radius,
                token_teleportation_and_duplication,
                update_dragged_token_position,
                player_lighting_masks_controller,
                tokens_temporary_visibility_hotkey,
            ),
        )
        .add_systems(Update, token_selection.before(reset_dragging_detector))
        .add_systems(Update, token_interaction.after(update_dragging_detector))
        .add_observer(on_update_token_label)
        .add_observer(on_teleport_selected_tokens)
        .add_observer(on_duplicate_last_spawned_token);
    }
}
#[derive(Component)]
struct TokenTarget;

#[derive(Component)]
struct TokenJoint;

#[derive(Component)]
struct FollowingToken;

#[derive(Component)]
struct LastSelectedToken;

#[derive(Component, Default, PartialEq)]
pub enum TokenInteractionMode {
    #[default]
    None,
    Scale(Vec3, f32),
    Torch,
}

// NOTE: can go up to 0.025;
const GRAB_COMPLIANCE: Scalar = 0.015;
// NOTE: can go down to 15.0
const GRAB_LINEAR_DAMPING: Scalar = 30.0;
const GRAB_ANGULAR_DAMPING: Scalar = 1.0;

fn set_active_token_on_mouse_over() -> impl Fn(
    On<Pointer<Over>>,
    Commands,
    Single<&mut EditorCam>,
    Res<State<HexMapToolState>>,
    Single<Entity, With<PrimaryWindow>>,
    Query<&TokenIsLocked>,
    Res<VttData>,
) {
    move |trigger, mut commands, mut panorbit, tool_state, window, locked_tokens, vtt_data| {
        if *tool_state == HexMapToolState::Selection {
            panorbit.enabled_motion.pan = false;
            panorbit.enabled_motion.zoom = false;
            commands.entity(trigger.entity).insert(ActiveToken);
            if !vtt_data.is_player() {
                if locked_tokens.contains(trigger.entity) {
                    commands
                        .entity(*window)
                        .try_insert(CursorIcon::System(SystemCursorIcon::NotAllowed));
                } else {
                    commands
                        .entity(*window)
                        .try_insert(CursorIcon::System(SystemCursorIcon::Default));
                }
            }
        }
    }
}

fn set_inactive_token_on_mouse_out() -> impl Fn(
    On<Pointer<Out>>,
    Commands,
    Single<&mut EditorCam>,
    Res<State<HexMapToolState>>,
    Single<Entity, With<PrimaryWindow>>,
) {
    move |trigger, mut commands, mut panorbit, tool_state, window| {
        if *tool_state == HexMapToolState::Selection
            || *tool_state == HexMapToolState::DialMenu
        {
            panorbit.enabled_motion.pan = true;
            panorbit.enabled_motion.zoom = true;
            commands.entity(trigger.entity).remove::<ActiveToken>();
            commands
                .entity(*window)
                .try_insert(CursorIcon::System(SystemCursorIcon::Default));
        }
    }
}

fn player_lighting_masks_controller(
    mut masks: Query<(Entity, &mut PointLight, &mut PlayerShadowMaskPointLight)>,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (entity, mut mask, mut timer) in masks.iter_mut() {
        match &mut *timer {
            PlayerShadowMaskPointLight::PostSpawnSetup(timer) => {
                if timer.is_finished() {
                    mask.is_player = true;
                } else {
                    timer.tick(time.delta());
                }
            }
            PlayerShadowMaskPointLight::Despawn(timer) => {
                if timer.is_finished() {
                    commands.entity(entity).try_despawn();
                } else {
                    timer.tick(time.delta());
                }
            }
        }
    }
}

fn begin_token_movement() -> impl Fn(
    On<Pointer<DragStart>>,
    Single<&mut EditorCam>,
    Commands,
    Res<AppSettings>,
    Query<&Transform, Without<TokenIsLocked>>,
    Query<(Entity, &Transform), With<SelectedToken>>,
    Query<&SelectedToken>,
    Query<(Entity, &ChildOf), With<PlayerShadowMaskPointLight>>,
    Res<VttData>,
) {
    move |trigger,
          mut panorbit,
          mut commands,
          settings,
          transforms,
          other,
          all_selected,
          lights,
          vtt_data| {
        panorbit.enabled_motion.pan = false;
        panorbit.enabled_motion.zoom = false;

        panorbit.restrict_camera_zoom(TOKEN_MOVEMENT_ZOOM_LIMIT);

        if settings.rulers_mode == RulersMode::ShowWhenMoving {
            if let Ok(transform) = transforms.get(trigger.entity) {
                // NOTE: If a player is grabbing a player-enabled token, then we assign
                // an exclusive mask shadow point light so to provide the player with a
                // the correct field-of-view.
                if vtt_data.is_player() {
                    reset_player_field_of_view(&mut commands, trigger.entity, lights);
                }
                let tgt = commands
                    .spawn((
                        TokenTarget,
                        RigidBody::Kinematic,
                        Position::from_xyz(
                            transform.translation.x,
                            0.0,
                            transform.translation.z,
                        ),
                    ))
                    .id();
                commands
                    .entity(trigger.entity)
                    .insert(RulerDragData::from_start_pos(transform.translation))
                    .remove::<ColliderDisabled>()
                    .insert(RigidBody::Dynamic);
                commands.spawn((
                    TokenJoint,
                    DistanceJoint::new(tgt, trigger.entity).with_compliance(GRAB_COMPLIANCE),
                    JointDamping {
                        linear: GRAB_LINEAR_DAMPING,
                        angular: GRAB_ANGULAR_DAMPING,
                    },
                ));

                if all_selected.contains(trigger.entity) {
                    for (o, t) in other.iter() {
                        if o != trigger.entity
                            && t.translation.distance(transform.translation) < 5.0
                        {
                            commands
                                .entity(o)
                                .remove::<ColliderDisabled>()
                                .insert(FollowingToken)
                                .insert(RigidBody::Dynamic);
                            commands.spawn((
                                TokenJoint,
                                DistanceJoint::new(tgt, o)
                                    .with_compliance(GRAB_COMPLIANCE)
                                    // TODO: Verify: with_rest_length seems to be deprecated.
                                    // .with_rest_length(2.5)
                                    .with_limits(0.1, 2.5),
                                JointDamping {
                                    linear: GRAB_LINEAR_DAMPING,
                                    angular: GRAB_ANGULAR_DAMPING,
                                },
                            ));
                        }
                    }
                }
            }
        }
    }
}

fn reset_player_field_of_view(
    commands: &mut Commands,
    token_entity: Entity,
    lights: Query<(Entity, &ChildOf), With<PlayerShadowMaskPointLight>>,
) {
    let mut skip = false;
    for (light, child_of) in lights.iter() {
        if child_of.0 == token_entity {
            skip = true;
        } else {
            commands
                .entity(light)
                .try_insert(PlayerShadowMaskPointLight::Despawn(Timer::from_seconds(
                    0.2,
                    TimerMode::Once,
                )));
        }
    }
    if !skip {
        commands.entity(token_entity).with_child((
            make_player_light(0.0),
            Transform::from_xyz(0.0, 0.5, 0.0),
            PlayerShadowMaskPointLight::PostSpawnSetup(Timer::from_seconds(
                0.1,
                TimerMode::Once,
            )),
        ));
    }
}

fn end_token_movement() -> impl Fn(
    On<Pointer<DragEnd>>,
    Single<&mut EditorCam>,
    Commands,
    Query<Entity, With<TokenTarget>>,
    Query<Entity, With<TokenJoint>>,
    Query<Entity, With<SelectedToken>>,
) {
    move |trigger, mut panorbit, mut commands, targets, joints, other| {
        panorbit.release_camera_zoom_restriction();
        commands.entity(trigger.entity).remove::<RulerDragData>();
        commands
            .entity(trigger.entity)
            .remove::<CollidingEntities>()
            .insert(CollidingEntities::default())
            .insert(RigidBody::Static)
            .insert(ColliderDisabled);
        for o in other.iter() {
            if o != trigger.entity {
                commands
                    .entity(o)
                    .insert(RigidBody::Static)
                    .insert(ColliderDisabled)
                    .remove::<CollidingEntities>()
                    .insert(CollidingEntities::default())
                    .remove::<FollowingToken>();
            }
        }
        for tgt in targets.iter() {
            commands.entity(tgt).despawn();
        }
        for joint in joints.iter() {
            commands.entity(joint).despawn();
        }
    }
}

#[allow(clippy::type_complexity)]
fn move_token<E>() -> impl Fn(
    On<Pointer<Drag>>,
    Query<&mut Transform, Without<TokenIsLocked>>,
    Commands,
    Query<&Window, With<PrimaryWindow>>,
    Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    Single<&mut EditorCam>,
    ResMut<DraggingMotionDetector>,
    Query<Entity, (With<TokenTarget>, Without<FollowingToken>)>,
    Query<&mut DistanceJoint>,
    Query<&CollidingEntities>,
    Query<Entity, (With<FollowingToken>, Without<TokenTarget>)>,
    Res<ToggleResourceWrapper<BattlemapsSnapping>>,
) {
    move |trigger,
          mut query,
          mut commands,
          q_window,
          q_camera,
          mut panorbit,
          mut motion_state,
          targets,
          mut joints,
          collisions,
          followers,
          snap| {
        *motion_state = DraggingMotionDetector::MovementRecorded;

        panorbit.enabled_motion.pan = false;
        panorbit.enabled_motion.zoom = false;

        if let Ok(t) = query.get_mut(trigger.entity) {
            if let Some(point) = pointer_world_position(q_window, q_camera) {
                for tgt in targets.iter() {
                    let collisions_set = collisions.get(trigger.entity).unwrap();

                    let colliding_with_something_other_than_followers = {
                        let followers_in_collision_set = followers
                            .iter()
                            .filter(|follower| collisions_set.contains(follower))
                            .count();
                        collisions_set.len() > followers_in_collision_set
                    };

                    let point = if point.distance(t.translation) > 2.0
                        && colliding_with_something_other_than_followers
                    {
                        for mut j in joints.iter_mut() {
                            if j.body2 == trigger.entity {
                                j.compliance = 0.05;
                            }
                        }
                        let vec1 = t.translation;
                        let vec2 = point;
                        vec1 + (vec2 - vec1).normalize() * 2.0
                    } else {
                        // FIXME: when the motion is too strong or when after
                        // the prev state and released, this can cause
                        // going through colliders
                        if point.distance(t.translation) < 3.0 {
                            for mut j in joints.iter_mut() {
                                j.compliance = GRAB_COMPLIANCE;
                            }
                            point
                        } else {
                            let vec1 = t.translation;
                            let vec2 = point;
                            vec1 + (vec2 - vec1).normalize()
                                * point.distance(t.translation).min(10.0)
                        }
                    };
                    let position = match snap.value {
                        BattlemapsSnapping::On => Position::from_xyz(
                            snap_to_quarter(point.x),
                            0.0,
                            snap_to_quarter(point.z),
                        ),
                        BattlemapsSnapping::Off => Position::from_xyz(point.x, 0.0, point.z),
                    };
                    commands.entity(tgt).insert(position);
                }
            }
        }
    }
}

#[derive(Component, Default, PartialEq, Clone)]
pub enum BattlemapsSnapping {
    #[default]
    On,
    Off,
}

fn snap_to_quarter(value: f32) -> f32 {
    (value * 4.0).round() / 4.0
}

fn update_dragged_token_position(
    mut commands: Commands,
    tokens: Query<
        (Entity, &Transform, &Token),
        (Without<ColliderDisabled>, Without<TokenIsLocked>),
    >,
    mut rulers: Query<&mut RulerDragData>,
) {
    for (entity, transform, token) in tokens.iter() {
        if let Ok(mut ruler) = rulers.get_mut(entity) {
            ruler.move_to(transform.translation.with_y(5.0));
        }
        commands.trigger(EventContext::from(TokenMessage::Update(
            TokenUpdateMessage::from_token_id(token.token_id).with_transform(*transform),
        )));
    }
}

fn rotate_token(
    mut commands: Commands,
    mut active_token: Query<
        (&mut Transform, &Token, &TokenInteractionMode),
        (With<ActiveToken>, Without<TokenIsLocked>),
    >,
    mut evr_scroll: MessageReader<MouseWheel>,
    visibility: Res<MapVisibilityController>,
) {
    if visibility.scale > TOKEN_ROTATION_ZOOM_LIMIT {
        return;
    }
    use bevy::input::mouse::MouseScrollUnit;
    for (mut t, token, _interaction) in active_token.iter_mut() {
        for ev in evr_scroll.read() {
            match ev.unit {
                MouseScrollUnit::Line => {
                    t.rotate_y(ev.y / 5.0);
                    commands.trigger(EventContext::from(TokenMessage::Update(
                        TokenUpdateMessage::from_token_id(token.token_id).with_transform(*t),
                    )));
                }
                MouseScrollUnit::Pixel => {}
            }
        }
    }
}

fn update_torch_radius(
    tokens: Query<&mut Token>,
    mut torches: Query<(&mut PointLight, &ChildOf), With<Torch>>,
) {
    for (mut torch, torch_parent) in torches.iter_mut() {
        if let Ok(token) = tokens.get(torch_parent.parent()) {
            torch.intensity = 5000.0 * token.light;
            torch.range = token.light + 0.5;
            torch.color = token.light_color;
        }
    }
}

#[derive(Component)]
pub struct TorchInteractionGizmo;

fn token_selection(
    mut commands: Commands,
    q_selected: Query<Entity, With<SelectedToken>>,
    last_selected: Query<Entity, With<LastSelectedToken>>,
    dmd: Res<DraggingMotionDetector>,
    active_tokens: Query<&ActiveToken>,
    keyboard: Res<ButtonInput<KeyCode>>,
    click: Res<ButtonInput<MouseButton>>,
    cam: Single<&Projection, With<MainCamera>>,
    gizmo: Query<(Entity, &BattlemapSelection), With<BattlemapSelectionFinalizing>>,
    tokens: Query<(Entity, &GlobalTransform), With<Token>>,
    input_mode: Res<InputMode>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) && input_mode.keyboard_available() {
        for lst in last_selected.iter() {
            commands.entity(lst).insert(SelectedToken);
        }
    }
    if let Projection::Orthographic(proj) = *cam {
        if ((click.just_released(MouseButton::Left)
            && !keyboard.pressed(KeyCode::ControlLeft))
            || proj.scale > TOKEN_DESELECTION_ZOOM_THRESHOLD)
            && !dmd.motion_detected()
            && active_tokens.is_empty()
        {
            for selected in q_selected.iter() {
                commands
                    .entity(selected)
                    .remove::<SelectedToken>()
                    .insert(LastSelectedToken);
            }
        }
    }

    if let Some((e, gizmo)) = gizmo.iter().next() {
        if click.just_released(MouseButton::Left) {
            for (te, tt) in tokens.iter() {
                if tt
                    .translation()
                    .with_y(HEIGHT_OF_TOKENS)
                    .distance(gizmo.from)
                    < gizmo.radius
                {
                    commands.entity(te).insert(SelectedToken);
                }
            }
            commands.entity(e).despawn();
        }
    }
}

fn on_teleport_selected_tokens(
    trigger: On<TeleportSelectedTokens>,
    mut commands: Commands,
    selected: Query<(Entity, &Transform, &Token), With<SelectedToken>>,
) {
    let mut positions: Vec<Vec3> = Vec::new();
    for (_, transform, _) in selected.iter() {
        positions.push(transform.translation);
    }

    let center =
        positions.iter().fold(Vec3::ZERO, |acc, pos| acc + pos) / positions.len() as f32;

    let updated_positions: Vec<Vec3> = positions
        .iter()
        .map(|pos| (pos + (trigger.event().teleport_to - center)).with_y(center.y))
        .collect();

    for (index, (token_entity, transform, token_data)) in selected.iter().enumerate() {
        let translation = updated_positions[index];
        let updated_transform = transform.with_translation(translation);
        commands.entity(token_entity).insert(updated_transform);
        commands.trigger(EventContext::from(TokenMessage::Update(
            TokenUpdateMessage::from_token_id(token_data.token_id)
                .with_transform(updated_transform),
        )));
    }
}

fn on_duplicate_last_spawned_token(
    trigger: On<DuplicateLastSpawnedToken>,
    mut commands: Commands,
    library: Option<Res<TokenLibrary>>,
) {
    let library = match library {
        Some(lib) => lib,
        None => return,
    };
    if let Some(token_template) = library.last_spawned.clone() {
        commands.trigger(EventContext::from(SpawnToken {
            token: Token::from_template(&token_template),
            transform: Transform::from_scale(Vec3::splat(1.0)).with_translation(Vec3::new(
                trigger.event().duplicate_pos.x,
                HEIGHT_OF_TOKENS,
                trigger.event().duplicate_pos.z,
            )),
        }));
    }
}

fn token_teleportation_and_duplication(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    input_mode: Res<InputMode>,
) {
    if let Some(point) = pointer_world_position(q_window, q_camera) {
        if keyboard.just_pressed(KeyCode::KeyB) && input_mode.keyboard_available() {
            commands.trigger(TeleportSelectedTokens { teleport_to: point });
        }

        if keyboard.just_pressed(KeyCode::KeyT)
            && keyboard.pressed(KeyCode::ControlLeft)
            && input_mode.keyboard_available()
        {
            commands.trigger(DuplicateLastSpawnedToken {
                duplicate_pos: point,
            });
        }
    }
}

fn tokens_temporary_visibility_hotkey(
    mut commands: Commands,
    input_mode: Res<InputMode>,
    keyboard: Res<ButtonInput<KeyCode>>,
    tokens: Query<Entity, (With<Token>, Without<SelectedToken>)>,
    vtt_data: Res<VttData>,
) {
    if vtt_data.mode.is_player() {
        return;
    }
    if keyboard.pressed(KeyCode::KeyM) && input_mode.keyboard_available() {
        for t in tokens.iter() {
            commands.entity(t).insert(Visibility::Hidden);
        }
    }
    if keyboard.just_released(KeyCode::KeyM) && input_mode.keyboard_available() {
        for t in tokens.iter() {
            commands.entity(t).insert(Visibility::Inherited);
        }
    }
}

fn token_interaction(
    mut commands: Commands,
    mut tokens: Query<
        (
            &mut Transform,
            &mut Token,
            &mut TokenInteractionMode,
            &GlobalTransform,
        ),
        Without<TorchInteractionGizmo>,
    >,
    click: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut q_torch_gizmos: Query<(Entity, &mut Transform), With<TorchInteractionGizmo>>,
    mut camera_motion_state: ResMut<DraggingMotionDetector>,
    mut editor_cam: Single<&mut EditorCam>,
) {
    if click.just_pressed(MouseButton::Left) {
        for (_, _, mut interaction, _) in tokens.iter_mut() {
            if *interaction != TokenInteractionMode::None {
                editor_cam.release_camera_zoom_restriction();
                camera_motion_state.set_detected();
                *interaction = TokenInteractionMode::None;
            }
        }
        for (e, _) in q_torch_gizmos.iter() {
            commands.entity(e).despawn();
        }
        return;
    }
    if tokens.is_empty() {
        return;
    }

    if let Some(point) = pointer_world_position(q_window, q_camera) {
        for (mut t, mut token, interaction, gt) in tokens.iter_mut() {
            match *interaction {
                TokenInteractionMode::Scale(pos, start) => {
                    camera_motion_state.set_detected();
                    let one = gt.translation().xz().distance(pos.xz());
                    let distance = point.xz().distance(gt.translation().xz());
                    let factor = distance / one;
                    t.scale = Vec3::new(start * factor, 1.0, start * factor);
                    if t.scale.x < 0.2 {
                        t.scale = Vec3::new(0.2, 1.0, 0.2);
                    }
                }
                TokenInteractionMode::Torch => {
                    camera_motion_state.set_detected();
                    let distance = point.xz().distance(gt.translation().xz());
                    let distance = distance / t.scale.x;
                    for (_, mut t) in q_torch_gizmos.iter_mut() {
                        t.scale = Vec3::splat(distance);
                    }
                    token.light = distance;
                }
                TokenInteractionMode::None => {
                    continue;
                }
            }
            commands.trigger(EventContext::from(TokenMessage::Update(
                TokenUpdateMessage::from_token(&token).with_transform(*t),
            )));
        }
    }
}

fn add_outline_to_tokens(
    mut commands: Commands,
    query: Query<
        (Entity, &MainTokenEntity),
        (With<SkinnedMesh>, Without<bevy_mod_outline::OutlineVolume>),
    >,
    selection: Query<&SelectedToken>,
    vtt_data: Res<VttData>,
) {
    for (e, p) in query.iter() {
        if selection.contains(p.0) || vtt_data.mode == HexMapMode::RefereeAsPlayer {
            commands
                .entity(e)
                .try_remove::<bevy_mod_outline::ComputedOutline>();
            commands.entity(e).try_insert((
                bevy_mod_outline::OutlineVolume {
                    visible: true,
                    width: 2.0,
                    colour: Color::srgb(1.0, 0.0, 0.0),
                },
                bevy_mod_outline::OutlineMode::FloodFlatDoubleSided,
            ));
        }
    }
}

fn remove_outline_from_tokens(
    mut commands: Commands,
    query: Query<
        (Entity, &MainTokenEntity),
        (With<SkinnedMesh>, With<bevy_mod_outline::OutlineVolume>),
    >,
    selection: Query<&SelectedToken>,
    vtt_data: Res<VttData>,
) {
    for (e, p) in query.iter() {
        if !selection.contains(p.0) && vtt_data.mode != HexMapMode::RefereeAsPlayer {
            commands
                .entity(e)
                .try_remove::<bevy_mod_outline::ComputedOutline>();
            commands.entity(e).try_remove::<(
                bevy_mod_outline::OutlineVolume,
                bevy_mod_outline::OutlineMode,
                bevy_mod_outline::ComputedOutline,
            )>();
        }
    }
}

pub fn update_token_material(
    commands: &mut Commands,
    finder: &Query<&TokenMeshEntity>,
    mesh_materials: &Query<&MeshMaterial3d<StandardMaterial>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    token: Entity,
    cc: Color,
) {
    if let Ok(mesh_entity) = finder.get(token) {
        let mesh_entity = mesh_entity.0;
        let material_asset = mesh_materials.get(mesh_entity).unwrap();
        let mut material = materials.get(material_asset.id()).unwrap().clone();
        material.base_color = cc;
        commands
            .entity(mesh_entity)
            .insert(MeshMaterial3d(materials.add(material)));
    }
}

pub trait TokenControls {
    fn observe_control_handlers(&mut self) -> &mut Self;
}
impl TokenControls for EntityCommands<'_> {
    fn observe_control_handlers(&mut self) -> &mut Self {
        self.insert(TokenInteractionMode::default())
            .observe(
                |trigger: On<Pointer<Click>>,
                 mut commands: Commands,
                 mut camera_motion_state: ResMut<DraggingMotionDetector>,
                 key: Res<ButtonInput<KeyCode>>,
                 currently_selected: Query<&SelectedToken>,
                 last_selected: Query<Entity, With<LastSelectedToken>>,
                 vtt_data: Res<VttData>| {
                    if trigger.event().button == PointerButton::Secondary {
                        commands.trigger(SpawnTokenDial {
                            token: trigger.entity,
                        });
                        if vtt_data.mode.is_player() {
                            commands.trigger(InitializeInitiativeSetup(trigger.entity));
                        }
                    }
                    if trigger.event().button == PointerButton::Primary {
                        if key.pressed(KeyCode::ControlLeft) {
                            if currently_selected.is_empty() {
                                for lst in last_selected.iter() {
                                    commands.entity(lst).remove::<LastSelectedToken>();
                                }
                            }
                            commands.entity(trigger.entity).insert(SelectedToken);
                        }
                        *camera_motion_state = DraggingMotionDetector::MovementRecorded;
                    }
                },
            )
            .observe(move_token::<Pointer<Drag>>())
            .observe(set_active_token_on_mouse_over())
            .observe(set_inactive_token_on_mouse_out())
            .observe(end_token_movement())
            .observe(begin_token_movement())
    }
}

#[derive(Event)]
pub struct UpdateTokenLabel {
    pub token_entity: Entity,
    pub label: String,
}

pub fn on_update_token_label(
    trigger: On<UpdateTokenLabel>,
    mut commands: Commands,
    token_label: Query<&TokenLabel>,
    children: Query<&Children>,
    tokens_data: Query<(Entity, &Token, &GlobalTransform)>,
    mut queue: ResMut<SpawnQueue>,
    map_resources: Res<HexMapResources>,
    app_config: Res<Config>,
) {
    if let Ok(token_label_entity) = token_label.get(trigger.token_entity) {
        if let Some(child_entities) = children.get(token_label_entity.label_entity).ok() {
            if let Some(child) = child_entities.iter().next() {
                if trigger.label.trim().is_empty() {
                    commands
                        .entity(token_label_entity.label_entity)
                        .try_despawn();
                    commands
                        .entity(trigger.token_entity)
                        .try_remove::<TokenLabel>();
                } else {
                    commands
                        .entity(child)
                        .try_insert(bevy_rich_text3d::Text3d::new(trigger.label.clone()));
                }
            }
        }
    } else {
        if let Ok((e, _, gt)) = tokens_data.get(trigger.token_entity) {
            let m = map_resources.token_labels_material.clone();
            spawn_token_labels(
                &mut queue,
                e,
                trigger.label.clone(),
                app_config.tokens_config.label_font_scale,
                m,
                gt.translation().x,
                gt.translation().z,
            );
        }
    }
}
