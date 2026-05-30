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

use bevy::window::CursorOptions;
use bevy::{
    color::color_difference::EuclideanDistance,
    window::{PrimaryWindow, SystemCursorIcon},
};

use bevy_tweening::lens::UiBackgroundColorLens;

use crate::content::ThemeBackgroundColor;

use super::buttons::MenuButtonDisabled;
use super::cursor::{CursorController, PointerExclusivityIsPreferred};

pub trait ContentHoverLink {
    fn hover_effect(&mut self) -> &mut Self;
    fn hover_effect_ex(&mut self, check_for_exclusivity: bool) -> &mut Self;
}

#[derive(Component)]
struct OriginalTextColor(Color);

impl ContentHoverLink for EntityCommands<'_> {
    fn hover_effect(&mut self) -> &mut Self {
        self.hover_effect_ex(false)
    }
    fn hover_effect_ex(&mut self, check_for_exclusivity: bool) -> &mut Self {
        self.observe(
            move |trigger: On<Pointer<Over>>,
                  mut commands: Commands,
                  children: Query<&Children>,
                  mut link_text: Query<(Entity, &mut TextColor)>,
                  window: Single<(Entity, &CursorOptions), With<PrimaryWindow>>,
                  button_disabled: Query<&MenuButtonDisabled>,
                  bg: Query<&BackgroundColor>,
                  exclusivity: Query<&PointerExclusivityIsPreferred>,
                  mut cursor_controller: ResMut<CursorController>| {
                if button_disabled.contains(trigger.entity) {
                    return;
                }
                if !exclusivity.is_empty() && check_for_exclusivity {
                    return;
                }
                if !window.1.visible {
                    return;
                }
                cursor_controller.set_cursor(
                    &mut commands,
                    window.0,
                    SystemCursorIcon::Pointer,
                );
                if let Ok(current_bg) = bg.get(trigger.entity) {
                    let target_color = Color::srgb_u8(223, 40, 109);
                    let distance = target_color.distance(&current_bg.0);
                    let tween = bevy_tweening::Tween::new(
                        EaseFunction::Linear,
                        Duration::from_millis((distance.abs() * 100.0) as u64),
                        UiBackgroundColorLens {
                            start: current_bg.0,
                            end: target_color,
                        },
                    );
                    commands
                        .entity(trigger.entity)
                        .insert(bevy_tweening::Animator::new(tween));
                }

                children
                    .iter_descendants(trigger.entity)
                    .for_each(|entity| {
                        if let Ok((text_entity, mut text_color)) = link_text.get_mut(entity) {
                            commands
                                .entity(text_entity)
                                .try_insert(OriginalTextColor(text_color.0.clone()));
                            text_color.0 = Color::WHITE;
                        }
                    });
            },
        );
        self.observe(
            |trigger: On<Pointer<Out>>,
             mut commands: Commands,
             children: Query<&Children>,
             mut link_text: Query<(Entity, &mut TextColor, &OriginalTextColor)>,
             window: Single<Entity, With<PrimaryWindow>>,
             bg: Query<&BackgroundColor>,
             theme_bg: Query<&ThemeBackgroundColor>,
             mut cursor_controller: ResMut<CursorController>| {
                cursor_controller.set_cursor(
                    &mut commands,
                    *window,
                    SystemCursorIcon::Default,
                );
                if let (Ok(current_bg), Ok(theme_bg)) =
                    (bg.get(trigger.entity), theme_bg.get(trigger.entity))
                {
                    let target_color = theme_bg.0;
                    let distance = target_color.distance(&current_bg.0);
                    let tween = bevy_tweening::Tween::new(
                        EaseFunction::Linear,
                        Duration::from_millis((distance.abs() * 100.0) as u64),
                        UiBackgroundColorLens {
                            start: current_bg.0,
                            end: target_color,
                        },
                    );
                    commands
                        .entity(trigger.entity)
                        .insert(bevy_tweening::Animator::new(tween));
                }
                children
                    .iter_descendants(trigger.entity)
                    .for_each(|entity| {
                        if let Ok((text_entity, mut text_color, original_text_color)) =
                            link_text.get_mut(entity)
                        {
                            text_color.0 = original_text_color.0;
                            commands
                                .entity(text_entity)
                                .try_remove::<OriginalTextColor>();
                        }
                    });
            },
        );
        self
    }
}
