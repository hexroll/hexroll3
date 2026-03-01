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

use bevy::{
    asset::{load_internal_asset, uuid_handle},
    prelude::*,
};
pub const HEX_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("d6d4c6d5-9b65-4c8e-9dad-f6b25635a024");

pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            HEX_SHADER_HANDLE,
            "shaders/hex_material.wgsl",
            Shader::from_wgsl
        );

        app.add_plugins(MaterialPlugin::<HexMaterial>::default());
    }
}

#[derive(Asset, TypePath, bevy::render::render_resource::AsBindGroup, Debug, Clone)]
pub struct HexMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub res: Vec4,

    pub alpha_mode: AlphaMode,
}

impl Material for HexMaterial {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        HEX_SHADER_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}
