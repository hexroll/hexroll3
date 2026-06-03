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
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};

#[derive(Event)]
pub struct FreezeScreenSnapshot;

#[derive(Event)]
pub struct ReleaseScreenSnapshot;

const SNAPSHOT_WATCHDOG_TIMEOUT_SECS: f32 = 5.0;

#[derive(Resource, Default)]
struct SnapshotWatchdogTimer(f32);

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
            );
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
enum SnapshotState {
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
) {
    // commands.insert_resource(ClearColor(Color::BLACK));
    if *current_state != SnapshotState::Idle {
        return;
    }
    next_state.set(SnapshotState::Capturing);

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
) {
    if *current_state == SnapshotState::Idle {
        return;
    }
    let mut img = trigger.event().image.clone();

    img.asset_usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;

    let handle = images.add(img);

    commands.spawn((
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
}

fn snapshot_watchdog(
    mut timer: ResMut<SnapshotWatchdogTimer>,
    time: Res<Time>,
    mut commands: Commands,
) {
    timer.0 += time.delta_secs();
    if timer.0 >= SNAPSHOT_WATCHDOG_TIMEOUT_SECS {
        warn!("SnapshotOverlay watchdog: overlay shown for too long, force-releasing");
        commands.remove_resource::<SnapshotWatchdogTimer>();
        commands.trigger(ReleaseScreenSnapshot);
    }
}

fn dismiss_snapshot(
    _: On<ReleaseScreenSnapshot>,
    mut commands: Commands,
    overlay: Query<Entity, With<SnapshotOverlay>>,
    mut next_state: ResMut<NextState<SnapshotState>>,
    refresh_state: Res<State<RemoteRefreshState>>,
    mut next_refresh_state: ResMut<NextState<RemoteRefreshState>>,
) {
    if *refresh_state == RemoteRefreshState::Initiated {
        return;
    }
    debug!("Screen freeze released!");
    for entity in overlay.iter() {
        commands.entity(entity).despawn();
    }
    next_state.set(SnapshotState::Idle);
    next_refresh_state.set(RemoteRefreshState::Idle);
    commands.insert_resource(ClearColor(Color::srgba(0.0, 0.0, 0.0, 0.0)));
}
