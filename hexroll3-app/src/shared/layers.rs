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

pub const HEIGHT_OF_HEX_MAP: f32 = 0.0;
pub const HEIGHT_OF_TILES_BACKGROUND_HEX: f32 = 0.0;
pub const HEIGHT_OF_TOP_MOST_LAYERED_TILE: f32 = 50.0;
pub const HEIGHT_OF_FEATURE_ON_LAYERED_TILE: f32 = 2.0;
pub const HEIGHT_OFFSET_OF_BATTLEMAP_CONTENT: f32 = -10.0;
pub const HEIGHT_OF_BATTLEMAP_ON_FEATURE: f32 = 0.0;
pub const HEIGHT_OF_AREA_NUMBERS_ON_BATTLEMAP: f32 = 0.6;

pub const HEIGHT_OF_REALM_LABELS: f32 = 120.0;
pub const HEIGHT_OF_REGION_LABELS: f32 = 121.0;
pub const HEIGHT_OF_SELECTION_HEX: f32 = 300.0;
pub const HEIGHT_OF_MAP_PINS: f32 = 301.0;

pub const HEIGHT_OF_TOKENS: f32 = HEIGHT_OF_FEATURE_ON_LAYERED_TILE - 4.5;

use avian3d::prelude::*;

#[derive(PhysicsLayer, Default)]
pub enum HexrollPhysicsLayer {
    #[default]
    Battlemaps,
    Dice,
    Walls,
    Tokens,
}

pub const RENDER_LAYER_MAP_LOD_LOW: bevy::camera::visibility::Layer = 0;
pub const RENDER_LAYER_MAP_LOD_MEDIUM: bevy::camera::visibility::Layer = 1;
pub const RENDER_LAYER_MAP_LOD_HIGH: bevy::camera::visibility::Layer = 2;

pub const RENDER_LAYER_MAP_COORDS_HIRES: bevy::camera::visibility::Layer = 13;
pub const RENDER_LAYER_MAP_COORDS_MEDRES: bevy::camera::visibility::Layer = 14;
pub const RENDER_LAYER_MAP_COORDS_LOWRES: bevy::camera::visibility::Layer = 15;

pub const RENDER_LAYER_DICE_SHADOW: bevy::camera::visibility::Layer = 3;
pub const RENDER_LAYER_DICE: bevy::camera::visibility::Layer = 4;
pub const RENDER_LAYER_TRANSLUCENT_DICE: bevy::camera::visibility::Layer = 0;

pub const RENDER_LAYER_CONTENT_OFFSCREEN: bevy::camera::visibility::Layer = 5;
pub const RENDER_LAYER_CONTENT_ONSCREEN: bevy::camera::visibility::Layer = 12;
