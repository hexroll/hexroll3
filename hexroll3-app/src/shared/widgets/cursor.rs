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

// Common Cursor Behaviors
//
// Provides two behaviors that can be applied using entity commands:
// `pointer_on_hover` will change the pointer on links and similar entities.
// `tooltip_on_hover` will show a tooltip on menu options or links.
use bevy::{
    prelude::*,
    window::{CursorIcon, PrimaryWindow, SystemCursorIcon},
};

use crate::hexmap::elements::MainCamera;

#[derive(Component)]
// Use this component to indicate your widget or ui element prefers
// having pointer exclusivity, blocking other ui elements from responding
// to events.
pub struct PointerExclusivityIsPreferred;

pub trait PointerOnHover {
    fn pointer_on_hover(&mut self) -> &mut Self;
    fn custom_pointer_on_hover(&mut self, cursor_icon: SystemCursorIcon) -> &mut Self;
    fn prefer_pointer_exclusivity(&mut self) -> &mut Self;
}

pub trait TooltipOnHover {
    fn tooltip_on_hover(&mut self, text: &str, timeout: f32) -> &mut Self;
}

impl PointerOnHover for EntityCommands<'_> {
    fn pointer_on_hover(&mut self) -> &mut Self {
        self.custom_pointer_on_hover(SystemCursorIcon::Pointer)
    }
    fn custom_pointer_on_hover(&mut self, cursor_icon: SystemCursorIcon) -> &mut Self {
        self.observe(
            move |_: On<Pointer<Over>>,
                  mut commands: Commands,
                  window: Single<Entity, With<PrimaryWindow>>| {
                commands
                    .entity(*window)
                    .try_insert(CursorIcon::System(cursor_icon));
            },
        );
        self.observe(
            move |_: On<Pointer<Out>>,
                  mut commands: Commands,
                  window: Single<Entity, With<PrimaryWindow>>| {
                commands
                    .entity(*window)
                    .try_insert(CursorIcon::System(SystemCursorIcon::Default));
            },
        );
        self
    }
    fn prefer_pointer_exclusivity(&mut self) -> &mut Self {
        self.observe(move |trigger: On<Pointer<Over>>, mut commands: Commands| {
            commands
                .entity(trigger.entity)
                .try_insert(PointerExclusivityIsPreferred);
        });
        self.observe(move |trigger: On<Pointer<Out>>, mut commands: Commands| {
            commands
                .entity(trigger.entity)
                .try_remove::<PointerExclusivityIsPreferred>();
        });
        self
    }
}

impl TooltipOnHover for EntityCommands<'_> {
    fn tooltip_on_hover(&mut self, text: &str, timeout: f32) -> &mut Self {
        self.insert(Tooltip::from_text(text));
        self.observe(move |trigger: On<Pointer<Over>>, mut commands: Commands| {
            commands
                .entity(trigger.entity)
                .try_insert(TooltipState::Pending(
                    timeout,
                    trigger.pointer_location.position,
                ));
        });
        self.observe(
            move |trigger: On<Pointer<Out>>,
                  mut commands: Commands,
                  tooltip_states: Query<&TooltipState>| {
                if let Ok(tooltip_state) = tooltip_states.get(trigger.entity) {
                    match tooltip_state {
                        TooltipState::Show(entity) => {
                            commands.entity(*entity).try_despawn();
                        }
                        _ => {}
                    }
                }
                commands.entity(trigger.entity).try_remove::<TooltipState>();
            },
        );
        self.observe(
            move |trigger: On<Pointer<Click>>,
                  mut commands: Commands,
                  tooltip_states: Query<&TooltipState>| {
                if let Ok(tooltip_state) = tooltip_states.get(trigger.entity) {
                    match tooltip_state {
                        TooltipState::Show(entity) => {
                            commands.entity(*entity).try_despawn();
                        }
                        _ => {}
                    }
                }
                commands.entity(trigger.entity).try_remove::<TooltipState>();
            },
        );
        self
    }
}

pub fn pointer_world_position(
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) -> Option<Vec3> {
    if let Ok((camera, camera_transform)) = q_camera.single() {
        if let Ok(window) = q_window.single() {
            let Some(cursor_position) = window.cursor_position() else {
                return None;
            };
            let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
                return None;
            };
            let Some(distance) =
                ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))
            else {
                return None;
            };
            return Some(ray.get_point(distance));
        }
    }
    None
}

pub fn tooltips_system(
    mut commands: Commands,
    tooltips: Query<(Entity, &Tooltip, &TooltipState)>,
    window: Single<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
) {
    for (e, tooltip, state) in tooltips.iter() {
        match state {
            TooltipState::Pending(timer, _) => {
                let Some(pos) = window.cursor_position() else {
                    return;
                };
                if *timer > 0.0 {
                    commands
                        .entity(e)
                        .try_insert(TooltipState::Pending(timer - time.delta_secs(), pos));
                } else {
                    let tooltip_entity = commands
                        .spawn((
                            Name::new(format!("Tooltip: {}", tooltip.text)),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(pos.x),
                                top: Val::Px(pos.y + 30.0),
                                padding: UiRect::all(Val::Px(8.0)),
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            Pickable {
                                should_block_lower: false,
                                is_hoverable: false,
                            },
                            BorderRadius::all(Val::Px(5.0)),
                            BackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.95).into()),
                            ZIndex(9999),
                        ))
                        .with_children(|c| {
                            c.spawn((
                                Pickable {
                                    should_block_lower: false,
                                    is_hoverable: false,
                                },
                                Node { ..default() },
                                TextSpan::default(),
                                TextColor::WHITE,
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextLayout { ..default() },
                                Text::new(tooltip.text.clone()),
                            ));
                        })
                        .id();

                    commands
                        .entity(e)
                        .try_insert(TooltipState::Show(tooltip_entity));
                }
            }
            _ => {}
        }
    }
}

#[derive(Component)]
pub enum TooltipState {
    Pending(f32, Vec2),
    Show(Entity),
}

#[derive(Component)]
pub struct Tooltip {
    text: String,
}

impl Tooltip {
    pub fn from_text(text: &str) -> Self {
        Self {
            text: text.to_string(),
        }
    }
}
