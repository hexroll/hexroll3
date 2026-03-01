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

// Dungeon doors module
use crate::{
    hexmap::{DoorState, MapMessage},
    shared::vtt::VttData,
    vtt::sync::SyncMapForPeers,
};
use avian3d::prelude::ColliderDisabled;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

#[derive(Component)]
pub struct DoorData {
    pub door_uid: String,
}

#[derive(Event)]
pub struct ToggleDungoenDoor {
    pub door_uid: String,
    pub is_open: bool,
}

pub fn create_door_frame_mesh() -> Mesh {
    let half_width = 0.25;
    let half_height = 0.5;
    let half_depth = 0.20;

    let positions = vec![
        [-half_width, -half_height, -half_depth],
        [half_width, -half_height, -half_depth],
        [half_width, half_height, -half_depth],
        [-half_width, half_height, -half_depth],
        [-half_width, -half_height, half_depth],
        [half_width, -half_height, half_depth],
        [half_width, half_height, half_depth],
        [-half_width, half_height, half_depth],
        [1.0 - half_width, -half_height, -half_depth],
        [1.0 + half_width, -half_height, -half_depth],
        [1.0 + half_width, half_height, -half_depth],
        [1.0 - half_width, half_height, -half_depth],
        [1.0 - half_width, -half_height, half_depth],
        [1.0 + half_width, -half_height, half_depth],
        [1.0 + half_width, half_height, half_depth],
        [1.0 - half_width, half_height, half_depth],
    ];

    let indices = vec![
        0, 1, 2, 0, 2, 3, // Front
        4, 5, 6, 4, 6, 7, // Back
        0, 3, 7, 0, 7, 4, // Left
        1, 5, 6, 1, 6, 2, // Right
        2, 3, 6, 6, 3, 7, // Top
        0, 4, 5, 0, 5, 1, // Bottom
        8, 9, 10, 8, 10, 11, // Front
        12, 13, 14, 12, 14, 15, // Back
        8, 11, 15, 8, 15, 12, // Left
        9, 13, 14, 9, 14, 10, // Right
        10, 11, 14, 14, 11, 15, //Top
        8, 12, 13, 8, 13, 9, // Bottom
    ];

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
    .with_computed_smooth_normals()
}

pub fn update_material_on(
    new_material: Handle<StandardMaterial>,
) -> impl Fn(On<Pointer<Over>>, Query<&mut MeshMaterial3d<StandardMaterial>>, Res<VttData>) {
    move |trigger, mut query, vtt_data| {
        if vtt_data.is_player() {
            return;
        }
        if let Ok(mut material) = query.get_mut(trigger.entity) {
            material.0 = new_material.clone();
        }
    }
}

pub fn update_material_out(
    new_material: Handle<StandardMaterial>,
) -> impl Fn(On<Pointer<Out>>, Query<&mut MeshMaterial3d<StandardMaterial>>, Res<VttData>) {
    move |trigger, mut query, vtt_data| {
        if vtt_data.is_player() {
            return;
        }
        if let Ok(mut material) = query.get_mut(trigger.entity) {
            material.0 = new_material.clone();
        }
    }
}

pub fn close_door(
    door_uid: String,
) -> impl Fn(On<Pointer<Click>>, Commands, Query<(Entity, &DoorData)>, ResMut<VttData>) {
    move |_trigger, mut commands, doors, mut vtt_data| {
        if vtt_data.is_player() {
            return;
        }
        for (e, d) in doors.iter() {
            if door_uid == d.door_uid {
                commands
                    .entity(e)
                    .insert(Visibility::Inherited)
                    .remove::<ColliderDisabled>();
                vtt_data.open_doors.remove(&door_uid);
                commands.trigger(SyncMapForPeers(MapMessage::DoorStateChange(DoorState {
                    door_uid: door_uid.clone(),
                    is_open: false,
                })));
            }
        }
    }
}

pub fn open_door(
    door_uid: String,
) -> impl Fn(On<Pointer<Click>>, Commands, Query<&ChildOf>, Query<&mut Visibility>, ResMut<VttData>)
{
    move |trigger, mut commands, mut query, mut visibilities, mut vtt_data| {
        if vtt_data.is_player() {
            return;
        }
        if let Ok(_t) = query.get_mut(trigger.entity) {
            commands.entity(trigger.entity).insert(ColliderDisabled);
            let mut vis = visibilities.get_mut(trigger.entity).unwrap();
            *vis = Visibility::Hidden;
            vtt_data.open_doors.insert(door_uid.clone());
            commands.trigger(SyncMapForPeers(MapMessage::DoorStateChange(DoorState {
                door_uid: door_uid.clone(),
                is_open: true,
            })));
        }
    }
}
