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

use crate::{
    dice::{Dice, DiceResources, DiceSet, DiceSets, RollDice},
    hud::drawer::{AutoDrawer, AutoDrawerVisiblity, make_auto_drawer_sensor},
    shared::{
        AppState,
        tweens::MenuIconMarginLensConfig,
        widgets::{buttons::*, cursor::TooltipOnHover},
    },
};
use bevy::prelude::*;

use super::menu::*;

pub struct DiceBarPlugin;
impl Plugin for DiceBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Live), setup_dicebar_overlay);
    }
}

fn setup_dicebar_overlay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    dice_sets: Res<Assets<DiceSets>>,
    dice_resources: Res<DiceResources>,
) {
    let Some(dice_sets) = dice_sets.get(&dice_resources.dice_sets) else {
        warn!("Dicesets asset was not found when creating the dicebar overlay");
        return;
    };
    let dice_set_names = dice_sets.dice_sets.keys();
    commands
        .spawn((
            Name::new("DICEBAR"),
            MenuMarker,
            MenuIconMarginLensConfig {
                factor_left: 1.0,
                factor_right: 0.0,
                factor_top: 0.0,
                factor_bottom: 0.0,
            },
            AutoDrawer::new(
                Vec2::new(80.0, 80.0),
                Vec2::new(560.0, 80.0),
                Srgba::new(0.0, 0.0, 0.0, 0.9).into(),
                Srgba::new(0.0, 0.0, 0.0, 0.9).into(),
            ),
            AutoDrawerVisiblity::VisibleToAll,
            Node {
                position_type: PositionType::Absolute,
                overflow: Overflow::hidden(),
                padding: UiRect::right(Val::Px(15.0)),
                left: Val::Px(20.0),
                top: Val::Px(20.0),
                width: Val::Px(80.0),
                height: Val::Px(80.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::NoWrap,
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
        ))
        .with_children(|c| {
            c.spawn(create_menu_icon_frame_bundle())
                .with_child((
                    ImageNode {
                        color: Color::srgba_u8(255, 255, 255, 255),
                        image: asset_server.load("icons/icon_d20.ktx2"),
                        ..default()
                    },
                    Node {
                        width: Val::Px(50.0),
                        height: Val::Px(50.0),
                        margin: UiRect::left(Val::Px(15.0)).with_right(Val::Px(15.0)),
                        ..default()
                    },
                    Pickable {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                ))
                .tooltip_on_hover("Roll a d20", 1.0)
                .menu_button_hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(RollDice {
                        dice: "d20".to_string(),
                    });
                });
            c.spawn(create_menu_icon_frame_bundle())
                .with_child(create_menu_icon_bundle(
                    &asset_server,
                    "icons/icon_d12.ktx2",
                    Val::Px(50.0),
                    true,
                    UiRect {
                        left: Val::Px(-100.0),
                        right: Val::ZERO,
                        top: Val::ZERO,
                        bottom: Val::ZERO,
                    },
                ))
                .tooltip_on_hover("Roll a d12", 1.0)
                .menu_button_hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(RollDice {
                        dice: "d12".to_string(),
                    });
                });
            c.spawn(create_menu_icon_frame_bundle())
                .with_child(create_menu_icon_bundle(
                    &asset_server,
                    "icons/icon_d10.ktx2",
                    Val::Px(50.0),
                    true,
                    UiRect {
                        left: Val::Px(-100.0),
                        right: Val::ZERO,
                        top: Val::ZERO,
                        bottom: Val::ZERO,
                    },
                ))
                .tooltip_on_hover("Roll a d10", 1.0)
                .menu_button_hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(RollDice {
                        dice: "d10".to_string(),
                    });
                });
            c.spawn(create_menu_icon_frame_bundle())
                .with_child(create_menu_icon_bundle(
                    &asset_server,
                    "icons/icon_d8.ktx2",
                    Val::Px(50.0),
                    true,
                    UiRect {
                        left: Val::Px(-100.0),
                        right: Val::ZERO,
                        top: Val::ZERO,
                        bottom: Val::ZERO,
                    },
                ))
                .tooltip_on_hover("Roll a d8", 1.0)
                .menu_button_hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(RollDice {
                        dice: "d8".to_string(),
                    });
                });
            c.spawn(create_menu_icon_frame_bundle())
                .with_child(create_menu_icon_bundle(
                    &asset_server,
                    "icons/icon_d6.ktx2",
                    Val::Px(50.0),
                    true,
                    UiRect {
                        left: Val::Px(-100.0),
                        right: Val::ZERO,
                        top: Val::ZERO,
                        bottom: Val::ZERO,
                    },
                ))
                .tooltip_on_hover("Roll a d6", 1.0)
                .menu_button_hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(RollDice {
                        dice: "d6".to_string(),
                    });
                });
            c.spawn(create_menu_icon_frame_bundle())
                .with_child(create_menu_icon_bundle(
                    &asset_server,
                    "icons/icon_d4.ktx2",
                    Val::Px(50.0),
                    true,
                    UiRect {
                        left: Val::Px(-100.0),
                        right: Val::ZERO,
                        top: Val::ZERO,
                        bottom: Val::ZERO,
                    },
                ))
                .tooltip_on_hover("Roll a d4", 1.0)
                .menu_button_hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(RollDice {
                        dice: "d4".to_string(),
                    });
                });
            c.spawn((
                Node {
                    overflow: Overflow::clip(),
                    width: Val::Px(80.0),
                    height: Val::Px(80.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Pickable {
                    should_block_lower: true,
                    is_hoverable: true,
                },
            ))
            .value_switch_button::<DiceSet>(
                DiceSet::from_value("plastic"),
                dice_set_names
                    .map(|v| {
                        (
                            v.as_str(),
                            asset_server.load(format!("icons/icon-material-{}.png", v)),
                        )
                    })
                    .collect(),
                64.0,
            )
            .tooltip_on_hover("Toggle hex reveal pattern", 1.0)
            .menu_button_hover_effect()
            .observe(
                |trigger: On<Pointer<Click>>,
                 mut commands: Commands,
                 any_dice: Query<&Dice>| {
                    if any_dice.is_empty() {
                        commands.entity(trigger.entity).trigger(|entity| {
                            ToggleButtonSwitcherEx {
                                entity,
                                trigger_state_as_event: false,
                                insert_state_as_resource: true,
                            }
                        });
                    }
                },
            );
            c.spawn(make_auto_drawer_sensor());
        });
}

impl SwitchValue for DiceSet {
    fn value(&self) -> &str {
        &self.value
    }
    fn from_value(value: &str) -> Self {
        Self {
            value: value.to_owned(),
        }
    }
}
