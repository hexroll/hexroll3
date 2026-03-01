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

use bevy::{ecs::relationship::RelatedSpawnerCommands, prelude::*};

use crate::{
    audio::AudioToggle,
    hexmap::{
        HexMapTime, HexmapTheme, LoadHexmapTheme, MapMessage, TileSetThemesMetadata,
        ToggleDayNight,
    },
    hud::drawer::AutoDrawer,
    shared::{
        labels::DespawnLabels,
        settings::{AppSettings, LabelsMode},
        widgets::{
            buttons::{MenuButtonSwitcher, Switch, ToggleButtonSwitcherEx, rotate_key},
            cursor::TooltipOnHover,
        },
    },
    vtt::sync::SyncMapForPeers,
};

use super::{
    drawer::{HasRelatedDrawer, make_auto_drawer_sensor},
    menu::MenuIconMarker,
};

use crate::shared::widgets::buttons::MenuButtonEffects;

pub fn on_show_toggles(
    p: Entity,
) -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Res<AssetServer>,
    Query<&TogglesMenuMarker>,
    Res<AppSettings>,
) {
    move |mut trigger, mut commands, asset_server, existing_menu, settings| {
        trigger.propagate(false);
        if !existing_menu.is_empty() {
            return;
        }
        commands.entity(p).try_insert(HasRelatedDrawer);
        commands
            .spawn((
                Name::new("toggles"),
                TogglesMenuMarker,
                super::drawer::AutoDrawerCommand::On,
                AutoDrawer::new(
                    Vec2::new(20.0, 220.0),
                    Vec2::new(220.0, 220.0),
                    Color::srgba_u8(0, 0, 0, 0),
                    Color::srgba_u8(0, 0, 0, 230),
                )
                .despawn_on_close()
                .max_mode()
                .with_secs_to_close(2.0)
                .with_related(p)
                .with_fade_out_children_on_close(),
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    position_type: PositionType::Absolute,
                    border: UiRect::all(Val::Px(4.)),
                    width: Val::Px(220.),
                    height: Val::Px(220.),
                    left: Val::Px(100.),
                    bottom: Val::Px(20.0),
                    padding: UiRect {
                        left: Val::Px(20.0),
                        right: Val::Px(20.0),
                        top: Val::Px(20.0),
                        bottom: Val::Px(20.0),
                    },
                    ..default()
                },
                BorderRadius::all(Val::Px(40.)),
                BackgroundColor(Color::srgba_u8(0, 0, 0, 0)),
            ))
            .with_children(|c| {
                c.spawn(create_toggle_icon_frame_bundle())
                    .menu_button_switch_ex::<LabelsMode>(
                        settings.labels_mode.clone(),
                        vec![
                            asset_server.load("icons/icon-labels-text.ktx2"),
                            asset_server.load("icons/icon-labels-numbers.ktx2"),
                            asset_server.load("icons/icon-labels-all.ktx2"),
                            asset_server.load("icons/icon-labels-clear.ktx2"),
                        ],
                        64.0,
                    )
                    .tooltip_on_hover("Toggle labels visibility", 1.0)
                    .menu_button_hover_effect()
                    .observe(
                        |trigger: On<Pointer<Click>>,
                         labels_modes: Query<&LabelsMode>,
                         mut commands: Commands,
                         mut settings: ResMut<AppSettings>| {
                            if let Ok(current_mode) = labels_modes.get(trigger.entity) {
                                let mut next_mode = current_mode.clone();
                                next_mode.cycle();
                                settings.labels_mode = next_mode;
                                commands.trigger(DespawnLabels);
                                commands.entity(trigger.entity).trigger(|entity| {
                                    ToggleButtonSwitcherEx {
                                        entity,
                                        trigger_state_as_event: false,
                                        insert_state_as_resource: false,
                                    }
                                });
                            }
                        },
                    );
                c.spawn(create_toggle_icon_frame_bundle())
                    .with_children(|c| {
                        c.spawn(create_toggle_icon_bundle(
                            &asset_server,
                            "icons/icon-daynight.ktx2",
                            Val::Px(70.0),
                            true,
                        ))
                        .insert(ToggleDayNightIconMarker);
                    })
                    .tooltip_on_hover("Toggle day/night mode", 1.0)
                    .menu_button_hover_effect()
                    .observe(
                        |_: On<Pointer<Click>>,
                         mut commands: Commands,
                         hexmap_time: Single<&HexMapTime>| {
                            toggle_day_night(&mut commands, &hexmap_time);
                        },
                    );

                spawn_audio_toggle(
                    c,
                    create_toggle_icon_frame_bundle(),
                    asset_server.as_ref(),
                );

                c.spawn(create_toggle_icon_frame_bundle())
                    .with_child(create_toggle_icon_bundle(
                        &asset_server,
                        "icons/icon-theme-picker.ktx2",
                        Val::Px(70.0),
                        true,
                    ))
                    .tooltip_on_hover("Toggle map theme", 1.0)
                    .menu_button_hover_effect()
                    .observe(
                        |_: On<Pointer<Click>>,
                         mut commands: Commands,
                         handle: Res<TileSetThemesMetadata>,
                         current_theme: Res<HexmapTheme>| {
                            toggle_map_theme(&mut commands, &handle, &current_theme);
                        },
                    );

                c.spawn(make_auto_drawer_sensor());
            });
    }
}

pub fn update_daynight_toggle(
    time: Single<&HexMapTime>,
    mut icon: Single<&mut UiTransform, With<ToggleDayNightIconMarker>>,
) {
    icon.rotation = Rot2::degrees(180.0 * time.day_night_analog);
}

pub fn create_toggle_icon_frame_bundle() -> impl Bundle {
    (
        Node {
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

pub fn create_toggle_icon_bundle(
    asset_server: &AssetServer,
    icon: &str,
    size: Val,
    enabled: bool,
) -> impl Bundle {
    (
        MenuIconMarker { enabled },
        ImageNode {
            color: Color::srgba_u8(255, 255, 255, 255),
            image: asset_server.load(icon.to_string()),
            ..default()
        },
        Node {
            overflow: Overflow::clip_x(),
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

pub fn spawn_audio_toggle<T>(
    c: &mut RelatedSpawnerCommands<'_, ChildOf>,
    bundle: T,
    asset_server: &AssetServer,
) where
    T: Bundle,
{
    c.spawn(bundle)
        .menu_button_switch_ex::<AudioToggle>(
            AudioToggle::default(),
            vec![
                asset_server.load("icons/icon-cassette-on.ktx2"),
                asset_server.load("icons/icon-cassette-off.ktx2"),
            ],
            64.0,
        )
        .tooltip_on_hover("Mute/Unmute hexmap soundscapes", 1.0)
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
}

#[derive(Component, PartialEq)]
pub struct TogglesMenuMarker;

#[derive(Component, PartialEq)]
pub struct ToggleDayNightIconMarker;

impl Switch for AudioToggle {
    fn rotate(&self) -> Self {
        match self {
            AudioToggle::On => AudioToggle::Off,
            AudioToggle::Off => AudioToggle::On,
        }
    }

    fn index(&self) -> usize {
        match self {
            AudioToggle::On => 0,
            AudioToggle::Off => 1,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => AudioToggle::On,
            1 => AudioToggle::Off,
            _ => unreachable!(),
        }
    }
}

impl Switch for LabelsMode {
    fn rotate(&self) -> Self {
        match self {
            LabelsMode::RegionsAndAreasOnly => LabelsMode::HexCoordinatesOnly,
            LabelsMode::HexCoordinatesOnly => LabelsMode::All,
            LabelsMode::All => LabelsMode::None,
            LabelsMode::None => LabelsMode::RegionsAndAreasOnly,
        }
    }

    fn index(&self) -> usize {
        match self {
            LabelsMode::RegionsAndAreasOnly => 0,
            LabelsMode::HexCoordinatesOnly => 1,
            LabelsMode::All => 2,
            LabelsMode::None => 3,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => LabelsMode::RegionsAndAreasOnly,
            1 => LabelsMode::HexCoordinatesOnly,
            2 => LabelsMode::All,
            3 => LabelsMode::None,
            _ => unreachable!(),
        }
    }
}

pub(crate) fn toggle_day_night(commands: &mut Commands, hexmap_time: &HexMapTime) {
    let day_night = hexmap_time.toggle();
    commands.trigger(ToggleDayNight {
        value: day_night.clone(),
    });
    commands.trigger(SyncMapForPeers(MapMessage::SwitchDayNight(day_night)));
}

pub(crate) fn toggle_map_theme(
    commands: &mut Commands,
    handle: &TileSetThemesMetadata,
    current_theme: &HexmapTheme,
) {
    let next_theme_name =
        rotate_key(handle.theme_names.as_ref().unwrap(), &current_theme.name)
            .unwrap()
            .to_string();
    commands.trigger(LoadHexmapTheme {
        theme: next_theme_name.clone(),
    });
    commands.trigger(SyncMapForPeers(MapMessage::ChangeTheme(next_theme_name)));
}
