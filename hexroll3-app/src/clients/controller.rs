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

use std::{path::PathBuf, time::Duration};

use bevy_tweening::{Tween, lens::TransformScaleLens};
use serde::{Deserialize, Serialize};

use bevy::prelude::*;
use serde_json::Value;

use crate::{
    battlemaps::{
        BattleMapConstructs, BattlemapFeatureUtils, RequestCityOrTownFromBackend,
        RequestDungeonFromBackend, RequestVillageFromBackend, SpawnCaveMap, SpawnCityMap,
        SpawnDungeonMap, SpawnVillageMap,
    },
    content::{
        ContentMode, ContentPageModel, EditableAttributeParams, EntityRenderingCompleted,
        RenameSandboxEntity, ScrollToAnchor, context::ContentContext,
    },
    hexmap::{
        MapMessage,
        elements::{
            AppendSandboxEntity, FetchEntityFromStorage, HexEntity, HexEntityCallbacks,
            HexMapData, HexToInvalidateMarker,
        },
        update_hex_map_tiles,
    },
    shared::{
        asynchttp::{ApiHandler, AsyncBackendTasks},
        camera::GimbalshotCameraMovement,
        settings::{AppSettings, CONFIG_DIR, UserSettings},
        vtt::{LoadVttState, VttData},
    },
    tokens::Token,
    vtt::sync::SyncMapForPeers,
};

use super::{
    RemoteBackendEvent, StandaloneBackendEvent,
    model::{FetchEntityReason, RerollEntity, SandboxMode, SearchResponse},
    standalone::StandaloneSandbox,
};

pub struct ApiControllerPlugin;

impl Plugin for ApiControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_render_entity_completed)
            .add_observer(backend_router::<RequestSandboxFromBackend>)
            .add_observer(backend_router::<FetchEntityFromStorage>)
            .add_observer(backend_router::<AppendSandboxEntity>)
            .add_observer(backend_router::<RequestMapFromBackend>)
            .add_observer(backend_router::<LoadVttState>)
            .add_observer(backend_router::<RequestDungeonFromBackend>)
            .add_observer(backend_router::<RequestCityOrTownFromBackend>)
            .add_observer(backend_router::<RequestVillageFromBackend>)
            .add_observer(backend_router::<RerollEntity>)
            .add_observer(backend_router::<RequestMapFromBackend>)
            .add_observer(backend_router::<SearchEntitiesInBackend>)
            .add_observer(backend_router::<RenameSandboxEntity>)
            .add_observer(backend_router::<PerformHexMapActionInBackend>)
            // ---------------------------------------------------------------------------------------------
            // API request and response handlers
            // ---------------------------------------------------------------------------------------------
            // Get sandbox 
            .register_api_callback::<_, String, Option<String>>(receive_sandbox)
            // Join VTT
            .register_api_callback::<_, String, VttSessionResponse>(receive_vtt_session)
            // Get entity content page 
            .register_api_callback::<_, String, (ContentPageModel, FetchEntityReason, Option<String>)>(receive_hex)
            // Roll a new feature
            .register_api_callback::<_, String, FeatureUidResponse>(receive_appended_feature)
            // Rename a sandbox entity
            .register_api_callback::<_, String, RenamingResponse>(receive_renaming_result)
            // Get sandbox hexmap
            .register_api_callback::<_, String, (Option<HexMapData>, PostMapLoadedOp)>(receive_hex_map)
            // Get battlemaps
            .register_api_callback::<_, (String, Entity), (BattleMapConstructs, String)>(
                receive_battlemaps_data.after(update_hex_map_tiles),
            )
            // Reroll an entity
            .register_api_callback::<_, String, (bool, String, Option<hexx::Hex>)>(receive_reroll_response)
            // Search
            .register_api_callback::<_, String, SearchResponse>(receive_search_results)
            // Hex Map Action
            .register_api_callback::<_, (String,String), String>(receive_hex_action_results)
            // ---------------------------------------------------------------------------------------------
            // <--
            ;
    }
}

// ---------------------------------------------------------------------------------------------------------
#[derive(Event, Default, Clone)]
pub struct RequestSandboxFromBackend {
    pub sandbox_uid: String,
    pub pairing_key: Option<String>,
}

pub fn receive_sandbox(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncBackendTasks<String, Option<String>>>,
) {
    http_tasks.poll_responses(|uid, data| {
        if let Some(ret) = data {
            if let Some(data) = ret {
                commands.trigger(RequestMapResult::Loaded(uid.clone(), Some(data)));
                commands.remove_resource::<StandaloneSandbox>();
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
    pub node_name: String,
}

pub fn receive_vtt_session(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncBackendTasks<String, VttSessionResponse>>,
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
#[derive(Event, Clone, Default)]
pub struct RequestMapFromBackend {
    pub post_map_loaded_op: PostMapLoadedOp,
}

// ---------------------------------------------------------------------------------------------------------
#[derive(Event, PartialEq)]
pub enum RequestMapResult {
    Loaded(String /* Sandbox Id */, Option<String> /* Key */),
    Joined(String /* Sandbox Id */, String /* Node name */),
    Failed,
}

pub fn receive_hex_map(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncBackendTasks<String, (Option<HexMapData>, PostMapLoadedOp)>>,
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
pub fn receive_hex(
    mut commands: Commands,
    mut http_tasks: ResMut<
        AsyncBackendTasks<String, (ContentPageModel, FetchEntityReason, Option<String>)>,
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
pub struct RenamingResponse(pub String, pub EditableAttributeParams);

pub fn receive_renaming_result(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncBackendTasks<String, RenamingResponse>>,
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

// ---------------------------------------------------------------------------------------------------------
#[derive(Debug)]
pub struct FeatureUidResponse(pub String, pub Option<hexx::Hex>, pub Option<String>);

pub fn receive_appended_feature(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncBackendTasks<String, FeatureUidResponse>>,
    hexes: Query<(Entity, &HexEntity)>,
    content_mode: Res<State<ContentMode>>,
) {
    http_tasks.poll_responses(|_, data| {
        if let Some(data) = data {
            if let Ok(parsed_data) = serde_json::from_str::<Value>(&data.0) {
                if let Some(hex_coords) = data.1 {
                    for (entity, hex) in hexes.iter() {
                        if hex.hex == hex_coords {
                            commands.entity(entity).insert(HexToInvalidateMarker);
                        }
                    }
                }
                if let Some(uid) = parsed_data.get("uuid").and_then(Value::as_str) {
                    commands.trigger(RequestMapFromBackend {
                        post_map_loaded_op: if content_mode.get() == &ContentMode::MapOnly
                            && data.2.is_some()
                        {
                            PostMapLoadedOp::InvalidateVisible
                        } else {
                            PostMapLoadedOp::FetchEntity(data.2.unwrap_or(uid.to_string()))
                        },
                    });
                }
                commands.trigger(SyncMapForPeers(MapMessage::ReloadMap(None)));
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
fn receive_battlemaps_data(
    mut commands: Commands,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, Entity), (BattleMapConstructs, String)>>,
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
pub fn receive_reroll_response(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncBackendTasks<String, (bool, String, Option<hexx::Hex>)>>,
    map_data: Res<HexMapData>,
    hexes: Query<(Entity, &HexEntity)>,
    mut content_stuff: ResMut<ContentContext>,
    mut cache: ResMut<crate::hexmap::elements::HexMapCache>,
    content_mode: Res<State<ContentMode>>,
    // callbacks: Res<HexEntityCallbacks>,
) {
    http_tasks.poll_responses(|rerolled_uid, data| {
        if let Some((reload, data, maybe_coords)) = data {
            if let Ok(parsed_data) = serde_json::from_str::<Value>(&data) {
                if let Some(uid) = parsed_data.get("uuid").and_then(Value::as_str) {
                    if let Some(coords) = maybe_coords {
                        for (entity, hex) in hexes.iter() {
                            if hex.hex == coords {
                                let _tween = Tween::new(
                                    EaseFunction::QuarticIn,
                                    Duration::from_millis(500),
                                    TransformScaleLens {
                                        start: Vec3::splat(1.0),
                                        end: Vec3::splat(0.0),
                                    },
                                );
                                commands.entity(entity).insert(HexToInvalidateMarker);
                            }
                        }
                        commands.trigger(RequestMapFromBackend::default());
                        commands.trigger(SyncMapForPeers(MapMessage::ReloadMap(None)));
                    } else if reload {
                        // TODO: We are currently refreshing the entire map, but a better solution
                        // would be to detect only the relevant parts of the map that were impacted
                        // by the change (for example, all the neighboring hexes that had trails
                        // added to a new settlement)
                        // commands.trigger(RequestMapFromBackend {
                        //     post_map_loaded_op: PostMapLoadedOp::FetchEntity(uid.to_string()),
                        // });
                        commands.trigger(RequestMapFromBackend::default());
                        debug!("In reload case");

                        if let Some(current_hex_uid) = content_stuff.current_hex_uid.as_ref() {
                            debug!("In content uid case");
                            if let Some(coords) = map_data.coords.get(current_hex_uid) {
                                debug!("In found coords case");
                                cache.invalidate_json(current_hex_uid);

                                // FIXME: Not working because not invalidating json on player side
                                commands.trigger(SyncMapForPeers(MapMessage::ReloadMap(
                                    Some(current_hex_uid.clone()),
                                )));

                                for (entity, hex) in hexes.iter() {
                                    if hex.hex == *coords {
                                        // FIXME: should be done differently.
                                        // The Invalidation should be triggered from the RequestMapTrigger handler.
                                        // .with_completed_system(callbacks.invalidate);
                                        commands.entity(entity).insert(HexToInvalidateMarker);
                                    }
                                }
                                // commands.run_system(callbacks.invalidate);
                            }
                        }
                    }
                    if let Some(current_entity_uid) = content_stuff.current_entity_uid.clone()
                    {
                        if &current_entity_uid == rerolled_uid {
                            content_stuff.invalidate_last_history_entry();
                        }

                        if *content_mode == ContentMode::SplitScreen {
                            // if !reload && *content_mode == ContentMode::SplitScreen {
                            debug!("we should not be here");
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

// ---------------------------------------------------------------------------------------------------------
#[derive(Event, Clone)]
pub struct SearchEntitiesInBackend {
    pub query: String,
}

pub fn receive_search_results(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncBackendTasks<String, SearchResponse>>,
) {
    http_tasks.poll_responses(|_, data| {
        if let Some(data) = data {
            commands.trigger(ShowSearchResults::from_response(data));
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
#[derive(Event, Default, Clone)]
pub struct PerformHexMapActionInBackend {
    pub uid: String,
    pub action: String,
    pub topic: Option<String>,
}

pub fn receive_hex_action_results(
    mut commands: Commands,
    mut http_tasks: ResMut<AsyncBackendTasks<(String, String), String>>,
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

#[derive(Event)]
pub struct RenderEntityContent {
    pub uid: String,
    pub data: ContentPageModel,
    pub anchor: Option<String>,
    pub why: FetchEntityReason,
}

pub fn on_render_entity_completed(
    trigger: On<EntityRenderingCompleted>,
    mut commands: Commands,
    map: Res<HexMapData>,
    mut content_stuff: ResMut<ContentContext>,
) {
    if let Some(coords) = &trigger.map_coords {
        content_stuff.current_hex_uid = Some(coords.hex.clone());
        commands.trigger(GimbalshotCameraMovement {
            coords: coords.clone(),
        });
    } else {
        if let Some(coord) = map.coords.get(&trigger.uid) {
            content_stuff.current_hex_uid = Some(trigger.uid.clone());
            if let Some(entity) = map.hexes.get(coord) {
                commands.trigger(GimbalshotCameraMovement {
                    coords: crate::shared::camera::MapCoords {
                        hex: entity.uid.clone(),
                        x: 0.0,
                        y: 0.0,
                        zoom: 4,
                    },
                });
            }
        }
    }

    if let Some(anchor) = &trigger.anchor {
        commands.trigger(ScrollToAnchor {
            anchor: anchor.clone(),
        });
    }
}

pub fn on_ingest_battlemap_data(
    key: &(String, Entity),
    data: (BattleMapConstructs, String),
    commands: &mut Commands,
    cache: &mut crate::hexmap::elements::HexMapCache,
) {
    if let Ok(mut entity) = commands.get_entity(key.1) {
        entity.mark_battlemap_has_valid_state();
    }
    match data.0 {
        BattleMapConstructs::Dungeon(dungeon_map_constructs) => {
            cache.jsons.insert(key.0.clone(), data.1);
            commands.trigger(SpawnDungeonMap {
                hex: key.1,
                data: dungeon_map_constructs,
            })
        }
        BattleMapConstructs::Cave(cave_map_constructs) => {
            cache.jsons.insert(key.0.clone(), data.1);
            commands.trigger(SpawnCaveMap {
                hex: key.1,
                data: cave_map_constructs,
            })
        }
        BattleMapConstructs::City(city_map_constructs) => commands.trigger(SpawnCityMap {
            hex: key.1,
            hex_uid: key.0.clone(),
            data: city_map_constructs,
        }),
        BattleMapConstructs::Village(village_map_constructs) => {
            commands.trigger(SpawnVillageMap {
                hex: key.1,
                hex_uid: key.0.clone(),
                data: village_map_constructs,
            })
        }
        _ => {}
    }
}

#[derive(Event)]
pub struct ShowSearchResults {
    pub search_response: SearchResponse,
}

impl ShowSearchResults {
    pub fn from_response(search_response: SearchResponse) -> Self {
        Self { search_response }
    }
}

#[derive(Serialize, Deserialize)]
pub struct TokenState {
    pub token: Token,
    pub transform: Transform,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ClientState {
    pub settings: AppSettings,
    pub vtt: VttData,
    pub tokens: Vec<TokenState>,
}

impl ClientState {
    pub fn path(sandbox: &str) -> PathBuf {
        let config_dir = dirs::config_dir().expect("Unable to get config dir");
        let config_path = config_dir
            .join(CONFIG_DIR)
            .join(format!("vtt_{}.json", sandbox));
        config_path
    }
}

#[derive(Resource, PartialEq)]
pub enum VttStateApiController {
    Unloaded,
    Idle,
    Staged(Timer),
    Inhibited,
}

// ---------------------------------------------------------------------------------------------------------
pub fn backend_router<T>(
    trigger: On<T>,
    mut commands: Commands,
    user_settings: Res<UserSettings>,
) where
    T: Event + Clone,
{
    let e = trigger.event();
    if user_settings.local.unwrap_or(false) {
        commands.trigger(StandaloneBackendEvent(e.clone()));
    } else {
        commands.trigger(RemoteBackendEvent(e.clone()));
    }
}
