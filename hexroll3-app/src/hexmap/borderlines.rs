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

use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::{
    hexmap::elements::{HexMapData, HexMapResources, RealmBorderline},
    shared::{
        layers::{HEIGHT_OF_REALM_BORDERLINES, RENDER_LAYER_MAP_LOD_LOW},
        vtt::VttData,
    },
};

pub fn spawn_realm_borderlines(
    mut commands: Commands,
    mut map_data: ResMut<HexMapData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<HexMapResources>,
    vtt_data: Res<VttData>,
) {
    if map_data.realm_borderlines.is_empty() || vtt_data.is_player() {
        return;
    }
    let batch_size = map_data.realm_borderlines.len().min(5);
    let batch: Vec<_> = map_data
        .realm_borderlines
        .drain(..batch_size)
        .map(|(mesh, material)| {
            (
                RealmBorderline,
                Name::new("RealmBorderline"),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(material)),
                RenderLayers::layer(RENDER_LAYER_MAP_LOD_LOW),
                Transform::from_xyz(0.0, HEIGHT_OF_REALM_BORDERLINES, 0.0),
                ChildOf(assets.labels_parent),
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
            )
        })
        .collect();
    commands.spawn_batch(batch);
}
