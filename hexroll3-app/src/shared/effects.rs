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

// Commonly used tween sequences.
//
// TODO: Reconsider this as a shared module
use std::time::Duration;

use bevy::{ecs::system::SystemId, prelude::*};
use bevy_tweening::*;
use lens::TransformRotateYLens;

use crate::hexmap::elements::HexEntity;

pub struct EffectsPlugin;

#[derive(Resource)]
pub struct EffectSystems {
    pub roll_feature_effect: SystemId,
}

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        let roll_feature_effect = app.register_system(roll_feature_effect);
        app.insert_resource(EffectSystems {
            roll_feature_effect,
        });
    }
}

#[derive(Debug, Component)]
pub struct RollNewFeatureEffect(pub hexx::Hex);

fn roll_feature_effect(
    mut commands: Commands,
    effects: Query<&RollNewFeatureEffect, Added<RollNewFeatureEffect>>,
    hexes: Query<(Entity, &HexEntity)>,
) {
    for effect in effects.iter() {
        for (entity, hex) in hexes.iter() {
            if hex.hex == effect.0 {
                let tween = Tween::new(
                    EaseFunction::ElasticInOut,
                    Duration::from_secs(1),
                    TransformRotateYLens {
                        start: 0.0,
                        end: std::f32::consts::PI * 2.0,
                    },
                )
                .with_repeat_count(5)
                .with_repeat_strategy(RepeatStrategy::Repeat);
                commands
                    .entity(entity)
                    .insert(bevy_tweening::Animator::new(tween));
            }
        }
    }
}
