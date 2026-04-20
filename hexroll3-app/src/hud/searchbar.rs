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

use bevy::{input_focus::InputFocus, prelude::*, text::FontSmoothing};
use bevy_simple_scroll_view::{ScrollView, ScrollableContent};
use bevy_ui_text_input::{
    TextInputBuffer, TextInputMode, TextInputNode, TextInputPlugin, TextInputPrompt,
};

use super::{
    OpenSearchBarAndFocus,
    drawer::{
        AutoDrawerButManual, AutoDrawerClosed, AutoDrawerVisiblity, HasRelatedDrawer,
        make_auto_drawer_sensor,
    },
};
use crate::{
    clients::{
        controller::{SearchEntitiesInBackend, ShowSearchResults},
        model::{FetchEntityReason, SearchResponse},
    },
    hexmap::elements::FetchEntityFromStorage,
    hud::drawer::AutoDrawer,
    shared::{
        AppState,
        input::InputMode,
        vtt::VttData,
        widgets::{
            buttons::{
                MenuButtonSwitcher, MenuButtonSwitcherIconShown, MenuButtonSwitcherState,
                ToggleButtonSwitcher,
            },
            list::{
                ItemSelected, ListDismissed, SelectableItemsContainer, SelectableListItem,
            },
        },
    },
};

pub struct SearchBarPlugin;

impl Plugin for SearchBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TextInputPlugin)
            .add_systems(OnEnter(AppState::Live), setup)
            .add_systems(Update, (initiate_search, search_spinner, search_hotkey))
            .add_observer(on_open_and_focus)
            .add_observer(on_received_search_results);
    }
}

fn on_open_and_focus(
    _: On<OpenSearchBarAndFocus>,
    mut commands: Commands,
    search_bar: Single<Entity, With<SearchBar>>,
    search_text: Single<Entity, (With<SearchText>, Without<SearchBar>)>,
) {
    commands.entity(*search_bar).try_insert(AutoDrawerButManual);
    commands.insert_resource(InputFocus(Some(*search_text)));
}

#[derive(Component)]
struct SearchText;

#[derive(Component)]
struct SearchBar;

#[derive(Component)]
struct SearchResultsDrawer;

#[derive(Component)]
struct SearchResultsPanel;

#[derive(Component)]
struct SearchIcon;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            Name::new("search"),
            SearchBar,
            AutoDrawer::new(
                Vec2::new(80.0, 75.0),
                Vec2::new(400.0, 75.0),
                Color::srgba_u8(0, 0, 0, 230),
                Color::srgba_u8(0, 0, 0, 230),
            ),
            AutoDrawerVisiblity::VisibleToRefereeOnly,
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(Val::Px(4.)),
                width: Val::Px(80.),
                height: Val::Px(75.),
                right: Val::Px(20.),
                top: Val::Px(20.),
                overflow: Overflow::clip(),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },
            Transform::from_xyz(0., 30., 0.),
            BorderRadius::all(Val::Percent(50.)),
            BackgroundColor(Color::srgba_u8(0, 0, 0, 230)),
        ))
        .with_children(|c| {
            c.spawn((
                Node {
                    width: Val::Px(64.0),
                    height: Val::Px(64.0),
                    margin: UiRect::left(Val::Px(3.0)),
                    align_self: AlignSelf::Center,
                    ..default()
                },
                Pickable {
                    should_block_lower: false,
                    is_hoverable: true,
                },
                UiTransform::from_scale(Vec2::new(0.7, 0.7)),
                SearchIcon,
                MenuButtonSwitcherState::Idle,
            ))
            .menu_button_switch::<SearchIconIdle, SearchIconWaiting>(
                asset_server.load("icons/icon-search.ktx2"),
                asset_server.load("icons/icon-spinner.ktx2"),
                64.0,
            );
            c.spawn((
                SearchText,
                TextInputPrompt {
                    text: "Search...".to_string(),
                    color: Some(Color::srgba(1.0, 1.0, 1.0, 0.1)),
                    ..Default::default()
                },
                TextInputNode {
                    clear_on_submit: false,
                    mode: TextInputMode::SingleLine,
                    ..default()
                },
                Node {
                    left: Val::Px(5.),
                    width: Val::Px(280.),
                    top: Val::Px(20.),
                    ..default()
                },
                Pickable {
                    is_hoverable: true,
                    should_block_lower: false,
                },
            ));
            c.spawn(make_auto_drawer_sensor());
        })
        .observe(|_: On<AutoDrawerClosed>, mut commands: Commands| {
            commands.insert_resource(InputFocus(None));
        });
}

fn initiate_search(
    mut commands: Commands,
    mut last: Local<String>,
    mut timer: Local<Option<f32>>,
    time: Res<Time>,
    query: Query<&TextInputBuffer, With<SearchText>>,
    icon: Query<Entity, With<SearchIcon>>,
) {
    if let (Some(text), Some(icon)) = (query.iter().next(), icon.iter().next()) {
        if *last != text.get_text() && text.get_text() != "" {
            *timer = Some(0.5);
            *last = text.get_text();
        }
        if let Some(timer_secs) = *timer {
            *timer = Some(timer_secs - time.delta_secs());
            if timer_secs < 0.0 {
                commands
                    .entity(icon)
                    .trigger(|entity| ToggleButtonSwitcher {
                        entity,
                        state: MenuButtonSwitcherState::Toggled,
                    });

                commands.trigger(SearchEntitiesInBackend {
                    query: text.get_text(),
                });
                *timer = None;
            }
        }
    }
}

fn populate_search_results_panel(
    commands: &mut Commands,
    panel: Entity,
    data: &SearchResponse,
    asset_server: &Res<AssetServer>,
) {
    commands.entity(panel).with_children(|c| {
        let mut items_container = c.spawn((
            ScrollableContent::default(),
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                margin: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            Pickable {
                is_hoverable: true,
                should_block_lower: false,
            },
        ));
        let mut items_container_list = SelectableItemsContainer::default();
        items_container.with_children(|c| {
            for r in data.results.iter() {
                let result = r.clone();
                let item_entity = c
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            margin: UiRect {
                                left: Val::Px(0.0),
                                right: Val::Px(00.0),
                                top: Val::Px(2.0),
                                bottom: Val::Px(2.0),
                            },
                            ..default()
                        },
                        BackgroundColor::from(Srgba::new(1.0, 0.0, 0.0, 0.01)),
                        Pickable {
                            is_hoverable: true,
                            should_block_lower: false,
                        },
                    ))
                    .with_children(|c| {
                        c.spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                width: Val::Percent(100.0),
                                ..default()
                            },
                            Pickable {
                                is_hoverable: true,
                                should_block_lower: false,
                            },
                        ))
                        .with_children(|c| {
                            c.spawn((
                                TextColor(Srgba::new(1.0, 1.0, 1.0, 0.9).into()),
                                TextFont {
                                    font: asset_server.load("fonts/FiraSans-Regular.ttf"),
                                    font_size: 20.0,
                                    font_smoothing: FontSmoothing::AntiAliased,
                                    ..default()
                                },
                                Text::new(format!("{}", r.value)),
                                TextLayout {
                                    ..Default::default()
                                },
                                Pickable {
                                    is_hoverable: true,
                                    should_block_lower: false,
                                },
                            ));
                            c.spawn((
                                TextColor(Srgba::new(1.0, 1.0, 1.0, 0.9).into()),
                                TextFont {
                                    font: asset_server.load("fonts/FiraSans-Regular.ttf"),
                                    font_size: 10.0,
                                    font_smoothing: FontSmoothing::AntiAliased,
                                    ..default()
                                },
                                Text::new(format!("{}", r.details)),
                                TextLayout {
                                    ..Default::default()
                                },
                                Pickable {
                                    is_hoverable: true,
                                    should_block_lower: false,
                                },
                            ));
                        });

                        c.spawn((
                            Node {
                                height: Val::Px(50.0),
                                ..default()
                            },
                            ImageNode {
                                color: Srgba::new(1.0, 0.0, 0.0, 0.01).into(),
                                image: asset_server.load("icons/icon-skull-256.ktx2"),
                                ..default()
                            },
                            Pickable {
                                is_hoverable: true,
                                should_block_lower: false,
                            },
                        ));
                    })
                    .observe(move |_: On<ItemSelected>, mut commands: Commands| {
                        commands.trigger(FetchEntityFromStorage {
                            uid: result.uuid.clone(),
                            anchor: Some(result.anchor.clone()),
                            why: FetchEntityReason::SandboxLink,
                        });
                    })
                    .make_selectable()
                    .id();
                items_container_list.children.push(item_entity);
            }
        });
        items_container.observe(
            |_: On<ListDismissed>,
             mut commands: Commands,
             results_drawer: Single<Entity, With<SearchResultsDrawer>>| {
                commands
                    .entity(*results_drawer)
                    .try_remove::<AutoDrawerButManual>();
            },
        );
        items_container.insert(items_container_list);
    });
}

fn on_received_search_results(
    trigger: On<ShowSearchResults>,
    mut commands: Commands,
    searchbar: Query<Entity, With<SearchBar>>,
    panel: Query<(Entity, &ChildOf), With<SearchResultsPanel>>,
    asset_server: Res<AssetServer>,
    icon: Query<Entity, With<SearchIcon>>,
) {
    if let Some(icon) = icon.iter().next() {
        commands
            .entity(icon)
            .trigger(|entity| ToggleButtonSwitcher {
                entity,
                state: MenuButtonSwitcherState::Idle,
            });
    }
    if trigger.search_response.results.is_empty() {
        return;
    }
    for p in searchbar.iter() {
        let (e, parent) = if panel.is_empty() {
            commands.entity(p).try_insert(HasRelatedDrawer);
            let parent = commands
                .spawn((
                    Name::new("search_results"),
                    SearchResultsDrawer,
                    AutoDrawerButManual,
                    AutoDrawer::new(
                        Vec2::new(400.0, 75.0),
                        Vec2::new(400.0, 600.0),
                        Color::srgba_u8(0, 0, 0, 0),
                        Color::srgba_u8(0, 0, 0, 230),
                    )
                    .despawn_on_close()
                    .with_secs_to_close(2.0)
                    .max_mode()
                    .with_related(p),
                    Node {
                        position_type: PositionType::Absolute,
                        border: UiRect::all(Val::Px(4.)),
                        width: Val::Px(400.),
                        max_height: Val::Px(75.),
                        right: Val::Px(20.),
                        top: Val::Px(105.0),
                        padding: UiRect {
                            left: Val::Px(20.0),
                            right: Val::Px(20.0),
                            top: Val::Px(20.0),
                            bottom: Val::Px(20.0),
                        },
                        ..default()
                    },
                    Transform::from_xyz(0., 30., 0.),
                    BorderRadius::all(Val::Px(20.)),
                    BackgroundColor(Color::srgba_u8(0, 0, 0, 0)),
                ))
                .id();
            let e = commands
                .spawn((
                    Name::new("search_results"),
                    ChildOf(parent),
                    SearchResultsPanel,
                    ScrollView {
                        scroll_speed: 9000.0,
                    },
                    Node {
                        width: Val::Percent(100.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                ))
                .id();
            (e, parent)
        } else {
            let (e, childof) = panel.iter().next().unwrap();
            (e, childof.0)
        };
        commands.entity(e).despawn_related::<Children>();
        populate_search_results_panel(
            &mut commands,
            e,
            &trigger.search_response,
            &asset_server,
        );

        // NOTE: The drawer sensor is deliberatly created last so it will
        // occupy the entire search results panel height.
        commands.entity(parent).with_children(|c| {
            c.spawn(make_auto_drawer_sensor());
        });
    }
}

fn search_spinner(
    mut spinner: Query<
        &mut UiTransform,
        (With<SearchIconWaiting>, With<MenuButtonSwitcherIconShown>),
    >,
) {
    if let Some(mut spinner_transform) = spinner.iter_mut().next() {
        spinner_transform.rotation =
            Rot2::radians(spinner_transform.rotation.as_radians() + 0.01);
    }
}

fn search_hotkey(
    mut commands: Commands,
    input_mode: Res<InputMode>,
    keyboard: Res<ButtonInput<KeyCode>>,
    vtt_data: Res<VttData>,
) {
    if vtt_data.mode.is_player() {
        return;
    }
    if keyboard.just_released(KeyCode::Backquote) && input_mode.keyboard_available() {
        commands.trigger(OpenSearchBarAndFocus);
    }
}

#[derive(Component, Default)]
struct SearchIconIdle;

#[derive(Component, Default)]
struct SearchIconWaiting;
