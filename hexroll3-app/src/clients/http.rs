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

use bevy::prelude::*;

use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

use crate::{
    battlemaps::{
        BattleMapConstructs, BattlemapFeatureUtils, CaveMapConstructs, CityMapConstructs,
        DungeonMapConstructs, RequestCityOrTownFromBackend, RequestDungeonFromBackend,
        RequestVillageFromBackend, VillageMapConstructs,
    },
    clients::{
        controller::{PostMapLoadedOpPrefix, RenamingResponse, TokenState},
        model::BackendUid,
    },
    content::{ContentPageModel, RenameSandboxEntity},
    hexmap::{
        HexMapJson, HexMapTileMaterials, HexmapTheme,
        elements::{
            AppendSandboxEntity, FetchEntityFromStorage, HexEntity, HexMapData,
            HexMapResources, VttDataState, hexx_to_hexroll_coords,
        },
        prepare_hex_map_data,
    },
    shared::{
        asynchttp::{ApiHandler, AsyncBackendTasks, HttpAgent},
        effects::{EffectSystems, RollNewFeatureEffect},
        settings::{AppSettings, UserSettings},
        vtt::{HexMapMode, LoadVttState, StoreVttState, VttData},
    },
    tokens::Token,
    vtt::sync::{EventContext, EventSource},
};

use super::controller::SearchEntitiesInBackend;
use super::{
    RemoteBackendEvent,
    controller::{
        ClientState, FeatureUidResponse, PerformHexMapActionInBackend, PostMapLoadedOp,
        RequestMapFromBackend, RequestSandboxFromBackend, RequestVttSessionFromBackend,
        VttSessionResponse, VttStateApiController,
    },
    model::{FetchEntityReason, RerollEntity, SearchResponse},
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
            // Join VTT
            .add_observer(request_vtt_session)
            // Get entity content page 
            .add_observer(fetch_hex)
            // Roll a new feature
            .add_observer(append_feature)
            // Rename a sandbox entity
            .add_observer(rename_entity)
            // Save vtt state
            .add_observer(save_vtt_state)
            .register_api_callback::<_, String, StoreStateResponse>(receive_vtt_store_response)
            // Load vtt state
            .add_observer(load_vtt_state)
            .register_api_callback::<_, String, LoadStateResponse>(receive_vtt_load_response)
            // Get sandbox hexmap
            .add_observer(request_hex_map)
            // Get battlemaps
            .add_observer(request_dungeon_map)
            .add_observer(request_city_or_town_map)
            .add_observer(request_village_map)
            // Reroll an entity
            .add_observer(request_a_reroll)
            // Search
            .add_observer(request_search)
            // Hex Map Action
            .add_observer(request_hex_action)
            // ---------------------------------------------------------------------------------------------
            // <--
            ;
    }
}
// ---------------------------------------------------------------------------------------------------------
pub fn request_sandbox(
    trigger: On<RemoteBackendEvent<RequestSandboxFromBackend>>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<AsyncBackendTasks<String, Option<String>>>,
    user_settings: Res<UserSettings>,
) {
    let event = &trigger.event().0;
    let sandbox_uid = &event.sandbox_uid;
    let server_host = &user_settings.server;
    let api_key = event.pairing_key.clone();

    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{}/body/{}/root", server_host, sandbox_uid);
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/body/{}/root", sandbox_uid);

    let pairing_key = event.pairing_key.clone();
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
                pairing_key.clone()
            }
        },
    );
}

// ---------------------------------------------------------------------------------------------------------
pub fn request_vtt_session(
    trigger: On<RequestVttSessionFromBackend>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<AsyncBackendTasks<String, VttSessionResponse>>,
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

// ---------------------------------------------------------------------------------------------------------
pub fn request_hex_map(
    trigger: On<RemoteBackendEvent<RequestMapFromBackend>>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<AsyncBackendTasks<String, (Option<HexMapData>, PostMapLoadedOp)>>,
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
    let post_map_loaded_op = trigger.0.post_map_loaded_op.clone();
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

// ---------------------------------------------------------------------------------------------------------
pub fn fetch_hex(
    trigger: On<RemoteBackendEvent<FetchEntityFromStorage>>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<
        AsyncBackendTasks<String, (ContentPageModel, FetchEntityReason, Option<String>)>,
    >,
    user_settings: Res<UserSettings>,
    mut commands: Commands,
    window: Single<Entity, With<PrimaryWindow>>,
) {
    let event = &trigger.event().0;
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/get/{}/{}", sandbox_uid, trigger.event().uid);
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/get/{}/{}", sandbox_uid, event.uid);
    let uid = event.uid.clone();
    let why = event.why.clone();

    commands
        .entity(*window)
        .insert(CursorIcon::System(SystemCursorIcon::Progress));

    let anchor = event.anchor.clone();
    let _ = http_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        event.uid.clone(),
        move |data| {
            (
                ContentPageModel::from_entity_html(&uid, &data),
                why.clone(),
                anchor.clone(),
            )
        },
    );
}

// ---------------------------------------------------------------------------------------------------------
pub fn append_feature(
    trigger: On<RemoteBackendEvent<AppendSandboxEntity>>,
    mut http_agent: ResMut<HttpAgent>,
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncBackendTasks<String, FeatureUidResponse>>,
    // hexes: Query<(Entity, &HexEntity)>,
    effects: Res<EffectSystems>,
    user_settings: Res<UserSettings>,
    window: Single<Entity, With<PrimaryWindow>>,
) {
    let event = &trigger.event().0;
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
        "entity": event.hex_uid,
        "type": event.what,
        "attribute": event.attr,
    });

    if let Some(hex_coords) = trigger.0.hex_coords {
        commands
            .spawn_empty()
            .insert(RollNewFeatureEffect(hex_coords));
        commands.run_system(effects.roll_feature_effect);

        if trigger.0.send_coords {
            let (x, y) = hexx_to_hexroll_coords(&hex_coords);
            json_data["nx"] = x.into();
            json_data["ny"] = y.into();
        }
    }
    let coords = trigger.0.hex_coords;
    let send_coords = trigger.0.send_coords;
    let hex_uid = trigger.0.hex_uid.clone();
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
    trigger: On<RemoteBackendEvent<RenameSandboxEntity>>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<AsyncBackendTasks<String, RenamingResponse>>,
    user_settings: Res<UserSettings>,
) {
    let event = &trigger.event().0;
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    let url = format!("{server_host}/rename/");
    let json_data = serde_json::json!({
        "instance": sandbox_uid,
        "entity": event.entity_uid,
        "attr": event.params.attr_name,
        "value": event.value,
    });

    let params = event.params.clone();
    let uid = event.entity_uid.clone();

    http_tasks.spawn_post(
        &mut http_agent,
        url,
        api_key,
        json_data,
        sandbox_uid.clone(),
        move |_| RenamingResponse(uid.clone(), params.clone()),
    );
}

// ---------------------------------------------------------------------------------------------------------

pub struct StoreStateResponse;
pub struct LoadStateResponse(pub ClientState);

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
    mut http_tasks: ResMut<AsyncBackendTasks<String, StoreStateResponse>>,
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

            if user_settings.local.unwrap_or(false) {
                *controller = VttStateApiController::Idle;
                return;
            }
            let server_host = &user_settings.server;
            let api_key = Some(user_settings.key.as_ref().unwrap().clone());
            let url = format!("{server_host}/state/{}/{}", sandbox_uid, "default3");

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
    mut http_tasks: ResMut<AsyncBackendTasks<String, StoreStateResponse>>,
) {
    http_tasks.poll_responses(|_, _data| {});
}

// ---------------------------------------------------------------------------------------------------------
pub fn load_vtt_state(
    _trigger: On<RemoteBackendEvent<LoadVttState>>,
    mut http_agent: ResMut<HttpAgent>,
    mut http_tasks: ResMut<AsyncBackendTasks<String, LoadStateResponse>>,
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
    mut http_tasks: ResMut<AsyncBackendTasks<String, LoadStateResponse>>,
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
    trigger: On<RemoteBackendEvent<RequestDungeonFromBackend>>,
    mut commands: Commands,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, Entity), (BattleMapConstructs, String)>>,
    user_settings: Res<UserSettings>,
    cache: Res<crate::hexmap::elements::HexMapCache>,
) {
    let event = &trigger.event().0;
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/dungeon/{}/{}", sandbox_uid, event.uid);
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/dungeon/{}/{}", sandbox_uid, event.uid);

    let uid = event.uid.clone();
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

    if let Some(data) = cache.jsons.get(&event.uid) {
        if my_tasks
            .spawn_cached(data.clone(), (event.uid.clone(), event.hex), task)
            .is_err()
        {
            commands.entity(event.hex).reset_battlemap_loading_state();
        }
    } else {
        if my_tasks
            .spawn_request(
                &mut http_agent,
                url,
                api_key,
                (event.uid.clone(), event.hex),
                task,
            )
            .is_err()
        {
            commands.entity(event.hex).reset_battlemap_loading_state();
        }
    }
}

pub fn request_city_or_town_map(
    trigger: On<RemoteBackendEvent<RequestCityOrTownFromBackend>>,
    mut commands: Commands,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, Entity), (BattleMapConstructs, String)>>,
    user_settings: Res<UserSettings>,
    cache: Res<crate::hexmap::elements::HexMapCache>,
) {
    let event = &trigger.event().0;
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/city/{}/{}", sandbox_uid, event.uid);
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/city/{}/{}", sandbox_uid, event.uid);
    if let Some(data) = cache.jsons.get(&event.uid) {
        if my_tasks
            .spawn_cached(data.clone(), (event.uid.clone(), event.hex), move |data| {
                (
                    BattleMapConstructs::City(CityMapConstructs::from(data.clone())),
                    data.clone(),
                )
            })
            .is_err()
        {
            commands.entity(event.hex).reset_battlemap_loading_state();
        }
    } else {
        if my_tasks
            .spawn_request(
                &mut http_agent,
                url,
                api_key,
                (event.uid.clone(), event.hex),
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
            commands.entity(event.hex).reset_battlemap_loading_state();
        }
    }
}

pub fn request_village_map(
    trigger: On<RemoteBackendEvent<RequestVillageFromBackend>>,
    mut commands: Commands,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, Entity), (BattleMapConstructs, String)>>,
    user_settings: Res<UserSettings>,
    cache: Res<crate::hexmap::elements::HexMapCache>,
) {
    let event = &trigger.event().0;
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/city/{}/{}", sandbox_uid, event.uid);
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/city/{}/{}", sandbox_uid, event.uid);
    let uid = event.uid.clone();
    if let Some(data) = cache.jsons.get(&event.uid) {
        if my_tasks
            .spawn_cached(data.clone(), (event.uid.clone(), event.hex), move |data| {
                (
                    BattleMapConstructs::Village(VillageMapConstructs::from(
                        BackendUid::from(uid.clone()),
                        data.clone(),
                    )),
                    data.clone(),
                )
            })
            .is_err()
        {
            commands.entity(event.hex).reset_battlemap_loading_state();
        }
    } else {
        if my_tasks
            .spawn_request(
                &mut http_agent,
                url,
                api_key,
                (event.uid.clone(), event.hex),
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
            commands.entity(event.hex).reset_battlemap_loading_state();
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
pub fn request_a_reroll(
    trigger: On<RemoteBackendEvent<RerollEntity>>,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncBackendTasks<String, (bool, String, Option<hexx::Hex>)>>,
    user_settings: Res<UserSettings>,
) {
    let event = &trigger.event().0;
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = format!(
        "/api/reroll/{}/{}/{}",
        sandbox_uid, event.uid, event.class_override
    );
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!(
        "{server_host}/reroll/{}/{}/{}",
        sandbox_uid, event.uid, "default"
    );
    let reload = event.is_map_reload_needed;
    let maybe_coords = event.coords;
    let _ = my_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        event.uid.clone(),
        move |data| (reload, data, maybe_coords),
    );
}

// ---------------------------------------------------------------------------------------------------------
pub fn request_search(
    trigger: On<RemoteBackendEvent<SearchEntitiesInBackend>>,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncBackendTasks<String, SearchResponse>>,
    user_settings: Res<UserSettings>,
) {
    let event = &trigger.event().0;
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = format!("/api/search/{}/{}", sandbox_uid, event.query);
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = format!("{server_host}/search/{}/{}", sandbox_uid, event.query,);
    let _ = my_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        sandbox_uid.clone(),
        move |data| serde_json::from_str(&data).unwrap_or_default(),
    );
}

// ---------------------------------------------------------------------------------------------------------
pub fn request_hex_action(
    trigger: On<RemoteBackendEvent<PerformHexMapActionInBackend>>,
    mut http_agent: ResMut<HttpAgent>,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, String), String>>,
    user_settings: Res<UserSettings>,
) {
    let event = &trigger.event().0;
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let url = if let Some(topic) = &event.topic {
        format!(
            "/api/{}/{}/{}/{}",
            event.action, sandbox_uid, event.uid, topic
        )
    } else {
        format!("/api/{}/{}/{}", event.action, sandbox_uid, event.uid)
    };
    let server_host = &user_settings.server;
    let api_key = Some(user_settings.key.as_ref().unwrap().clone());
    #[cfg(not(target_arch = "wasm32"))]
    let url = if let Some(topic) = &event.topic {
        format!(
            "{server_host}/{}/{}/{}/{}",
            event.action, sandbox_uid, event.uid, topic
        )
    } else {
        format!(
            "{server_host}/{}/{}/{}",
            event.action, sandbox_uid, event.uid
        )
    };
    let _ = my_tasks.spawn_request(
        &mut http_agent,
        url,
        api_key,
        (sandbox_uid.clone(), event.uid.clone()),
        move |data| data,
    );
}
