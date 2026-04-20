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

// Menus are specialized Drawers that when hovered-on, also reveal a set of menu items
use bevy::prelude::*;

use bevy::picking::hover::PickingInteraction;

use crate::{
    content::ContentMode,
    hexmap::elements::HexMapToolState,
    hud::drawer::AutoDrawer,
    shared::{
        tweens::{MenuIconMarginLensConfig, UiImageNodeAlphaLens, UiNodeMarginsLens},
        vtt::{HexMapMode, VttData},
        widgets::modal::DiscreteAppState,
    },
};

use super::drawer::{AutoDrawerSensor, AutoDrawerVisiblity};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PickingInteraction>()
            .add_systems(Update, menu_visibility_controller)
            .add_systems(Update, menu_state_enforcer);
    }
}
#[derive(Component)]
pub struct MenuMarker;

#[derive(Component)]
pub struct MenuIconMarker {
    pub enabled: bool,
}

pub fn create_menu_icon_frame_bundle() -> impl Bundle {
    (
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
    )
}

pub fn create_menu_icon_bundle(
    asset_server: &AssetServer,
    icon: &str,
    size: Val,
    enabled: bool,
    margin: UiRect,
) -> impl Bundle {
    create_menu_icon_bundle_inner(
        asset_server,
        icon,
        size,
        enabled,
        margin,
        Color::srgba_u8(255, 255, 255, 0),
    )
}

pub fn create_menu_icon_bundle_inner(
    asset_server: &AssetServer,
    icon: &str,
    size: Val,
    enabled: bool,
    margin: UiRect,
    color: Color,
) -> impl Bundle {
    (
        MenuIconMarker { enabled },
        ImageNode {
            color,
            image: asset_server.load(icon.to_string()),
            ..default()
        },
        Node {
            overflow: Overflow::clip_x(),
            margin,
            width: size,
            height: size,
            ..default()
        },
        Pickable {
            should_block_lower: false,
            is_hoverable: false,
        },
    )
}

fn menu_visibility_controller(
    vtt_data: Res<VttData>,
    nodes: Query<(Entity, &AutoDrawerVisiblity), With<AutoDrawer>>,
    mut commands: Commands,
    discrete_state: Res<State<DiscreteAppState>>,
    content_mode: Res<State<ContentMode>>,
    hexmap_tool_state: Res<State<HexMapToolState>>,
) {
    for (node, visibility) in nodes.iter() {
        commands.entity(node).try_insert(
            if (vtt_data.mode == HexMapMode::Player && visibility.player_restricted())
                || (vtt_data.mode.is_referee() && visibility.referee_restricted())
                || *discrete_state == DiscreteAppState::Modal
                || *content_mode == ContentMode::SplitScreen
                || *hexmap_tool_state == HexMapToolState::Draw
                || *hexmap_tool_state == HexMapToolState::Edit
            {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            },
        );
    }
}

fn menu_state_enforcer(
    mut commands: Commands,
    sensor: Query<(&AutoDrawerSensor, Option<&PickingInteraction>, &ChildOf)>,
    node: Query<(Entity, &AutoDrawer, &MenuIconMarginLensConfig), With<MenuMarker>>,
    icons: Query<(Entity, &MenuIconMarker)>,
    children: Query<&Children>,
) {
    for (_, picking, child_of) in sensor.iter() {
        if let Some(picking) = picking {
            if let Ok((e, menu_marker, lens_config)) = node.get(child_of.0) {
                let menu_items_should_now_appear_and_animate =
                    *picking == PickingInteraction::Hovered && !menu_marker.is_open;
                if menu_items_should_now_appear_and_animate {
                    const LENGTH: u64 = 500;
                    let mut index = 0;
                    children.iter_descendants(e).for_each(|entity| {
                        if let Ok((icon, icon_state)) = icons.get(entity) {
                            let alpha_tween = bevy_tweening::Tween::new(
                                EaseFunction::CubicOut,
                                std::time::Duration::from_millis(LENGTH * 2),
                                if icon_state.enabled {
                                    UiImageNodeAlphaLens { from: 0.1, to: 1.0 }
                                } else {
                                    UiImageNodeAlphaLens { from: 1.0, to: 0.1 }
                                },
                            );
                            let margins_tween = bevy_tweening::Tween::new(
                                EaseFunction::CubicOut,
                                std::time::Duration::from_millis(LENGTH),
                                UiNodeMarginsLens {
                                    index,
                                    config: lens_config.clone(),
                                },
                            );
                            commands
                                .entity(icon)
                                .insert(bevy_tweening::Animator::new(alpha_tween));
                            commands
                                .entity(icon)
                                .insert(bevy_tweening::Animator::new(margins_tween));
                            index += 1;
                        }
                    });
                }
            }
        }
    }
}
