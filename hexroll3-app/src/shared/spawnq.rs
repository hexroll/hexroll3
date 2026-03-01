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

// Hexroll-specific spawning queue
//
// This queue is currently used in two places:
// 1) When spawning labels
// 2) When spawning dungeon constructs
//
// There are two queues for two separate use-cases:
// a) Major queue, for spawning main entities
// b) Minor queue, for spawning child entities
//
// Each queue type has a different batch size.
//
// Queuing hexes is done differently inside the hexmap modules.
use std::collections::VecDeque;

use bevy::prelude::*;

pub struct SpawnQueuePlugin;
impl Plugin for SpawnQueuePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpawnQueue::default())
            .add_systems(Update, cleanup_at_aisle_six)
            .add_systems(PostUpdate, spawn_from_queue);
    }
}

const MAJOR_QUEUE_BATCH_SIZE: usize = 5;
const MINOR_QUEUE_BATCH_SIZE: usize = 30;

#[derive(Resource, Default)]
pub struct SpawnQueue {
    pub major_queue: VecDeque<Box<dyn FnOnce(&mut Commands) + Send + Sync + 'static>>,
    pub minor_queue: VecDeque<(
        Entity,
        Box<dyn FnOnce(Entity, &mut Commands) + Send + Sync + 'static>,
    )>,
}

impl SpawnQueue {
    pub fn queue<F>(&mut self, f: F)
    where
        F: Send + Sync + FnOnce(&mut Commands) + 'static,
    {
        self.major_queue.push_back(Box::new(f));
    }

    pub fn queue_children<F>(&mut self, e: Entity, f: F)
    where
        F: Send + Sync + FnOnce(Entity, &mut Commands) + 'static,
    {
        self.minor_queue.push_back((e, Box::new(f)));
    }
}

fn spawn_from_queue(mut commands: Commands, mut q: ResMut<SpawnQueue>) {
    let major_queue_len = q.major_queue.len();
    q.major_queue
        .drain(..std::cmp::min(MAJOR_QUEUE_BATCH_SIZE, major_queue_len))
        .for_each(|item| item(&mut commands));
    let minor_queue_len = q.minor_queue.len();
    q.minor_queue
        .drain(..std::cmp::min(MINOR_QUEUE_BATCH_SIZE, minor_queue_len))
        .for_each(|(e, item)| {
            item(e, &mut commands);
        });
}

#[derive(Component)]
pub struct MustBeChildOf;

fn cleanup_at_aisle_six(
    mut commands: Commands,
    q: Query<Entity, (With<MustBeChildOf>, Without<ChildOf>)>,
) {
    for e in q.iter() {
        commands.entity(e).try_despawn();
    }
}
