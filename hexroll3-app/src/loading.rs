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
use hexroll3_app::{
    battlemaps::BattlemapEffects,
    dice::{DiceResources, DiceSets},
    hexmap::{HexMapTileMaterials, TileSetThemesMetadata},
    shared::LoadingState,
};

pub fn update_loading_state(
    asset_server: Res<AssetServer>,
    current_state: Res<State<LoadingState>>,
    mut next_state: ResMut<NextState<LoadingState>>,
    handle: ResMut<TileSetThemesMetadata>,
    tile_materials: Option<Res<HexMapTileMaterials>>,
    dice_sets: Res<Assets<DiceSets>>,
    dice_resources: Res<DiceResources>,
    battlemap_effects: Res<BattlemapEffects>,
) {
    match *current_state.get() {
        LoadingState::Unready => {
            let mut transition = true;
            transition &= asset_server.is_loaded(&handle.themes);
            transition &= asset_server.is_loaded(&battlemap_effects.vfx_library);
            if transition {
                next_state.set(LoadingState::Loading);
            }
        }
        LoadingState::Loading => {
            let mut transition = true;
            transition &= tile_materials.is_some();
            transition &= dice_sets.contains(&dice_resources.dice_sets);
            transition &= battlemap_effects.loading_completed;
            if transition {
                next_state.set(LoadingState::Ready);
            }
        }
        LoadingState::Ready => {}
    };
}
