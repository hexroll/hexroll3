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

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    input_focus::InputFocus,
    prelude::*,
};
use bevy_simple_scroll_view::{ScrollTarget, ScrollableContent};

use super::cursor::PointerOnHover;

pub struct ListPlugin;
impl Plugin for ListPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_keyboard_events);
    }
}

pub trait SelectableListItem {
    fn make_selectable(&mut self) -> &mut Self;
}

impl SelectableListItem for EntityCommands<'_> {
    fn make_selectable(&mut self) -> &mut Self {
        self.observe(move |trigger: On<Pointer<Click>>, mut commands: Commands| {
            commands
                .entity(trigger.entity)
                .trigger(|entity| ItemSelected { entity });
        })
        .pointer_on_hover()
        .observe(
            move |_: On<Pointer<Move>>,
                  mut commands: Commands,
                  keyboard_selection: Query<Entity, With<KeyboardSelection>>| {
                for e in keyboard_selection.iter() {
                    commands.entity(e).try_remove::<KeyboardSelection>();
                }
            },
        )
        .observe(
            move |t: On<Pointer<Over>>,
                  mut commands: Commands,
                  keyboard_selection: Query<Entity, With<KeyboardSelection>>| {
                if !keyboard_selection.is_empty() {
                    return;
                }
                commands.entity(t.entity).try_insert(MouseSelection);
            },
        )
        .observe(move |t: On<Pointer<Out>>, mut commands: Commands| {
            commands.entity(t.entity).try_remove::<MouseSelection>();
        })
    }
}

#[derive(EntityEvent)]
pub struct ItemSelected {
    #[event_target]
    pub entity: Entity,
}

#[derive(EntityEvent)]
pub struct ListDismissed {
    #[event_target]
    pub entity: Entity,
    pub submit: bool,
}

#[derive(Component, PartialEq, Default)]
#[component(on_insert = on_selection)]
#[component(on_remove = on_deselection)]
pub struct MouseSelection;

#[derive(Component, PartialEq, Default)]
#[component(on_insert = on_selection)]
#[component(on_remove = on_deselection)]
pub struct KeyboardSelection(i32);

#[derive(Component, Default)]
pub struct SelectableItemsContainer {
    pub children: Vec<Entity>,
}

fn on_selection(mut world: DeferredWorld, context: HookContext) {
    // This should be a message to the widget to set its own style
    world
        .commands()
        .entity(context.entity)
        .try_insert(BackgroundColor::from(Srgba::new(1.0, 0.0, 0.0, 0.1)));
}

fn on_deselection(mut world: DeferredWorld, context: HookContext) {
    world
        .commands()
        .entity(context.entity)
        .try_insert(BackgroundColor::from(Srgba::new(1.0, 0.0, 0.0, 0.01)));
}

pub fn handle_keyboard_events(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    list: Single<(
        &SelectableItemsContainer,
        Entity,
        &ScrollableContent,
        &ChildOf,
    )>,
    selection: Query<(Entity, &KeyboardSelection), Without<MouseSelection>>,
    unselection: Query<Entity, With<MouseSelection>>,
    global_transforms: Query<&UiGlobalTransform>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        commands.entity(list.1).trigger(|entity| ListDismissed {
            entity,
            submit: false,
        });
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        if let Some(entity_to_select) = {
            if let Some(e) = selection.iter().next() {
                Some(e.0)
            } else if let Some(e) = unselection.iter().next() {
                Some(e)
            } else {
                None
            }
        } {
            commands
                .entity(entity_to_select)
                .trigger(|entity| ItemSelected { entity });
        }
        commands.entity(list.1).trigger(|entity| ListDismissed {
            entity,
            submit: true,
        });
    }
    let mut index_modifier: i32 = 0;
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        index_modifier = 1;
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        index_modifier = -1;
    }
    if index_modifier != 0 {
        commands.insert_resource(InputFocus(None));
        // Preserve and clear mouse selection is one existed
        let mouse_selection = if let Some(e) = unselection.iter().next() {
            commands.entity(e).try_remove::<MouseSelection>();
            list.0.children.iter().enumerate().find(|(_, v)| **v == e)
        } else {
            None
        };
        if let Some((current_entity, current_index)) = if selection.is_empty() {
            // Set initial selection based on whether or not we already had a previous
            // selection from using the mouse
            if let Some((mouse_selection_index, mouse_selection_entity)) = mouse_selection {
                commands
                    .entity(*mouse_selection_entity)
                    .try_insert(KeyboardSelection(mouse_selection_index as i32));
                Some((*mouse_selection_entity, mouse_selection_index as i32))
            } else {
                if let Some(first) = list.0.children.iter().next() {
                    commands.entity(*first).try_insert(KeyboardSelection(0));
                }
                None
            }
        } else {
            if let Some((selected_entity, selected)) = selection.iter().next() {
                Some((selected_entity, selected.0))
            } else {
                None
            }
        } {
            if let Some(next) = list
                .0
                .children
                .get((current_index + index_modifier) as usize)
            {
                if let Ok(panel_global_transform) = global_transforms.get(list.3.0) {
                    if let Ok(global_transform) = global_transforms.get(*next) {
                        commands.entity(list.1).try_insert(ScrollTarget::from_value(
                            list.2.pos_y - global_transform.translation.y
                                + panel_global_transform.translation.y,
                            list.2.max_scroll,
                        ));
                    }
                }
                commands
                    .entity(*next)
                    .try_insert(KeyboardSelection(current_index + index_modifier));
                commands
                    .entity(current_entity)
                    .try_remove::<KeyboardSelection>();
            }
        }
    }
}
