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
use hexx::{EdgeDirection, Hex};

use crate::{
    battlemaps::BattlemapFeatureUtils,
    hexmap::{
        elements::{HexCoordsForFeature, HexMapData, HexMask},
        sync::{HexState, MapMessage},
    },
    shared::vtt::{HexMapMode, HexRevealState, VttData},
    vtt::sync::SyncMapForPeers,
};

use super::elements::{HexEntity, HexRevealPattern};

pub fn reveal_hex_or_ocean(
    commands: &mut Commands,
    coord: Hex,
    vtt_map: &mut VttData,
    map: &mut HexMapData,
    mut masks: Query<(Entity, &mut HexMask)>,
    features: Query<(Entity, &HexCoordsForFeature)>,
    hexes: Query<(Entity, &HexEntity)>,
    reveal_pattern: &HexRevealPattern,
) {
    let mut ret: Vec<(Hex, HexRevealInstruction)> = Vec::new();
    match reveal_pattern {
        &HexRevealPattern::Flower => {
            ret.push((coord, HexRevealInstruction::PromoteUntilFull));
            for dir in EdgeDirection::ALL_DIRECTIONS {
                let neighor = coord.neighbor(dir);
                ret.push((neighor, HexRevealInstruction::PromoteUntilPartial));
            }
        }
        &HexRevealPattern::Single => {
            ret.push((coord, HexRevealInstruction::Cycle));
        }
    }

    for (hex, state) in ret.iter() {
        // - - - - - - - - - - - - - - - - - - - - - - - - - - -
        // Revealing a pre-generated hex
        let coord = *hex;
        if map.hexes.contains_key(&coord) {
            let reveal_state = vtt_map.modify_hex_reveal_state(&coord, state);

            commands.trigger(SyncMapForPeers(MapMessage::HexStateChange(HexState {
                coords: coord,
                is_ocean: false,
                state: reveal_state,
            })));

            //FIXME: Can we avoid iterating the full set of visible hexes?
            for (e, mask) in masks.iter_mut() {
                if mask.0 == coord {
                    commands
                        .entity(e)
                        .try_insert(vtt_map.get_reveal_state_components(&coord));
                }
            }
            // NOTE: This will invalidate any already generated battlemaps.
            // For example, this will invalidate the partially generated
            // dungeon and its top transparent battlemap, so that we can
            // regenerate the full dungeon without the top layer.
            for (e, feature) in features.iter() {
                if feature.hex == coord {
                    commands.entity(e).invalidate_battlemap_in_hex_feature();
                }
            }
        // - - - - - - - - - - - - - - - - - - - - - - - - - - -
        //
        // Revealing a not-yet-generated ocean hex
        } else {
            let reveal_state =
                vtt_map.modify_empty_ocean_reveal_state(hex, state, commands, hexes);

            commands.trigger(SyncMapForPeers(MapMessage::HexStateChange(HexState {
                coords: coord,
                is_ocean: true,
                state: reveal_state,
            })));
            if reveal_state.is_some() {
                vtt_map.invalidate_map = true;
            }
        }
    }
}

pub enum HexRevealInstruction {
    PromoteUntilFull,
    PromoteUntilPartial,
    Cycle,
}

pub trait VttHexRevealer {
    fn modify_hex_reveal_state(
        &mut self,
        hex: &Hex,
        action: &HexRevealInstruction,
    ) -> Option<HexRevealState>;
    fn modify_empty_ocean_reveal_state(
        &mut self,
        hex: &Hex,
        action: &HexRevealInstruction,
        commands: &mut Commands,
        hexes: Query<(Entity, &HexEntity)>,
    ) -> Option<HexRevealState>;
    fn get_reveal_state_components(&self, hex: &Hex) -> (Transform, Visibility);
}

impl VttHexRevealer for VttData {
    fn modify_hex_reveal_state(
        &mut self,
        hex: &Hex,
        action: &HexRevealInstruction,
    ) -> Option<HexRevealState> {
        match self.revealed.get_mut(hex) {
            Some(mut reveal_state) => match (&mut reveal_state, action) {
                (HexRevealState::Partial, HexRevealInstruction::PromoteUntilPartial) => {
                    Some(HexRevealState::Partial)
                }
                (HexRevealState::Partial, HexRevealInstruction::PromoteUntilFull) => {
                    *reveal_state = HexRevealState::Full;
                    Some(HexRevealState::Full)
                }
                (HexRevealState::Full, HexRevealInstruction::Cycle) => {
                    self.revealed.remove(hex);
                    None
                }
                (HexRevealState::Partial, HexRevealInstruction::Cycle) => {
                    *reveal_state = HexRevealState::Full;
                    Some(HexRevealState::Full)
                }
                (HexRevealState::Full, HexRevealInstruction::PromoteUntilFull) => {
                    Some(HexRevealState::Full)
                }
                (HexRevealState::Full, HexRevealInstruction::PromoteUntilPartial) => {
                    Some(HexRevealState::Full)
                }
            },
            None => {
                self.revealed.insert(*hex, HexRevealState::Partial);
                Some(HexRevealState::Partial)
            }
        }
    }

    fn modify_empty_ocean_reveal_state(
        &mut self,
        hex: &Hex,
        action: &HexRevealInstruction,
        commands: &mut Commands,
        hexes: Query<(Entity, &HexEntity)>,
    ) -> Option<HexRevealState> {
        match action {
            HexRevealInstruction::Cycle => {
                if self.revealed_ocean.contains(hex) {
                    self.revealed_ocean.remove(hex);
                    for (entity, hex_data) in hexes.iter() {
                        if hex_data.hex == *hex {
                            commands.entity(entity).try_despawn();
                        }
                    }
                    None
                } else {
                    self.revealed_ocean.insert(*hex);
                    Some(HexRevealState::Full)
                }
            }
            HexRevealInstruction::PromoteUntilPartial => {
                if !self.revealed_ocean.contains(hex) {
                    self.revealed_ocean.insert(*hex);
                }
                Some(HexRevealState::Full)
            }
            HexRevealInstruction::PromoteUntilFull => {
                if !self.revealed_ocean.contains(hex) {
                    self.revealed_ocean.insert(*hex);
                    Some(HexRevealState::Full)
                } else {
                    None
                }
            }
        }
    }
    fn get_reveal_state_components(&self, hex: &Hex) -> (Transform, Visibility) {
        let base_transform = Transform::from_xyz(
            0.0,
            crate::shared::layers::HEIGHT_OF_TOP_MOST_LAYERED_TILE + 150.0,
            0.0,
        );
        let maybe_visible = if self.mode == HexMapMode::RefereeRevealing {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Some(current_state) = self.revealed.get(hex) {
            match current_state {
                HexRevealState::Partial => {
                    (base_transform.with_scale(Vec3::splat(0.5)), maybe_visible)
                }
                HexRevealState::Full => (base_transform, Visibility::Hidden),
            }
        } else {
            (base_transform, maybe_visible)
        }
    }
}
