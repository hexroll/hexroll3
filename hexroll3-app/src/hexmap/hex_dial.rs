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
use hexx::*;

use crate::{
    clients::{controller::PerformHexMapActionInBackend, model::RerollEntity},
    hexmap::elements::{AppendSandboxEntity, HexMapData},
    shared::{
        settings::UserSettings,
        vtt::VttData,
        widgets::{
            dial::{
                DialAssets, DialButton, DialButtonState, DialMenuCommands, DialMenuOptions,
                MenuItemSpawner, placeholder_click_handler,
            },
            modal::DiscreteAppState,
        },
    },
};

use super::{
    TerrainType,
    editor::{MapEditor, PenType},
    elements::{HexMapState, HexMapToolState, y_inverted_hexmap_layout},
    selecting::{detect_click, track_hex_under_cursor},
};

pub struct HexDialPlugin;
impl Plugin for HexDialPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(
                Update,
                spawn_dial_menu_when_selecting
                    .run_if(in_state(HexMapToolState::Selection))
                    .after(track_hex_under_cursor)
                    .after(detect_click),
            )
            .add_observer(on_spawn_hex_dial);
    }
}

#[derive(Component, PartialEq)]
pub enum MenuIconLock {
    Locked,
    Unlocked,
}

#[derive(Hash, Clone, PartialEq, Eq, Copy)]
pub enum DialIcon {
    Dice,
    Broom,
    Trash,
    Dungeon,
    Settlement,
    City,
    Town,
    Village,
    Inn,
    Dwelling,
    River,
    Realm,
    Trail,
    Hex,
    Region,
    Feature,
    Pencil,
    RealmLands,
    RealmEmpire,
    RealmKingdom,
    RealmDuchy,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut dial_assets: DialAssets<DialIcon> = DialAssets::new(
        meshes.add(Plane3d::new(Vec3::NEG_Z, Vec2::splat(80.0))),
        meshes.add(Circle::new(1.0)),
    );
    dial_assets
        .add_item(
            DialIcon::Dice,
            "icons/icon-dice.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Broom,
            "icons/icon-broom.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Pencil,
            "icons/icon-pencil-256.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Trash,
            "icons/icon-trash.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Dungeon,
            "icons/icon-dungeon.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Feature,
            "icons/icon-feature.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Trail,
            "icons/icon-trail.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Hex,
            "icons/icon-hex.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Region,
            "icons/icon-region.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Settlement,
            "icons/icon-settlement.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::City,
            "icons/icon-city.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Town,
            "icons/icon-town.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Village,
            "icons/icon-village.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Inn,
            "icons/icon-inn.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Dwelling,
            "icons/icon-dwelling.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::River,
            "icons/icon-river.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Realm,
            "icons/icon-realm.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::RealmDuchy,
            "icons/icon-realm-duchy.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::RealmKingdom,
            "icons/icon-realm-kingdom.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::RealmEmpire,
            "icons/icon-realm-empire.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::RealmLands,
            "icons/icon-realm-lands.ktx2",
            &mut materials,
            &asset_server,
        );
    commands.insert_resource(dial_assets);
}

#[derive(Event)]
struct SpawnHexDial {
    pub hex: Hex,
}

fn spawn_dial_menu_when_selecting(
    mut commands: Commands,
    map: Res<HexMapData>,
    click: Res<ButtonInput<MouseButton>>,
) {
    if click.just_pressed(MouseButton::Right) {
        if map.cursor.is_some()
            && let Some(hex) = map.selected
        {
            commands.trigger(SpawnHexDial { hex });
        }
    }
}

#[derive(Component)]
pub struct LockableDialButton(pub bool);

fn on_spawn_hex_dial(
    trigger: On<SpawnHexDial>,
    locked: Single<&MenuIconLock>,
    mut commands: Commands,
    mut dial_menu_commands: DialMenuCommands,
    dial_assets: Res<DialAssets<DialIcon>>,
    vtt_data: Res<VttData>,
    app_state: Res<State<DiscreteAppState>>,
    map_state: Res<State<HexMapState>>,
    user_settings: Res<UserSettings>,
) {
    if vtt_data.mode.is_player()
        || *app_state != DiscreteAppState::Normal
        || *map_state != HexMapState::Active
    {
        return;
    }

    let layout = y_inverted_hexmap_layout();

    let calc_scale = |v: f32| -> f32 { if v > 1.0 { 1.0 + (v - 1.0).ln_1p() } else { v } };
    let is_visible = |v: f32| v < 20.0 && v > 0.2;
    let pos = layout.hex_to_world_pos(trigger.event().hex);

    if let Some(menu_entity) = dial_menu_commands.spawn_menu(DialMenuOptions {
        pos,
        calc_scale,
        is_visible,
    }) {
        commands.entity(menu_entity).with_children(|c| {
            const MAX_ITEMS_IN_DIAL: i32 = 12;
            c.spawn_empty()
                .spawn_menu_item(
                    12,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Dice,
                    dial_menu_roll_menu(trigger.hex),
                    &dial_assets,
                    "Roll new entities",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    10,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Broom,
                    dial_menu_clean_menu(trigger.hex),
                    &dial_assets,
                    "Clean/revise entities",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    5,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Trash,
                    dial_menu_trash_menu(),
                    &dial_assets,
                    "Remove entities",
                )
                .make_conditional_and_lockable(&locked, true);
            let standalone_sandbox = user_settings.local.unwrap_or(false);
            if standalone_sandbox {
                c.spawn_empty()
                    .spawn_menu_item(
                        2,
                        MAX_ITEMS_IN_DIAL,
                        DialIcon::Pencil,
                        dial_menu_draw_realm(),
                        &dial_assets,
                        "Draw new realms",
                    )
                    .make_conditional_and_lockable(&locked, true);
            }
        });
    }
}

fn create_append_trigger_closure(
    attr: &str,
    what: &str,
) -> impl Fn(On<Pointer<Click>>, Commands, ResMut<NextState<HexMapToolState>>, Res<HexMapData>)
{
    move |_, mut commands, mut next_state, map_data| {
        if let Some(uid) = map_data.get_selected_uid() {
            commands.trigger(AppendSandboxEntity {
                hex_coords: map_data.selected,
                what: what.into(),
                attr: attr.into(),
                hex_uid: uid,
                send_coords: false,
            });
        }
        next_state.set(HexMapToolState::Selection);
    }
}

fn create_editor_trigger_closure(
    realm_type: &str,
) -> impl Fn(On<Pointer<Click>>, ResMut<MapEditor>, ResMut<NextState<HexMapToolState>>) {
    move |_, mut editor, mut next_tool_state| {
        next_tool_state.set(HexMapToolState::Edit);
        editor.realm_type = realm_type.to_string();
        editor.pen = PenType::Brush;
        editor.terrain = TerrainType::MountainsHex;
    }
}

#[allow(clippy::type_complexity)]
fn dial_menu_roll_menu(
    hex: Hex,
) -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<&ChildOf>,
    Query<Entity, With<DialButton>>,
    Res<DialAssets<DialIcon>>,
    Single<&MenuIconLock>,
    Res<HexMapData>,
) {
    move |trigger, mut commands, parents, prev, dial_assets, locked, map_data| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let entity_pointer = parents.get(trigger.entity).unwrap();
        let (hex_is_empty, can_source_a_river) =
            if let Some(prepared_hex) = map_data.hexes.get(&hex) {
                (prepared_hex.is_empty(), prepared_hex.can_source_a_river())
            } else {
                (false, false)
            };
        commands.entity(entity_pointer.parent()).with_children(|c| {
            const MAX_ITEMS_IN_DIAL: i32 = 12;
            c.spawn_empty()
                .spawn_menu_item(
                    10,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Dungeon,
                    create_append_trigger_closure("Dungeon", "Dungeon"),
                    &dial_assets,
                    "Roll a dungeon",
                )
                .make_conditional_and_lockable(&locked, hex_is_empty);
            c.spawn_empty()
                .spawn_menu_item(
                    8,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Settlement,
                    dial_menu_roll_settlement(),
                    &dial_assets,
                    "Roll a settlement",
                )
                .make_conditional_and_lockable(&locked, hex_is_empty);
            c.spawn_empty()
                .spawn_menu_item(
                    6,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::River,
                    move |_: On<Pointer<Click>>,
                          mut commands: Commands,
                          mut next_state: ResMut<NextState<HexMapToolState>>,
                          map_data: Res<HexMapData>| {
                        if let Some(uid) = map_data.get_selected_uid() {
                            commands.trigger(PerformHexMapActionInBackend {
                                uid: uid,
                                action: "draw".into(),
                                topic: Some("river".into()),
                            });
                        }
                        next_state.set(HexMapToolState::Selection);
                    },
                    &dial_assets,
                    "Roll a river (from mountain)",
                )
                .make_conditional_and_lockable(&locked, can_source_a_river);
            c.spawn_empty()
                .spawn_menu_item(
                    4,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Realm,
                    placeholder_click_handler(),
                    &dial_assets,
                    "Roll a realm",
                )
                .make_conditional_and_lockable(&locked, false);
        });
    }
}

#[allow(clippy::type_complexity)]
fn dial_menu_draw_realm() -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<&ChildOf>,
    Query<Entity, With<DialButton>>,
    Res<DialAssets<DialIcon>>,
    Single<&MenuIconLock>,
) {
    move |trigger, mut commands, parents, prev, dial_assets, locked| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let entity_pointer = parents.get(trigger.entity).unwrap();
        commands.entity(entity_pointer.parent()).with_children(|c| {
            const MAX_ITEMS_IN_DIAL: i32 = 12;
            c.spawn_empty()
                .spawn_menu_item(
                    10,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::RealmLands,
                    create_editor_trigger_closure("RealmTypeLands"),
                    &dial_assets,
                    "Draw lands",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    8,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::RealmDuchy,
                    create_editor_trigger_closure("RealmTypeDuchy"),
                    &dial_assets,
                    "Draw a duchy",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    6,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::RealmKingdom,
                    create_editor_trigger_closure("RealmTypeKingdom"),
                    &dial_assets,
                    "Draw a kingdom",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    4,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::RealmEmpire,
                    create_editor_trigger_closure("RealmTypeEmpire"),
                    &dial_assets,
                    "Draw an empire",
                )
                .make_conditional_and_lockable(&locked, true);
        });
    }
}

#[allow(clippy::type_complexity)]
fn dial_menu_roll_settlement() -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<&ChildOf>,
    Query<Entity, With<DialButton>>,
    Res<DialAssets<DialIcon>>,
    Single<&MenuIconLock>,
) {
    move |trigger, mut commands, parents, prev, dial_assets, locked| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let entity_pointer = parents.get(trigger.entity).unwrap();
        commands.entity(entity_pointer.parent()).with_children(|c| {
            const MAX_ITEMS_IN_DIAL: i32 = 12;
            c.spawn_empty()
                .spawn_menu_item(
                    10,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::City,
                    create_append_trigger_closure("Settlement", "City"),
                    &dial_assets,
                    "Roll a city",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    8,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Town,
                    create_append_trigger_closure("Settlement", "Town"),
                    &dial_assets,
                    "Roll a town",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    6,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Village,
                    create_append_trigger_closure("Settlement", "Village"),
                    &dial_assets,
                    "Roll a village",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    4,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Inn,
                    create_append_trigger_closure("Inn", "Inn"),
                    &dial_assets,
                    "Roll an inn",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty().spawn_menu_item(
                2,
                MAX_ITEMS_IN_DIAL,
                DialIcon::Dwelling,
                create_append_trigger_closure("Residency", "Residency"),
                &dial_assets,
                "Roll a dwelling",
            );
        });
    }
}

#[allow(clippy::type_complexity)]
fn dial_menu_clean_menu(
    hex: Hex,
) -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<&ChildOf>,
    Query<Entity, With<DialButton>>,
    Res<DialAssets<DialIcon>>,
    Single<&MenuIconLock>,
    Res<HexMapData>,
) {
    move |trigger, mut commands, parents, prev, dial_assets, locked, map_data| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let entity_pointer = parents.get(trigger.entity).unwrap();
        let (has_a_river, has_a_trail) = if let Some(prepared_hex) = map_data.hexes.get(&hex) {
            (prepared_hex.has_a_river(), prepared_hex.has_a_trail())
        } else {
            (false, false)
        };
        commands.entity(entity_pointer.parent()).with_children(|c| {
            const MAX_ITEMS_IN_DIAL: i32 = 12;
            c.spawn_empty()
                .spawn_menu_item(
                    10,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Trail,
                    move |_: On<Pointer<Click>>,
                          mut commands: Commands,
                          mut next_state: ResMut<NextState<HexMapToolState>>,
                          map_data: Res<HexMapData>| {
                        if let Some(uid) = map_data.get_selected_uid() {
                            commands.trigger(PerformHexMapActionInBackend {
                                uid: uid,
                                action: "clear".into(),
                                topic: Some("trails".into()),
                            });
                        }
                        next_state.set(HexMapToolState::Selection);
                    },
                    &dial_assets,
                    "Restructure this trail",
                )
                .make_conditional_and_lockable(&locked, has_a_trail);
            c.spawn_empty()
                .spawn_menu_item(
                    8,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Feature,
                    move |_: On<Pointer<Click>>,
                          mut commands: Commands,
                          mut next_state: ResMut<NextState<HexMapToolState>>,
                          map_data: Res<HexMapData>| {
                        if let Some((hex_uid, class)) = map_data.get_selected_uid_and_class() {
                            if let Some(hex_coords) = map_data.selected {
                                commands.trigger(RerollEntity {
                                    coords: Some(hex_coords),
                                    class_override: class,
                                    uid: hex_uid,
                                    is_map_reload_needed: false,
                                });
                            }
                        }
                        next_state.set(HexMapToolState::Selection);
                    },
                    &dial_assets,
                    "Clear this hex",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    6,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::River,
                    move |_: On<Pointer<Click>>,
                          mut commands: Commands,
                          mut next_state: ResMut<NextState<HexMapToolState>>,
                          map_data: Res<HexMapData>| {
                        if let Some(uid) = map_data.get_selected_uid() {
                            commands.trigger(PerformHexMapActionInBackend {
                                uid: uid,
                                action: "clear".into(),
                                topic: Some("river".into()),
                            });
                        }
                        next_state.set(HexMapToolState::Selection);
                    },
                    &dial_assets,
                    "Remove river",
                )
                .make_conditional_and_lockable(&locked, has_a_river);
        });
    }
}

#[allow(clippy::type_complexity)]
fn dial_menu_trash_menu() -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<&ChildOf>,
    Query<Entity, With<DialButton>>,
    Res<DialAssets<DialIcon>>,
    Single<&MenuIconLock>,
) {
    move |trigger, mut commands, parents, prev, dial_assets, locked| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let entity_pointer = parents.get(trigger.entity).unwrap();
        commands.entity(entity_pointer.parent()).with_children(|c| {
            const MAX_ITEMS_IN_DIAL: i32 = 12;
            c.spawn_empty()
                .spawn_menu_item(
                    10,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Hex,
                    move |_: On<Pointer<Click>>,
                          mut commands: Commands,
                          mut next_state: ResMut<NextState<HexMapToolState>>,
                          map_data: Res<HexMapData>| {
                        if let Some(uid) = map_data.get_selected_uid() {
                            commands.trigger(PerformHexMapActionInBackend {
                                uid: uid,
                                action: "remove".into(),
                                topic: None,
                            });
                        }
                        next_state.set(HexMapToolState::Selection);
                    },
                    &dial_assets,
                    "Remove this hex",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    8,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Region,
                    placeholder_click_handler(),
                    &dial_assets,
                    "Remove this region",
                )
                .make_conditional_and_lockable(&locked, true);
            c.spawn_empty()
                .spawn_menu_item(
                    6,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Realm,
                    placeholder_click_handler(),
                    &dial_assets,
                    "Remove this realm",
                )
                .make_conditional_and_lockable(&locked, true);
        });
    }
}

trait MakeLockableDialButton {
    fn make_conditional_and_lockable(
        &mut self,
        locked: &MenuIconLock,
        cond: bool,
    ) -> &mut Self;
}
impl MakeLockableDialButton for EntityCommands<'_> {
    fn make_conditional_and_lockable(
        &mut self,
        locked: &MenuIconLock,
        cond: bool,
    ) -> &mut Self {
        self.insert(LockableDialButton(cond)).insert(
            if *locked == MenuIconLock::Locked || !cond {
                DialButtonState::Disabled
            } else {
                DialButtonState::Enabled
            },
        );
        self
    }
}
