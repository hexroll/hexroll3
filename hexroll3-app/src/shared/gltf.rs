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

// GLTF utilities:
//
// * Simple async gltf processer to run custom systems when a gltf asset is
//   done loading.
//   TODO: Check if Bevy already has something builtin for this.
//
// * Animations struct
//
use bevy::prelude::*;

pub struct GltfProcessorPlugin;

impl Plugin for GltfProcessorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, execute_gltf_processor_jobs);
    }
}
pub struct GltfProcessorData<'a> {
    pub gltf: &'a Gltf,
    pub mats: &'a mut Assets<StandardMaterial>,
    pub clips: &'a mut Assets<AnimationClip>,
    pub asset_server: &'a Res<'a, AssetServer>,
}

#[derive(Component)]
pub struct GltfProcessorJob {
    pub gltf: Handle<Gltf>,
    pub callback: bevy::ecs::system::SystemId,
}

fn execute_gltf_processor_jobs(
    jobs: Query<(Entity, &GltfProcessorJob)>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for (e, j) in jobs.iter() {
        if asset_server.is_loaded_with_dependencies(j.gltf.id()) {
            commands.run_system(j.callback);
            commands.entity(e).despawn();
        }
    }
}

pub struct Animations {
    pub animations: Vec<AnimationNodeIndex>,
    pub graph: Handle<AnimationGraph>,
}
