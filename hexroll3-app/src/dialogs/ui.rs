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
use bevy::{input_focus::InputFocus, prelude::*};
use bevy_ui_text_input::{actions::TextInputAction, *};
use rand::Rng;
use rand::distributions::Alphanumeric;

use crate::{
    clients::{
        RemoteBackendEvent, StandaloneBackendEvent,
        controller::{
            PostMapLoadedOp, RequestMapFromBackend, RequestMapResult,
            RequestSandboxFromBackend, RequestVttSessionFromBackend,
        },
        model::SandboxMode,
        roll_new_sandbox,
    },
    hexmap::{
        MapEditor, PenType, TerrainType,
        elements::{HexMapToolState, VttDataState},
    },
    shared::{
        settings::{SandboxRef, UserSettings},
        vtt::{HexMapMode, LoadVttState, VttData},
        widgets::{
            Button, ButtonSpawner, InputSpawner, LayoutSpawner, TextButton, border_radius_pct,
            modal::{DiscreteAppState, ModalWindow},
            text_centered,
        },
    },
    vtt::network::ConnectVtt,
};

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(show_pairing_modal)
            .add_observer(show_join_vtt_modal)
            .add_observer(show_sandbox_options)
            .add_observer(fetch_map_result);
    }
}

#[derive(Event)]
pub struct OpenPairingModal;
#[derive(Event)]
pub struct OpenJoinVTTModal;

#[derive(Component)]
struct SandboxIdInput;

#[derive(Component)]
struct VttNodeNameInput;

use chrono::Utc;

use super::OpenSandboxOptionsModal;

fn time_ago(timestamp: u64) -> String {
    let now = Utc::now().timestamp() as u64;
    let elapsed = now - timestamp;

    let one_minute = 60;
    let one_hour = one_minute * 60;
    let one_day = one_hour * 24;
    let one_week = one_day * 7;
    let one_month = one_day * 30;

    match elapsed {
        _ if elapsed < one_minute => format!("just now"),
        _ if elapsed < one_hour => {
            let minutes = elapsed / one_minute;
            format!(
                "{} minute{} ago",
                minutes,
                if minutes > 1 { "s" } else { "" }
            )
        }
        _ if elapsed < one_day => {
            let hours = elapsed / one_hour;
            format!("{} hour{} ago", hours, if hours > 1 { "s" } else { "" })
        }
        _ if elapsed < one_week => {
            let days = elapsed / one_day;
            format!("{} day{} ago", days, if days > 1 { "s" } else { "" })
        }
        _ if elapsed < one_month => {
            let weeks = elapsed / one_week;
            format!("{} week{} ago", weeks, if weeks > 1 { "s" } else { "" })
        }
        _ => {
            let months = elapsed / one_month;
            format!("{} month{} ago", months, if months > 1 { "s" } else { "" })
        }
    }
}

fn show_sandbox_options(
    _: On<OpenSandboxOptionsModal>,
    mut next_state: ResMut<NextState<DiscreteAppState>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    user_settings: Res<UserSettings>,
) {
    next_state.set(DiscreteAppState::Modal);
    commands
        .spawn(modal_window("PairSandboxDialog"))
        .with_children(|c| {
            c.spawn_row(AlignItems::Start, |c| {
                c.spawn_col(|c| {
                    // c.spawn_spacer();
                    c.spawn(text_centered("Your sandboxes"));
                    c.spawn_spacer();
                    c.spawn_list(|c| {
                        user_settings.sandboxes.iter().for_each(|v| {
                            let v_sid = v.sandbox.clone().unwrap();
                            if v.local.unwrap_or(false) && !UserSettings::sandbox_exists(&v_sid) {
                                return;
                            }
                            let v_key = v.key.clone();
                            c.spawn_list_item(
                                &format!(
                                    "{} ({})",
                                    v.sandbox.clone().unwrap(),
                                    time_ago(v.last_used.clone().unwrap())
                                ),
                                Val::Px(360.0),
                                Val::Percent(20.0),
                            ).observe(
                                move |_: On<Pointer<Click>>, mut commands:Commands,
                                    mut user_settings: ResMut<UserSettings>, mut next_state: ResMut<NextState<DiscreteAppState>> | {
                                    if let Some(current_sandbox_id) = &user_settings.sandbox {
                                        if *current_sandbox_id == v_sid.to_owned() {
                                            next_state.set(DiscreteAppState::Normal);
                                            return;
                                        }
                                    }
                                    if v_key.is_none() {
                                        user_settings.local = Some(true);
                                    } else {
                                        user_settings.local = None;
                                    }
                                    commands.trigger(RequestSandboxFromBackend {
                                        sandbox_uid: v_sid.to_owned().to_string(),
                                        pairing_key: v_key.clone(),
                                    });
                                },
                            );
                        });
                    });
                });
                c.spawn_spacer();
                c.spawn_col(|c| {
                    c.spawn_spacer();
                    c.spawn_text_button(
                        TextButton::from_text(
                            "Roll a new sandbox",
                            Button::from_image(asset_server.load("icons/icon-dice-256.ktx2"))
                                // .color(Color::srgba_u8(255, 255, 255, 50))
                                .button_size(Val::Px(60.0))
                                .image_size(Val::Px(32.0))
                                .border_radius(Val::Percent(20.0)),
                        )
                        .width(Val::Px(360.0)),
                    )
                        .observe(|_: On<Pointer<Click>>,
                                  mut user_settings: ResMut<UserSettings>,
                                  mut next_state: ResMut<NextState<DiscreteAppState>>,
                                  modals: Query<Entity, With<ModalWindow>>,
                                  mut commands: Commands | {
                                    let mut rng = rand::thread_rng();
                                    let sandbox_id : String= (0..10).map(|_| rng.sample(Alphanumeric) as char).collect();

                                    if let Ok(_) = roll_new_sandbox(&sandbox_id) {
                                        user_settings.sandbox = Some(sandbox_id.clone());
                                                            user_settings.local=Some(true);
                                        if let Some(existing) = user_settings
                                            .sandboxes
                                            .iter_mut()
                                            .find(|s| s.sandbox.as_ref() == Some(&sandbox_id))
                                        {
                                            existing.last_used = Some(chrono::Utc::now().timestamp() as u64);
                                        } else {
                                            user_settings.sandboxes.push(SandboxRef {
                                                sandbox: Some(sandbox_id.clone()),
                                                key: None,
                                                last_used: Some(chrono::Utc::now().timestamp() as u64),
                                                local: Some(true)
                                            });
                                        }
                                        commands.trigger(StandaloneBackendEvent(RequestSandboxFromBackend {
                                            sandbox_uid: sandbox_id.to_string(),
                                            pairing_key: None,
                                        }));
                                        next_state.set(DiscreteAppState::Normal);
                                        user_settings.save();
                                        for modal in modals.iter() {
                                            commands.entity(modal).try_despawn();
                                        }
                                        commands.run_system_cached(show_new_sandbox_options);
                                    }
                                });
                    c.spawn_spacer();
                    c.spawn_text_button(
                        TextButton::from_text(
                            "Join a VTT session",
                            Button::from_image(asset_server.load("icons/icon-dice-256.ktx2"))
                                .button_size(Val::Px(60.0))
                                .image_size(Val::Px(32.0))
                                .border_radius(Val::Percent(20.0)),
                        )
                            .width(Val::Px(360.0))).observe(
                        |_: On<Pointer<Click>>,
                         mut commands: Commands,
                         modals: Query<Entity, With<ModalWindow>>| {
                            for modal in modals.iter() {
                                commands.entity(modal).despawn();
                            }
                            commands.trigger(OpenJoinVTTModal);
                        },
                    );
                    c.spawn_spacer();
                    c.spawn_text_button(
                        TextButton::from_text(
                            "Pair sandbox",
                            Button::from_image(asset_server.load("icons/icon-paste-256.ktx2"))
                                .button_size(Val::Px(60.0))
                                .image_size(Val::Px(32.0))
                                .border_radius(Val::Percent(20.0)),
                        )
                            .width(Val::Px(360.0))).observe(
                        |_: On<Pointer<Click>>,
                         mut commands: Commands,
                         modals: Query<Entity, With<ModalWindow>>| {
                            for modal in modals.iter() {
                                commands.entity(modal).despawn();
                            }
                            commands.trigger(OpenPairingModal);
                        },
                    );
                    c.spawn_spacer();
                    c.spawn_text_button(
                        TextButton::from_text(
                            "Quit to desktop",
                            Button::from_image(asset_server.load("icons/icon-exit-256.ktx2"))
                                .button_size(Val::Px(60.0))
                                .image_size(Val::Px(32.0))
                                .border_radius(Val::Percent(20.0)),
                        )
                            .width(Val::Px(360.0))).observe(
                        |_: On<Pointer<Click>>, mut exit: MessageWriter<AppExit>| {
                            exit.write(AppExit::Success);
                        },
                    );
                });
            });
        });
}

fn show_new_sandbox_options(
    mut next_state: ResMut<NextState<DiscreteAppState>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    next_state.set(DiscreteAppState::Modal);
    commands
        .spawn(modal_window("NewSandbox"))
        .with_children(|c| {
            c.spawn(text_centered("Choose your first realm"));
            c.spawn_spacer();
            c.spawn_row_with_wrap(AlignItems::Center, |c| {
                let mut button = |realm_type: &str| {
                    let realm_type = realm_type.to_string();
                    c.spawn_text_button(
                        TextButton::from_text(
                            &realm_type,
                            Button::from_image(asset_server.load(format!("icons/icon-realm-{}.ktx2", realm_type.to_lowercase())))
                                .button_size(Val::Px(60.0))
                                .image_size(Val::Px(48.0))
                                .border_radius(Val::Percent(20.0)),
                        )
                        .width(Val::Px(200.0)),
                    )
                    .observe(
                        move |_: On<Pointer<Click>>,
                        mut next_state: ResMut<NextState<DiscreteAppState>>,
                        mut editor: ResMut<MapEditor>,
                        mut next_tool_state: ResMut<NextState<HexMapToolState>>
                        | {
                            next_tool_state.set(HexMapToolState::Edit);
                            editor.realm_type = format!("RealmType{}", realm_type);
                            editor.pen = PenType::Brush;
                            editor.terrain = TerrainType::MountainsHex;
                            next_state.set(DiscreteAppState::Normal);
                        },
                    );
                };
                button("Lands");
                button("Empire");
                button("Kingdom");
                button("Duchy");
            });
        });
}

fn show_pairing_modal(
    _: On<OpenPairingModal>,
    mut next_state: ResMut<NextState<DiscreteAppState>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    next_state.set(DiscreteAppState::Modal);
    commands
        .spawn(modal_window("PairSandboxDialog"))
        .with_children(|c| {
            c.spawn(text_centered(
                "Copy your sandbox pairing key from the web \
                 application",
            ));
            c.spawn_spacer();
            c.spawn(text_centered("and paste it here:"));
            c.spawn_spacer();
            c.spawn_row(AlignItems::Center, |c| {
                c.spawn_button(
                    Button::from_image(asset_server.load("icons/icon-paste-256.ktx2"))
                        .button_size(Val::Px(60.0))
                        .image_size(Val::Px(32.0))
                        .border_radius(Val::Percent(20.0)),
                )
                .observe(
                    |_: On<Pointer<Click>>,
                     mut iq: Query<(Entity, &mut TextInputQueue), With<SandboxIdInput>>,
                     mut focus: ResMut<InputFocus>| {
                        let Some((e, mut iq)) = iq.iter_mut().next() else {
                            return;
                        };
                        focus.0 = Some(e);
                        iq.add(TextInputAction::Paste);
                    },
                );
                c.spawn_input(Val::Px(540.0), 18.0, SandboxIdInput)
                    .observe(|_: On<Pointer<Click>>| {});
            });
            c.spawn_spacer();
            c.spawn(text_centered("Then press play button to pair"));
            c.spawn_spacer();
            c.spawn_button(
                Button::from_image(asset_server.load("icons/icon-play-256.ktx2"))
                    .button_size(Val::Px(100.0))
                    .image_size(Val::Px(128.0))
                    .border_radius(Val::Percent(50.0)),
            )
            .observe(
                |_: On<Pointer<Click>>,
                 mut commands: Commands,
                 user_settings: Res<UserSettings>,
                 query: Query<&TextInputBuffer, With<SandboxIdInput>>,
                 mut next_state: ResMut<NextState<DiscreteAppState>>| {
                    if let Some(text) = query.iter().next() {
                        let pairing_text = text.get_text();
                        if pairing_text.len() == 40 {
                            let (sandbox_uid, pairing_key) = pairing_text.split_at(8);
                            if let Some(current_sandbox_id) = &user_settings.sandbox {
                                if current_sandbox_id == &sandbox_uid {
                                    next_state.set(DiscreteAppState::Normal);
                                    return;
                                }
                            }
                            commands.trigger(RemoteBackendEvent(RequestSandboxFromBackend {
                                sandbox_uid: sandbox_uid.to_string(),
                                pairing_key: Some(pairing_key.to_string()),
                            }));
                        } else {
                            // Handle the error case where the length is not as expected.
                        }
                    }
                },
            );
        });
}

fn show_join_vtt_modal(
    _: On<OpenJoinVTTModal>,
    mut next_state: ResMut<NextState<DiscreteAppState>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    next_state.set(DiscreteAppState::Modal);
    commands
        .spawn(modal_window("JoinVTTDialog"))
        .with_children(|c| {
            c.spawn(text_centered("Copy & paste your VTT Sandbox code here:"));
            c.spawn_spacer();
            c.spawn_row(AlignItems::Center, |c| {
                c.spawn_button(
                    Button::from_image(asset_server.load("icons/icon-paste-256.ktx2"))
                        .button_size(Val::Px(60.0))
                        .image_size(Val::Px(32.0))
                        .border_radius(Val::Percent(20.0)),
                )
                .observe(
                    |_: On<Pointer<Click>>,
                     mut iq: Query<(Entity, &mut TextInputQueue), With<SandboxIdInput>>,
                     mut focus: ResMut<InputFocus>| {
                        let Some((e, mut iq)) = iq.iter_mut().next() else {
                            return;
                        };
                        focus.0 = Some(e);
                        iq.add(TextInputAction::Paste);
                    },
                );
                c.spawn_input(Val::Px(240.0), 40.0, SandboxIdInput)
                    .observe(|_: On<Pointer<Click>>| {});
            });
            c.spawn_spacer();
            c.spawn(text_centered("name your node:"));
            c.spawn_spacer();
            c.spawn_input(Val::Px(340.0), 40.0, VttNodeNameInput)
                .observe(|_: On<Pointer<Click>>| {});
            c.spawn_spacer();
            c.spawn(text_centered("and press the play button:"));
            c.spawn_spacer();
            c.spawn_button(
                Button::from_image(asset_server.load("icons/icon-play-256.ktx2"))
                    .button_size(Val::Px(100.0))
                    .image_size(Val::Px(128.0))
                    .border_radius(Val::Percent(50.0)),
            )
            .observe(
                |_: On<Pointer<Click>>,
                 mut commands: Commands,
                 user_settings: Res<UserSettings>,
                 sandbox_input: Query<
                    &TextInputBuffer,
                    (With<SandboxIdInput>, Without<VttNodeNameInput>),
                >,
                 node_input: Query<
                    &TextInputBuffer,
                    (With<VttNodeNameInput>, Without<SandboxIdInput>),
                >,
                 mut next_state: ResMut<NextState<DiscreteAppState>>| {
                    let Some(text) = sandbox_input.iter().next() else {
                        return;
                    };
                    let Some(node) = node_input.iter().next() else {
                        return;
                    };
                    let sandbox_uid = text.get_text();
                    let node_name = node.get_text();
                    if sandbox_uid.len() == 8 {
                        if let Some(current_sandbox_id) = &user_settings.sandbox {
                            if current_sandbox_id == &sandbox_uid {
                                next_state.set(DiscreteAppState::Normal);
                                return;
                            }
                        }
                        commands.trigger(RequestVttSessionFromBackend {
                            sandbox_uid,
                            node_name,
                        });
                    } else {
                        // TODO: handle failure here
                    }
                },
            );
        });
}

fn fetch_map_result(
    trigger: On<RequestMapResult>,
    mut user_settings: ResMut<UserSettings>,
    mut next_state: ResMut<NextState<DiscreteAppState>>,
    mut next_vtt_data_state: ResMut<NextState<VttDataState>>,
    mut commands: Commands,
) {
    match trigger.event() {
        RequestMapResult::Loaded(sandbox_uid, pairing_key) => {
            user_settings.sandbox = Some(sandbox_uid.clone());
            user_settings.key = pairing_key.clone();
            user_settings.local = Some(pairing_key.is_none());
            if let Some(existing) = user_settings
                .sandboxes
                .iter_mut()
                .find(|s| s.sandbox.as_ref() == Some(&sandbox_uid))
            {
                existing.key = pairing_key.clone();
                existing.last_used = Some(chrono::Utc::now().timestamp() as u64);
            } else {
                user_settings.sandboxes.push(SandboxRef {
                    sandbox: Some(sandbox_uid.clone()),
                    local: Some(pairing_key.is_none()),
                    key: pairing_key.clone(),
                    last_used: Some(chrono::Utc::now().timestamp() as u64),
                });
            }
            commands.trigger(RequestMapFromBackend {
                post_map_loaded_op: PostMapLoadedOp::Initialize(SandboxMode::Referee),
            });
            commands.trigger(LoadVttState);
            next_state.set(DiscreteAppState::Normal);
            user_settings.save();
        }
        RequestMapResult::Joined(sandbox_uid, node_name) => {
            user_settings.sandbox = Some(sandbox_uid.clone());
            user_settings.key = Some("".to_string());
            commands.trigger(RequestMapFromBackend {
                post_map_loaded_op: PostMapLoadedOp::Initialize(SandboxMode::Player),
            });
            let mut vtt_data = VttData::default();
            vtt_data.mode = HexMapMode::Player;
            vtt_data.node_name = node_name.to_string();
            // TODO: At this point we could still have some leftover vtt
            // data, specifically anything tokens related. We rely on the
            // vtt init message to trigger invalidate_state (from sync.rs)
            // to clean this up.
            // We could also invalidate state at this point, although
            // it might be redundant.
            // NOTE(vtt1): We explicitly avoid commands.trigger(TriggerLoadVttState {});
            // in player vtt sessions. This is reserved to the referee node.
            commands.insert_resource(vtt_data);
            next_vtt_data_state.set(VttDataState::Available);
            commands.trigger(ConnectVtt);
            next_state.set(DiscreteAppState::Normal);
        }
        RequestMapResult::Failed => {
            // TODO: handle failure here
        }
    }
}
fn node_modal() -> Node {
    Node {
        position_type: PositionType::Absolute,
        justify_self: JustifySelf::Center,
        align_self: AlignSelf::Center,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Start,
        flex_direction: FlexDirection::Column,
        padding: UiRect::new(Val::Px(30.0), Val::Px(30.0), Val::Px(30.0), Val::Px(30.0)),
        ..default()
    }
}

fn modal_window(name: &str) -> impl Bundle {
    (
        Name::new(name.to_string()),
        ModalWindow,
        node_modal(),
        border_radius_pct(3.0),
        Pickable {
            should_block_lower: true,
            is_hoverable: true,
        },
        BackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.95).into()),
        ZIndex(999),
    )
}
