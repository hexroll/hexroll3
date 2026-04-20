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

use bevy::prelude::*;

use bevy::ecs::{lifecycle::HookContext, world::DeferredWorld};
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

use crate::shared::tweens::{UiImageNodeAlphaLens, UiTransformScaleLens};

#[derive(Component, PartialEq, Default)]
#[component(on_insert = on_menu_button_disabled)]
#[component(on_remove = on_menu_button_enabled)]
pub struct MenuButtonDisabled;

fn on_menu_button_disabled(mut world: DeferredWorld, context: HookContext) {
    if let Some(children) = world.entity(context.entity).get_components::<&Children>() {
        let image_entities: Vec<Entity> = if children
            .iter()
            .any(|v| world.entity(v).contains::<MenuButtonSwitcherIconShown>())
        {
            children
                .iter()
                .filter(|v| {
                    world.entity(*v).contains::<ImageNode>()
                        && world.entity(*v).contains::<MenuButtonSwitcherIconShown>()
                })
                .collect()
        } else {
            children
                .iter()
                .filter(|v| world.entity(*v).contains::<ImageNode>())
                .collect()
        };
        for image_entity in image_entities {
            world
                .commands()
                .entity(image_entity)
                .insert(bevy_tweening::Animator::new(bevy_tweening::Tween::new(
                    EaseFunction::QuarticOut,
                    Duration::from_millis(300),
                    UiImageNodeAlphaLens { from: 1.0, to: 0.1 },
                )));
        }
    }
}

fn on_menu_button_enabled(mut world: DeferredWorld, context: HookContext) {
    if let Some(children) = world.entity(context.entity).get_components::<&Children>() {
        let image_entities: Vec<Entity> = if children
            .iter()
            .any(|v| world.entity(v).contains::<MenuButtonSwitcherIconShown>())
        {
            children
                .iter()
                .filter(|v| {
                    world.entity(*v).contains::<ImageNode>()
                        && world.entity(*v).contains::<MenuButtonSwitcherIconShown>()
                })
                .collect()
        } else {
            children
                .iter()
                .filter(|v| world.entity(*v).contains::<ImageNode>())
                .collect()
        };
        for image_entity in image_entities {
            world
                .commands()
                .entity(image_entity)
                .insert(bevy_tweening::Animator::new(bevy_tweening::Tween::new(
                    EaseFunction::QuarticIn,
                    Duration::from_millis(300),
                    UiImageNodeAlphaLens { from: 0.1, to: 1.0 },
                )));
        }
    }
}

pub trait MenuButtonEffects {
    fn menu_button_hover_effect(&mut self) -> &mut Self;
}

impl MenuButtonEffects for EntityCommands<'_> {
    fn menu_button_hover_effect(&mut self) -> &mut Self {
        self.observe(
            |mut trigger: On<Pointer<Over>>,
             mut commands: Commands,
             button_disabled: Query<&MenuButtonDisabled>,
             children: Query<&Children>,
             window: Single<Entity, With<PrimaryWindow>>| {
                trigger.propagate(false);
                if button_disabled.contains(trigger.entity) {
                    return;
                }
                commands
                    .entity(*window)
                    .insert(CursorIcon::System(SystemCursorIcon::Pointer));
                children
                    .iter_descendants(trigger.original_event_target())
                    .for_each(|entity| {
                        let tween = bevy_tweening::Tween::new(
                            EaseFunction::QuadraticOut,
                            Duration::from_millis(100),
                            UiTransformScaleLens {
                                start: Vec2::splat(1.0),
                                end: Vec2::splat(1.4),
                            },
                        );
                        commands
                            .entity(entity)
                            .insert(bevy_tweening::Animator::new(tween));
                    });
            },
        );
        self.observe(
            |mut trigger: On<Pointer<Out>>,
             mut commands: Commands,
             button_disabled: Query<&MenuButtonDisabled>,
             children: Query<&Children>,
             window: Single<Entity, With<PrimaryWindow>>| {
                trigger.propagate(false);
                if button_disabled.contains(trigger.entity) {
                    return;
                }
                commands
                    .entity(*window)
                    .insert(CursorIcon::System(SystemCursorIcon::Default));
                children
                    .iter_descendants(trigger.entity)
                    .for_each(|entity| {
                        let tween = bevy_tweening::Tween::new(
                            EaseFunction::QuadraticIn,
                            Duration::from_millis(100),
                            UiTransformScaleLens {
                                start: Vec2::splat(1.4),
                                end: Vec2::splat(1.0),
                            },
                        );
                        commands
                            .entity(entity)
                            .insert(bevy_tweening::Animator::new(tween));
                    });
            },
        );
        self
    }
}

#[derive(Component, Clone, Debug)]
pub enum MenuButtonSwitcherState {
    Idle,
    Toggled,
}

#[derive(Component, Clone, Debug)]
pub struct MenuButtonSwitcherIconShown;

impl MenuButtonSwitcherState {
    pub fn toggled(&self) -> bool {
        match self {
            MenuButtonSwitcherState::Idle => false,
            MenuButtonSwitcherState::Toggled => true,
        }
    }
}
pub trait Switch {
    fn rotate(&self) -> Self;
    fn index(&self) -> usize;
    fn from_index(index: usize) -> Self;
}
pub trait SwitchValue {
    fn from_value(value: &str) -> Self;
    fn value(&self) -> &str;
}
pub trait MenuButtonSwitcher {
    fn menu_button_switch<T1, T2>(
        &mut self,
        a: Handle<Image>,
        b: Handle<Image>,
        size: f32,
    ) -> &mut Self
    where
        T1: Component + Default,
        T2: Component + Default;

    fn menu_button_switch_ex<T>(
        &mut self,
        state: T,
        b: Vec<Handle<Image>>,
        size: f32,
    ) -> &mut Self
    where
        T: Component + Default + Switch + PartialEq + Clone;
    fn value_switch_button<T>(
        &mut self,
        state: T,
        a: Vec<(&str, Handle<Image>)>,
        size: f32,
    ) -> &mut Self
    where
        T: Component + Default + SwitchValue + PartialEq + Clone;
}

#[derive(EntityEvent)]
pub struct ToggleButtonSwitcher {
    pub state: MenuButtonSwitcherState,
    pub entity: Entity,
}

#[derive(Component)]
pub struct SwitchIcon;

#[derive(EntityEvent)]
pub struct ToggleButtonSwitcherEx {
    pub entity: Entity,
    pub trigger_state_as_event: bool,
    pub insert_state_as_resource: bool,
}

#[derive(Event)]
pub struct ToggleEventWrapper<T> {
    pub value: T,
}

#[derive(Resource)]
pub struct ToggleResourceWrapper<T> {
    pub value: T,
}

pub fn rotate_key<'a>(keys: &'a [String], current: &str) -> Option<&'a str> {
    let i = keys.iter().position(|k| *k == current)?;
    Some(&keys[(i + 1) % keys.len()])
}

impl MenuButtonSwitcher for EntityCommands<'_> {
    fn value_switch_button<T>(
        &mut self,
        state: T,
        a: Vec<(&str, Handle<Image>)>,
        size: f32,
    ) -> &mut Self
    where
        T: Component + Default + SwitchValue + PartialEq + Clone,
    {
        self.insert(state.clone());
        let keys: Vec<String> = a
            .iter()
            .map(|(key, _)| key.to_string())
            .collect::<Vec<String>>();

        for (index, img) in a.iter() {
            self.with_child((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(size),
                    height: Val::Px(size),
                    align_self: AlignSelf::Center,
                    ..default()
                },
                T::from_value(index),
                SwitchIcon,
                ImageNode {
                    color: Color::WHITE.with_alpha(if T::from_value(index) == state {
                        1.0
                    } else {
                        0.0
                    }),
                    image: img.clone(),
                    image_mode: NodeImageMode::Auto,
                    ..default()
                },
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
            ));
        }
        self.observe(
            move |trigger: On<ToggleButtonSwitcherEx>,
                  mut icons: Query<(Entity, &T), With<SwitchIcon>>,
                  current_states: Query<&T>,
                  mut commands: Commands| {
                let Ok(current_state) = current_states.get(trigger.entity) else {
                    return;
                };
                let keys = keys.clone();

                let next_state = rotate_key(keys.as_slice(), current_state.value()).unwrap();

                for (icon, icon_state) in icons.iter_mut() {
                    if *icon_state.value() == *next_state {
                        commands.entity(icon).insert(MenuButtonSwitcherIconShown);
                        commands.entity(icon).insert(bevy_tweening::Animator::new(
                            bevy_tweening::Tween::new(
                                EaseFunction::CubicOut,
                                std::time::Duration::from_millis(200),
                                UiImageNodeAlphaLens { from: 0.0, to: 1.0 },
                            ),
                        ));
                    }
                    if icon_state.value() == current_state.value() {
                        commands.entity(icon).insert(MenuButtonSwitcherIconShown);
                        commands.entity(icon).insert(bevy_tweening::Animator::new(
                            bevy_tweening::Tween::new(
                                EaseFunction::CubicIn,
                                std::time::Duration::from_millis(200),
                                UiImageNodeAlphaLens { from: 1.0, to: 0.0 },
                            ),
                        ));
                    }
                }
                if trigger.trigger_state_as_event {
                    commands.trigger(ToggleEventWrapper::<T> {
                        value: T::from_value(next_state),
                    });
                }
                if trigger.insert_state_as_resource {
                    commands.insert_resource(ToggleResourceWrapper::<T> {
                        value: T::from_value(next_state),
                    });
                }
                commands
                    .entity(trigger.entity)
                    .insert(T::from_value(next_state));
            },
        );
        self
    }

    fn menu_button_switch_ex<T>(
        &mut self,
        state: T,
        a: Vec<Handle<Image>>,
        size: f32,
    ) -> &mut Self
    where
        T: Component + Default + Switch + PartialEq + Clone,
    {
        self.insert(state.clone());
        for (index, img) in a.iter().enumerate() {
            self.with_child((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(size),
                    height: Val::Px(size),
                    align_self: AlignSelf::Center,
                    ..default()
                },
                T::from_index(index),
                SwitchIcon,
                ImageNode {
                    color: Color::WHITE.with_alpha(if T::from_index(index) == state {
                        1.0
                    } else {
                        0.0
                    }),
                    image: img.clone(),
                    image_mode: NodeImageMode::Auto,
                    ..default()
                },
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
            ));
        }
        self.observe(
            |trigger: On<ToggleButtonSwitcherEx>,
             mut icons: Query<(Entity, &T), With<SwitchIcon>>,
             current_states: Query<&T>,
             mut commands: Commands| {
                let Ok(current_state) = current_states.get(trigger.entity) else {
                    return;
                };

                let next_state = current_state.rotate();

                for (icon, icon_state) in icons.iter_mut() {
                    if *icon_state == next_state {
                        commands.entity(icon).insert(MenuButtonSwitcherIconShown);
                        commands.entity(icon).insert(bevy_tweening::Animator::new(
                            bevy_tweening::Tween::new(
                                EaseFunction::CubicOut,
                                std::time::Duration::from_millis(200),
                                UiImageNodeAlphaLens { from: 0.0, to: 1.0 },
                            ),
                        ));
                    }
                    if icon_state == current_state {
                        commands.entity(icon).insert(MenuButtonSwitcherIconShown);
                        commands.entity(icon).insert(bevy_tweening::Animator::new(
                            bevy_tweening::Tween::new(
                                EaseFunction::CubicIn,
                                std::time::Duration::from_millis(200),
                                UiImageNodeAlphaLens { from: 1.0, to: 0.0 },
                            ),
                        ));
                    }
                }
                if trigger.trigger_state_as_event {
                    commands.trigger(ToggleEventWrapper::<T> {
                        value: next_state.clone(),
                    });
                }
                if trigger.insert_state_as_resource {
                    commands.insert_resource(ToggleResourceWrapper::<T> {
                        value: next_state.clone(),
                    });
                }
                commands.entity(trigger.entity).insert(next_state);
            },
        );
        self
    }

    fn menu_button_switch<T1, T2>(
        &mut self,
        a: Handle<Image>,
        b: Handle<Image>,
        size: f32,
    ) -> &mut Self
    where
        T1: Component + Default,
        T2: Component + Default,
    {
        self.with_child((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(size),
                height: Val::Px(size),
                align_self: AlignSelf::Center,
                ..default()
            },
            T1::default(),
            ImageNode {
                color: Color::WHITE.with_alpha(1.0),
                image: a,
                image_mode: NodeImageMode::Auto,
                ..default()
            },
            Pickable {
                should_block_lower: false,
                is_hoverable: false,
            },
            MenuButtonSwitcherIconShown,
        ));
        self.with_child((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(size),
                height: Val::Px(size),
                align_self: AlignSelf::Center,
                ..default()
            },
            T2::default(),
            ImageNode {
                color: Color::WHITE.with_alpha(0.0),
                image: b,
                image_mode: NodeImageMode::Auto,
                ..default()
            },
            Pickable {
                should_block_lower: false,
                is_hoverable: false,
            },
        ));
        self.observe(
            |trigger: On<ToggleButtonSwitcher>,
             mut icon_node_off: Query<Entity, (With<T1>, Without<T2>)>,
             mut icon_node_on: Query<Entity, (With<T2>, Without<T1>)>,
             mut commands: Commands| {
                commands
                    .entity(trigger.entity)
                    .insert(trigger.state.clone());
                let state = &trigger.state;
                if let Some(icon_node_entity) = icon_node_on.iter_mut().next() {
                    if state.toggled() {
                        commands
                            .entity(icon_node_entity)
                            .insert(MenuButtonSwitcherIconShown);
                    } else {
                        commands
                            .entity(icon_node_entity)
                            .remove::<MenuButtonSwitcherIconShown>();
                    }
                    commands
                        .entity(icon_node_entity)
                        .insert(bevy_tweening::Animator::new(bevy_tweening::Tween::new(
                            if state.toggled() {
                                EaseFunction::CubicOut
                            } else {
                                EaseFunction::CubicIn
                            },
                            std::time::Duration::from_millis(200),
                            if state.toggled() {
                                UiImageNodeAlphaLens { from: 0.0, to: 1.0 }
                            } else {
                                UiImageNodeAlphaLens { from: 1.0, to: 0.0 }
                            },
                        )));
                }
                if let Some(icon_node_entity) = icon_node_off.iter_mut().next() {
                    if state.toggled() {
                        commands
                            .entity(icon_node_entity)
                            .remove::<MenuButtonSwitcherIconShown>();
                    } else {
                        commands
                            .entity(icon_node_entity)
                            .insert(MenuButtonSwitcherIconShown);
                    }
                    commands
                        .entity(icon_node_entity)
                        .insert(bevy_tweening::Animator::new(bevy_tweening::Tween::new(
                            if state.toggled() {
                                EaseFunction::CubicIn
                            } else {
                                EaseFunction::CubicOut
                            },
                            std::time::Duration::from_millis(200),
                            if !state.toggled() {
                                UiImageNodeAlphaLens { from: 0.0, to: 1.0 }
                            } else {
                                UiImageNodeAlphaLens { from: 1.0, to: 0.0 }
                            },
                        )));
                }
            },
        );
        self
    }
}
