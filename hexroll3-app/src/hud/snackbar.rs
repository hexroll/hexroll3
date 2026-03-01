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

use bevy::{prelude::*, text::FontSmoothing};
use bevy_tweening::Tween;
use bevy_ui_text_input::clipboard::Clipboard;

use crate::{
    dice::DiceRollHelpers,
    shared::widgets::{ButtonSpawner, cursor::PointerOnHover},
};

use super::{DiceMessage, ShowTransientUserMessage};

pub struct SnackbarPlugin;
impl Plugin for SnackbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, dice_result_box_timer)
            .add_observer(on_show_snackbar_message)
            .add_observer(on_show_dice_snackbar);
    }
}

fn on_show_snackbar_message(
    trigger: On<ShowTransientUserMessage>,
    mut commands: Commands,
    mut previous: Query<(Entity, &mut Node), With<DiceResultBox>>,
    asset_server: Res<AssetServer>,
    app_config: Res<crate::shared::settings::Config>,
) {
    for (e, p) in previous.iter_mut() {
        if let Val::Px(v) = p.bottom {
            let tween = Tween::new(
                EaseFunction::QuarticIn,
                Duration::from_millis(300),
                DiceResultBoxLens {
                    bottom_start: v,
                    bottom_end: v + app_config.snackbar_config.font_size * 4.0 + 20.0,
                    right_start: 20.0,
                    right_end: 20.0,
                },
            );
            commands
                .entity(e)
                .insert(bevy_tweening::Animator::new(tween));
        }
    }
    let tween = bevy_tweening::Delay::new(Duration::from_millis(300)).then(Tween::new(
        EaseFunction::QuarticIn,
        Duration::from_millis(300),
        DiceResultBoxLens {
            bottom_start: 20.0,
            bottom_end: 20.0,
            right_start: -1000.0,
            right_end: 20.0,
        },
    ));
    commands
        .spawn((
            Name::new("SnackbarMessageBox"),
            DiceResultBox(trigger.keep_alive.unwrap_or(Duration::from_secs(10))),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(-3000.0),
                bottom: Val::Px(20.0),
                width: Val::Auto,
                height: Val::Auto,
                padding: UiRect::all(Val::Px(app_config.snackbar_config.font_size)),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::WrapReverse,
                align_items: AlignItems::Center,
                align_content: AlignContent::FlexStart,
                ..default()
            },
            BorderRadius::all(Val::Percent(10.0)),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            BackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.9).into()),
            bevy_tweening::Animator::new(tween),
        ))
        .with_children(|c| {
            c.spawn((
                TextFont {
                    font: asset_server.load("fonts/eczar.ttf"),
                    font_size: app_config.snackbar_config.font_size,
                    font_smoothing: FontSmoothing::AntiAliased,
                    ..default()
                },
                Text::new(format!("{}", trigger.text)),
            ));

            if let Some(special) = &trigger.special {
                let copyable = special.clone();
                c.spawn((
                    Node {
                        width: Val::Auto,
                        height: Val::Auto,
                        padding: UiRect::all(Val::Px(10.0)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderRadius::all(Val::Percent(10.0)),
                    BackgroundColor(Srgba::new(1.0, 1.0, 1.0, 0.9).into()),
                ))
                .prefer_pointer_exclusivity()
                .with_children(|c| {
                    c.spawn((
                        TextColor(Srgba::new(0.0, 0.0, 0.0, 0.9).into()),
                        TextFont {
                            font: asset_server.load("fonts/eczar.ttf"),
                            font_size: app_config.snackbar_config.font_size,
                            font_smoothing: FontSmoothing::AntiAliased,
                            ..default()
                        },
                        Text::new(format!("{}", special)),
                        TextLayout {
                            justify: Justify::Center,
                            ..Default::default()
                        },
                    ));
                    c.spawn_button(
                        crate::shared::widgets::Button::from_image(
                            asset_server.load("icons/icon-copy.ktx2"),
                        )
                        .button_size(Val::Px(40.0))
                        .image_size(Val::Px(32.0))
                        .margins(UiRect::left(Val::Px(10.0)))
                        .border_radius(Val::Percent(10.0)),
                    )
                    .observe(
                        move |_: On<Pointer<Click>>, mut clipboard: ResMut<Clipboard>| {
                            let _ = clipboard.set_text(copyable.clone());
                        },
                    );
                });
            }
        });
}

impl DiceMessage {
    pub fn display_results(&self, max_results_to_show: usize) -> String {
        let mut result_string = format!("{} rolled ", self.roller);
        let (terms, details) = self.dice_roll.to_strings();
        result_string.push_str(&terms.join(" + "));
        if details.len() < max_results_to_show {
            result_string.push_str(" and got (");
            result_string.push_str(&details.join("+"));
            result_string.push(')');
        }
        result_string
    }
}

fn on_show_dice_snackbar(
    trigger: On<DiceMessage>,
    mut commands: Commands,
    mut previous: Query<(Entity, &mut Node), With<DiceResultBox>>,
    asset_server: Res<AssetServer>,
    app_config: Res<crate::shared::settings::Config>,
) {
    let message = trigger.event();
    for (e, p) in previous.iter_mut() {
        if let Val::Px(v) = p.bottom {
            let tween = Tween::new(
                EaseFunction::QuarticIn,
                Duration::from_millis(300),
                DiceResultBoxLens {
                    bottom_start: v,
                    bottom_end: v + app_config.snackbar_config.font_size * 4.0 + 20.0,
                    right_start: 20.0,
                    right_end: 20.0,
                },
            );
            commands
                .entity(e)
                .insert(bevy_tweening::Animator::new(tween));
        }
    }
    let tween = bevy_tweening::Delay::new(Duration::from_millis(300)).then(Tween::new(
        EaseFunction::QuarticIn,
        Duration::from_millis(300),
        DiceResultBoxLens {
            bottom_start: 20.0,
            bottom_end: 20.0,
            right_start: -1000.0,
            right_end: 20.0,
        },
    ));
    commands
        .spawn((
            Name::new("DiceResultBox"),
            DiceResultBox(Duration::from_secs(10)),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(-3000.0),
                bottom: Val::Px(20.0),
                width: Val::Auto,
                height: Val::Auto,
                padding: UiRect::all(Val::Px(app_config.snackbar_config.font_size)),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::WrapReverse,
                align_items: AlignItems::Center,
                align_content: AlignContent::FlexStart,
                ..default()
            },
            BorderRadius::all(Val::Percent(10.0)),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            BackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.9).into()),
            bevy_tweening::Animator::new(tween),
        ))
        .with_children(|c| {
            c.spawn((
                TextFont {
                    font: asset_server.load("fonts/eczar.ttf"),
                    font_size: app_config.snackbar_config.font_size,
                    font_smoothing: FontSmoothing::AntiAliased,
                    ..default()
                },
                Text::new(format!(
                    "{} = ",
                    message
                        .display_results(app_config.snackbar_config.max_dice_results_to_show)
                )),
            ));
            c.spawn((
                Node {
                    width: Val::Auto,
                    height: Val::Auto,
                    padding: UiRect::all(Val::Px(10.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderRadius::all(Val::Percent(10.0)),
                BackgroundColor(Srgba::new(1.0, 1.0, 1.0, 0.9).into()),
            ))
            .with_children(|c| {
                c.spawn((
                    TextColor(Srgba::new(0.0, 0.0, 0.0, 0.9).into()),
                    TextFont {
                        font: asset_server.load("fonts/eczar.ttf"),
                        font_size: app_config.snackbar_config.font_size,
                        font_smoothing: FontSmoothing::AntiAliased,
                        ..default()
                    },
                    Text::new(format!("{}", message.dice_roll.total())),
                    TextLayout {
                        justify: Justify::Center,
                        ..Default::default()
                    },
                ));
            });
        });
}

fn dice_result_box_timer(
    mut commands: Commands,
    time: Res<Time>,
    mut result_boxes: Query<(Entity, &mut DiceResultBox)>,
) {
    for (e, mut r) in result_boxes.iter_mut() {
        r.0 = r.0.saturating_sub(time.delta());
        if r.0 == Duration::ZERO {
            commands.entity(e).despawn();
        }
    }
}

#[derive(Component)]
struct DiceResultBox(Duration);

#[derive(Debug, Copy, Clone, PartialEq)]
struct DiceResultBoxLens {
    bottom_start: f32,
    bottom_end: f32,
    right_start: f32,
    right_end: f32,
}

impl bevy_tweening::Lens<Node> for DiceResultBoxLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<Node>, ratio: f32) {
        target.bottom =
            Val::Px(self.bottom_start + (self.bottom_end - self.bottom_start) * ratio);
        target.right = Val::Px(self.right_start + (self.right_end - self.right_start) * ratio);
    }
}
