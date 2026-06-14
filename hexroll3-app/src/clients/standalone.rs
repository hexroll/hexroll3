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

use std::{collections::BTreeMap, fs};

use hexroll3_cartographer::dungeons::map_data_providers;
use serde_json::json;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use hexroll3_cartographer::{
    dungeons::{prep_cave_map, prep_dungeon_map, prepare_dungeon_data},
    hexmap::{apply_map_context, extract_map_context, generate_hex_map_json},
    watabou::json::{Coords, PointOfInterest},
};

use hexroll3_scroll::{
    ValueUuidExt,
    frame::load_all_unused_uids,
    generators::*,
    instance::{SandboxInstance, *},
    renderer::{render_entity, render_entity_html},
    repository::ReadOnlyLoader,
};

use crate::{
    battlemaps::{
        BattleMapConstructs, BattlemapFeatureUtils, CaveMapConstructs, CityMapConstructs,
        DungeonMapConstructs, RequestCityOrTownFromBackend, RequestDungeonFromBackend,
        RequestVillageFromBackend, VillageMapConstructs,
    },
    clients::controller::RenamingResponse,
    content::{ContentMode, ContentPageModel, RenameSandboxEntity},
    hexmap::{
        HexMapJson, HexMapTileMaterials, HexmapTheme,
        elements::{
            AppendSandboxEntity, AppendSubject, FetchEntityFromStorage, HexMapData,
            HexMapResources, HexMapToolState, RemoveSandboxEntity, hexx_to_hexroll_coords,
        },
        prepare_hex_map_data,
    },
    shared::{
        asynchttp::AsyncBackendTasks,
        effects::{EffectSystems, RollNewFeatureEffect},
        settings::UserSettings,
        vtt::LoadVttState,
    },
};
use crate::{content::context::ContentContext, shared::widgets::cursor::CursorController};

use super::NodeBackendEvent;
use super::model::FetchEntityReason;
use super::{
    StandaloneBackendEvent,
    controller::{
        ClientState, FeatureUidResponse, PerformHexMapActionInBackend, PostMapLoadedOp,
        PostMapLoadedOpPrefix, RequestMapFromBackend, RequestMapResult,
        RequestSandboxFromBackend, SearchEntitiesInBackend,
    },
    http::LoadStateResponse,
    model::{BackendUid, RerollEntity, SearchResponse},
};

pub struct StandaloneClientPlugin;

impl Plugin for StandaloneClientPlugin {
    fn build(&self, app: &mut App) {
        app // -->
            .add_observer(request_sandbox_standalone)
            .add_observer(fetch_hex_standalone)
            .add_observer(append_feature_standalone)
            .add_observer(remove_entity_standalone)
            .add_observer(request_hex_map_standalone)
            .add_observer(load_vtt_state_standalone)
            .add_observer(request_dungeon_map_standalone)
            .add_observer(request_city_standalone)
            .add_observer(request_village_standalone)
            .add_observer(request_a_reroll_standalone)
            .add_observer(request_hex_action_standlone)
            .add_observer(request_search_standalone)
            .add_observer(rename_entity_standalone)
            // NOTE: Standalone player nodes are serverless as well:
            .add_observer(request_hex_map_for_standalone_player_node)
            .add_observer(request_dungeon_map_for_standalone_player_node)
            .add_observer(request_city_map_for_standalone_player_node)
            .add_observer(request_village_map_for_standalone_player_node)
            // NOTE: Rollback support
            .add_systems(Update, handle_user_triggered_rollback.run_if(resource_exists::<StandaloneSandbox>))
            // <--
            ;
    }
}

#[derive(Resource)]
pub struct StandaloneSandbox {
    pub instance: SandboxInstance,
    pub index: std::sync::Arc<std::sync::Mutex<SandboxIndex>>,
}

pub struct SandboxIndex {
    pub terms: BTreeMap<String, String>,
}
impl SandboxIndex {
    pub fn new() -> Self {
        SandboxIndex {
            terms: BTreeMap::new(),
        }
    }
}

pub fn request_sandbox_standalone(
    trigger: On<StandaloneBackendEvent<RequestSandboxFromBackend>>,
    mut commands: Commands,
) {
    let event = &trigger.event().0;

    let mut instance = SandboxInstance::new();

    let scroll_path = UserSettings::sandbox_main_scroll_path(&event.sandbox_uid);

    if let Err(err) = instance.with_scroll(scroll_path) {
        error!("Failed to load scroll: {err}");
        return;
    }

    let filepath = UserSettings::sandbox_path(&event.sandbox_uid);
    if let Err(err) = instance.open(filepath.to_str().unwrap()) {
        error!(
            "Failed to open sandbox instance (uid={} path={}): {err}",
            event.sandbox_uid,
            filepath.display()
        );
        return;
    }

    commands.insert_resource(StandaloneSandbox {
        instance,
        index: std::sync::Arc::new(std::sync::Mutex::new(SandboxIndex::new())),
    });
    commands.trigger(RequestMapResult::Loaded(event.sandbox_uid.clone(), None));
}

// ---------------------------------------------------------------------------------------------------------
pub fn append_feature_standalone(
    trigger: On<StandaloneBackendEvent<AppendSandboxEntity>>,
    sandbox: Res<StandaloneSandbox>,
    mut commands: Commands,
    effects: Res<EffectSystems>,
    mut async_tasks: ResMut<AsyncBackendTasks<String, FeatureUidResponse>>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut cursor_controller: ResMut<CursorController>,
) {
    cursor_controller.loading(&mut commands, *window);

    let sid = sandbox.instance.sid.clone().unwrap();
    let event = trigger.event().0.clone();

    // NOTE: Resolve the target into the three pieces the backend needs:
    // hex_uid: the entity to append onto
    // maybe_coords: hex coordinates to stamp onto the new entity if any
    // maybe_building_index: building slot to stamp onto the new entity if any
    // fetch_uid: the UID to open in the content panel after appending
    //            (None means use the new entity's own UID)
    let (hex_uid, maybe_coords, maybe_building_index, fetch_uid) = match &event.target {
        AppendSubject::Hex { uid, coords } => {
            if let Some(coords) = coords {
                commands.spawn_empty().insert(RollNewFeatureEffect(*coords));
                commands.run_system(effects.roll_feature_effect);
            }
            (uid.clone(), *coords, None, Some(uid.clone()))
        }
        AppendSubject::Ocean { coords } => {
            commands.spawn_empty().insert(RollNewFeatureEffect(*coords));
            commands.run_system(effects.roll_feature_effect);
            (
                sandbox.instance.sid.clone().unwrap(),
                Some(*coords),
                None,
                None,
            )
        }
        AppendSubject::SettlementDistrict {
            district_uid,
            building_index,
        } => (district_uid.clone(), None, Some(*building_index), None),
    };

    let mut instance = sandbox.instance.clone();

    if async_tasks
        .spawn_standalone(sid, move || -> Option<FeatureUidResponse> {
            let mut hex_map = hexroll3_cartographer::hexmap::HexMap::new();
            hex_map.reconstruct(&mut instance);

            let builder = SandboxBuilder::from_instance(&instance);
            let Ok(mut blueprint) = builder.sandbox.blueprint.lock() else {
                return None;
            };

            blueprint.map_data_provider = map_data_providers();

            match builder.sandbox.repo.mutate(|mut tx| {
                let uids = append(
                    &builder,
                    &mut blueprint,
                    &mut tx,
                    &hex_uid,
                    &event.attr,
                    Some(&event.what),
                    1,
                )?;
                let Some(uid) = uids.first() else {
                    return Err(anyhow::anyhow!("Something went wrong with appending"));
                };

                // Stamp hex coordinates onto the new entity if provided.
                if let Some(coords) = maybe_coords {
                    let entity = tx.load(&uid)?;
                    let (x, y) = hexx_to_hexroll_coords(&coords);
                    entity["$coords"]["x"] = x.into();
                    entity["$coords"]["y"] = y.into();
                    tx.save(&uid)?;
                }

                // Stamp building index onto the new entity for settlement district appends.
                if let Some(building_index) = maybe_building_index {
                    let entity = tx.load(&uid)?;
                    entity["building_index"] = building_index.into();
                    tx.save(&uid)?;

                    // Refresh the settlement map so the new entity occupies its building slot.
                    let district = tx.load(&hex_uid)?.clone();
                    let rendered =
                        render_entity(&builder.sandbox, &mut blueprint, tx, &district, true)?;
                    let setuid = rendered["SettlementUUID"].as_str().unwrap().to_string();
                    hexroll3_cartographer::watabou::refresh_city_map(
                        tx,
                        &builder.randomizer,
                        &setuid,
                    )?;
                }

                if let Some(on_roll) = tx.load(&uid)?.clone().get("$on_roll") {
                    if on_roll == "roll_settlement_map" {
                        let builder = SandboxBuilder::from_instance(&instance);
                        hexroll3_cartographer::watabou::map_settlement(
                            tx,
                            &builder.randomizer,
                            &mut hex_map,
                            &hex_uid,
                        )?;
                        hex_map.stage_trails(tx)?;
                    }
                }

                {
                    let rerolls = tx.load("rerolls").unwrap().clone();
                    tx.emplace_and_save("rerolls", json!({"entities": []}))?;
                    for r in rerolls["entities"].as_array().unwrap() {
                        let uid = r.as_str().unwrap();
                        if tx.load(uid).is_ok() {
                            reroll(&builder, &mut blueprint, tx, uid, None)?;
                        }
                    }
                }

                Ok(uid.clone())
            }) {
                Ok(uid) => Some(FeatureUidResponse(
                    format!("{{ \"uuid\":\"{}\" }}", uid),
                    maybe_coords,
                    fetch_uid.clone(),
                )),
                Err(e) => {
                    error!("{}", e.to_string());
                    None
                }
            }
        })
        .is_err()
    {
        cursor_controller.done(&mut commands, *window);
    }
}

pub struct RemoveResponse {
    pub removed_entity: String,
    pub maybe_parent_id: Option<String>,
    pub history_to_invalidate: Vec<String>,
}

// ---------------------------------------------------------------------------------------------------------
pub fn remove_entity_standalone(
    trigger: On<StandaloneBackendEvent<RemoveSandboxEntity>>,
    sandbox: Res<StandaloneSandbox>,
    mut commands: Commands,
    mut async_tasks: ResMut<AsyncBackendTasks<String, RemoveResponse>>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut cursor_controller: ResMut<CursorController>,
    content_context: Res<ContentContext>,
) {
    cursor_controller.loading(&mut commands, *window);

    let event = trigger.event().0.clone();
    let instance = sandbox.instance.clone();
    let maybe_context_uid = content_context.current_entity_uid.clone();
    let content_history_to_verify = content_context.history.clone();

    if async_tasks
        .spawn_standalone(event.uid.clone(), move || -> Option<RemoveResponse> {
            let builder = SandboxBuilder::from_instance(&instance);
            let Ok(mut blueprint) = builder.sandbox.blueprint.try_lock() else {
                error!("Unable to obtain blueprint lock when removing entity");
                return None;
            };

            match builder.sandbox.repo.mutate(|tx| {
                let entity = tx.load(&event.uid)?.clone();
                let maybe_settlement_related = {
                    let rendered =
                        render_entity(&builder.sandbox, &mut blueprint, tx, &entity, true)?;
                    rendered["SettlementUUID"]
                        .as_str()
                        .and_then(|v| Some(v.to_string()))
                };
                remove(&builder, &mut blueprint, tx, &event.uid)?;

                if let Some(setuid) = maybe_settlement_related {
                    hexroll3_cartographer::watabou::refresh_city_map(
                        tx,
                        &builder.randomizer,
                        &setuid,
                    )?;
                }
                let maybe_parent_id = match maybe_context_uid.as_ref() {
                    Some(content_uid) if !tx.exists(content_uid)? => {
                        entity["parent_uid"].as_str().map(|s| s.to_string())
                    }
                    _ => None,
                };

                let mut history_to_invalidate: Vec<String> = Vec::new();
                for history_entry_to_verify in content_history_to_verify.iter() {
                    if !tx.exists(&history_entry_to_verify)? {
                        history_to_invalidate.push(history_entry_to_verify.clone());
                    }
                }

                Ok((maybe_parent_id, history_to_invalidate))
            }) {
                Ok((maybe_parent_id, history_to_invalidate)) => Some(RemoveResponse {
                    removed_entity: event.uid.clone(),
                    maybe_parent_id,
                    history_to_invalidate,
                }),
                Err(e) => {
                    error!("{}", e.to_string());
                    None
                }
            }
        })
        .is_err()
    {
        // Error
        cursor_controller.done(&mut commands, *window);
    }
}

// ---------------------------------------------------------------------------------------------------------
pub fn fetch_hex_standalone(
    trigger: On<StandaloneBackendEvent<FetchEntityFromStorage>>,
    sandbox: Res<StandaloneSandbox>,
    mut commands: Commands,
    window: Single<Entity, With<PrimaryWindow>>,
    mut cursor_controller: ResMut<CursorController>,
    mut async_tasks: ResMut<
        AsyncBackendTasks<String, (ContentPageModel, FetchEntityReason, Option<String>)>,
    >,
) {
    let uid = trigger.event().0.uid.clone();
    let why = trigger.event().0.why.clone();
    cursor_controller.loading(&mut commands, *window);
    let anchor = trigger.event().0.anchor.clone();
    let instance = sandbox.instance.clone();
    if async_tasks
        .spawn_standalone(
            uid.clone(),
            move || -> Option<(ContentPageModel, FetchEntityReason, Option<String>)> {
                let Ok(mut blueprint) = instance.blueprint.try_lock() else {
                    error!("Error trying to lock the sandbox blueprint");
                    return None;
                };
                match instance.repo.inspect(|tx| {
                    let e = tx.load(&uid)?;
                    let (header_html, body_html) =
                        render_entity_html(&instance, &mut blueprint, tx, &e.value)?;
                    let ret = (
                        ContentPageModel::from_entity_html(&uid, &(header_html + &body_html)),
                        why.clone(),
                        anchor.clone(),
                    );
                    Ok(ret)
                }) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        error!("{:?}", e);
                        None
                    }
                }
            },
        )
        .is_err()
    {
        cursor_controller.done(&mut commands, *window);
    }
}

// ---------------------------------------------------------------------------------------------------------
pub fn request_hex_map_standalone(
    trigger: On<StandaloneBackendEvent<RequestMapFromBackend>>,
    mut async_tasks: ResMut<
        AsyncBackendTasks<String, (Option<HexMapData>, PostMapLoadedOp, Option<HexMapJson>)>,
    >,
    sandbox: Option<Res<StandaloneSandbox>>,
    mut commands: Commands,
    // callbacks: Res<HexEntityCallbacks>,
    assets: Res<HexMapResources>,
    tiles: Res<HexMapTileMaterials>,
    theme: Res<HexmapTheme>,
    map_data: Res<HexMapData>,
) {
    let Some(sandbox) = sandbox else {
        return;
    };
    if map_data.generating {
        commands.trigger(trigger.0.post_map_loaded_op.clone());
        return;
    }
    commands.trigger(PostMapLoadedOpPrefix {
        post_map_op: trigger.0.post_map_loaded_op.clone(),
    });

    let sid = sandbox.instance.sid.clone().unwrap().clone();
    let curved_mesh_tile_set = assets.curved_mesh_tile_set.clone();
    let tiles = tiles.clone();
    let post_map_loaded_op = trigger.0.post_map_loaded_op.clone();
    let scale_calculator = theme.tile_scale_values();
    let instance = sandbox.instance.clone();

    if async_tasks
        .spawn_standalone(
            sid,
            move || -> Option<(Option<HexMapData>, PostMapLoadedOp, Option<HexMapJson>)> {
                let map = generate_hex_map_json(&instance);
                let s = match map {
                    Ok(m) => m.to_string(),
                    Err(err) => {
                        error!("Failed to load hex map: {}", err);
                        return None;
                    }
                };
                if let Ok(map) = serde_json::from_str::<HexMapJson>(&s) {
                    Some((
                        Some(prepare_hex_map_data(
                            map.clone(),
                            curved_mesh_tile_set.clone(),
                            tiles.clone(),
                            scale_calculator,
                        )),
                        post_map_loaded_op.clone(),
                        Some(map),
                    ))
                } else {
                    None
                }
            },
        )
        .is_err()
    {};
}

pub fn load_vtt_state_standalone(
    _trigger: On<StandaloneBackendEvent<LoadVttState>>,
    sandbox: Res<StandaloneSandbox>,
    user_settings: Res<UserSettings>,
    mut async_tasks: ResMut<AsyncBackendTasks<String, LoadStateResponse>>,
) {
    let Some(sandbox_uid) = user_settings.clone().sandbox else {
        return;
    };
    let sid = sandbox.instance.sid.clone().unwrap().clone();
    if async_tasks
        .spawn_standalone(sid.clone(), move || -> Option<LoadStateResponse> {
            let config_path = ClientState::path(&sandbox_uid.clone());

            if let Ok(data) = fs::read_to_string(config_path) {
                if let Ok(state) = serde_json::from_str::<ClientState>(&data) {
                    return Some(LoadStateResponse(state));
                }
            }
            return Some(LoadStateResponse(ClientState::default()));
        })
        .is_err()
    {};
}

fn retrieve_settlement_map_data(
    tx: &mut hexroll3_scroll::repository::ReadOnlyTransaction,
    hex_uid: &str,
) -> anyhow::Result<Option<(serde_json::Value, Vec<PointOfInterest>)>> {
    let hex = tx.retrieve(hex_uid)?;
    let settlement_map = tx.retrieve(hex.value["Settlement"].uuid_as_str())?;
    if settlement_map.value["$map_data"].is_null() {
        return Ok(None);
    }
    let content = settlement_map.value["$map_data"].clone();
    let d1 = settlement_map.value.get("districts");
    let d2 = settlement_map.value.get("District");
    let ds = if d2.is_some() {
        d2.unwrap().as_array().unwrap()
    } else {
        d1.unwrap().as_array().unwrap()
    };
    let mut pois = Vec::new();
    for d in ds {
        let d_data = tx.retrieve(d.as_str().unwrap())?;
        if d_data.value["Tavern"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
        {
            let t_uid = d_data.value["Tavern"][0].as_str().unwrap();
            let t_data = tx.retrieve(t_uid)?;
            pois.push(PointOfInterest {
                coords: Coords { x: 0.0, y: 0.0 },
                title: t_data.value["Title"].as_str().unwrap().to_string(),
                uuid: t_uid.to_string(),
                building: t_data.value["building_index"]
                    .as_i64()
                    .map(|i: i64| i as i32),
            });
        }
        for s in d_data.value["shops"].as_array().unwrap() {
            let s_data = tx.retrieve(s.as_str().unwrap())?;
            pois.push(PointOfInterest {
                coords: Coords { x: 0.0, y: 0.0 },
                title: s_data.value["Title"].as_str().unwrap().to_string(),
                uuid: s.as_str().unwrap().to_string(),
                building: s_data.value["building_index"]
                    .as_i64()
                    .map(|i: i64| i as i32),
            });
        }
    }
    Ok(Some((content, pois)))
}

pub fn request_city_standalone(
    trigger: On<StandaloneBackendEvent<RequestCityOrTownFromBackend>>,
    sandbox: Res<StandaloneSandbox>,
    mut commands: Commands,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, Entity), (BattleMapConstructs, String)>>,
) {
    let event = trigger.event().0.clone();
    let repo = sandbox.instance.repo.clone();
    let task = move || -> Option<(BattleMapConstructs, String)> {
        repo.inspect(|tx| {
            Ok(
                retrieve_settlement_map_data(tx, &event.uid)?.map(|(content, pois)| {
                    let data = json!({"map_data": content, "poi": pois}).to_string();
                    (
                        BattleMapConstructs::City(CityMapConstructs::from(data.clone())),
                        data,
                    )
                }),
            )
        })
        .unwrap_or_else(|e| {
            error!("{:?}", e);
            None
        })
    };

    if my_tasks
        .spawn_standalone((trigger.0.uid.clone(), trigger.0.hex), task)
        .is_err()
    {
        commands
            .entity(trigger.0.hex)
            .reset_battlemap_loading_state();
    }
}

pub fn request_village_standalone(
    trigger: On<StandaloneBackendEvent<RequestVillageFromBackend>>,
    sandbox: Res<StandaloneSandbox>,
    mut commands: Commands,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, Entity), (BattleMapConstructs, String)>>,
) {
    let event = trigger.event().0.clone();
    let repo = sandbox.instance.repo.clone();
    let task = move || -> Option<(BattleMapConstructs, String)> {
        repo.inspect(|tx| {
            Ok(
                retrieve_settlement_map_data(tx, &event.uid)?.map(|(content, pois)| {
                    let data = json!({"map_data": content, "poi": pois}).to_string();
                    (
                        BattleMapConstructs::Village(VillageMapConstructs::from(
                            BackendUid::from(event.uid.clone()),
                            data.clone(),
                        )),
                        data,
                    )
                }),
            )
        })
        .unwrap_or_else(|e| {
            error!("{:?}", e);
            None
        })
    };

    if my_tasks
        .spawn_standalone((trigger.0.uid.clone(), trigger.0.hex), task)
        .is_err()
    {
        commands
            .entity(trigger.0.hex)
            .reset_battlemap_loading_state();
    }
}

pub fn request_dungeon_map_standalone(
    trigger: On<StandaloneBackendEvent<RequestDungeonFromBackend>>,
    sandbox: Res<StandaloneSandbox>,
    mut commands: Commands,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, Entity), (BattleMapConstructs, String)>>,
) {
    let event = trigger.event().0.clone();
    let instance = &sandbox.instance;
    let repo = instance.repo.clone();
    let task = move || -> Option<(BattleMapConstructs, String)> {
        repo.mutate_ex(false, |tx| {
            let data = prepare_dungeon_data(tx, &event.uid)?;

            Ok(if data.contains("areas") {
                Some((
                    BattleMapConstructs::Dungeon(DungeonMapConstructs::from(data.clone())),
                    data,
                ))
            } else if data.contains("caverns") {
                Some((
                    BattleMapConstructs::Cave(CaveMapConstructs::from(
                        data.clone(),
                        BackendUid::from(event.uid.clone()),
                    )),
                    data,
                ))
            } else {
                // (BattleMapConstructs::Empty, data.clone())
                None
            })
        })
        .unwrap_or_else(|e| {
            error!("{:?}", e);
            None
        })
    };

    if my_tasks
        .spawn_standalone((trigger.0.uid.clone(), trigger.0.hex), task)
        .is_err()
    {
        commands
            .entity(trigger.0.hex)
            .reset_battlemap_loading_state();
    }
}

pub fn request_a_reroll_standalone(
    trigger: On<StandaloneBackendEvent<RerollEntity>>,
    sandbox: Res<StandaloneSandbox>,
    mut my_tasks: ResMut<AsyncBackendTasks<String, (bool, String, Option<hexx::Hex>)>>,
) {
    let event = trigger.event().0.clone();
    let mut instance = sandbox.instance.clone();
    let class_override = event.class_override.clone();
    let uid = event.uid.clone();
    let is_map_reload_needed = event.is_map_reload_needed;
    let maybe_coords = event.coords;
    if my_tasks
        .spawn_standalone(
            uid.clone(),
            move || -> Option<(bool, String, Option<hexx::Hex>)> {
                let mut hex_map = hexroll3_cartographer::hexmap::HexMap::new();
                hex_map.reconstruct(&mut instance);
                let builder = SandboxBuilder::from_instance(&instance);
                let Ok(mut blueprint) = builder.sandbox.blueprint.lock() else {
                    return None;
                };

                blueprint.map_data_provider = |builder, mut blueprint, tx, class_name| {
                    match class_name {
                        "CaveMap" => Some(prep_cave_map(builder, &mut blueprint, tx)),
                        "DungeonMap" => Some(prep_dungeon_map(builder, &mut blueprint, tx)),
                        _ => None,
                    }
                    .transpose()
                };

                if let Ok(uid) = builder.sandbox.repo.mutate(|tx| {
                    let map_context = extract_map_context(tx, &uid)?;

                    let new_uid = reroll(
                        &builder,
                        &mut blueprint,
                        tx,
                        &uid.clone(),
                        if event.class_override == "default" {
                            None
                        } else {
                            Some(&class_override)
                        },
                    )?;

                    if let Some(map_context) = map_context {
                        apply_map_context(&instance, &mut hex_map, tx, map_context, &new_uid)?;
                    }

                    let entity = tx.load(&new_uid)?.clone();

                    if let Some(on_reroll) = entity.get("$on_reroll") {
                        if on_reroll == "remap_in_settlement" {
                            let rendered = render_entity(
                                &builder.sandbox,
                                &mut blueprint,
                                tx,
                                &entity,
                                true,
                            )?;
                            let setuid =
                                rendered["SettlementUUID"].as_str().unwrap().to_string();
                            hexroll3_cartographer::watabou::refresh_city_map(
                                tx,
                                &builder.randomizer,
                                &setuid,
                            )?;
                        }
                    }

                    Ok(new_uid)
                }) {
                    Some((
                        is_map_reload_needed,
                        format!("{{ \"uuid\":\"{}\" }}", uid),
                        maybe_coords,
                    ))
                } else {
                    None
                }
            },
        )
        .is_err()
    {}
}

pub fn request_hex_action_standlone(
    trigger: On<StandaloneBackendEvent<PerformHexMapActionInBackend>>,
    sandbox: Res<StandaloneSandbox>,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, String), String>>,
    map: Res<HexMapData>,
    mut commands: Commands,
    window: Single<Entity, With<PrimaryWindow>>,
    mut cursor_controller: ResMut<CursorController>,
) {
    let event = trigger.event().0.clone();
    let mut instance = sandbox.instance.clone();
    let Some(hex_coords) = map.coords.get(&event.uid) else {
        error!("Hex coordinates not found for uid: {}", event.uid);
        return;
    };
    let hex_uid = event.uid;
    let (x, y) = hexx_to_hexroll_coords(hex_coords);
    let start = hexroll3_cartographer::hexmap::Hex::new(x, y);
    let Some(topic) = event.topic.clone() else {
        return;
    };
    cursor_controller.loading(&mut commands, *window);
    if my_tasks
        .spawn_standalone(
            (hex_uid.clone(), "action".to_string()),
            move || -> Option<String> {
                let mut hex_map = hexroll3_cartographer::hexmap::HexMap::new();
                hex_map.reconstruct(&mut instance);

                let builder = SandboxBuilder::from_instance(&instance);

                if let Ok(()) = builder.sandbox.repo.mutate(|tx| {
                    if event.action == "draw" && topic == "river" {
                        hex_map.draw_river(tx, &builder.randomizer, start, hex_uid.clone())?;
                    }
                    if event.action == "draw" && topic == "trails" {
                        hex_map.stage_trails(tx)?;
                    }
                    if event.action == "clear" && topic == "river" {
                        hex_map.clear_river(tx, &builder.randomizer, &hex_uid)?;
                    }
                    if event.action == "clear" && topic == "trails" {
                        hex_map.fix_trails(tx, &hex_uid)?;
                    }

                    Ok(())
                }) {
                    Some("".to_string())
                } else {
                    None
                }
            },
        )
        .is_err()
    {
        cursor_controller.done(&mut commands, *window);
    }
}

// ---------------------------------------------------------------------------------------------------------
pub fn request_search_standalone(
    trigger: On<StandaloneBackendEvent<SearchEntitiesInBackend>>,
    mut my_tasks: ResMut<AsyncBackendTasks<String, SearchResponse>>,
    sandbox: Res<StandaloneSandbox>,
    user_settings: Res<UserSettings>,
) {
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };
    let instance = sandbox.instance.clone();
    let index = sandbox.index.clone();
    let term = trigger.0.query.clone();
    if my_tasks
        .spawn_standalone(
            sandbox_uid.to_string(),
            move || -> Option<SearchResponse> {
                let Ok(mut search_index) = index.lock() else {
                    return None;
                };

                let result = find_term(&instance, &mut search_index, term.clone()).to_string();
                Some(serde_json::from_str(&result).unwrap_or_default())
            },
        )
        .is_err()
    {}
}

fn find_term(
    instance: &SandboxInstance,
    search_index: &mut SandboxIndex,
    term: String,
) -> serde_json::Value {
    let Some(sid) = instance.sid.clone() else {
        return serde_json::Value::Null;
    };
    let Ok(mut blueprint) = instance.blueprint.lock() else {
        return serde_json::Value::Null;
    };
    instance
        .repo
        .inspect(|tx| {
            let mut ret = serde_json::json!({"results":[]});
            let term = term.to_lowercase();

            let index_uids = load_all_unused_uids(tx, &sid, "IndexedEntity")?;
            let builder = SandboxBuilder::from_instance(instance);

            let mut count: usize = 0;
            for uid_as_str in index_uids.iter() {
                let mut result = String::new();
                if let Some(cached_term) = search_index.terms.get(uid_as_str) {
                    result.push_str(cached_term);
                } else {
                    let entity = tx.retrieve(uid_as_str)?;
                    let parent_uid = entity.value["parent_uid"].as_str().unwrap();
                    if entity.value.get("Render").is_some() {
                        let parent = tx.retrieve(parent_uid)?;
                        let key_as_str = entity.value["Render"].as_str().ok_or_else(|| {
                            anyhow::anyhow!("IndexedEntity Render is not string")
                        })?;
                        if parent.value[key_as_str].is_array() {
                            let sub = tx.retrieve(parent.value[key_as_str].uuid_as_str())?;

                            let rendered =
                                render_entity(instance, &mut blueprint, tx, &sub.value, true)?;

                            let rendered_result = builder
                                .templating_env
                                .render_str(
                                    entity.value["Search"].as_str().ok_or_else(|| {
                                        anyhow::anyhow!("IndexedEntity Search is not string")
                                    })?,
                                    &rendered,
                                )
                                .map_err(anyhow::Error::new)?;
                            result.push_str(&rendered_result);
                        } else {
                            result.push_str(
                                parent.value[key_as_str].as_str().ok_or_else(|| {
                                    anyhow::anyhow!("Parent key is not string")
                                })?,
                            );
                        }
                    } else if entity.value.get("Value").is_some()
                        && entity.value.get("Self").is_some()
                    {
                        let data =
                            render_entity(instance, &mut blueprint, tx, &entity.value, true)?;

                        let rendered_result = builder
                            .templating_env
                            .render_str(
                                entity.value["Value"].as_str().ok_or_else(|| {
                                    anyhow::anyhow!("IndexedEntity Value is not string")
                                })?,
                                &data,
                            )
                            .map_err(anyhow::Error::new)?;
                        result.push_str(&rendered_result);
                    } else if entity.value.get("Value").is_some() {
                        let rendered_result = builder
                            .templating_env
                            .render_str(
                                entity.value["Value"].as_str().ok_or_else(|| {
                                    anyhow::anyhow!("IndexedEntity Value is not string")
                                })?,
                                &entity.value,
                            )
                            .map_err(anyhow::Error::new)?;
                        result.push_str(&rendered_result);
                    } else {
                        if entity.value.get("Search").is_none() {
                            return Err(anyhow::anyhow!(
                                "Malformed IndexedEntity (missing Search): {}",
                                entity.value
                            ));
                        } else {
                            result.push_str(entity.value["Search"].as_str().ok_or_else(
                                || anyhow::anyhow!("IndexedEntity Search is not string"),
                            )?);
                        }
                    }
                    search_index
                        .terms
                        .insert(uid_as_str.to_string(), result.clone());
                }

                let result_copy = result.clone();
                let result_lower = result.to_lowercase();

                if result_lower.contains(&term) {
                    //
                    let entity = tx.retrieve(uid_as_str)?;
                    //

                    let mut result_record = serde_json::json!({});
                    result_record["value"] = serde_json::Value::String(result_copy);

                    if entity.value.get("Details").is_some() {
                        let parent_uid = entity.value["parent_uid"].as_str().unwrap();
                        let parent = tx.retrieve(parent_uid)?;
                        let render_as_root_is_false_for_speed = false;
                        let rendered_parent = render_entity(
                            instance,
                            &mut blueprint,
                            tx,
                            &parent.value,
                            render_as_root_is_false_for_speed,
                        )?;

                        let details = builder
                            .templating_env
                            .render_str(
                                entity.value["Details"]
                                    .as_str()
                                    .ok_or_else(|| anyhow::anyhow!("Details is not string"))?,
                                &rendered_parent,
                            )
                            .map_err(anyhow::Error::new)?;
                        result_record["details"] = serde_json::Value::String(details);
                    } else {
                        result_record["details"] = serde_json::Value::String(String::new());
                    }

                    let rendered_entity =
                        render_entity(instance, &mut blueprint, tx, &entity.value, true)?;

                    result_record["uuid"] = rendered_entity
                        .get("Link")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    result_record["type"] = rendered_entity
                        .get("Type")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    result_record["icon"] = rendered_entity
                        .get("Icon")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    result_record["anchor"] = rendered_entity
                        .get("Anchor")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);

                    ret["results"]
                        .as_array_mut()
                        .ok_or_else(|| anyhow::anyhow!("ret.results is not an array"))?
                        .push(result_record);

                    count += 1;
                    if count > 42 {
                        break;
                    }
                }
            }

            Ok(ret)
        })
        .unwrap_or_else(|e| {
            error!("{:?}", e);
            serde_json::json!({"results":[]})
        })
}

// ---------------------------------------------------------------------------------------------------------
pub fn rename_entity_standalone(
    trigger: On<StandaloneBackendEvent<RenameSandboxEntity>>,
    sandbox: Res<StandaloneSandbox>,
    mut async_tasks: ResMut<AsyncBackendTasks<String, RenamingResponse>>,
    user_settings: Res<UserSettings>,
) {
    let event = &trigger.event().0;
    let Some(sandbox_uid) = &user_settings.sandbox else {
        return;
    };

    let params = event.params.clone();
    let uid = event.entity_uid.clone();
    let value = sanitize_renaming_value(&event.value);
    if value.is_empty() {
        return;
    }
    let instance = sandbox.instance.clone();
    let index = sandbox.index.clone();

    if async_tasks
        .spawn_standalone(
            sandbox_uid.to_string(),
            move || -> Option<RenamingResponse> {
                let Ok(mut search_index) = index.lock() else {
                    error!("Unable to aquire search index lock");
                    return None;
                };

                if let Err(tx_err) = instance.repo.mutate(|tx| {
                    let name_entity_uid = params.attr_entity.as_ref().unwrap_or(&uid);
                    let entity = tx.load(name_entity_uid)?;
                    entity[params.clone().attr_name] = value.clone().into();
                    tx.save(name_entity_uid)?;
                    Ok(())
                }) {
                    error!("{}", tx_err);
                    return None;
                }

                let cache_uid = params
                    .cache_entity
                    .as_ref()
                    .unwrap_or_else(|| params.attr_entity.as_ref().unwrap_or(&uid));

                // NOTE: We're doing some acrobatics to selectivly clear the term
                // from the index cache. Alternatively we could have just
                // cleared the entire cache using `search_index.terms.clear();`
                if let Ok(cache_uid) = instance
                    .repo
                    .inspect(|tx| {
                        let entity = tx.fetch(cache_uid)?;
                        if let Some(index_ref) =
                            entity.get("$IndexRef").and_then(|v| v.as_array())
                        {
                            if let Some(first_index) = index_ref.iter().next() {
                                if let Some(index_str) = first_index.as_str() {
                                    return Ok(index_str.to_string());
                                }
                            }
                        }
                        Err(anyhow::anyhow!("Unable to get the index ref uid"))
                    })
                    .map_err(|err| error!("Index cache: {}", err.to_string()))
                {
                    search_index.terms.remove(&cache_uid);
                }

                Some(RenamingResponse(uid.clone(), params.clone()))
            },
        )
        .is_err()
    {}
}

pub fn sanitize_renaming_value(input: &str) -> String {
    input
        .chars()
        .filter(|&c| c.is_alphabetic() || c == ' ' || c == '\'' || c == '-')
        .take(80)
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// Standalone (serverless) player node observers
// ---------------------------------------------------------------------------------------------------------
pub fn request_hex_map_for_standalone_player_node(
    trigger: On<NodeBackendEvent<RequestMapFromBackend>>,
    mut async_tasks: ResMut<
        AsyncBackendTasks<String, (Option<HexMapData>, PostMapLoadedOp, Option<HexMapJson>)>,
    >,
    mut commands: Commands,
    assets: Res<HexMapResources>,
    tiles: Res<HexMapTileMaterials>,
    theme: Res<HexmapTheme>,
    vtt_data: Res<crate::shared::vtt::VttData>,
    user_settings: Res<UserSettings>,
) {
    let Some(sandbox_id) = user_settings.sandbox.clone() else {
        return;
    };
    commands.trigger(PostMapLoadedOpPrefix {
        post_map_op: trigger.0.post_map_loaded_op.clone(),
    });

    let curved_mesh_tile_set = assets.curved_mesh_tile_set.clone();
    let tiles = tiles.clone();
    let post_map_loaded_op = trigger.0.post_map_loaded_op.clone();
    let scale_calculator = theme.tile_scale_values();
    let map = vtt_data.cache.clone();

    if async_tasks
        .spawn_standalone(
            sandbox_id,
            move || -> Option<(Option<HexMapData>, PostMapLoadedOp, Option<HexMapJson>)> {
                if let Some(map) = map.clone() {
                    Some((
                        Some(prepare_hex_map_data(
                            map.clone(),
                            curved_mesh_tile_set.clone(),
                            tiles.clone(),
                            scale_calculator,
                        )),
                        post_map_loaded_op.clone(),
                        Some(map),
                    ))
                } else {
                    None
                }
            },
        )
        .is_err()
    {};
}

pub fn request_dungeon_map_for_standalone_player_node(
    trigger: On<NodeBackendEvent<RequestDungeonFromBackend>>,
    mut commands: Commands,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, Entity), (BattleMapConstructs, String)>>,
    cache: Res<crate::hexmap::elements::HexMapCache>,
) {
    let event = trigger.event().0.clone();

    if let Some(data) = cache.jsons.get(&trigger.event().0.uid) {
        let data = data.to_string();
        if my_tasks
            .spawn_standalone(
                (trigger.0.uid.clone(), trigger.0.hex),
                move || -> Option<(BattleMapConstructs, String)> {
                    if data.contains("areas") {
                        Some((
                            BattleMapConstructs::Dungeon(DungeonMapConstructs::from(
                                data.clone(),
                            )),
                            data.to_string(),
                        ))
                    } else if data.contains("caverns") {
                        Some((
                            BattleMapConstructs::Cave(CaveMapConstructs::from(
                                data.clone(),
                                BackendUid::from(event.uid.clone()),
                            )),
                            data.to_string(),
                        ))
                    } else {
                        // (BattleMapConstructs::Empty, data.clone())
                        None
                    }
                },
            )
            .is_err()
        {
            commands
                .entity(trigger.0.hex)
                .reset_battlemap_loading_state();
        }
    }
}

pub fn request_city_map_for_standalone_player_node(
    trigger: On<NodeBackendEvent<RequestCityOrTownFromBackend>>,
    mut commands: Commands,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, Entity), (BattleMapConstructs, String)>>,
    cache: Res<crate::hexmap::elements::HexMapCache>,
) {
    if let Some(data) = cache.jsons.get(&trigger.event().0.uid) {
        let data = data.to_string();
        if my_tasks
            .spawn_standalone(
                (trigger.0.uid.clone(), trigger.0.hex),
                move || -> Option<(BattleMapConstructs, String)> {
                    Some((
                        BattleMapConstructs::City(CityMapConstructs::from(data.clone())),
                        data.clone(),
                    ))
                },
            )
            .is_err()
        {
            commands
                .entity(trigger.0.hex)
                .reset_battlemap_loading_state();
        }
    }
}

pub fn request_village_map_for_standalone_player_node(
    trigger: On<NodeBackendEvent<RequestVillageFromBackend>>,
    mut commands: Commands,
    mut my_tasks: ResMut<AsyncBackendTasks<(String, Entity), (BattleMapConstructs, String)>>,
    cache: Res<crate::hexmap::elements::HexMapCache>,
) {
    let event = trigger.event().0.clone();

    if let Some(data) = cache.jsons.get(&trigger.event().0.uid) {
        let data = data.to_string();
        if my_tasks
            .spawn_standalone(
                (trigger.0.uid.clone(), trigger.0.hex),
                move || -> Option<(BattleMapConstructs, String)> {
                    Some((
                        BattleMapConstructs::Village(VillageMapConstructs::from(
                            BackendUid::from(event.uid.clone()),
                            data.clone(),
                        )),
                        data.clone(),
                    ))
                },
            )
            .is_err()
        {
            commands
                .entity(trigger.0.hex)
                .reset_battlemap_loading_state();
        }
    }
}

pub fn handle_user_triggered_rollback(
    mut sandbox: ResMut<StandaloneSandbox>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut content_context: ResMut<ContentContext>,
    mut next_content_mode: ResMut<NextState<ContentMode>>,
    hex_map_tool_state: Res<State<HexMapToolState>>,
) {
    // NOTE: Ctrl-Z is also captured when drawing - so we need to ignore it here.
    if *hex_map_tool_state == HexMapToolState::Draw {
        return;
    }
    if keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
        && keyboard.just_pressed(KeyCode::KeyZ)
    {
        if sandbox
            .instance
            .repo
            .rollback()
            .map_err(|e| error!("Rollback failed: {}", e.to_string()))
            .is_ok()
        {
            // NOTE: We could be invalidating a bunch of stuff so lets clear
            // any trace of potentially non-existing entities.
            next_content_mode.set(ContentMode::MapOnly);
            content_context.history.clear();
            content_context.fistory.clear();
            content_context.current_entity_uid = None;
            content_context.current_hex_uid = None;
            commands.trigger(RequestMapFromBackend {
                post_map_loaded_op: PostMapLoadedOp::InvalidateVisible,
            });
        }
    }
}
