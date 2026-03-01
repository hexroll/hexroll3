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

use std::fs::File;
use std::time::Duration;

use bevy::prelude::*;

use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

use serde_json::Value;

use bevy_tweening::{Tween, lens::TransformScaleLens};

use crate::battlemaps::BattlemapFeatureUtils;
use crate::clients::controller::TokenState;
use crate::content::{EditableAttributeParams, RenameSandboxEntity};
use crate::{
    battlemaps::{
        BattleMapConstructs, CaveMapConstructs, CityMapConstructs, DungeonMapConstructs,
        RequestCityOrTownFromBackend, RequestDungeonFromBackend, RequestVillageFromBackend,
        VillageMapConstructs,
    },
    clients::model::BackendUid,
    content::{ContentPageModel, context::ContentContext},
    hexmap::{
        HexMapJson, HexMapTileMaterials, HexmapTheme, MapMessage,
        elements::{
            AppendSandboxEntity, FetchEntityFromStorage, HexEntity, HexEntityCallbacks,
            HexMapData, HexMapResources, HexToInvalidateMarker, RerollHex, VttDataState,
            hexx_to_hexroll_coords,
        },
        prepare_hex_map_data, update_hex_map_tiles,
    },
    shared::{
        asynchttp::{ApiHandler, AsyncHttpTasks, HttpAgent},
        effects::{EffectSystems, RollNewFeatureEffect},
        settings::{AppSettings, UserSettings},
        vtt::{HexMapMode, LoadVttState, StoreVttState, VttData},
    },
    tokens::Token,
    vtt::sync::{EventContext, EventSource, SyncMapForPeers},
};

use super::controller::{ClientState, VttStateApiController};
use super::{
    controller::{RenderEntityContent, ShowSearchResults, on_ingest_battlemap_data},
    model::{FetchEntityReason, RerollEntity, SandboxMode, SearchResponse},
};

pub struct ApiClientPlugin;

impl Plugin for ApiClientPlugin {
    fn build(&self, app: &mut App) {
        app // -->
            .insert_resource(HttpAgent::default())
            .insert_resource(VttStateApiController::Unloaded)
            .add_systems(Update, handle_save_vtt_state)
            // ---------------------------------------------------------------------------------------------
            // API request and response handlers
            // ---------------------------------------------------------------------------------------------
            // Get sandbox 
            .add_observer(request_sandbox)
            .register_api_callback::<_, String, Option<String>>(receive_sandbox)
            // Join VTT
            .add_observer(request_vtt_session)
            .register_api_callback::<_, String, VttSessionResponse>(receive_vtt_session)
            // Get entity content page 
            .add_observer(fetch_hex)
            .register_api_callback::<_, String, (ContentPageModel, FetchEntityReason, Option<String>)>(receive_hex)
            // Roll a new feature
            .add_observer(append_feature)
            .register_api_callback::<_, String, FeatureUidResponse>(receive_appended_feature)
            // Rename a sandbox entity
            .add_observer(rename_entity)
            .register_api_callback::<_, String, RenamingResponse>(receive_renaming_result)
            // Save vtt state
            .add_observer(save_vtt_state)
            .register_api_callback::<_, String, StoreStateResponse>(receive_vtt_store_response)
            // Load vtt state
            .add_observer(load_vtt_state)
            .register_api_callback::<_, String, LoadStateResponse>(receive_vtt_load_response)
            // Reroll a hex
            .add_observer(reroll_hex)
            .register_api_callback::<_, String, RerollHexResponse>(receive_hex_reroll)
            // Get sandbox hexmap
            .add_observer(request_hex_map)
            .register_api_callback::<_, String, (Option<HexMapData>, PostMapLoadedOp)>(receive_hex_map)
            // Get battlemaps
            .add_observer(request_dungeon_map)
            .add_observer(request_city_or_town_map)
            .add_observer(request_village_map)
            .register_api_callback::<_, (String, Entity), (BattleMapConstructs, String)>(
                receive_battlemaps_data.after(update_hex_map_tiles),
            )
            // Reroll an entity
            .add_observer(request_a_reroll)
            .register_api_callback::<_, String, (bool, String)>(receive_reroll_response)
            // Search
            .add_observer(request_search)
            .register_api_callback::<_, String, SearchResponse>(receive_search_results)
            // Hex Map Action
            .add_observer(request_hex_action)
            .register_api_callback::<_, (String,String), String>(receive_hex_action_results)
            // ---------------------------------------------------------------------------------------------
            // <--
            ;
    }
}
// ---------------------------------------------------------------------------------------------------------
#[derive(Event, Default)]
pub struct RequestSandboxFromBackend {
    pub sandbox_uid: String,
    pub pairing_key: String,
}

pub fn request_sandbox(
    trigger: On<RequestSandboxFromBackend>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<AsyncHttpTasks<String, Option<String>>>,
    user_settings: Res<UserSettings>,
) {
    let sandbox_uid = &trigger.sandbox_uid;
    let server_host = &user_settings.server;
    let api_key = Some(trigger.pairing_key.clone());

    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{}/body/{}/root", server_host, sandbox_uid);
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/body/{}/root", sandbox_uid);

    let pairing_key = trigger.pairing_key.clone();
    let _ = http_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        sandbox_uid.clone(),
        move |data| {
            debug!("body data is {}", data);
            if data.contains("Not found") || data.contains("message") {
                None
            } else {
                Some(pairing_key.clone())
            }
        },
    );
}

pub fn receive_sandbox(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, Option<String>>>,
) {
    http_tasks.poll_responses(|uid, data| {
        if let Some(ret) = data {
            if let Some(data) = ret {
                commands.trigger(RequestMapResult::Loaded(uid.clone(), data));
            } else {
                commands.trigger(RequestMapResult::Failed);
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
#[derive(Event, Default)]
pub struct RequestVttSessionFromBackend {
    pub sandbox_uid: String,
    pub node_name: String,
}

pub struct VttSessionResponse {
    node_name: String,
}

pub fn request_vtt_session(
    trigger: On<RequestVttSessionFromBackend>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<AsyncHttpTasks<String, VttSessionResponse>>,
    user_settings: Res<UserSettings>,
) {
    let sandbox_uid = &trigger.sandbox_uid;
    let server_host = &user_settings.server;
    let api_key = Some("".to_string());

    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{}/join3/{}", server_host, sandbox_uid);
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/join/{}", sandbox_uid);

    let node_name = trigger.node_name.clone();
    let _ = http_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        sandbox_uid.clone(),
        move |_data| VttSessionResponse {
            node_name: node_name.clone(),
        },
    );
}

pub fn receive_vtt_session(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, VttSessionResponse>>,
) {
    http_tasks.poll_responses(|uid, data| {
        if let Some(data) = data {
            commands.trigger(RequestMapResult::Joined(
                uid.clone(),
                data.node_name.clone(),
            ));
        }
    });
}

#[derive(Default, Clone, Event)]
pub struct PostMapLoadedOpPrefix {
    pub post_map_op: PostMapLoadedOp,
}

#[derive(Default, Clone, PartialEq, Event)]
pub enum PostMapLoadedOp {
    #[default]
    None,
    Initialize(SandboxMode),
    InvalidateVisible,
    FetchEntity(String),
}

// ---------------------------------------------------------------------------------------------------------
#[derive(Event, Default)]
pub struct RequestMapFromBackend {
    pub post_map_loaded_op: PostMapLoadedOp,
}

// ---------------------------------------------------------------------------------------------------------
#[derive(Event, PartialEq)]
pub enum RequestMapResult {
    Loaded(String /* Sandbox Id */, String /* Key */),
    Joined(String /* Sandbox Id */, String /* Node name */),
    Failed,
}

pub fn request_hex_map(
    trigger: On<RequestMapFromBackend>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<AsyncHttpTasks<String, (Option<HexMapData>, PostMapLoadedOp)>>,
    assets: Res<HexMapResources>,
    tiles: Res<HexMapTileMaterials>,
    user_settings: Res<UserSettings>,
    theme: Res<HexmapTheme>,
    mut commands: Commands,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{}/map/{}", server_host, sandbox_uid);
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/map/{}", sandbox_uid);
    let curved_mesh_tile_set = assets.curved_mesh_tile_set.clone();
    let tiles = tiles.clone();
    let post_map_loaded_op = trigger.post_map_loaded_op.clone();
    let scale_calculator = theme.tile_scale_values();

    commands.trigger(PostMapLoadedOpPrefix {
        post_map_op: post_map_loaded_op.clone(),
    });

    let _ = http_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        sandbox_uid.clone(),
        move |data| {
            if let Ok(map) = serde_json::from_str::<HexMapJson>(&data) {
                (
                    Some(prepare_hex_map_data(
                        map,
                        curved_mesh_tile_set.clone(),
                        tiles.clone(),
                        scale_calculator,
                    )),
                    post_map_loaded_op.clone(),
                )
            } else {
                (None, post_map_loaded_op.clone())
            }
        },
    );
}

pub fn receive_hex_map(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, (Option<HexMapData>, PostMapLoadedOp)>>,
    callbacks: Res<HexEntityCallbacks>,
) {
    http_tasks.poll_responses(|_, data| {
        if let Some((data, post_map_loaded_op)) = data {
            if let Some(data) = data {
                commands.insert_resource(data);
                commands.run_system(callbacks.invalidate);
                commands.trigger(post_map_loaded_op);
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
pub fn fetch_hex(
    trigger: On<FetchEntityFromStorage>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<
        AsyncHttpTasks<String, (ContentPageModel, FetchEntityReason, Option<String>)>,
    >,
    user_settings: Res<UserSettings>,
    mut commands: Commands,
    window: Single<Entity, With<PrimaryWindow>>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/get/{}/{}", sandbox_uid, trigger.event().uid);
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/get/{}/{}", sandbox_uid, trigger.event().uid);
    let uid = trigger.event().uid.clone();
    let why = trigger.why.clone();

    commands
        .entity(*window)
        .insert(CursorIcon::System(SystemCursorIcon::Progress));

    let anchor = trigger.event().anchor.clone();
    let _ = http_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        trigger.event().uid.clone(),
        move |data| {
            (
                ContentPageModel::from_entity_html(&uid, &data),
                why.clone(),
                anchor.clone(),
            )
        },
    );
}

pub fn receive_hex(
    mut commands: Commands,
    mut http_tasks: ResMut<
        AsyncHttpTasks<String, (ContentPageModel, FetchEntityReason, Option<String>)>,
    >,
) {
    http_tasks.poll_responses(|uid, ret| {
        if let Some((data, why, anchor)) = ret {
            commands.trigger(RenderEntityContent {
                uid: uid.to_string(),
                data,
                anchor,
                why,
            });
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
pub fn append_feature(
    trigger: On<AppendSandboxEntity>,
    mut http_agent: ResMut<HttpAgent>,
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, FeatureUidResponse>>,
    // hexes: Query<(Entity, &HexEntity)>,
    effects: Res<EffectSystems>,
    user_settings: Res<UserSettings>,
    window: Single<Entity, With<PrimaryWindow>>,
) {
    commands
        .entity(*window)
        .insert(CursorIcon::System(SystemCursorIcon::Progress));

    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    let url = format!("{server_host}/append/");
    let mut json_data = serde_json::json!({
        "instance": sandbox_uid,
        "entity": trigger.event().hex_uid,
        "type": trigger.event().what,
        "attribute": trigger.event().attr,
    });

    if let Some(hex_coords) = trigger.hex_coords {
        commands
            .spawn_empty()
            .insert(RollNewFeatureEffect(hex_coords));
        commands.run_system(effects.roll_feature_effect);

        if trigger.send_coords {
            let (x, y) = hexx_to_hexroll_coords(hex_coords);
            json_data["nx"] = x.into();
            json_data["ny"] = y.into();
        }
    }
    let coords = trigger.hex_coords;
    let send_coords = trigger.send_coords;
    let hex_uid = trigger.hex_uid.clone();
    http_tasks.spawn_post(
        &mut http_agent,
        url,
        api_key,
        json_data,
        sandbox_uid.clone(),
        move |data| {
            FeatureUidResponse(
                data,
                coords,
                if send_coords {
                    None
                } else {
                    Some(hex_uid.clone())
                },
            )
        },
    );
}

// ---------------------------------------------------------------------------------------------------------
pub fn rename_entity(
    trigger: On<RenameSandboxEntity>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<AsyncHttpTasks<String, RenamingResponse>>,
    user_settings: Res<UserSettings>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    let url = format!("{server_host}/rename/");
    let json_data = serde_json::json!({
        "instance": sandbox_uid,
        "entity": trigger.event().entity_uid,
        "attr": trigger.event().params.attr_name,
        "value": trigger.event().value,
    });

    let params = trigger.event().params.clone();
    let uid = trigger.event().entity_uid.clone();

    http_tasks.spawn_post(
        &mut http_agent,
        url,
        api_key,
        json_data,
        sandbox_uid.clone(),
        move |_| RenamingResponse(uid.clone(), params.clone()),
    );
}

pub struct RenamingResponse(String, EditableAttributeParams);

pub fn receive_renaming_result(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, RenamingResponse>>,
) {
    http_tasks.poll_responses(|_, data| {
        if let Some(data) = data {
            if data.1.is_a_map_label {
                debug!("received renaming result - refreshing map");
                commands.trigger(RequestMapFromBackend {
                    post_map_loaded_op: PostMapLoadedOp::FetchEntity(data.0),
                });
            } else {
                debug!("received renaming result - refreshing entity");
                commands.trigger(FetchEntityFromStorage {
                    uid: data.0,
                    anchor: None,
                    why: FetchEntityReason::Refresh,
                });
            }
        }
    });
}

pub struct FeatureUidResponse(String, Option<hexx::Hex>, Option<String>);

pub fn receive_appended_feature(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, FeatureUidResponse>>,
    hexes: Query<(Entity, &HexEntity)>,
) {
    http_tasks.poll_responses(|_, data| {
        if let Some(data) = data {
            if let Ok(parsed_data) = serde_json::from_str::<Value>(&data.0) {
                debug!("{:?}", parsed_data);
                if let Some(hex_coords) = data.1 {
                    for (entity, hex) in hexes.iter() {
                        if hex.hex == hex_coords {
                            commands.entity(entity).insert(HexToInvalidateMarker);
                        }
                    }
                }
                if let Some(uid) = parsed_data.get("uuid").and_then(Value::as_str) {
                    commands.trigger(RequestMapFromBackend {
                        post_map_loaded_op: PostMapLoadedOp::FetchEntity(
                            data.2.unwrap_or(uid.to_string()),
                        ),
                    });
                }
                commands.trigger(SyncMapForPeers(MapMessage::ReloadMap(None)));
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
pub struct RerollHexResponse(hexx::Hex);

pub fn reroll_hex(
    trigger: On<RerollHex>,
    mut http_agent: ResMut<HttpAgent>,
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, RerollHexResponse>>,
    effects: Res<EffectSystems>,
    user_settings: Res<UserSettings>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    commands
        .spawn_empty()
        .insert(RollNewFeatureEffect(trigger.hex_coords));
    commands.run_system(effects.roll_feature_effect);
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    let url = format!(
        "{server_host}/reroll/{}/{}/{}",
        sandbox_uid,
        trigger.event().hex_uid,
        trigger.event().class,
    );
    let coords = trigger.hex_coords;
    let _ = http_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        sandbox_uid.clone(),
        move |_data| RerollHexResponse(coords),
    );
}

pub fn receive_hex_reroll(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, RerollHexResponse>>,
    hexes: Query<(Entity, &HexEntity)>,
) {
    http_tasks.poll_responses(|_, data| {
        if let Some(data) = data {
            for (entity, hex) in hexes.iter() {
                if hex.hex == data.0 {
                    let _tween = Tween::new(
                        EaseFunction::QuarticIn,
                        Duration::from_millis(500),
                        TransformScaleLens {
                            start: Vec3::splat(1.0),
                            end: Vec3::splat(0.0),
                        },
                    );
                    commands.entity(entity).insert(HexToInvalidateMarker);
                    // NOTE(CRITICAL): This seems to crash Avian3D!!!
                    // .insert(bevy_tweening::Animator::new(tween));
                }
            }
            commands.trigger(RequestMapFromBackend::default());
            commands.trigger(SyncMapForPeers(MapMessage::ReloadMap(None)));
        }
    });
}

// ---------------------------------------------------------------------------------------------------------

pub struct StoreStateResponse;
pub struct LoadStateResponse(ClientState);

pub fn save_vtt_state(
    _trigger: On<StoreVttState>,
    mut controller: ResMut<VttStateApiController>,
    vtt_data: Res<VttData>,
) {
    if *controller != VttStateApiController::Inhibited && vtt_data.mode != HexMapMode::Player {
        *controller = VttStateApiController::Staged(Timer::from_seconds(3.0, TimerMode::Once));
    }
}

pub fn handle_save_vtt_state(
    mut http_agent: ResMut<HttpAgent>,
    mut controller: ResMut<VttStateApiController>,
    mut http_tasks: ResMut<AsyncHttpTasks<String, StoreStateResponse>>,
    settings: Res<AppSettings>,
    vtt_data: Res<VttData>,
    tokens: Query<(&Token, &Transform)>,
    time: Res<Time>,
    user_settings: Res<UserSettings>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    if let VttStateApiController::Staged(timer) = &mut *controller {
        if timer.tick(time.delta()).is_finished() {
            debug!("Save state timer expired.");
            let server_host = &user_settings.server;
            let api_key = Some(user_settings.key.as_ref().unwrap().clone());
            let url = format!("{server_host}/state/{}/{}", sandbox_uid, "default3");
            let tokens: Vec<TokenState> = tokens
                .iter()
                .map(|(token, transform)| TokenState {
                    token: token.clone(),
                    transform: transform.clone(),
                })
                .collect();
            let client_state = ClientState {
                settings: settings.clone(),
                vtt: vtt_data.clone(),
                tokens,
            };
            let json_data = serde_json::json!(client_state);

            // TODO: local data handling will eventually use a to-be-implemented
            // AsyncStandaloneTasks type.
            // It was important enough to add sooner than later so that we can safely
            // get rid of http vtt state management without users loosing any data.
            let config_path = ClientState::path(sandbox_uid);
            let file =
                File::create(config_path).expect("Failed to create vtt state data file");
            let writer = std::io::BufWriter::new(file);
            serde_json::to_writer_pretty(writer, &json_data)
                .expect("Failed to write vtt state data file");

            http_tasks.spawn_post(
                &mut http_agent,
                url,
                api_key,
                json_data,
                sandbox_uid.clone(),
                move |_data| StoreStateResponse {},
            );

            *controller = VttStateApiController::Idle;
        }
    }
}

pub fn receive_vtt_store_response(
    mut http_tasks: ResMut<AsyncHttpTasks<String, StoreStateResponse>>,
) {
    http_tasks.poll_responses(|_, _data| {});
}

// ---------------------------------------------------------------------------------------------------------
pub fn load_vtt_state(
    _trigger: On<LoadVttState>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<AsyncHttpTasks<String, LoadStateResponse>>,
    mut controller: ResMut<VttStateApiController>,
    user_settings: Res<UserSettings>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    *controller = VttStateApiController::Inhibited;
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());

    #[cfg(target_arch = "wasm32")]
    let url = format!("api/state/{}/{}", sandbox_uid, "default3");
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/state/{}/{}", sandbox_uid, "default3");

    let _ = http_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        sandbox_uid.clone(),
        move |data| {
            if let Ok(response) = serde_json::from_str(&data) {
                LoadStateResponse(response)
            } else {
                LoadStateResponse(ClientState::default())
            }
        },
    );
}

pub fn receive_vtt_load_response(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, LoadStateResponse>>,
    mut controller: ResMut<VttStateApiController>,
    hexes: Query<Entity, With<HexEntity>>,
    existing_vtt_data: Res<VttData>,
    mut next_vtt_data_state: ResMut<NextState<VttDataState>>,
) {
    http_tasks.poll_responses(|_, data| {
        if let Some(data) = data {
            // NOTE: This is just in case we already generated the map
            for e in hexes.iter() {
                commands.entity(e).try_despawn();
            }
            commands.insert_resource(data.0.settings);
            let mut vtt_data = data.0.vtt;
            vtt_data.patch_ephemeral_state(&existing_vtt_data);
            vtt_data.invalidate_map = true;
            commands.insert_resource(vtt_data);
            for t in data.0.tokens {
                commands.trigger(
                    EventContext::from(crate::tokens::SpawnToken {
                        token: t.token.clone(),
                        transform: t.transform.clone(),
                    })
                    .with_source(EventSource::Save),
                );
            }
            *controller = VttStateApiController::Idle;
            next_vtt_data_state.set(VttDataState::Available);
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
pub fn request_dungeon_map(
    trigger: On<RequestDungeonFromBackend>,
    mut commands: Commands,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncHttpTasks<(String, Entity), (BattleMapConstructs, String)>>,
    user_settings: Res<UserSettings>,
    cache: Res<crate::hexmap::elements::HexMapCache>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/dungeon/{}/{}", sandbox_uid, trigger.uid);
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/dungeon/{}/{}", sandbox_uid, trigger.uid);

    let uid = trigger.uid.clone();
    let task = move |data: String| -> (BattleMapConstructs, String) {
        if data.contains("areas") {
            (
                BattleMapConstructs::Dungeon(DungeonMapConstructs::from(data.clone())),
                data,
            )
        } else if data.contains("caverns") {
            (
                BattleMapConstructs::Cave(CaveMapConstructs::from(
                    data.clone(),
                    BackendUid::from(uid.clone()),
                )),
                data,
            )
        } else {
            (BattleMapConstructs::Empty, data.clone())
        }
    };

    if let Some(data) = cache.jsons.get(&trigger.uid) {
        if my_tasks
            .spawn_cached(data.clone(), (trigger.uid.clone(), trigger.hex), task)
            .is_err()
        {
            commands.entity(trigger.hex).reset_battlemap_loading_state();
        }
    } else {
        if my_tasks
            .spawn_request(
                &mut http_agent,
                url,
                api_key,
                (trigger.uid.clone(), trigger.hex),
                task,
            )
            .is_err()
        {
            commands.entity(trigger.hex).reset_battlemap_loading_state();
        }
    }
}

pub fn request_city_or_town_map(
    trigger: On<RequestCityOrTownFromBackend>,
    mut commands: Commands,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncHttpTasks<(String, Entity), (BattleMapConstructs, String)>>,
    user_settings: Res<UserSettings>,
    cache: Res<crate::hexmap::elements::HexMapCache>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/city/{}/{}", sandbox_uid, trigger.uid);
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/city/{}/{}", sandbox_uid, trigger.uid);
    if let Some(data) = cache.jsons.get(&trigger.uid) {
        if my_tasks
            .spawn_cached(
                data.clone(),
                (trigger.uid.clone(), trigger.hex),
                move |data| {
                    (
                        BattleMapConstructs::City(CityMapConstructs::from(data.clone())),
                        data.clone(),
                    )
                },
            )
            .is_err()
        {
            commands.entity(trigger.hex).reset_battlemap_loading_state();
        }
    } else {
        if my_tasks
            .spawn_request(
                &mut http_agent,
                url,
                api_key,
                (trigger.uid.clone(), trigger.hex),
                move |data| {
                    info!("Processing city map in task");
                    (
                        BattleMapConstructs::City(CityMapConstructs::from(data.clone())),
                        data.clone(),
                    )
                },
            )
            .is_err()
        {
            commands.entity(trigger.hex).reset_battlemap_loading_state();
        }
    }
}

pub fn request_village_map(
    trigger: On<RequestVillageFromBackend>,
    mut commands: Commands,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncHttpTasks<(String, Entity), (BattleMapConstructs, String)>>,
    user_settings: Res<UserSettings>,
    cache: Res<crate::hexmap::elements::HexMapCache>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/city/{}/{}", sandbox_uid, trigger.uid);
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/city/{}/{}", sandbox_uid, trigger.uid);
    let uid = trigger.uid.clone();
    if let Some(data) = cache.jsons.get(&trigger.uid) {
        if my_tasks
            .spawn_cached(
                data.clone(),
                (trigger.uid.clone(), trigger.hex),
                move |data| {
                    (
                        BattleMapConstructs::Village(VillageMapConstructs::from(
                            BackendUid::from(uid.clone()),
                            data.clone(),
                        )),
                        data.clone(),
                    )
                },
            )
            .is_err()
        {
            commands.entity(trigger.hex).reset_battlemap_loading_state();
        }
    } else {
        if my_tasks
            .spawn_request(
                &mut http_agent,
                url,
                api_key,
                (trigger.uid.clone(), trigger.hex),
                move |data| {
                    (
                        BattleMapConstructs::Village(VillageMapConstructs::from(
                            BackendUid::from(uid.clone()),
                            data.clone(),
                        )),
                        data.clone(),
                    )
                },
            )
            .is_err()
        {
            commands.entity(trigger.hex).reset_battlemap_loading_state();
        }
    }
}

fn receive_battlemaps_data(
    mut commands: Commands,
    mut my_tasks: ResMut<AsyncHttpTasks<(String, Entity), (BattleMapConstructs, String)>>,
    mut cache: ResMut<crate::hexmap::elements::HexMapCache>,
) {
    my_tasks.poll_responses(|key, data| {
        if let Some(data) = data
            && !data.1.is_empty()
        {
            debug!("ingesting battlemap map for {}", key.0);
            on_ingest_battlemap_data(key, data, &mut commands, cache.as_mut());
        } else {
            // NOTE: Seems like fetching a battlemap failed.
            // By removing the SubMapMarker the battlemaps module will attempt another fetch.
            error!("error in battlemap map for {}", key.0);
            let (_, entity) = key;
            commands.entity(*entity).reset_battlemap_loading_state();
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
pub fn request_a_reroll(
    trigger: On<RerollEntity>,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncHttpTasks<String, (bool, String)>>,
    user_settings: Res<UserSettings>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = format!(
        "/api/reroll/{}/{}/{}",
        sandbox_uid, trigger.uid, trigger.class_override
    );
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!(
        "{server_host}/reroll/{}/{}/{}",
        sandbox_uid, trigger.uid, "default"
    );
    let reload = trigger.is_map_reload_needed;
    let _ = my_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        trigger.uid.clone(),
        move |data| (reload, data),
    );
}

pub fn receive_reroll_response(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, (bool, String)>>,
    map_data: Res<HexMapData>,
    hexes: Query<(Entity, &HexEntity)>,
    mut content_stuff: ResMut<ContentContext>,
    mut cache: ResMut<crate::hexmap::elements::HexMapCache>,
) {
    http_tasks.poll_responses(|rerolled_uid, data| {
        if let Some((reload, data)) = data {
            if let Ok(parsed_data) = serde_json::from_str::<Value>(&data) {
                if let Some(uid) = parsed_data.get("uuid").and_then(Value::as_str) {
                    if reload {
                        // TODO: We are currently refreshing the entire map, but a better solution
                        // would be to detect only the relevant parts of the map that were impacted
                        // by the change (for example, all the neighboring hexes that had trails
                        // added to a new settlement)
                        commands.trigger(RequestMapFromBackend {
                            post_map_loaded_op: PostMapLoadedOp::FetchEntity(uid.to_string()),
                        });

                        if let Some(current_hex_uid) = content_stuff.current_hex_uid.as_ref() {
                            let coords = map_data.coords.get(current_hex_uid).unwrap();
                            cache.invalidate_json(current_hex_uid);

                            // FIXME: Not working because not invalidating json on player side
                            commands.trigger(SyncMapForPeers(MapMessage::ReloadMap(Some(
                                current_hex_uid.clone(),
                            ))));

                            for (entity, hex) in hexes.iter() {
                                if hex.hex == *coords {
                                    // FIXME: should be done differently.
                                    // The Invalidation should be triggered from the RequestMapTrigger handler.
                                    // .with_completed_system(callbacks.invalidate);
                                    commands.entity(entity).insert(HexToInvalidateMarker);
                                }
                            }
                        } else {
                            unreachable!();
                        }
                    }
                    if let Some(current_entity_uid) = content_stuff.current_entity_uid.clone()
                    {
                        if &current_entity_uid == rerolled_uid {
                            content_stuff.invalidate_last_history_entry();
                        }
                        if !reload {
                            commands.trigger(FetchEntityFromStorage {
                                // NOTE: it might be that the following condition is redundant
                                // in case when we are not reloading means we never hit the
                                // if branch and only use the else.
                                uid: if &current_entity_uid == rerolled_uid {
                                    uid.to_string()
                                } else {
                                    current_entity_uid.clone()
                                },
                                anchor: None,
                                why: FetchEntityReason::SandboxLink,
                            });
                        }
                    }
                } else {
                    error!("UID not found in reroll response");
                }
            } else {
                error!("Failed to parse reroll response JSON: {}", data);
            }
        }
    });
}

#[derive(Event)]
pub struct SearchEntitiesInBackend {
    pub query: String,
}

// ---------------------------------------------------------------------------------------------------------
pub fn request_search(
    trigger: On<SearchEntitiesInBackend>,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncHttpTasks<String, SearchResponse>>,
    user_settings: Res<UserSettings>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/search/{}/{}", sandbox_uid, trigger.query);
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/search/{}/{}", sandbox_uid, trigger.query,);
    let _ = my_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        sandbox_uid.clone(),
        move |data| serde_json::from_str(&data).unwrap_or_default(),
    );
}

pub fn receive_search_results(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<String, SearchResponse>>,
) {
    http_tasks.poll_responses(|_, data| {
        if let Some(data) = data {
            commands.trigger(ShowSearchResults::from_response(data));
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
#[derive(Event, Default)]
pub struct PerformHexMapActionInBackend {
    pub uid: String,
    pub action: String,
    pub topic: Option<String>,
}
pub fn request_hex_action(
    trigger: On<PerformHexMapActionInBackend>,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncHttpTasks<(String, String), String>>,
    user_settings: Res<UserSettings>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = if let Some(topic) = &trigger.topic {
        format!(
            "/api/{}/{}/{}/{}",
            trigger.action, sandbox_uid, trigger.uid, topic
        )
    } else {
        format!("/api/{}/{}/{}", trigger.action, sandbox_uid, trigger.uid)
    };
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = if let Some(topic) = &trigger.topic {
        format!(
            "{server_host}/{}/{}/{}/{}",
            trigger.action, sandbox_uid, trigger.uid, topic
        )
    } else {
        format!(
            "{server_host}/{}/{}/{}",
            trigger.action, sandbox_uid, trigger.uid
        )
    };
    let _ = my_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        (sandbox_uid.clone(), trigger.uid.clone()),
        move |data| data,
    );
}

pub fn receive_hex_action_results(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncHttpTasks<(String, String), String>>,
) {
    http_tasks.poll_responses(|key, data| {
        if data.is_some() {
            commands.trigger(RequestMapFromBackend {
                post_map_loaded_op: PostMapLoadedOp::InvalidateVisible,
            });

            commands.trigger(SyncMapForPeers(MapMessage::ReloadMap(Some(key.1.clone()))));
        }
    });
}
