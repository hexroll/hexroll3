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
    asset::RenderAssetUsages,
    platform::collections::HashSet,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    window::PrimaryWindow,
};
use hexx::Hex;

use crate::{
    hexmap::elements::HexMapData, shared::widgets::cursor::PointerExclusivityIsPreferred,
};

use super::{vtt::VttData, widgets::cursor::CursorController};

#[derive(Event)]
pub struct FreezeScreenSnapshot;

#[derive(Event)]
pub struct ReleaseScreenSnapshot;

#[derive(Component)]
pub struct SnapshotPendingRefresh(pub Hex);

const SNAPSHOT_WATCHDOG_TIMEOUT_SECS: f32 = 5.0;
const SNAPSHOT_FADE_DURATION_SECS: f32 = 0.3;

#[derive(Resource, Default)]
struct SnapshotWatchdogTimer(f32);

#[derive(Component)]
struct SnapshotFadingOut(f32);

pub struct SnapshotPlugin;

impl Plugin for SnapshotPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<SnapshotState>()
            .init_state::<RemoteRefreshState>()
            .add_observer(trigger_snapshot)
            .add_observer(dismiss_snapshot)
            .add_systems(
                Update,
                snapshot_watchdog.run_if(in_state(SnapshotState::Showing)),
            )
            .add_systems(Update, snapshot_fade_out);
    }
}

#[derive(States, Default, PartialEq, Eq, Hash, Clone, Debug)]
pub enum RemoteRefreshState {
    #[default]
    Idle,
    Initiated,
    InProgress,
}

#[derive(States, Default, PartialEq, Eq, Hash, Clone, Debug)]
pub enum SnapshotState {
    #[default]
    Idle,
    Capturing,
    Showing,
}

#[derive(Component)]
struct SnapshotOverlay;

fn trigger_snapshot(
    _: On<FreezeScreenSnapshot>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<SnapshotState>>,
    current_state: Res<State<SnapshotState>>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut cursor_controller: ResMut<CursorController>,
) {
    if *current_state != SnapshotState::Idle {
        return;
    }
    next_state.set(SnapshotState::Capturing);
    cursor_controller.loading(&mut commands, *window);

    commands
        .spawn(Screenshot::primary_window())
        .observe(on_screenshot_captured);
}

fn on_screenshot_captured(
    trigger: On<ScreenshotCaptured>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    current_state: Res<State<SnapshotState>>,
    mut next_state: ResMut<NextState<SnapshotState>>,
    pending: Query<(Entity, &SnapshotPendingRefresh)>,
    mut map_data: ResMut<HexMapData>,
    mut vtt_data: ResMut<VttData>,
) {
    if *current_state == SnapshotState::Idle {
        return;
    }
    let mut img = trigger.event().image.clone();

    img.asset_usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;

    let handle = images.add(img);

    commands.spawn((
        PointerExclusivityIsPreferred,
        Name::new("SnapshotOverlay"),
        SnapshotOverlay,
        ImageNode {
            image: handle,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ZIndex(i32::MAX),
        Pickable {
            should_block_lower: false,
            ..default()
        },
    ));

    next_state.set(SnapshotState::Showing);
    commands.insert_resource(SnapshotWatchdogTimer::default());
    debug!("Screen freeze set!");
    let mut added: HashSet<Hex> = HashSet::new();
    for (e, p) in pending {
        if !added.contains(&p.0) {
            map_data.force_refresh.push(p.0);
            added.insert(p.0);
        }
        commands.entity(e).try_despawn();
    }
    vtt_data.invalidate_map = true;
}

fn snapshot_watchdog(
    mut timer: ResMut<SnapshotWatchdogTimer>,
    time: Res<Time>,
    mut commands: Commands,
) {
    timer.0 += time.delta_secs();
    if timer.0 >= SNAPSHOT_WATCHDOG_TIMEOUT_SECS {
        warn!("Snapshot overlay shown for too long - force despawning");
        commands.remove_resource::<SnapshotWatchdogTimer>();
        commands.trigger(ReleaseScreenSnapshot);
    }
}

fn dismiss_snapshot(
    _: On<ReleaseScreenSnapshot>,
    mut commands: Commands,
    overlay: Query<Entity, (With<SnapshotOverlay>, Without<SnapshotFadingOut>)>,
    mut next_state: ResMut<NextState<SnapshotState>>,
    refresh_state: Res<State<RemoteRefreshState>>,
    mut next_refresh_state: ResMut<NextState<RemoteRefreshState>>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut cursor_controller: ResMut<CursorController>,
) {
    if *refresh_state == RemoteRefreshState::Initiated {
        return;
    }
    for entity in overlay.iter() {
        commands.entity(entity).insert(SnapshotFadingOut(0.0));
    }
    next_state.set(SnapshotState::Idle);
    next_refresh_state.set(RemoteRefreshState::Idle);
    commands.insert_resource(ClearColor(Color::srgba(0.0, 0.0, 0.0, 0.0)));
    cursor_controller.done(&mut commands, *window);
}

fn snapshot_fade_out(
    mut commands: Commands,
    mut overlays: Query<(Entity, &mut ImageNode, &mut SnapshotFadingOut)>,
    time: Res<Time>,
) {
    for (entity, mut image_node, mut fade) in overlays.iter_mut() {
        fade.0 += time.delta_secs();
        let alpha = (1.0 - fade.0 / SNAPSHOT_FADE_DURATION_SECS).max(0.0);
        image_node.color = Color::WHITE.with_alpha(alpha);
        if alpha <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
