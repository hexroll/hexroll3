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

/// The fader module is responsible for fading in and out different parts of the hex map
/// according to the zoom level (the scale from the camera projection).
use bevy::prelude::*;

use crate::battlemaps::BattlemapMaterial;

use super::{
    elements::{HexMapResources, MainCamera},
    grid::HexMaterial,
    tiles::{
        BackgroundMaterial, HexMapTileMaterials, RiverMaterial, TileMaterial, TrailMaterial,
    },
};

pub fn hex_map_zoom_fader(
    mut river_materials: ResMut<Assets<RiverMaterial>>,
    mut trail_materials: ResMut<Assets<TrailMaterial>>,
    mut hex_grid_materials: ResMut<Assets<HexMaterial>>,
    mut tile_art_materials: ResMut<Assets<TileMaterial>>,
    mut tile_bg_materials: ResMut<Assets<BackgroundMaterial>>,
    mut battlemap_materials: ResMut<Assets<BattlemapMaterial>>,
    camera_projection: Query<&Projection, With<MainCamera>>,
    tiles: ResMut<HexMapTileMaterials>,
    assets: ResMut<HexMapResources>,
    selector: Single<Entity, With<crate::hexmap::elements::SelectionEntity>>,
    mut commands: Commands,
) {
    if let Ok(proj) = camera_projection.single() {
        if let Projection::Orthographic(proj) = proj {
            if proj.scale < 1.0 {
                commands.entity(*selector).insert(Visibility::Hidden);
            } else {
                commands.entity(*selector).insert(Visibility::Inherited);
            }

            let value = if proj.scale >= 0.25 {
                0.0
            } else if proj.scale <= 0.025 {
                0.5
            } else {
                (0.25 - proj.scale) / (0.25 - 0.025) * 0.5
            };
            battlemap_materials
                .get_mut(assets.battlemap_material.id())
                .unwrap()
                .controls
                .blend = value;

            let scale = proj.scale;

            // NOTE: river and trail lines require special care to ensure fine anti-aliased
            // lines until their vanishing zoom point
            // let main_alpha_fader = 1.0 - ((scale - 0.208).clamp(0.0, 1.0) / 0.417).clamp(0.0, 1.0);
            let main_alpha_fader =
                1.0 - ((scale - 0.208).clamp(0.0, 1.0) / 0.337).clamp(0.0, 1.0);
            {
                let river_flow_material = river_materials
                    .get_mut(&assets.river_tile_materials.river_tile_material)
                    .unwrap();
                river_flow_material.time += 0.01;
                river_flow_material.res = Vec4::splat(1.0 / scale * 1.388);
                river_flow_material.color.set_alpha(1.0 - main_alpha_fader);
            }
            {
                let river_flow_material2 = river_materials
                    .get_mut(&assets.river_tile_materials.river_battlemap_material)
                    .unwrap();
                river_flow_material2.time -= 0.001;
                river_flow_material2.res = Vec4::splat(1.0);
            }
            for (_, trail_material) in trail_materials.iter_mut() {
                trail_material.res = Vec4::splat(scale * 0.05);
                trail_material.color.set_alpha(0.8 - main_alpha_fader * 0.8);
            }

            // NOTE: The hex grid also requires special care to ensure fine anti-aliased
            // lines until its vanishing zoom point
            let hex_grid_alpha_fader = ((scale - 0.7) / (14.0 - 0.7)).clamp(0.0, 1.0);
            let hex_grid_line_width =
                ((scale - 0.7) / (7.0 - 0.7)).clamp(0.0, 1.0) * (0.04 - 0.01) + 0.01;
            let hex_grid_line_smoothing =
                ((scale - 0.7) / (7.0 - 0.7)).clamp(0.0, 1.0) * (0.009 - 0.0001) + 0.0001;
            for hex_material in hex_grid_materials.iter_mut() {
                hex_material.1.color =
                    hex_material.1.color.with_alpha(1.0 - hex_grid_alpha_fader);
                hex_material.1.res.z = hex_grid_line_width;
                hex_material.1.res.w = hex_grid_line_smoothing;
            }

            for (_, m) in tiles.terrain_rim_materials.iter() {
                tile_art_materials.get_mut(m).unwrap().mixer.y = 1.0 - main_alpha_fader;
            }
            for (_, m) in tiles.terrain_materials.iter() {
                tile_art_materials.get_mut(m).unwrap().mixer.y = 1.0 - main_alpha_fader;
            }
            // NOTE: We could just traverse the material resources to save cycles
            // but this is way we can trace bugs related to dangling material handles.
            for (_, m) in tiles.terrain_feature_materials.iter() {
                for (_, m) in m.iter() {
                    tile_art_materials.get_mut(m).unwrap().mixer.y = 1.0 - main_alpha_fader;
                }
            }
            // Fade out the base_color of tiles with dungeon so that we can show
            // the actual dungeon tile lying under it. For dungeon tiles, the layer_color
            // is set to fully transparent so that it will not block anything.
            for (_, m) in tiles.terrain_background_materials.iter() {
                tile_bg_materials
                    .get_mut(&m.overlayer)
                    .unwrap()
                    .base_color
                    .alpha = 1.0 - main_alpha_fader;
            }
        }
    }
}
