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

use bevy::prelude::*;
use bevy_ui_text_input::clipboard::Clipboard;

use super::demidom::{DemidomClipboardText, DemidomRenderContext};
use super::tts::TtsHandle;
use crate::shared::settings::UserSettings;
use crate::shared::widgets::buttons::MenuButtonEffects;
use crate::shared::widgets::cursor::TooltipOnHover;

pub trait CopyOnRightClick {
    fn make_clipboard_container(&mut self, context: &DemidomRenderContext) -> &mut Self;
    fn copy_on_right_click(&mut self, context: &DemidomRenderContext) -> &mut Self;
}

#[derive(Component)]
struct DemidomClipboardRectMarker;

#[derive(Component)]
struct DemidomClipboardMenuMarker;

impl CopyOnRightClick for EntityCommands<'_> {
    fn make_clipboard_container(&mut self, context: &DemidomRenderContext) -> &mut Self {
        self.insert((
            Node {
                position_type: PositionType::Relative,
                display: Display::Flex,
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },
            Pickable {
                should_block_lower: true,
                ..default()
            },
        ))
        .copy_on_right_click(&context)
        .insert(ChildOf(context.parent))
    }
    fn copy_on_right_click(&mut self, context: &DemidomRenderContext) -> &mut Self {
        let _color = context.theme.text_color.clone();
        self.observe(
            move |mut trigger: On<Pointer<Click>>,
                  mut commands: Commands,
                  previous_rects: Query<(Entity, &ChildOf), With<DemidomClipboardRectMarker>>,
                  texts: Query<&DemidomClipboardText>,
                  asset_server: Res<AssetServer>| {
                trigger.propagate(false);
                if trigger.button == PointerButton::Primary {
                    return;
                }
                if let Ok(text) = texts.get(trigger.entity) {
                    let clipboard_copy = text.text.clone();
                    let command_copy = text.text.clone();
                    let mut is_self = false;
                    for (prev_rect, child_of) in previous_rects.iter() {
                        commands.entity(prev_rect).try_despawn();
                        if child_of.0 == trigger.entity {
                            is_self = true;
                        }
                    }
                    if is_self {
                        return;
                    }
                    commands.entity(trigger.entity).with_children(|c| {
                        c.spawn((
                            DemidomClipboardRectMarker,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border: UiRect::all(Val::Px(2.0)),
                                margin: UiRect::all(Val::Px(0.0)),
                                ..default()
                            },
                            BorderRadius::all(Val::Px(5.0)),
                            BorderColor::all(Color::BLACK.with_alpha(0.5)),
                            BackgroundColor(Color::BLACK.with_alpha(0.3)),
                            ZIndex(99),
                            Pickable {
                                should_block_lower: false,
                                is_hoverable: false,
                            },
                        ))
                        .with_children(|c| {
                            c.spawn((
                                Name::new("ClipboardMenu"),
                                Node {
                                    position_type: PositionType::Absolute,
                                    right: Val::Px(-5.0),
                                    top: Val::Px(0.0),
                                    // left: Val::Px(pos.x + 25.0),
                                    // top: Val::Px(pos.y - 0.0),
                                    ..default()
                                },
                                DemidomClipboardMenuMarker,
                            ))
                            .with_children(|c| {
                                c.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(0.0),
                                        top: Val::Px(8.0),
                                        width: Val::Px(30.0),
                                        height: Val::Px(30.0),
                                        border: UiRect::all(Val::Px(2.0)),
                                        margin: UiRect::all(Val::Px(0.0)),
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    BackgroundColor(Color::BLACK.with_alpha(0.95)),
                                    BorderRadius::all(Val::Percent(20.0)),
                                ))
                                .tooltip_on_hover("Copy to clipboard", 1.0)
                                .with_child((
                                    Node {
                                        width: Val::Px(16.0),
                                        height: Val::Px(16.0),
                                        align_self: AlignSelf::Center,
                                        ..default()
                                    },
                                    ImageNode {
                                        color: Color::WHITE.with_alpha(1.0),
                                        image: asset_server.load("icons/icon-copy.ktx2"),
                                        ..default()
                                    },
                                    Pickable {
                                        should_block_lower: false,
                                        is_hoverable: false,
                                    },
                                ))
                                .menu_button_hover_effect()
                                .observe(
                                    move |_: On<Pointer<Click>>,
                                        marker_to_despawn: Query<Entity, With<DemidomClipboardRectMarker>>,
                                        mut commands: Commands,
                                        mut clipboard: ResMut<Clipboard> | {
                                            marker_to_despawn.iter().for_each(|e| commands.entity(e).try_despawn());
                                            let _ = clipboard.set_text(clipboard_copy.clone());
                                    },
                                );
                                c.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(0.0),
                                        top: Val::Px(40.0),
                                        width: Val::Px(30.0),
                                        height: Val::Px(30.0),
                                        border: UiRect::all(Val::Px(2.0)),
                                        margin: UiRect::all(Val::Px(0.0)),
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    BackgroundColor(Color::BLACK.with_alpha(0.95)),
                                    BorderRadius::all(Val::Percent(20.0)),
                                ))
                                .tooltip_on_hover("Call TTS Command", 1.0)
                                .with_child((
                                    Node {
                                        width: Val::Px(16.0),
                                        height: Val::Px(16.0),
                                        align_self: AlignSelf::Center,
                                        ..default()
                                    },
                                    ImageNode {
                                        color: Color::WHITE.with_alpha(1.0),
                                        image: asset_server.load("icons/icon-tts.ktx2"),
                                        ..default()
                                    },
                                    Pickable {
                                        should_block_lower: false,
                                        is_hoverable: false,
                                    },
                                ))
                                .menu_button_hover_effect()
                                .observe(
                                    move |_: On<Pointer<Click>>,
                                        marker_to_despawn: Query<Entity, With<DemidomClipboardRectMarker>>,
                                        mut commands: Commands,
                                          user_settings: Res<UserSettings> | {
                                              marker_to_despawn.iter().for_each(|e| commands.entity(e).try_despawn());
                                              if let Some(tts_cmd) = &user_settings.tts_command {
                                                  let _ = TtsHandle::new(tts_cmd).send_text(command_copy.clone());
                                    }
                                });
                            });
                        });
                    });
                }
            },
        )
    }
}
