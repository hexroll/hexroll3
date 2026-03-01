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

// Dial-shaped menus support.
//
// Hexroll has three main dial menus:
// - Hex map editing menu
// - Battlemap tools menu
// - Token controls menu
//
// These menus share their common behavior using this module.
use std::time::Duration;

use bevy::{
    ecs::{
        lifecycle::HookContext,
        system::{IntoObserverSystem, SystemParam},
        world::DeferredWorld,
    },
    platform::collections::HashMap,
    prelude::*,
    window::{CursorIcon, PrimaryWindow, SystemCursorIcon},
};

use bevy_tweening::{Animator, lens::TransformScaleLens};
use bevy_vector_shapes::{
    ShapePlugin,
    prelude::{ShapeCommands, ShapeEntityCommands},
    shapes::DiscSpawner,
};

use crate::{
    hexmap::elements::{HexMapToolState, MainCamera},
    shared::widgets::cursor::TooltipOnHover,
};

use super::super::{dragging::DraggingMotionDetector, tweens::StandardMaterialOpacityLens};

pub struct DialPlugin;
impl Plugin for DialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ShapePlugin::default())
            .add_systems(Startup, setup)
            .add_systems(Update, dial_zoom_controller)
            .add_systems(OnExit(HexMapToolState::DialMenu), despawn_dial)
            .add_observer(on_despawn_dial);
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let dial_core_assets =
        DialCoreAssets::new(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(100000.0))));
    commands.insert_resource(dial_core_assets);
}

#[derive(Component)]
pub struct DialButton;

#[derive(Component)]
pub struct DialButtonIcon;

// TODO: possible optimization here would be to change standard material into
//       a custom material with a layered texture.
#[derive(Resource)]
pub struct DialAssets<T>
where
    T: Copy + std::hash::Hash + Eq,
{
    pub menu_color_disc: Handle<Mesh>,
    pub menu_item_billboard: Handle<Mesh>,
    pub icon_materials: HashMap<T, Handle<StandardMaterial>>,
}

impl<T> DialAssets<T>
where
    T: Copy + std::hash::Hash + Eq,
{
    pub fn new(menu_item_billboard: Handle<Mesh>, menu_color_disc: Handle<Mesh>) -> Self {
        DialAssets {
            menu_color_disc,
            menu_item_billboard,
            icon_materials: HashMap::new(),
        }
    }

    pub fn add_item(
        &mut self,
        icon: T,
        filename: &str,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        asset_server: &Res<AssetServer>,
    ) -> &mut Self {
        self.icon_materials.insert(
            icon,
            materials.add(StandardMaterial {
                unlit: true,
                base_color_texture: Some(asset_server.load(filename.to_string())),
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
        );
        self
    }
}

#[derive(SystemParam)]
pub struct DialMenuCommands<'w, 's> {
    commands: Commands<'w, 's>,
    shape_commands: ShapeCommands<'w, 's>,
    dials: Query<'w, 's, Entity, With<DialController>>,
    proj: Single<'w, 's, &'static Projection, With<MainCamera>>,
    dial_assets: Res<'w, DialCoreAssets>,
    next_state: ResMut<'w, NextState<HexMapToolState>>,
}

impl<'w, 's> DialMenuCommands<'w, 's> {
    pub fn spawn_menu(&mut self, options: DialMenuOptions) -> Option<Entity> {
        for d in self.dials.iter() {
            self.commands.entity(d).despawn();
        }
        let tween = if let Projection::Orthographic(proj) = *self.proj {
            if !(options.is_visible)(proj.scale) {
                return None;
            }
            let scale = (options.calc_scale)(proj.scale);

            bevy_tweening::Tween::new(
                EaseFunction::ElasticOut,
                Duration::from_millis(300),
                TransformScaleLens {
                    start: Vec3::ZERO,
                    end: Vec3::splat(scale),
                },
            )
        } else {
            return None;
        };
        self.commands
            .spawn((
                DialBgMarker,
                Mesh3d(self.dial_assets.bg_mesh.clone()),
                Transform::from_xyz(0.0, 399.0, 0.0),
            ))
            .observe(
                |_trigger: On<Pointer<Press>>,
                 mut dmd: ResMut<DraggingMotionDetector>,
                 mut next_state: ResMut<NextState<HexMapToolState>>| {
                    *dmd = DraggingMotionDetector::Pending;
                    next_state.set(HexMapToolState::Selection);
                },
            );
        self.next_state.set(HexMapToolState::DialMenu);
        Some(
            self.shape_commands
                .dial_mode(options.pos)
                .insert(DialController {
                    calc_scale: options.calc_scale,
                    is_visible: options.is_visible,
                })
                .insert(bevy_tweening::Animator::new(tween))
                .id(),
        )
    }
}

pub struct DialMenuOptions {
    pub pos: Vec2,
    pub calc_scale: fn(f32) -> f32,
    pub is_visible: fn(f32) -> bool,
}

pub trait MenuItemSpawner {
    fn spawn_menu_item<T, E, B, M>(
        &mut self,
        index: i32,
        total: i32,
        icon: T,
        handler: impl IntoObserverSystem<E, B, M>,
        dial_assets: &Res<DialAssets<T>>,
        tooltip: &str,
    ) -> &mut Self
    where
        T: Copy + std::hash::Hash + Eq + std::marker::Send + Sync + 'static,
        B: Bundle,
        E: EntityEvent;
    fn spawn_menu_color<'a, T, E, B, M>(
        &mut self,
        index: i32,
        color: &Color,
        handler: impl IntoObserverSystem<E, B, M>,
        dial_assets: &Res<DialAssets<T>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) -> &mut Self
    where
        T: Copy + std::hash::Hash + Eq + std::marker::Send + Sync + 'static,
        B: Bundle,
        E: EntityEvent;
}

impl MenuItemSpawner for EntityCommands<'_> {
    fn spawn_menu_item<'a, T, E, B, M>(
        &mut self,
        index: i32,
        total: i32,
        icon: T,
        handler: impl IntoObserverSystem<E, B, M>,
        dial_assets: &Res<DialAssets<T>>,
        tooltip: &str,
    ) -> &mut Self
    where
        T: Copy + std::hash::Hash + Eq + std::marker::Send + Sync + 'static,
        B: Bundle,
        E: EntityEvent,
    {
        {
            fn calculate_position(radius: f32, angle_degrees: f32) -> Vec2 {
                let angle_radians = angle_degrees.to_radians();
                let x = radius * angle_radians.cos();
                let y = radius * angle_radians.sin();
                Vec2::new(x, y)
            }
            let pos = calculate_position(220.0, (360.0 / total as f32) * (index + 9) as f32);
            let scale_tween = bevy_tweening::Tween::new(
                EaseFunction::ElasticOut,
                Duration::from_millis(rand::Rng::gen_range(
                    &mut rand::thread_rng(),
                    600..=800,
                )),
                TransformScaleLens {
                    start: Vec3::ZERO,
                    end: Vec3::ONE,
                },
            );
            self.insert((
                Name::new("DialButton"),
                DialButton,
                Mesh3d(dial_assets.menu_item_billboard.clone()),
                Transform::from_xyz(pos.x, pos.y, -1.0),
                Animator::new(scale_tween),
            ))
            .with_child((
                DialButtonIcon,
                Mesh3d(dial_assets.menu_item_billboard.clone()),
                MeshMaterial3d(dial_assets.icon_materials.get(&icon).unwrap().clone()),
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
            ))
            .tooltip_on_hover(tooltip, 1.0)
            .observe(mouse_over())
            .observe(mouse_out())
            .observe(handler)
        }
    }
    fn spawn_menu_color<'a, T, E, B, M>(
        &mut self,
        index: i32,
        color: &Color,
        handler: impl IntoObserverSystem<E, B, M>,
        dial_assets: &Res<DialAssets<T>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) -> &mut Self
    where
        T: Copy + std::hash::Hash + Eq + std::marker::Send + Sync + 'static,
        B: Bundle,
        E: EntityEvent,
    {
        {
            fn calculate_position(radius: f32, angle_degrees: f32) -> Vec2 {
                let angle_radians = angle_degrees.to_radians();
                let x = radius * angle_radians.cos();
                let y = radius * angle_radians.sin();
                Vec2::new(x, y)
            }
            let pos = calculate_position(220.0, 30.0 * (index + 9) as f32);
            let scale_tween = bevy_tweening::Tween::new(
                EaseFunction::ElasticOut,
                Duration::from_millis(rand::Rng::gen_range(
                    &mut rand::thread_rng(),
                    600..=800,
                )),
                TransformScaleLens {
                    start: Vec3::ZERO,
                    end: Vec3::ONE,
                },
            );
            self.insert((
                Name::new("DialButton"),
                DialButton,
                Mesh3d(dial_assets.menu_color_disc.clone()),
                Transform::from_xyz(pos.x, pos.y, -1.0)
                    .with_rotation(Quat::from_rotation_x(3.1415)),
                Animator::new(scale_tween),
            ))
            .with_child((
                Mesh3d(dial_assets.menu_color_disc.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: color.clone(),
                    unlit: true,
                    ..default()
                })),
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
            ))
            .observe(mouse_over())
            .observe(mouse_out())
            .observe(handler)
        }
    }
}

trait DialMaker {
    fn dial_mode(&mut self, pos: Vec2) -> ShapeEntityCommands<'_, '_>;
}

impl<'a, 'b> DialMaker for ShapeCommands<'a, 'b> {
    fn dial_mode(&mut self, pos: Vec2) -> ShapeEntityCommands<'_, '_> {
        self.set_3d();
        self.set_rotation(Quat::from_rotation_x(std::f32::consts::PI / 2.0));
        self.set_translation(Vec3::new(pos.x, 400.0, pos.y));
        self.thickness = 160.0;
        self.hollow = true;
        self.set_color(Color::srgba_u8(0, 0, 0, 230));
        self.circle(300.0)
    }
}

pub fn placeholder_click_handler()
-> impl Fn(On<Pointer<Click>>, ResMut<NextState<HexMapToolState>>) {
    move |_trigger, mut next_state| {
        next_state.set(HexMapToolState::Selection);
    }
}

#[derive(Event)]
struct DespawnDial;

fn despawn_dial(mut commands: Commands) {
    commands.trigger(DespawnDial);
}

fn on_despawn_dial(
    _trigger: On<DespawnDial>,
    mut commands: Commands,
    dials: Query<(Entity, &DialController)>,
    dial_bgs: Query<Entity, With<DialBgMarker>>,
    proj: Single<&Projection, With<MainCamera>>,
) {
    for entity in dial_bgs.iter() {
        commands.entity(entity).despawn();
    }
    for (entity, dial_marker) in dials.iter() {
        if let Projection::Orthographic(proj) = *proj {
            let scale = (dial_marker.calc_scale)(proj.scale);
            let tween = bevy_tweening::Tween::new(
                EaseFunction::ElasticIn,
                Duration::from_millis(300),
                TransformScaleLens {
                    start: Vec3::splat(scale),
                    end: Vec3::ZERO,
                },
            );
            commands
                .entity(entity)
                .insert(Animator::new(tween))
                .insert(DialOff);
        }
    }
}

fn mouse_over()
-> impl Fn(On<Pointer<Over>>, Commands, Single<Entity, With<PrimaryWindow>>, Query<&Children>)
{
    move |trigger, mut commands, window, children| {
        commands
            .entity(*window)
            .insert(CursorIcon::System(SystemCursorIcon::Pointer));
        children
            .iter_descendants(trigger.original_event_target())
            .for_each(|entity| {
                let tween = bevy_tweening::Tween::new(
                    EaseFunction::QuadraticOut,
                    Duration::from_millis(100),
                    TransformScaleLens {
                        start: Vec3::splat(1.0),
                        end: Vec3::splat(1.4),
                    },
                );
                commands
                    .entity(entity)
                    .insert(bevy_tweening::Animator::new(tween));
            });
    }
}

fn mouse_out()
-> impl Fn(On<Pointer<Out>>, Commands, Single<Entity, With<PrimaryWindow>>, Query<&Children>) {
    move |trigger, mut commands, window, children| {
        commands
            .entity(*window)
            .insert(CursorIcon::System(SystemCursorIcon::Default));
        children
            .iter_descendants(trigger.original_event_target())
            .for_each(|entity| {
                let tween = bevy_tweening::Tween::new(
                    EaseFunction::QuadraticIn,
                    Duration::from_millis(100),
                    TransformScaleLens {
                        start: Vec3::splat(1.4),
                        end: Vec3::splat(1.0),
                    },
                );
                commands
                    .entity(entity)
                    .insert(bevy_tweening::Animator::new(tween));
            });
    }
}

fn dial_zoom_controller(
    mut commands: Commands,
    proj: Single<&Projection, With<MainCamera>>,
    mut dial: Query<
        (
            Entity,
            &mut Transform,
            &bevy_tweening::Animator<Transform>,
            &DialController,
        ),
        (Without<DialOff>, With<DialController>),
    >,
    mut next_state: ResMut<NextState<HexMapToolState>>,
) {
    if let Some((entity, mut t, animator, dial_marker)) = dial.iter_mut().next() {
        if animator.tweenable().progress() == 1.0 {
            if let Projection::Orthographic(proj) = *proj {
                let scale = (dial_marker.calc_scale)(proj.scale);
                t.scale = Vec3::splat(scale);

                if !(dial_marker.is_visible)(proj.scale) {
                    next_state.set(HexMapToolState::Selection);
                    let tween = bevy_tweening::Tween::new(
                        EaseFunction::ElasticIn,
                        Duration::from_millis(300),
                        TransformScaleLens {
                            start: Vec3::splat(scale),
                            end: Vec3::ZERO,
                        },
                    );
                    commands
                        .entity(entity)
                        .insert(Animator::new(tween))
                        .insert(DialOff);
                }
            }
        }
    }
}

#[derive(Component, PartialEq, Default)]
#[component(on_replace = on_dial_button_state_replaced)]
#[component(on_add = on_dial_button_state_added)]
pub enum DialButtonState {
    #[default]
    Enabled,
    Disabled,
}

fn apply_dial_button_state(
    world: &mut DeferredWorld,
    entity: Entity,
    pickable_block_lower: bool,
    pickable_hoverable: bool,
    opacity_from: f32,
    opacity_to: f32,
) {
    if let Some(children) = world.entity(entity).get_components::<&Children>() {
        let image_entities: Vec<Entity> = children
            .iter()
            .filter(|v| world.entity(*v).contains::<DialButtonIcon>())
            .collect();

        world.commands().entity(entity).try_insert(Pickable {
            should_block_lower: pickable_block_lower,
            is_hoverable: pickable_hoverable,
        });

        for image_entity in image_entities {
            world.commands().entity(image_entity).try_insert(
                bevy_tweening::AssetAnimator::new(bevy_tweening::Tween::new(
                    EaseFunction::QuadraticOut,
                    Duration::from_millis(300),
                    StandardMaterialOpacityLens {
                        from: opacity_from,
                        to: opacity_to,
                    },
                )),
            );
        }
    }
}

fn on_dial_button_state_replaced(mut world: DeferredWorld, context: HookContext) {
    let new_state = world
        .entity(context.entity)
        .components::<&DialButtonState>();

    match new_state {
        DialButtonState::Disabled => {
            apply_dial_button_state(&mut world, context.entity, true, true, 0.1, 1.0)
        }
        DialButtonState::Enabled => {
            apply_dial_button_state(&mut world, context.entity, false, false, 1.0, 0.1)
        }
    }
}

fn on_dial_button_state_added(mut world: DeferredWorld, context: HookContext) {
    let new_state = world
        .entity(context.entity)
        .components::<&DialButtonState>();

    match new_state {
        DialButtonState::Enabled => {
            apply_dial_button_state(&mut world, context.entity, true, true, 0.1, 1.0)
        }
        DialButtonState::Disabled => {
            apply_dial_button_state(&mut world, context.entity, false, false, 1.0, 0.1)
        }
    }
}

#[derive(Component)]
struct DialController {
    calc_scale: fn(f32) -> f32,
    is_visible: fn(f32) -> bool,
}

#[derive(Component)]
struct DialBgMarker;

#[derive(Component)]
struct DialOff;

#[derive(Resource)]
struct DialCoreAssets {
    bg_mesh: Handle<Mesh>,
}

impl DialCoreAssets {
    fn new(bg_mesh: Handle<Mesh>) -> Self {
        DialCoreAssets { bg_mesh }
    }
}
