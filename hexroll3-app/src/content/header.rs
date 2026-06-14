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

use bevy::ecs::relationship::RelatedSpawnerCommands;
use bevy::prelude::*;
use bevy_ui_text_input::SubmitText;

use crate::{
    clients::model::{FetchEntityReason, RerollEntity},
    content::{RenameSandboxEntity, context::ContentContext, page::ContentHeader},
    hexmap::{SandboxLock, elements::FetchEntityFromStorage},
    shared::widgets::{
        buttons::{
            MenuButtonDisabled, MenuButtonEffects, MenuButtonSwitcher,
            MenuButtonSwitcherState, ToggleButtonSwitcherEx, ToggleResourceWrapper,
        },
        link::ContentHoverLink,
    },
};

use super::{
    EditableAttributeParams, ThemeBackgroundColor, context::Spoilers, demidom::RollerIcon,
    spoiler::SpoilerMaskMarker,
};

#[derive(Component)]
pub struct RerollButtonMarker;

#[derive(Component)]
pub struct LockButtonMarker;

#[derive(Component)]
pub struct BackButtonMarker;

#[derive(Component)]
pub struct ForwardButtonMarker;

#[derive(Component)]
pub struct ContentSpoilersMarker;

pub fn make_header_bundle(
    c: &mut RelatedSpawnerCommands<'_, ChildOf>,
    asset_server: &Res<AssetServer>,
    spoilers: &Spoilers,
) {
    c.spawn((
        Name::new("ContentHeaderPanel"),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Px(100.0),
            ..default()
        },
        BackgroundColor(Color::srgb_u8(30, 30, 30)),
    ))
    .with_children(|c| {
        c.spawn((
            Name::new("ContentHeaderBack"),
            Node {
                width: Val::Px(36.0),
                height: Val::Px(100.0),
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgb_u8(20, 20, 20)),
            ThemeBackgroundColor(Color::srgb_u8(20, 20, 20)),
            BackButtonMarker,
            MenuButtonDisabled,
        ))
        .with_child((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(24.0),
                align_self: AlignSelf::Center,
                ..default()
            },
            ImageNode {
                color: Color::WHITE.with_alpha(0.1),
                image: asset_server.load("icons/icon-chevron-128-left.ktx2"),
                image_mode: NodeImageMode::Auto,
                ..default()
            },
        ))
        .hover_effect()
        .observe(
            |trigger: On<Pointer<Click>>,
             mut content_stuff: ResMut<ContentContext>,
             button_disabled: Query<&MenuButtonDisabled>,
             mut commands: Commands| {
                if button_disabled.contains(trigger.entity) {
                    return;
                }
                if let Some(uid) = content_stuff.go_back() {
                    commands.trigger(FetchEntityFromStorage {
                        uid,
                        anchor: None,
                        why: FetchEntityReason::History,
                    });
                }
            },
        );
        c.spawn((
            Name::new("ContentHeader"),
            ContentHeader,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(100.0),
                align_items: AlignItems::Baseline,
                align_content: AlignContent::Start,
                flex_wrap: FlexWrap::WrapReverse,
                row_gap: Val::Px(-5.0),
                padding: UiRect {
                    top: Val::Px(8.),
                    left: Val::Percent(3.),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(Color::srgb_u8(30, 30, 30)),
        ));
        c.spawn((
            Name::new("ContentHeaderForward"),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                width: Val::Px(36.0),
                height: Val::Px(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb_u8(20, 20, 20)),
            ThemeBackgroundColor(Color::srgb_u8(20, 20, 20)),
            ForwardButtonMarker,
            MenuButtonDisabled,
        ))
        .with_child((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(24.0),
                align_self: AlignSelf::Center,
                ..default()
            },
            ImageNode {
                color: Color::WHITE.with_alpha(0.1),
                image: asset_server.load("icons/icon-chevron-128-left.ktx2"),
                flip_x: true,
                image_mode: NodeImageMode::Auto,
                ..default()
            },
        ))
        .hover_effect()
        .observe(
            |trigger: On<Pointer<Click>>,
             mut content_stuff: ResMut<ContentContext>,
             button_disabled: Query<&MenuButtonDisabled>,
             mut commands: Commands| {
                if button_disabled.contains(trigger.entity) {
                    return;
                }
                if let Some(uid) = content_stuff.go_forward() {
                    commands.trigger(FetchEntityFromStorage {
                        uid,
                        anchor: None,
                        why: FetchEntityReason::History,
                    });
                }
            },
        );
        c.spawn((
            Name::new("ContentButtons"),
            BorderRadius::top_left(Val::Px(10.0)),
            BackgroundColor(Color::srgb_u8(30, 30, 30).with_alpha(0.9)),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(36.0 * 4.0),
                height: Val::Px(36.0),
                right: Val::Px(36.0),
                bottom: Val::Px(0.0),
                margin: UiRect::bottom(Val::Px(5.0)),
                justify_content: JustifyContent::SpaceEvenly,
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn((
                Name::new("ContentSpoilersButton"),
                ContentSpoilersMarker,
                Node {
                    width: Val::Px(36.0),
                    height: Val::Px(36.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ))
            .menu_button_switch_ex::<Spoilers>(
                spoilers.clone(),
                vec![
                    asset_server.load("icons/icon-mask-off.ktx2"),
                    asset_server.load("icons/icon-mask-on.ktx2"),
                ],
                32.0,
            )
            .menu_button_hover_effect()
            .observe(
                |trigger: On<Pointer<Click>>,
                 mut commands: Commands,
                 current: Res<ToggleResourceWrapper<Spoilers>>,
                 mut masks: Query<(&mut Node, &SpoilerMaskMarker)>| {
                    commands
                        .entity(trigger.entity)
                        .trigger(|entity| ToggleButtonSwitcherEx {
                            entity,
                            trigger_state_as_event: false,
                            insert_state_as_resource: true,
                        });
                    if current.value == Spoilers::Hidden {
                        masks.iter_mut().for_each(|(mut node, _)| {
                            node.display = Display::None;
                        });
                    } else {
                        masks.iter_mut().for_each(|(mut node, _)| {
                            node.display = Display::DEFAULT;
                        });
                    }
                },
            );

            c.spawn((
                Name::new("ContentLockButton"),
                Node {
                    width: Val::Px(36.0),
                    height: Val::Px(36.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                MenuButtonSwitcherState::Idle,
                LockButtonMarker,
            ))
            .menu_button_switch_ex::<SandboxLock>(
                SandboxLock::default(),
                vec![
                    asset_server.load("icons/icon-locked.ktx2"),
                    asset_server.load("icons/icon-unlocked.ktx2"),
                ],
                32.0,
            )
            .menu_button_hover_effect()
            .observe(|trigger: On<Pointer<Click>>, mut commands: Commands| {
                commands
                    .entity(trigger.entity)
                    .trigger(|entity| ToggleButtonSwitcherEx {
                        entity,
                        trigger_state_as_event: false,
                        insert_state_as_resource: true,
                    });
            });
            c.spawn((
                Name::new("ContentPageButton"),
                Node {
                    width: Val::Px(36.0),
                    height: Val::Px(36.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                RerollButtonMarker,
                MenuButtonDisabled,
            ))
            .with_child((
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    align_self: AlignSelf::Center,
                    ..default()
                },
                ImageNode {
                    color: Color::WHITE.with_alpha(0.1),
                    image: asset_server.load("icons/icon-dice-256.ktx2"),
                    flip_x: true,
                    image_mode: NodeImageMode::Auto,
                    ..default()
                },
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
            ))
            .menu_button_hover_effect()
            .observe(
                |trigger: On<Pointer<Click>>,
                 button_disabled: Query<&MenuButtonDisabled>,
                 content_stuff: Res<ContentContext>,
                 mut commands: Commands| {
                    if button_disabled.contains(trigger.entity) {
                        return;
                    }
                    if let Some(uid) = &content_stuff.current_entity_uid {
                        commands.trigger(RerollEntity::from_uid(uid.clone()));
                    }
                },
            );
        });
    });
}

pub fn update_header_buttons_state(
    mut commands: Commands,
    context: Res<ContentContext>,
    disabled_buttons: Query<&MenuButtonDisabled>,
    back_button: Single<Entity, With<BackButtonMarker>>,
    forward_button: Single<Entity, With<ForwardButtonMarker>>,
) {
    if context.history.is_empty() {
        if !disabled_buttons.contains(*back_button) {
            commands.entity(*back_button).try_insert(MenuButtonDisabled);
        }
    } else {
        if disabled_buttons.contains(*back_button) {
            commands
                .entity(*back_button)
                .try_remove::<MenuButtonDisabled>();
        }
    }
    if context.fistory.is_empty() {
        if !disabled_buttons.contains(*forward_button) {
            commands
                .entity(*forward_button)
                .try_insert(MenuButtonDisabled);
        }
    } else {
        if disabled_buttons.contains(*forward_button) {
            commands
                .entity(*forward_button)
                .try_remove::<MenuButtonDisabled>();
        }
    }
}

#[derive(Component)]
pub struct EditableTitleInput(pub EditableAttributeParams);

pub fn submit_editable_title(
    mut events: MessageReader<SubmitText>,
    mut commands: Commands,
    content_stuff: Res<ContentContext>,
    entry: Query<&EditableTitleInput>,
) {
    if let Some(uid) = &content_stuff.current_entity_uid {
        if !entry.is_empty() {
            for event in events.read() {
                if let Ok(editable_title_input) = entry.get(event.entity) {
                    let value = event.text.clone();
                    commands.trigger(RenameSandboxEntity {
                        entity_uid: uid.clone(),
                        value,
                        params: editable_title_input.0.clone(),
                    });
                }
            }
        }
    }
}

pub fn update_lock_state(
    mut content_stuff: ResMut<ContentContext>,
    lock: Res<ToggleResourceWrapper<SandboxLock>>,
    page_rollers: Query<(Entity, &RerollButtonMarker)>,
    mut rollers: Query<(Entity, &mut Node, &RollerIcon)>,
    mut commands: Commands,
) {
    // NOTE: Toggle functionality
    if lock.value.off() {
        content_stuff.unlocked = true;
        rollers.iter_mut().for_each(|(e, mut node, _)| {
            node.display = Display::DEFAULT;
            commands.entity(e).insert(RollerIcon::Visible);
        });
        if content_stuff.rerollable {
            for (e, _) in page_rollers.iter() {
                commands.entity(e).remove::<MenuButtonDisabled>();
            }
        }
    } else {
        content_stuff.unlocked = false;
        rollers.iter_mut().for_each(|(e, mut node, _)| {
            node.display = Display::None;
            commands.entity(e).insert(RollerIcon::Hidden);
        });
        if content_stuff.rerollable {
            for (e, _) in page_rollers.iter() {
                commands.entity(e).insert(MenuButtonDisabled);
            }
        }
    }
}
