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

use crate::{
    hexmap::elements::{
        AppendSandboxEntity, AppendSubject, HexMapState, HexToInvalidatePostLoadMarker,
        MainCamera, RemoveSandboxEntity,
    },
    shared::{
        snapshot::FreezeScreenSnapshot,
        vtt::VttData,
        widgets::{
            cursor::pointer_world_position,
            dial::{DialAssets, DialMenuCommands, DialMenuOptions, MenuItemSpawner},
            modal::DiscreteAppState,
        },
    },
};

pub struct SettlementDialPlugin;
impl Plugin for SettlementDialPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_observer(on_spawn_battlemap_dial);
    }
}

#[derive(Hash, Clone, PartialEq, Eq, Copy)]
pub enum DialIcon {
    Shop,
    Inn,
    Trash,
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
    dial_assets.add_item(
        DialIcon::Shop,
        "icons/icon-dwelling.ktx2",
        &mut materials,
        &asset_server,
    );
    dial_assets.add_item(
        DialIcon::Inn,
        "icons/icon-inn.ktx2",
        &mut materials,
        &asset_server,
    );
    dial_assets.add_item(
        DialIcon::Trash,
        "icons/icon-trash.ktx2",
        &mut materials,
        &asset_server,
    );
    commands.insert_resource(dial_assets);
}

#[derive(Event, Clone)]
pub struct SpawnSettlementDial {
    pub pos: Vec3,
    pub district_uid: String,
    pub building_index: i32,
    pub hex_entity: String,
    pub building_uid: Option<String>,
}

fn on_spawn_battlemap_dial(
    trigger: On<SpawnSettlementDial>,
    mut commands: Commands,
    mut dial_menu_commands: DialMenuCommands,
    dial_assets: Res<DialAssets<DialIcon>>,
    vtt_data: Res<VttData>,
    app_state: Res<State<DiscreteAppState>>,
    map_state: Res<State<HexMapState>>,
) {
    if vtt_data.is_remote_player()
        || *app_state != DiscreteAppState::Normal
        || *map_state != HexMapState::Active
    {
        return;
    }

    debug!("AND NOW");
    let calc_scale = |v: f32| -> f32 {
        if v > 0.10 {
            v * 0.10 / (0.10 + (v - 0.10).ln_1p()) * 0.75
        } else {
            v * 0.75
        }
    };
    let is_visible = |v: f32| v < 0.2;
    let pos = trigger.event().pos;
    let building_index = trigger.building_index;
    if let Some(menu_entity) = dial_menu_commands.spawn_menu(DialMenuOptions {
        pos: pos.xz(),
        calc_scale,
        is_visible,
    }) {
        let hex_entity = trigger.hex_entity.clone();
        let district_uid = trigger.district_uid.clone();
        commands.entity(menu_entity).with_children(move |c| {
            const MAX_ITEMS_IN_DIAL: i32 = 8;
            c.spawn_empty().spawn_menu_item(
                0,
                MAX_ITEMS_IN_DIAL,
                DialIcon::Shop,
                move |_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(FreezeScreenSnapshot);
                    commands.trigger(AppendSandboxEntity {
                        target: AppendSubject::SettlementDistrict {
                            district_uid: district_uid.clone(),
                            building_index,
                        },
                        attr: "shops".into(),
                        what: "ShopPlaceholder".into(),
                    });
                    commands.spawn(HexToInvalidatePostLoadMarker(hex_entity.clone()));
                },
                &dial_assets,
                "Generate a Shop",
            );
            let hex_entity = trigger.hex_entity.clone();
            let district_uid = trigger.district_uid.clone();
            c.spawn_empty().spawn_menu_item(
                1,
                MAX_ITEMS_IN_DIAL,
                DialIcon::Inn,
                move |_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(FreezeScreenSnapshot);
                    commands.trigger(AppendSandboxEntity {
                        target: AppendSubject::SettlementDistrict {
                            district_uid: district_uid.clone(),
                            building_index,
                        },
                        attr: "Tavern".into(),
                        what: "DistrictTavern".into(),
                    });
                    commands.spawn(HexToInvalidatePostLoadMarker(hex_entity.clone()));
                },
                &dial_assets,
                "Generate an Inn",
            );
            let hex_entity = trigger.hex_entity.clone();
            if let Some(uid) = trigger.building_uid.clone() {
                c.spawn_empty().spawn_menu_item(
                    5,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Trash,
                    move |_: On<Pointer<Click>>, mut commands: Commands| {
                        commands.trigger(FreezeScreenSnapshot);
                        let uid = commands.trigger(RemoveSandboxEntity { uid: uid.clone() });
                        // commands.spawn(HexToInvalidatePostLoadMarker(hex_entity.clone()));
                    },
                    &dial_assets,
                    "Clear Building",
                );
            }
        });
    }
}

pub trait SettlementDialProvider {
    fn settlement_dial_provider(&mut self, e: SpawnSettlementDial) -> &mut Self;
}

impl SettlementDialProvider for EntityCommands<'_> {
    fn settlement_dial_provider(&mut self, e: SpawnSettlementDial) -> &mut Self {
        self.observe(
            move |trigger: On<Pointer<Click>>,
                  mut commands: Commands,
                  q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
                  q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>| {
                let mut e = e.clone();
                if trigger.event().button == PointerButton::Secondary {
                    if let Some(pos) = pointer_world_position(q_window, q_camera) {
                        debug!("Spawning settlement dial");
                        e.pos = pos;
                        commands.trigger(e.clone());
                    }
                }
            },
        );
        self
    }
}
