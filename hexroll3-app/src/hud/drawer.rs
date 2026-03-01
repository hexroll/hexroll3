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

// Drawers are HUD widgets that auto-expand then hovered-on
use bevy::{picking::hover::PickingInteraction, prelude::*};
use bevy_tweening::lens::UiBackgroundColorLens;

use crate::shared::{
    tweens::{UiNodeSizeLens, UiNodeSizeLensMode},
    widgets::cursor::PointerExclusivityIsPreferred,
};

pub struct DrawerPlugin;

impl Plugin for DrawerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drawers_size_enforcer)
            .add_systems(Update, despawn_drawers)
            .add_systems(Update, update_picking_drawers);
    }
}

#[derive(EntityEvent)]
pub struct AutoDrawerOpened {
    #[event_target]
    entity: Entity,
}

#[derive(EntityEvent)]
pub struct AutoDrawerClosed {
    #[event_target]
    entity: Entity,
}

#[derive(Component, PartialEq)]
pub enum AutoDrawerVisiblity {
    VisibleToAll,
    VisibleToRefereeOnly,
    VisibleToPlayerOnly,
}

impl AutoDrawerVisiblity {
    pub fn player_restricted(&self) -> bool {
        *self == Self::VisibleToRefereeOnly
    }
    pub fn referee_restricted(&self) -> bool {
        *self == Self::VisibleToPlayerOnly
    }
}

#[derive(Component)]
pub struct DrawerIsHoveredOn;

#[derive(Component)]
pub struct AutoDrawerButManual;

#[derive(Component)]
struct AutoDrawerDespawnTimer(Timer);

#[derive(Component, PartialEq)]
pub enum AutoDrawerCommand {
    On,
    Off,
}

#[derive(Component)]
pub struct AutoDrawerSensor;

#[derive(Component, Default)]
pub struct AutoDrawer {
    pub is_open: bool,
    pub is_closing: bool,
    pub timer: f32,
    pub keep_open: bool,
    pub closed_size: Vec2,
    pub opened_size: Vec2,
    pub closed_background: Color,
    pub open_background: Color,
    pub despawn_on_close: bool,
    pub max_mode: bool,
    pub related_to: Option<Entity>,
    fade_out_children_on_close: bool,
    secs_to_close: f32,
}

impl AutoDrawer {
    pub fn new(
        closed_size: Vec2,
        opened_size: Vec2,
        closed_background: Color,
        open_background: Color,
    ) -> Self {
        Self {
            is_open: false,
            is_closing: false,
            timer: 0.0,
            keep_open: false,
            despawn_on_close: false,
            max_mode: false,
            related_to: None,
            closed_size,
            opened_size,
            closed_background,
            open_background,
            fade_out_children_on_close: false,
            secs_to_close: 1.0,
        }
    }

    pub fn despawn_on_close(mut self) -> Self {
        self.despawn_on_close = true;
        self
    }

    pub fn max_mode(mut self) -> Self {
        self.max_mode = true;
        self
    }

    pub fn with_related(mut self, entity: Entity) -> Self {
        self.related_to = Some(entity);
        self
    }

    pub fn with_fade_out_children_on_close(mut self) -> Self {
        self.fade_out_children_on_close = true;
        self
    }

    pub fn with_secs_to_close(mut self, secs: f32) -> Self {
        self.secs_to_close = secs;
        self
    }
}

// Adding this components to an drawer will prevent it from closing until this
// component is removed by its related_to sibling
#[derive(Component)]
pub struct HasRelatedDrawer;

fn drawers_size_enforcer(
    mut commands: Commands,
    mut node: Query<(
        Entity,
        Option<&AutoDrawerCommand>,
        &mut AutoDrawer,
        Option<&HasRelatedDrawer>,
    )>,
    time: Res<Time>,
    children: Query<&Children>,
    manual_overrides: Query<&AutoDrawerButManual>,
) {
    for (entity, drawer_command, mut menu_state, related) in node.iter_mut() {
        let temp = |menu_state: &mut AutoDrawer, commands: &mut Commands| {
            menu_state.is_open = true;
            menu_state.is_closing = false;
            menu_state.timer = 0.0;
            let node_size_tween = bevy_tweening::Tween::new(
                EaseFunction::QuarticOut,
                std::time::Duration::from_millis(200),
                UiNodeSizeLens {
                    mode: if menu_state.max_mode {
                        UiNodeSizeLensMode::MaxHeight
                    } else {
                        UiNodeSizeLensMode::Both
                    },
                    start: menu_state.closed_size,
                    end: menu_state.opened_size,
                },
            );
            let background_color_tween = bevy_tweening::Tween::new(
                EaseFunction::QuarticOut,
                std::time::Duration::from_millis(200),
                UiBackgroundColorLens {
                    start: menu_state.closed_background,
                    end: menu_state.open_background,
                },
            );
            commands
                .entity(entity)
                .try_insert(bevy_tweening::Animator::new(node_size_tween))
                .try_insert(bevy_tweening::Animator::new(background_color_tween));
        };

        let manual_override = manual_overrides.contains(entity);
        menu_state.keep_open = manual_override;

        if let Some(drawer_command) = drawer_command {
            if *drawer_command == AutoDrawerCommand::Off
                && menu_state.is_open
                && !menu_state.is_closing
                && !menu_state.keep_open
            {
                if related.is_none() {
                    menu_state.timer = menu_state.secs_to_close;
                    menu_state.is_closing = true;
                }
            } else if (*drawer_command == AutoDrawerCommand::On || menu_state.keep_open)
                && !menu_state.is_open
            {
                temp(&mut menu_state, &mut commands);
            } else if *drawer_command == AutoDrawerCommand::On || menu_state.keep_open {
                menu_state.is_open = true;
                menu_state.is_closing = false;
                menu_state.timer = 0.0;
                commands
                    .entity(entity)
                    .trigger(|entity| AutoDrawerOpened { entity });
            }
        } else if menu_state.keep_open && !menu_state.is_open {
            temp(&mut menu_state, &mut commands);
        }

        if menu_state.is_closing {
            if menu_state.timer > 0.0 {
                menu_state.timer -= time.delta_secs();
            } else {
                let node_size_tween = bevy_tweening::Tween::new(
                    EaseFunction::QuarticOut,
                    std::time::Duration::from_millis(200),
                    UiNodeSizeLens {
                        mode: if menu_state.max_mode {
                            UiNodeSizeLensMode::MaxHeight
                        } else {
                            UiNodeSizeLensMode::Both
                        },
                        start: menu_state.opened_size,
                        end: menu_state.closed_size,
                    },
                );
                let bg_color_tween = bevy_tweening::Tween::new(
                    EaseFunction::QuarticOut,
                    std::time::Duration::from_millis(200),
                    UiBackgroundColorLens {
                        start: menu_state.open_background,
                        end: menu_state.closed_background,
                    },
                );

                if menu_state.fade_out_children_on_close {
                    children.iter_descendants(entity).for_each(|child_entity| {
                        commands.entity(child_entity).try_insert(
                            bevy_tweening::Animator::new(bevy_tweening::Tween::new(
                                EaseFunction::CubicOut,
                                std::time::Duration::from_millis(200),
                                crate::shared::tweens::UiImageNodeAlphaLens {
                                    from: 1.0,
                                    to: 0.0,
                                },
                            )),
                        );
                    });
                }

                commands
                    .entity(entity)
                    .try_insert_if(
                        AutoDrawerDespawnTimer(Timer::from_seconds(0.2, TimerMode::Once)),
                        || menu_state.despawn_on_close,
                    )
                    .try_insert(bevy_tweening::Animator::new(node_size_tween))
                    .try_insert(bevy_tweening::Animator::new(bg_color_tween));
                menu_state.is_open = false;
                menu_state.is_closing = false;
                if let Some(related_entity) = menu_state.related_to {
                    commands
                        .entity(related_entity)
                        .try_remove::<AutoDrawerButManual>();
                }
                commands
                    .entity(entity)
                    .trigger(|entity| AutoDrawerClosed { entity });
            }
        }
    }
}

fn update_picking_drawers(
    mut commands: Commands,
    sensor: Query<(Entity, Option<&PickingInteraction>, &ChildOf), With<AutoDrawerSensor>>,
) {
    for (sensor, picking, child_of) in sensor.iter() {
        if let Some(picking) = picking {
            match picking {
                PickingInteraction::Pressed | PickingInteraction::Hovered => {
                    commands
                        .entity(child_of.0)
                        .try_remove::<AutoDrawerButManual>();
                    commands
                        .entity(child_of.0)
                        .try_insert(AutoDrawerCommand::On)
                        .try_insert(PointerExclusivityIsPreferred)
                        .try_insert(DrawerIsHoveredOn);
                }
                PickingInteraction::None => {
                    commands
                        .entity(child_of.0)
                        .try_insert(AutoDrawerCommand::Off)
                        .try_remove::<PointerExclusivityIsPreferred>()
                        .try_remove::<DrawerIsHoveredOn>();
                }
            }
        } else {
            commands.entity(sensor).try_insert(PickingInteraction::None);
            commands
                .entity(child_of.0)
                .try_remove::<AutoDrawerCommand>();
        }
    }
}

fn despawn_drawers(
    mut commands: Commands,
    mut drawers_to_despawn: Query<(Entity, &AutoDrawer, &mut AutoDrawerDespawnTimer)>,
    time: Res<Time>,
) {
    for (entity, ex, mut timer) in drawers_to_despawn.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.is_finished() {
            if let Some(related_to) = ex.related_to {
                commands.entity(related_to).try_remove::<HasRelatedDrawer>();
            }
            commands.entity(entity).despawn();
        }
    }
}

pub fn make_auto_drawer_sensor() -> impl Bundle {
    (
        Name::new("AutoDrawerSensor"),
        AutoDrawerSensor,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(0.0),
            top: Val::Percent(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        Pickable {
            should_block_lower: false,
            is_hoverable: true,
        },
    )
}
