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

use bevy::app::{App, Plugin};

mod control;
mod initiative;
mod library;
mod spawn;
mod sync;
mod token_dial;
mod tokens;

pub struct TokensPlugin;
impl Plugin for TokensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((tokens::Tokens, token_dial::TokenDial));
    }
}

pub use control::BattlemapsSnapping;
pub use control::update_token_material;
pub use sync::TokenMessage;
pub use sync::TokenUpdateMessage;
pub use tokens::Token;
// This event is the main way to spawn a new token on the map
// using the tokens library window.
#[derive(Event)]
pub struct SpawnTokenFromLibrary {
    pub pos: Vec3,
}

// Any selected token will have this component
#[derive(Component)]
pub struct SelectedToken;

// The SpawnToken even can be used when re-spawning serialized tokens
pub struct SpawnToken {
    pub token: Token,
    pub transform: Transform,
}

// Use the DespawnToken to despawn tokens. Do not directly call `.despawn()`.
#[derive(Event)]
pub struct DespawnToken {
    pub token_entity: Entity,
}

#[derive(Event)]
pub struct DespawnVisibleTokens;

#[derive(Event)]
pub struct TeleportSelectedTokens {
    pub teleport_to: Vec3,
}

#[derive(Event)]
pub struct DuplicateLastSpawnedToken {
    pub duplicate_pos: Vec3,
}

#[derive(Component)]
pub struct MainTokenEntity(pub Entity);

#[derive(Component)]
pub struct TokenMeshEntity(pub Entity);

pub const TOKEN_VISIBILITY_ZOOM_THRESHOLD: f32 = 0.7;
pub const TOKEN_VISIBILITY_FRUSTRUM_BUFFER: f32 = 10.0;
pub const TOKEN_MAP_PINS_SCALE: f32 = 1.0;
pub const TOKEN_MAP_PINS_CLOSEST_VISIBILITY_ZOOM: f32 = 0.1;
pub const TOKEN_MAP_PINS_OPAQUE_VISIBILITY_ZOOM: f32 = 0.3;

pub const TOKEN_MOVEMENT_ZOOM_LIMIT: f64 = 0.02;
pub const TOKEN_SIZE_SCALING_ZOOM_LIMIT: f64 = 0.02;
pub const TOKEN_TORCH_SCALING_ZOOM_LIMIT: f64 = 0.02;
pub const TOKEN_ROTATION_ZOOM_LIMIT: f32 = 0.07;

pub const TOKEN_DESELECTION_ZOOM_THRESHOLD: f32 = 40.0;
