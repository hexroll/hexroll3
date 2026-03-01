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
    prelude::*,
    window::{CursorIcon, PrimaryWindow, SystemCursorIcon},
};

use super::cursor::PointerExclusivityIsPreferred;

pub struct ModalPlugin;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, States)]
pub enum DiscreteAppState {
    #[default]
    Normal,
    Modal,
}

impl Plugin for ModalPlugin {
    fn build(&self, app: &mut App) {
        app.insert_state(DiscreteAppState::default())
            .add_systems(OnEnter(DiscreteAppState::Modal), on_entering_modal_state)
            .add_systems(OnExit(DiscreteAppState::Modal), on_exiting_modal_state)
            .add_systems(
                Update,
                on_modal_keys.run_if(in_state(DiscreteAppState::Modal)),
            );
    }
}

#[derive(Component)]
pub struct ModalWindow;

#[derive(Component)]
struct Dimmer;

fn on_entering_modal_state(mut commands: Commands, dimmer: Query<Entity, With<Dimmer>>) {
    if dimmer.is_empty() {
        commands
            .spawn((
                Name::new("Modal Dimmer"),
                Dimmer,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ZIndex(998),
                BackgroundColor(Color::srgba_u8(0, 0, 0, 200)),
                PointerExclusivityIsPreferred,
            ))
            .observe(
                |_: On<Pointer<Click>>,
                 mut next_state: ResMut<NextState<DiscreteAppState>>| {
                    next_state.set(DiscreteAppState::Normal);
                },
            );
    }
}

fn on_exiting_modal_state(
    mut commands: Commands,
    modals: Query<Entity, With<ModalWindow>>,
    dimmer: Query<Entity, With<Dimmer>>,
    window: Single<Entity, With<PrimaryWindow>>,
) {
    for modal in modals.iter() {
        commands.entity(modal).try_despawn();
    }
    for dimmer in dimmer.iter() {
        commands.entity(dimmer).try_despawn();
    }
    // NOTE: This is for when we existed the modal via clicking
    // a button, and the cursor was modified when the button
    // was hovered on.
    commands
        .entity(*window)
        .insert(CursorIcon::System(SystemCursorIcon::Default));
}

fn on_modal_keys(
    key: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<DiscreteAppState>>,
) {
    if key.just_pressed(KeyCode::Escape) {
        next_state.set(DiscreteAppState::Normal);
    }
}
