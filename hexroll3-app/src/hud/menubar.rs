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

use crate::{
    dialogs::OpenSandboxOptionsModal,
    hexmap::{LockableDialButton, MenuIconLock},
    hud::drawer::AutoDrawer,
    shared::{
        AppState,
        tweens::MenuIconMarginLensConfig,
        widgets::{buttons::MenuButtonEffects, cursor::TooltipOnHover, dial::DialButtonState},
    },
    vtt::network::{NetworkingConnection, on_click_vtt},
};

use super::{
    drawer::{AutoDrawerVisiblity, make_auto_drawer_sensor},
    menu::*,
    toggles::{on_show_toggles, spawn_audio_toggle, update_daynight_toggle},
};

pub struct MenuBarPlugin;

impl Plugin for MenuBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Live), setup)
            .add_systems(Update, update_daynight_toggle)
            .add_systems(
                OnEnter(NetworkingConnection::Connected),
                enable_connect_button,
            )
            .add_systems(
                OnExit(NetworkingConnection::Connected),
                disable_connect_button,
            );
    }
}

#[derive(Component)]
struct MenuIconConnect;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Player menubar
    {
        let mut e = commands.spawn(create_menubar_bundle(
            "PlayerMenu",
            AutoDrawerVisiblity::VisibleToPlayerOnly,
        ));
        e.with_children(|c| {
            spawn_audio_toggle(c, create_menu_icon_frame_bundle(), asset_server.as_ref());
        });
    }
    // Referee menubar
    {
        let mut e = commands.spawn(create_menubar_bundle(
            "RefereeMenu",
            AutoDrawerVisiblity::VisibleToRefereeOnly,
        ));
        let p = e.id();
        e.with_children(|c| {
            c.spawn(create_menu_icon_frame_bundle())
                .with_child((
                    ImageNode {
                        color: Color::srgba_u8(255, 255, 255, 255),
                        image: asset_server.load("icons/icon-earth.ktx2"),
                        ..default()
                    },
                    Node {
                        width: Val::Px(80.0),
                        height: Val::Px(80.0),
                        margin: UiRect::bottom(Val::Px(2.0)),
                        ..default()
                    },
                    Pickable {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                ))
                .tooltip_on_hover("Sandbox Options", 1.0)
                .menu_button_hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(OpenSandboxOptionsModal);
                });
            c.spawn(create_menu_icon_frame_bundle())
                .with_child(create_menu_icon_bundle(
                    &asset_server,
                    "icons/icon-settings.ktx2",
                    Val::Px(70.0),
                    true,
                    UiRect {
                        left: Val::ZERO,
                        right: Val::ZERO,
                        top: Val::ZERO,
                        bottom: Val::Px(-100.0),
                    },
                ))
                .tooltip_on_hover("Toggle labels visibility", 1.0)
                .menu_button_hover_effect()
                .observe(on_show_toggles(p));
            c.spawn(create_menu_icon_frame_bundle())
                .with_children(|c| {
                    c.spawn(create_menu_icon_bundle(
                        &asset_server,
                        "icons/icon-locked2.ktx2",
                        Val::Px(64.0),
                        true,
                        UiRect {
                            left: Val::ZERO,
                            right: Val::ZERO,
                            top: Val::ZERO,
                            bottom: Val::Px(-100.0),
                        },
                    ))
                    .insert(MenuIconLock::Locked);
                })
                .tooltip_on_hover("Lock/unlock map for editing", 1.0)
                .menu_button_hover_effect()
                .observe(
                    |_: On<Pointer<Click>>,
                     mut commands: Commands,
                     asset_server: Res<AssetServer>,
                     lockables: Query<(Entity, &LockableDialButton)>,
                     icons: Query<(Entity, &MenuIconLock)>| {
                        icons.iter().for_each(|(e, lock)| match lock {
                            MenuIconLock::Locked => {
                                commands
                                    .entity(e)
                                    .insert(ImageNode {
                                        image: asset_server.load("icons/icon-unlocked2.ktx2"),
                                        ..default()
                                    })
                                    .insert(MenuIconLock::Unlocked);
                                for (lockable_entity, lockable_state) in lockables.iter() {
                                    if lockable_state.0 {
                                        commands
                                            .entity(lockable_entity)
                                            .try_insert(DialButtonState::Enabled);
                                    }
                                }
                            }
                            MenuIconLock::Unlocked => {
                                commands
                                    .entity(e)
                                    .insert(ImageNode {
                                        image: asset_server.load("icons/icon-locked2.ktx2"),
                                        ..default()
                                    })
                                    .insert(MenuIconLock::Locked);
                                for (lockable_entity, _) in lockables.iter() {
                                    commands
                                        .entity(lockable_entity)
                                        .try_insert(DialButtonState::Disabled);
                                }
                            }
                        });
                    },
                );
            c.spawn(create_menu_icon_frame_bundle())
                .with_children(|c| {
                    c.spawn(create_menu_icon_bundle(
                        &asset_server,
                        "icons/icon-vtt-128.ktx2",
                        Val::Px(70.0),
                        false,
                        UiRect {
                            left: Val::ZERO,
                            right: Val::ZERO,
                            top: Val::ZERO,
                            bottom: Val::Px(-100.0),
                        },
                    ))
                    .insert(MenuIconConnect);
                })
                .tooltip_on_hover("Connect/disconnect VTT", 1.0)
                .menu_button_hover_effect()
                .observe(on_click_vtt());
            c.spawn(make_auto_drawer_sensor());
        });
    }
}
fn enable_connect_button(
    mut commands: Commands,
    mut q: Single<(Entity, &mut ImageNode), With<MenuIconConnect>>,
) {
    commands
        .entity(q.0)
        .remove::<bevy_tweening::Animator<ImageNode>>()
        .insert(MenuIconMarker { enabled: true });
    q.1.color.set_alpha(1.0);
}

fn disable_connect_button(
    mut commands: Commands,
    mut q: Single<(Entity, &mut ImageNode), With<MenuIconConnect>>,
) {
    commands
        .entity(q.0)
        .insert(MenuIconMarker { enabled: false });

    q.1.color.set_alpha(0.1);
}

fn create_menubar_bundle(name: &str, visibility: AutoDrawerVisiblity) -> impl Bundle {
    (
        Name::new(name.to_string()),
        MenuMarker,
        MenuIconMarginLensConfig {
            factor_left: 0.0,
            factor_right: 0.0,
            factor_top: 0.0,
            factor_bottom: 1.0,
        },
        AutoDrawer::new(
            Vec2::new(80.0, 80.0),
            Vec2::new(80.0, 330.0),
            Srgba::new(0.0, 0.0, 0.0, 0.9).into(),
            Srgba::new(0.0, 0.0, 0.0, 0.9).into(),
        ),
        visibility,
        Node {
            position_type: PositionType::Absolute,
            overflow: Overflow::hidden_x(),
            left: Val::Px(20.0),
            bottom: Val::Px(20.0),
            width: Val::Px(80.0),
            height: Val::Px(80.0),
            flex_direction: FlexDirection::ColumnReverse,
            flex_wrap: FlexWrap::WrapReverse,
            align_content: AlignContent::FlexStart,
            ..default()
        },
        BorderRadius::new(
            Val::Percent(50.0),
            Val::Percent(50.0),
            Val::Percent(50.0),
            Val::Percent(50.0),
        ),
        Pickable {
            should_block_lower: true,
            is_hoverable: true,
        },
        BackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.9).into()),
    )
}
