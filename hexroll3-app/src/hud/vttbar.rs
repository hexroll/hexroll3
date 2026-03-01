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
    battlemaps::BattlemapsRuler,
    hexmap::{elements::HexRevealPattern, plugin::RefereeRevealing},
    hud::{ShowTransientUserMessage, drawer::AutoDrawer},
    shared::{
        tweens::MenuIconMarginLensConfig,
        vtt::PlayerPreview,
        widgets::{
            buttons::{MenuButtonSwitcher, *},
            cursor::TooltipOnHover,
        },
    },
    tokens::{BattlemapsSnapping, DespawnVisibleTokens},
    vtt::{network::NetworkingConnection, sync::FramePlayerCamera},
};

use super::{
    drawer::{
        AutoDrawerButManual, AutoDrawerCommand, AutoDrawerVisiblity, make_auto_drawer_sensor,
    },
    menu::*,
    toggles::create_toggle_icon_frame_bundle,
};

pub struct VttBarPlugin;
impl Plugin for VttBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(NetworkingConnection::Connected), show_vttbar)
            .add_systems(OnExit(NetworkingConnection::Connected), hide_vttbar);
    }
}

fn show_vttbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            Name::new("VttMenubar"),
            VttBar,
            MenuIconMarginLensConfig {
                factor_left: 0.0,
                factor_right: 0.0,
                factor_top: 0.0,
                factor_bottom: 1.0,
            },
            AutoDrawer::new(
                Vec2::new(528.0, 80.0),
                Vec2::new(528.0, 80.0),
                Srgba::new(0.0, 0.0, 0.0, 0.9).into(),
                Srgba::new(0.0, 0.0, 0.0, 0.9).into(),
            ),
            AutoDrawerButManual,
            AutoDrawerCommand::On,
            AutoDrawerVisiblity::VisibleToRefereeOnly,
            Node {
                position_type: PositionType::Absolute,
                overflow: Overflow::hidden_x(),
                justify_self: JustifySelf::Center,
                bottom: Val::Px(20.0),
                width: Val::Px(528.0),
                height: Val::Px(80.0),
                ..default()
            },
            BorderRadius::all(Val::Percent(50.0)),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            BackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.9).into()),
        ))
        .with_children(|c| {
            c.spawn(create_menu_icon_frame_bundle())
                .with_child(create_menu_icon_bundle_inner(
                    &asset_server,
                    "icons/icon-target.ktx2",
                    Val::Px(64.0),
                    true,
                    UiRect::AUTO,
                    Color::WHITE,
                ))
                .tooltip_on_hover("Frame players camera to view", 1.0)
                .menu_button_hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(FramePlayerCamera);
                    commands.trigger(ShowTransientUserMessage {
                        text: String::from("Players viewpoint changed"),
                        special: None,
                        keep_alive: None,
                    });
                });

            c.spawn(create_toggle_icon_frame_bundle())
                .menu_button_switch_ex::<PlayerPreview>(
                    PlayerPreview::default(),
                    vec![
                        asset_server.load("icons/icon-tv-off.ktx2"),
                        asset_server.load("icons/icon-tv-on.ktx2"),
                    ],
                    64.0,
                )
                .tooltip_on_hover("Toggle player preview", 1.0)
                .menu_button_hover_effect()
                .observe(|trigger: On<Pointer<Click>>, mut commands: Commands| {
                    commands
                        .entity(trigger.entity)
                        .trigger(|entity| ToggleButtonSwitcherEx {
                            entity,
                            trigger_state_as_event: true,
                            insert_state_as_resource: false,
                        });
                });

            c.spawn(create_toggle_icon_frame_bundle())
                .menu_button_switch_ex::<RefereeRevealing>(
                    RefereeRevealing::default(),
                    vec![
                        asset_server.load("icons/icon-reveal-off.ktx2"),
                        asset_server.load("icons/icon-reveal-on.ktx2"),
                    ],
                    64.0,
                )
                .tooltip_on_hover("Toggle hex reveal mode", 1.0)
                .menu_button_hover_effect()
                .observe(|trigger: On<Pointer<Click>>, mut commands: Commands| {
                    commands
                        .entity(trigger.entity)
                        .trigger(|entity| ToggleButtonSwitcherEx {
                            entity,
                            trigger_state_as_event: true,
                            insert_state_as_resource: false,
                        });
                });

            c.spawn(create_toggle_icon_frame_bundle())
                .menu_button_switch_ex::<HexRevealPattern>(
                    HexRevealPattern::default(),
                    vec![
                        asset_server.load("icons/icon-flower.ktx2"),
                        asset_server.load("icons/icon-partial-flower.ktx2"),
                    ],
                    64.0,
                )
                .tooltip_on_hover("Toggle hex reveal pattern", 1.0)
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

            c.spawn(create_toggle_icon_frame_bundle())
                .menu_button_switch_ex::<BattlemapsRuler>(
                    BattlemapsRuler::default(),
                    vec![
                        asset_server.load("icons/icon-ruler-on.ktx2"),
                        asset_server.load("icons/icon-ruler-off.ktx2"),
                    ],
                    64.0,
                )
                .tooltip_on_hover("Toggle token ruler", 1.0)
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

            c.spawn(create_toggle_icon_frame_bundle())
                .menu_button_switch_ex::<BattlemapsSnapping>(
                    BattlemapsSnapping::default(),
                    vec![
                        asset_server.load("icons/icon-snap-on.ktx2"),
                        asset_server.load("icons/icon-snap-off.ktx2"),
                    ],
                    64.0,
                )
                .tooltip_on_hover("Toggle snap to grid", 1.0)
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

            c.spawn(create_menu_icon_frame_bundle())
                .with_child(create_menu_icon_bundle_inner(
                    &asset_server,
                    "icons/icon-delete-tokens.ktx2",
                    Val::Px(64.0),
                    true,
                    UiRect::AUTO,
                    Color::srgb_u8(200, 0, 0),
                ))
                .tooltip_on_hover("Delete all unselected tokens in view", 1.0)
                .menu_button_hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(DespawnVisibleTokens);
                });
            c.spawn(make_auto_drawer_sensor());
        });
}

fn hide_vttbar(mut commands: Commands, vtt_bars: Query<Entity, With<VttBar>>) {
    vtt_bars
        .iter()
        .for_each(|e| commands.entity(e).try_despawn());
}

#[derive(Component, Default)]
struct VttBar;

impl Switch for RefereeRevealing {
    fn rotate(&self) -> Self {
        match self {
            RefereeRevealing::Off => RefereeRevealing::On,
            RefereeRevealing::On => RefereeRevealing::Off,
        }
    }

    fn index(&self) -> usize {
        match self {
            RefereeRevealing::Off => 0,
            RefereeRevealing::On => 1,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => RefereeRevealing::Off,
            1 => RefereeRevealing::On,
            _ => unreachable!(),
        }
    }
}

impl Switch for HexRevealPattern {
    fn rotate(&self) -> Self {
        match self {
            HexRevealPattern::Flower => HexRevealPattern::Single,
            HexRevealPattern::Single => HexRevealPattern::Flower,
        }
    }

    fn index(&self) -> usize {
        match self {
            HexRevealPattern::Flower => 0,
            HexRevealPattern::Single => 1,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => HexRevealPattern::Flower,
            1 => HexRevealPattern::Single,
            _ => unreachable!(),
        }
    }
}

impl Switch for BattlemapsRuler {
    fn rotate(&self) -> Self {
        match self {
            BattlemapsRuler::On => BattlemapsRuler::Off,
            BattlemapsRuler::Off => BattlemapsRuler::On,
        }
    }

    fn index(&self) -> usize {
        match self {
            BattlemapsRuler::On => 0,
            BattlemapsRuler::Off => 1,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => BattlemapsRuler::On,
            1 => BattlemapsRuler::Off,
            _ => unreachable!(),
        }
    }
}

impl Switch for BattlemapsSnapping {
    fn rotate(&self) -> Self {
        match self {
            BattlemapsSnapping::On => BattlemapsSnapping::Off,
            BattlemapsSnapping::Off => BattlemapsSnapping::On,
        }
    }

    fn index(&self) -> usize {
        match self {
            BattlemapsSnapping::On => 0,
            BattlemapsSnapping::Off => 1,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => BattlemapsSnapping::On,
            1 => BattlemapsSnapping::Off,
            _ => unreachable!(),
        }
    }
}

impl Switch for PlayerPreview {
    fn rotate(&self) -> Self {
        match self {
            PlayerPreview::Off => PlayerPreview::On,
            PlayerPreview::On => PlayerPreview::Off,
        }
    }

    fn index(&self) -> usize {
        match self {
            PlayerPreview::Off => 0,
            PlayerPreview::On => 1,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => PlayerPreview::Off,
            1 => PlayerPreview::On,
            _ => unreachable!(),
        }
    }
}
