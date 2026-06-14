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

use bevy::{
    prelude::*,
    window::CursorIcon,
    window::{PrimaryWindow, SystemCursorIcon},
};
use bevy_vector_shapes::prelude::*;

use crate::{
    hexmap::{
        HexState, MapMessage, SandboxLock,
        elements::{HexMapData, HexMapState, MainCamera},
    },
    shared::{
        layers::HEIGHT_OF_TOKENS,
        snapshot::{FreezeScreenSnapshot, SnapshotPendingRefresh},
        vtt::{HexRevealState, VttData},
        widgets::{
            buttons::ToggleResourceWrapper,
            cursor::{PointerExclusivityIsPreferred, pointer_world_position},
            dial::{
                DialAssets, DialButton, DialButtonState, DialMenuCommands, DialMenuOptions,
                MenuItemSpawner, placeholder_click_handler,
            },
            modal::DiscreteAppState,
        },
    },
    tokens::{DuplicateLastSpawnedToken, SpawnTokenFromLibrary, TeleportSelectedTokens},
    vtt::sync::SyncMapForPeers,
};

use crate::hexmap::elements::HexMapToolState;

use super::{BattlemapUserDrawing, BattlemapUserDrawingInProgress};

pub struct BattlemapDialPlugin;
impl Plugin for BattlemapDialPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, battlemap_selection_tool)
            .add_observer(on_spawn_battlemap_dial);
    }
}

#[derive(Hash, Clone, PartialEq, Eq, Copy)]
pub enum DialIcon {
    Spawn,
    Draw,
    Select,
    Teleport,
    Vfx,
    Respawn,
    LayerOverland,
    Layers,
    Layer1,
    Layer2,
    Layer3,
    Layer4,
    Layer5,
    Layer6,
    Layer7,
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
            DialIcon::Spawn,
            "icons/icon-skull-256.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Draw,
            "icons/icon-pencil-256.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Select,
            "icons/icon-circle-select.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Teleport,
            "icons/icon-bullseye-256.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Vfx,
            "icons/icon-vfx-256.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Respawn,
            "icons/icon-skulls-256.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Layers,
            "icons/icon-layers.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::LayerOverland,
            "icons/icon-overland.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Layer1,
            "icons/icon-layer1.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Layer2,
            "icons/icon-layer2.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Layer3,
            "icons/icon-layer3.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Layer4,
            "icons/icon-layer4.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Layer5,
            "icons/icon-layer5.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Layer6,
            "icons/icon-layer6.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            DialIcon::Layer7,
            "icons/icon-layer7.ktx2",
            &mut materials,
            &asset_server,
        );
    commands.insert_resource(dial_assets);
}

#[derive(Event)]
pub struct SpawnBattlemapDial {
    pub pos: Vec3,
}

fn on_spawn_battlemap_dial(
    trigger: On<SpawnBattlemapDial>,
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

    let calc_scale = |v: f32| -> f32 {
        if v > 0.10 {
            v * 0.10 / (0.10 + (v - 0.10).ln_1p()) * 0.75
        } else {
            v * 0.75
        }
    };
    let is_visible = |v: f32| v < 0.2;
    let pos = trigger.event().pos;

    if let Some(menu_entity) = dial_menu_commands.spawn_menu(DialMenuOptions {
        pos: pos.xz(),
        calc_scale,
        is_visible,
    }) {
        commands.entity(menu_entity).with_children(|c| {
            const MAX_ITEMS_IN_DIAL: i32 = 7;
            c.spawn_empty().spawn_menu_item(
                0,
                MAX_ITEMS_IN_DIAL,
                DialIcon::Spawn,
                move |_: On<Pointer<Click>>,
                      mut commands: Commands,
                      mut next_state: ResMut<NextState<HexMapToolState>>| {
                    commands.trigger(SpawnTokenFromLibrary {
                        pos: Vec3::new(pos.x, HEIGHT_OF_TOKENS, pos.z),
                    });
                    next_state.set(HexMapToolState::Selection);
                },
                &dial_assets,
                "Spawn new token",
            );
            c.spawn_empty().spawn_menu_item(
                1,
                MAX_ITEMS_IN_DIAL,
                DialIcon::Respawn,
                move |_: On<Pointer<Click>>,
                      mut commands: Commands,
                      mut next_state: ResMut<NextState<HexMapToolState>>| {
                    commands.trigger(DuplicateLastSpawnedToken { duplicate_pos: pos });
                    next_state.set(HexMapToolState::Selection);
                },
                &dial_assets,
                "Respawn last spawned token",
            );
            c.spawn_empty().spawn_menu_item(
                2,
                MAX_ITEMS_IN_DIAL,
                DialIcon::Select,
                move |_: On<Pointer<Click>>,
                      mut commands: Commands,
                      mut next_state: ResMut<NextState<HexMapToolState>>| {
                    commands.spawn((BattlemapSelection::from_start_pos(Vec3::new(
                        pos.x,
                        HEIGHT_OF_TOKENS,
                        pos.z,
                    )),));

                    next_state.set(HexMapToolState::Selection);
                },
                &dial_assets,
                "Select tokens by region",
            );
            c.spawn_empty().spawn_menu_item(
                3,
                MAX_ITEMS_IN_DIAL,
                DialIcon::Teleport,
                move |_: On<Pointer<Click>>,
                      mut commands: Commands,
                      mut next_state: ResMut<NextState<HexMapToolState>>| {
                    commands.trigger(TeleportSelectedTokens { teleport_to: pos });
                    next_state.set(HexMapToolState::Selection);
                },
                &dial_assets,
                "Teleport selected tokens here",
            );
            c.spawn_empty()
                .spawn_menu_item(
                    4,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Vfx,
                    placeholder_click_handler(),
                    &dial_assets,
                    "Spawn a visual effect",
                )
                .insert(DialButtonState::Disabled);
            c.spawn_empty().spawn_menu_item(
                5,
                MAX_ITEMS_IN_DIAL,
                DialIcon::Draw,
                move |_: On<Pointer<Click>>,
                      mut commands: Commands,
                      mut next_state: ResMut<NextState<HexMapToolState>>| {
                    commands.spawn((
                        BattlemapUserDrawing::from_start_pos(Vec3::new(
                            pos.x,
                            HEIGHT_OF_TOKENS,
                            pos.z,
                        )),
                        BattlemapUserDrawingInProgress {},
                    ));
                    next_state.set(HexMapToolState::Selection);
                },
                &dial_assets,
                "Dry-erase marker",
            );
            c.spawn_empty().spawn_menu_item(
                6,
                MAX_ITEMS_IN_DIAL,
                DialIcon::Layers,
                dial_menu_layers_menu(),
                &dial_assets,
                "Change Layers",
            );
        });
    }
}

impl BattlemapSelection {
    pub fn from_start_pos(start: Vec3) -> Self {
        BattlemapSelection {
            from: start,
            radius: 0.0,
        }
    }
}

fn battlemap_selection_tool(
    mut commands: Commands,
    mut painter: ShapePainter,
    mut gizmo: Query<(Entity, &mut BattlemapSelection), Without<BattlemapSelectionFinalizing>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut next_tool_state: ResMut<NextState<HexMapToolState>>,
    window: Single<Entity, With<PrimaryWindow>>,
) {
    if let Some((e, mut gizmo)) = gizmo.iter_mut().next() {
        commands
            .entity(*window)
            .insert(CursorIcon::System(SystemCursorIcon::Crosshair));
        commands.entity(e).try_insert(PointerExclusivityIsPreferred);
        next_tool_state.set(HexMapToolState::Draw);
        if let Some(pos) = pointer_world_position(q_window, q_camera) {
            painter.set_3d();
            painter.color = Color::BLACK;
            painter.thickness_type = ThicknessType::Pixels;
            painter.origin = Some(Vec3::Y * 10.0);
            painter.hollow = true;
            painter.rotate_x(-std::f32::consts::PI / 2.0);
            painter.set_translation(gizmo.from);
            let radius = gizmo.from.distance(pos.with_y(HEIGHT_OF_TOKENS));
            painter.circle(radius);
            gizmo.radius = radius;
        }
        if mouse.just_pressed(MouseButton::Left) {
            commands.entity(e).insert(BattlemapSelectionFinalizing);
            next_tool_state.set(HexMapToolState::Selection);
            commands
                .entity(e)
                .try_remove::<PointerExclusivityIsPreferred>();
            commands
                .entity(*window)
                .insert(CursorIcon::System(SystemCursorIcon::Default));
        }
    }
}

pub trait BattlemapDialProvider {
    fn battlemap_dial_provider(&mut self, locked_mode_only: bool) -> &mut Self;
}

impl BattlemapDialProvider for EntityCommands<'_> {
    fn battlemap_dial_provider(&mut self, locked_mode_only: bool) -> &mut Self {
        self.observe(
            move |trigger: On<Pointer<Click>>,
                  mut commands: Commands,
                  sandbox_lock: Res<ToggleResourceWrapper<SandboxLock>>,
                  q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
                  q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>| {
                if locked_mode_only && sandbox_lock.value.off() {
                    return;
                }
                if trigger.event().button == PointerButton::Secondary {
                    if let Some(pos) = pointer_world_position(q_window, q_camera) {
                        commands.trigger(SpawnBattlemapDial { pos });
                    }
                }
            },
        );
        self
    }
}

#[derive(Component)]
pub struct BattlemapSelectionFinalizing;

#[derive(Component)]
pub struct BattlemapSelection {
    pub from: Vec3,
    pub radius: f32,
}

#[allow(clippy::type_complexity)]
pub fn dial_menu_layers_menu() -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Res<HexMapData>,
    Query<&ChildOf>,
    Query<Entity, With<DialButton>>,
    Res<DialAssets<DialIcon>>,
) {
    move |trigger, mut commands, map_data, parents, prev, dial_assets| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let entity_pointer = parents.get(trigger.entity).unwrap();
        let Some(selected) = map_data.selected else {
            return;
        };
        let num_of_layers = if let Some(hex_data) = map_data.hexes.get(&selected) {
            hex_data.num_of_layers
        } else {
            1
        };
        commands.entity(entity_pointer.parent()).with_children(|c| {
            const MAX_ITEMS_IN_DIAL: i32 = 7;
            if num_of_layers > 0 {
                c.spawn_empty().spawn_menu_item(
                    0,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::LayerOverland,
                    make_layer_handler(0),
                    &dial_assets,
                    "Reveal Overland Layer",
                );
            }
            if num_of_layers > 1 {
                c.spawn_empty().spawn_menu_item(
                    1,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Layer1,
                    make_layer_handler(1),
                    &dial_assets,
                    "Reveal Layer 1",
                );
            }
            // .make_conditional_and_lockable(&locked, true);
            if num_of_layers > 2 {
                c.spawn_empty().spawn_menu_item(
                    2,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Layer2,
                    make_layer_handler(2),
                    &dial_assets,
                    "Reveal Layer 2",
                );
            }
            if num_of_layers > 3 {
                c.spawn_empty().spawn_menu_item(
                    3,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Layer3,
                    make_layer_handler(3),
                    &dial_assets,
                    "Reveal Layer 3",
                );
            }
            if num_of_layers > 4 {
                c.spawn_empty().spawn_menu_item(
                    4,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Layer4,
                    make_layer_handler(4),
                    &dial_assets,
                    "Reveal Layer 4",
                );
            }
            if num_of_layers > 5 {
                c.spawn_empty().spawn_menu_item(
                    5,
                    MAX_ITEMS_IN_DIAL,
                    DialIcon::Layer5,
                    make_layer_handler(5),
                    &dial_assets,
                    "Reveal Layer 5",
                );
            }
        });
    }
}

fn make_layer_handler(
    layer: u32,
) -> impl Fn(On<Pointer<Click>>, Commands, Res<HexMapData>, ResMut<VttData>) {
    return move |_: On<Pointer<Click>>,
                 mut commands: Commands,
                 map_data: Res<HexMapData>,
                 mut vtt_data: ResMut<VttData>| {
        let Some(selected) = map_data.selected else {
            return;
        };
        if let Some(hex_data) = map_data.hexes.get(&selected) {
            if hex_data.num_of_layers == 1 {
                return;
            }
        }
        commands.trigger(FreezeScreenSnapshot);
        let new_state = if let Some(s) = vtt_data.revealed.get(&selected) {
            match s {
                HexRevealState::Unrevealed(_) => HexRevealState::Unrevealed(Some(layer)),
                HexRevealState::Partial(_) => HexRevealState::Partial(Some(layer)),
                HexRevealState::Full(_) => HexRevealState::Full(Some(layer)),
            }
        } else {
            HexRevealState::Unrevealed(Some(layer))
        };
        vtt_data.revealed.insert(selected, new_state);
        if let Some(player_state) = new_state.player_state() {
            commands.trigger(SyncMapForPeers(MapMessage::HexStateChange(HexState {
                coords: selected,
                is_ocean: false,
                state: Some(player_state),
            })));
        }
        commands.spawn(SnapshotPendingRefresh(selected));
    };
}
