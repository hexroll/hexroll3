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

use bevy::{post_process::bloom::Bloom, prelude::*};
use bevy_tweening::component_animator_system;
pub struct SharedTweensPlugin;
impl Plugin for SharedTweensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, component_animator_system::<Camera>)
            .add_systems(Update, component_animator_system::<UiTransform>)
            .add_systems(Update, component_animator_system::<ImageNode>)
            .add_systems(Update, component_animator_system::<Projection>)
            .add_systems(Update, component_animator_system::<Bloom>)
            .add_systems(Update, component_animator_system::<Node>)
            .add_systems(
                Update,
                bevy_tweening::asset_animator_system::<
                    StandardMaterial,
                    MeshMaterial3d<StandardMaterial>,
                >,
            );
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UiNodeSizeLensMode {
    Both,
    Width,
    Height,
    MaxHeight,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UiNodeSizeLens {
    pub mode: UiNodeSizeLensMode,
    pub start: Vec2,
    pub end: Vec2,
}

impl bevy_tweening::Lens<Node> for UiNodeSizeLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<Node>, ratio: f32) {
        if self.mode == UiNodeSizeLensMode::Both || self.mode == UiNodeSizeLensMode::Width {
            target.width = Val::Px(self.start.x + (self.end.x - self.start.x) * ratio);
        }
        if self.mode == UiNodeSizeLensMode::Both || self.mode == UiNodeSizeLensMode::Height {
            target.height = Val::Px(self.start.y + (self.end.y - self.start.y) * ratio);
        }
        if self.mode == UiNodeSizeLensMode::MaxHeight {
            target.max_height = Val::Px(self.start.y + (self.end.y - self.start.y) * ratio);
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UiNodeSizePercentLens {
    pub mode: UiNodeSizeLensMode,
    pub start: Vec2,
    pub end: Vec2,
}

impl bevy_tweening::Lens<Node> for UiNodeSizePercentLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<Node>, ratio: f32) {
        if self.mode == UiNodeSizeLensMode::Both || self.mode == UiNodeSizeLensMode::Width {
            target.width = Val::Percent(self.start.x + (self.end.x - self.start.x) * ratio);
        }
        if self.mode == UiNodeSizeLensMode::Both || self.mode == UiNodeSizeLensMode::Height {
            target.height = Val::Percent(self.start.y + (self.end.y - self.start.y) * ratio);
        }
        if self.mode == UiNodeSizeLensMode::MaxHeight {
            target.max_height =
                Val::Percent(self.start.y + (self.end.y - self.start.y) * ratio);
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UiTransformRotationLens {
    pub start: Rot2,
    pub end: Rot2,
}

impl bevy_tweening::Lens<UiTransform> for UiTransformRotationLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<UiTransform>, ratio: f32) {
        target.rotation = self.start.slerp(self.end, ratio);
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UiTransformScaleLens {
    pub start: Vec2,
    pub end: Vec2,
}

impl bevy_tweening::Lens<UiTransform> for UiTransformScaleLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<UiTransform>, ratio: f32) {
        target.scale = self.start + (self.end - self.start) * ratio;
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CameraViewportLens {
    pub size_start: UVec2,
    pub size_end: UVec2,
    pub pos_start: UVec2,
    pub pos_end: UVec2,
}

impl bevy_tweening::Lens<Camera> for CameraViewportLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<Camera>, ratio: f32) {
        let physical_size = (self.size_start.as_vec2()
            + ((self.size_end.as_vec2() - self.size_start.as_vec2()) * ratio))
            .as_uvec2();
        if physical_size.x == 0 || physical_size.y == 0 {
            target.is_active = false;
        } else {
            target.is_active = true;
            target.viewport.as_mut().unwrap().physical_size = physical_size;
            target.viewport.as_mut().unwrap().physical_position = (self.pos_start.as_vec2()
                + ((self.pos_end.as_vec2() - self.pos_start.as_vec2()) * ratio))
                .as_uvec2();
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UiNodeSizePos {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UiNodeLens {
    pub start: UiNodeSizePos,
    pub end: UiNodeSizePos,
}

impl bevy_tweening::Lens<Node> for UiNodeLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<Node>, ratio: f32) {
        target.left = Val::Px(self.start.left + (self.end.left - self.start.left) * ratio);
        target.top = Val::Px(self.start.top + (self.end.top - self.start.top) * ratio);
        target.height =
            Val::Px(self.start.height + (self.end.height - self.start.height) * ratio);
        target.width = Val::Px(self.start.width + (self.end.width - self.start.width) * ratio);
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UiImageNodeAlphaLens {
    pub from: f32,
    pub to: f32,
}

impl bevy_tweening::Lens<ImageNode> for UiImageNodeAlphaLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<ImageNode>, ratio: f32) {
        target
            .color
            .set_alpha(self.from + (self.to - self.from) * ratio);
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UiNodeMarginsLens {
    pub index: usize,
    pub config: MenuIconMarginLensConfig,
}

#[derive(Component, Copy, Clone, PartialEq, Debug)]
pub struct MenuIconMarginLensConfig {
    pub factor_left: f32,
    pub factor_right: f32,
    pub factor_top: f32,
    pub factor_bottom: f32,
}

impl bevy_tweening::Lens<Node> for UiNodeMarginsLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<Node>, ratio: f32) {
        let v = self.index as f32 * 100.0;
        target.margin.left = Val::Px((-v + v * ratio) * self.config.factor_left);
        target.margin.right = Val::Px((-v + v * ratio) * self.config.factor_right);
        target.margin.top = Val::Px((-v + v * ratio) * self.config.factor_top);
        target.margin.bottom = Val::Px((-v + v * ratio) * self.config.factor_bottom);
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CameraBloomLens {
    pub start: f32,
    pub end: f32,
}

impl bevy_tweening::Lens<Bloom> for CameraBloomLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<Bloom>, ratio: f32) {
        target.intensity = self.start + (self.end - self.start) * ratio;
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ProjectionScaleLens {
    pub start: f32,
    pub end: f32,
}

impl bevy_tweening::Lens<Projection> for ProjectionScaleLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<Projection>, ratio: f32) {
        if let Projection::Orthographic(proj) = target.target_mut() {
            proj.scale = self.start + (self.end - self.start) * ratio;
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct StandardMaterialOpacityLens {
    pub from: f32,
    pub to: f32,
}

impl bevy_tweening::Lens<StandardMaterial> for StandardMaterialOpacityLens {
    fn lerp(
        &mut self,
        target: &mut dyn bevy_tweening::Targetable<StandardMaterial>,
        ratio: f32,
    ) {
        target
            .base_color
            .set_alpha(self.from + (self.to - self.from) * ratio);
    }
}
