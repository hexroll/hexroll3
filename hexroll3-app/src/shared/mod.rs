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

pub mod asynchttp;
pub mod camera;
pub mod curve;
pub mod disc;
pub mod dragging;
pub mod effects;
pub mod geometry;
pub mod gltf;
pub mod input;
pub mod labels;
pub mod layers;
pub mod poly;
pub mod settings;
pub mod spawnq;
pub mod svg;
pub mod tweens;
pub mod vtt;
pub mod widgets;

#[derive(bevy::state::state::States, Debug, Default, Hash, PartialEq, Eq, Clone)]
pub enum AppState {
    #[default]
    Boot,
    Intro,
    Live,
}

#[derive(bevy::state::state::States, Debug, Default, Hash, PartialEq, Eq, Clone)]
pub enum LoadingState {
    #[default]
    Unready,
    Loading,
    Ready,
}
