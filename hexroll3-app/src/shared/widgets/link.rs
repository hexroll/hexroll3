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

use bevy::{
    color::color_difference::EuclideanDistance,
    window::{CursorIcon, PrimaryWindow, SystemCursorIcon},
};

use bevy_tweening::lens::UiBackgroundColorLens;

use crate::content::ThemeBackgroundColor;

use super::buttons::MenuButtonDisabled;

pub trait ContentHoverLink {
    fn hover_effect(&mut self) -> &mut Self;
}

#[derive(Component)]
struct OriginalTextColor(Color);

impl ContentHoverLink for EntityCommands<'_> {
    fn hover_effect(&mut self) -> &mut Self {
        self.observe(
            |trigger: On<Pointer<Over>>,
             mut commands: Commands,
             children: Query<&Children>,
             mut link_text: Query<(Entity, &mut TextColor)>,
             window: Single<Entity, With<PrimaryWindow>>,
             button_disabled: Query<&MenuButtonDisabled>,
             bg: Query<&BackgroundColor>| {
                if button_disabled.contains(trigger.entity) {
                    return;
                }
                commands
                    .entity(*window)
                    .insert(CursorIcon::System(SystemCursorIcon::Pointer));
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
             theme_bg: Query<&ThemeBackgroundColor>| {
                commands
                    .entity(*window)
                    .insert(CursorIcon::System(SystemCursorIcon::Default));
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
