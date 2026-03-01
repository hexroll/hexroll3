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
use bevy_mod_billboard::BillboardText;

use crate::{
    dice::{DiceRollHelpers, RollDice},
    hexmap::elements::HexMapResources,
    hud::DiceMessage,
    shared::input::InputMode,
};

pub struct TokenInitiativePlugin;

impl Plugin for TokenInitiativePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_initiative_modifiers_state) // TODO: Consider a state for this
            .add_observer(on_initiative_setup)
            .add_observer(on_initiative_dice);
    }
}

#[derive(Component)]
struct TokenInitiativeLabel;

#[derive(Component)]
struct TokenAwaitingDice(usize);

#[derive(Component)]
struct TokenInitiativeModifier(i32, bool);

#[derive(Component)]
struct TokenMemoizedInitiativeModifier(i32);

#[derive(Component)]
struct TokenInitiativeRoll;

#[derive(Component)]
struct InitiativeSetupTimer(Duration);

impl InitiativeSetupTimer {
    pub fn tick(&self, delta: f32) -> Self {
        let new_duration = (self.0.as_secs_f32() - delta).max(0.0);
        let timer = std::time::Duration::from_secs_f32(new_duration);
        Self(timer)
    }
    pub fn is_done(&self) -> bool {
        self.0.is_zero()
    }
}

#[derive(Component)]
struct TokenInitiativeLabelChild(Entity);

#[derive(Event)]
pub struct InitializeInitiativeSetup(pub Entity);

fn on_initiative_setup(
    trigger: On<InitializeInitiativeSetup>,
    mut commands: Commands,
    modifiers: Query<&TokenInitiativeModifier>,
    memoized_modifiers: Query<&TokenMemoizedInitiativeModifier>,
    key: Res<ButtonInput<KeyCode>>,
    labels: Query<&TokenInitiativeLabelChild>,
) {
    let token_entity = trigger.event().0;
    let mut initiative_label_entity: Option<Entity> = None;
    commands
        .entity(token_entity)
        .insert(InitiativeSetupTimer(Duration::from_secs(3)));

    if !labels.contains(token_entity) {
        commands
            .entity(token_entity)
            .with_children(|c| {
                let e = c
                    .spawn((
                        BillboardText::default(),
                        TokenInitiativeLabel,
                        TextLayout::new_with_justify(Justify::Center),
                        Transform::from_translation(Vec3::new(0.0, 6.0, 0.0))
                            .with_scale(Vec3::splat(0.0025)),
                    ))
                    .id();
                initiative_label_entity = Some(e);
            })
            .insert(TokenInitiativeLabelChild(initiative_label_entity.unwrap()));
    }

    if let Ok(modifier) = modifiers.get(token_entity) {
        let acc = if key.pressed(KeyCode::ShiftLeft) {
            -1
        } else {
            1
        };
        commands
            .entity(token_entity)
            .insert(TokenInitiativeModifier(modifier.0 + acc, false));
    } else {
        let memoized = if let Ok(mm) = memoized_modifiers.get(token_entity) {
            mm.0
        } else {
            0
        };
        commands
            .entity(token_entity)
            .insert(TokenInitiativeModifier(memoized, false));
    }
}

fn update_initiative_modifiers_state(
    mut commands: Commands,
    key: Res<ButtonInput<KeyCode>>,
    timers: Query<(
        Entity,
        &InitiativeSetupTimer,
        &TokenInitiativeLabelChild,
        &TokenInitiativeModifier,
    )>,
    labels: Query<Entity, With<TokenInitiativeLabel>>,
    time: Res<Time>,
    map_resources: Res<HexMapResources>,
    awaiting: Query<&TokenAwaitingDice>,
    unresolved_dice: Query<Entity, With<TokenAwaitingDice>>,
    input_mode: Res<InputMode>,
) {
    if key.just_pressed(KeyCode::KeyI) && input_mode.keyboard_available() {
        for e in labels.iter() {
            commands.entity(e).insert(Visibility::Hidden);
        }
    }
    for (e, t, label_pointer, modifier) in timers.iter() {
        let updated_timer = t.tick(time.delta_secs());
        let label_entity = label_pointer.0;
        if updated_timer.is_done() && unresolved_dice.is_empty() {
            commands.entity(label_entity).insert(Visibility::Hidden);
            commands.trigger(RollDice {
                dice: if modifier.0 >= 0 {
                    format!("1d20+{}", modifier.0)
                } else {
                    format!("1d20{}", modifier.0)
                },
            });
            commands
                .entity(e)
                .remove::<InitiativeSetupTimer>()
                .remove::<TokenInitiativeModifier>()
                .insert(TokenMemoizedInitiativeModifier(modifier.0))
                .insert(TokenAwaitingDice(awaiting.iter().len())); // FIXME: use correct queue order number
            break;
        } else {
            commands.entity(e).insert(updated_timer);
            if !modifier.1 {
                debug!("Updating initiative label with modifier {}", modifier.0);
                commands.entity(label_entity).insert(Visibility::Inherited);

                commands
                    .entity(label_entity)
                    .despawn_related::<Children>()
                    .with_child((
                        TextSpan::new(if modifier.0 >= 0 {
                            format!("+{}", modifier.0)
                        } else {
                            format!("{}", modifier.0)
                        }),
                        TextFont::from(map_resources.coords_font.clone()).with_font_size(60.0),
                        TextColor::from(Color::WHITE),
                    ));
                commands
                    .entity(e)
                    .insert(TokenInitiativeModifier(modifier.0, true));
            }
        }
    }
}

fn on_initiative_dice(
    trigger: On<DiceMessage>,
    mut commands: Commands,
    awaiting: Query<(Entity, &TokenAwaitingDice, &TokenInitiativeLabelChild)>,
    map_resources: Res<HexMapResources>,
) {
    // FIXME: perhaps condition the system on a state?
    if awaiting.is_empty() {
        return;
    }
    let min_awaiting = awaiting
        .iter()
        .map(|(_, awaiting, _)| awaiting.0)
        .min()
        .unwrap();

    for (entity, awaiting, label_pointer) in awaiting.iter() {
        if awaiting.0 == min_awaiting {
            let label_entity = label_pointer.0;
            let roll_result = trigger.event().dice_roll.total();
            commands
                .entity(entity)
                .insert(TokenInitiativeRoll)
                .remove::<TokenAwaitingDice>();
            commands
                .entity(label_entity)
                .insert(Visibility::Inherited)
                .despawn_related::<Children>()
                .with_child((
                    TextSpan::new(format!("{}", roll_result)),
                    TextFont::from(map_resources.coords_font.clone()).with_font_size(60.0),
                    TextColor::from(Color::WHITE),
                ));
        }
    }
}
