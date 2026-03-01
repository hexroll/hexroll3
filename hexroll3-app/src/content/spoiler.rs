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

use std::time::Duration;

use bevy::prelude::*;

use crate::shared::tweens::{UiNodeSizeLensMode, UiNodeSizePercentLens};
use crate::shared::widgets::cursor::PointerOnHover;

pub trait ContentIsSpoiler {
    fn content_is_spoiler(&mut self, spoiler: bool) -> &mut Self;
}

#[derive(Component)]
pub struct SpoilerMaskMarker;

impl ContentIsSpoiler for EntityCommands<'_> {
    fn content_is_spoiler(&mut self, spoiler: bool) -> &mut Self {
        self.with_children(|c| {
            c.spawn((
                Node {
                    display: if spoiler {
                        Display::DEFAULT
                    } else {
                        Display::None
                    },
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::BLACK),
                SpoilerMaskMarker,
                ZIndex(99999),
            ))
            .pointer_on_hover()
            .observe(|trigger: On<Pointer<Click>>, mut commands: Commands| {
                commands
                    .entity(trigger.entity)
                    .try_insert(bevy_tweening::Animator::new(bevy_tweening::Tween::new(
                        EaseFunction::QuarticIn,
                        Duration::from_millis(300),
                        UiNodeSizePercentLens {
                            mode: UiNodeSizeLensMode::Width,
                            start: Vec2::new(100.0, 100.0),
                            end: Vec2::new(0.0, 0.0),
                        },
                    )));
            });
        });
        self
    }
}
