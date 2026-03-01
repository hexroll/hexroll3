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

use ron::de::from_bytes;
use serde::Deserialize;
use serde::de::Deserializer;
use std::io;

use bevy::{asset::AssetLoader, platform::collections::HashMap, prelude::*};

use crate::shared::vtt::HexMapMode;

use super::data::TerrainType;

#[derive(Resource, Debug, Clone)]
pub struct HexmapTheme {
    pub name: String,
    pub def: TileSetDefinition,
}

impl HexmapTheme {
    pub fn terrain_colors(&self) -> HashMap<TerrainType, TerrainColors> {
        self.def.terrain_colors.clone()
    }

    pub fn theme_folder(&self) -> &str {
        &self.name
    }

    pub fn clear_color_by_mode(&self, mode: &HexMapMode, day_night: f32) -> ClearColorConfig {
        if mode.is_player() {
            ClearColorConfig::Custom(
                self.def
                    .clear_color_for_player
                    .day
                    .mix(&self.def.clear_color_for_player.night, day_night),
            )
        } else if *mode == HexMapMode::RefereeRevealing {
            ClearColorConfig::Custom(
                self.def
                    .clear_color_for_revealing
                    .day
                    .mix(&self.def.clear_color_for_revealing.night, day_night),
            )
        } else {
            ClearColorConfig::Custom(
                self.def
                    .clear_color_for_referee
                    .day
                    .mix(&self.def.clear_color_for_referee.night, day_night),
            )
        }
    }

    pub fn use_rim_for_rivers(&self) -> bool {
        self.def.use_rim_for_rivers
    }

    pub fn tile_offset(&self) -> f32 {
        self.def.tile_offset
    }

    pub fn tile_scale_values(&self) -> (f32, f32) {
        (self.def.scale_in_rim, self.def.scale_not_in_rim)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TerrainColors {
    #[serde(deserialize_with = "deserialize_color_u8")]
    pub day: Color,
    #[serde(deserialize_with = "deserialize_color_u8")]
    pub night: Color,
}

impl TerrainColors {
    pub fn mix(&self, ratio: f32) -> Color {
        self.day.mix(&self.night, ratio)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TileSetDefinition {
    pub use_rim_for_rivers: bool,
    pub tile_offset: f32,
    pub scale_in_rim: f32,
    pub scale_not_in_rim: f32,

    pub clear_color_for_player: ClearColorDefinition,
    pub clear_color_for_revealing: ClearColorDefinition,
    pub clear_color_for_referee: ClearColorDefinition,

    pub terrain_colors: HashMap<TerrainType, TerrainColors>,
    pub river_color: TerrainColors,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ClearColorDefinition {
    #[serde(deserialize_with = "deserialize_color_u8")]
    pub day: Color,
    #[serde(deserialize_with = "deserialize_color_u8")]
    pub night: Color,
}

#[derive(Asset, TypePath, Debug, Deserialize)]
pub struct TileSetThemes {
    pub themes: HashMap<String, TileSetDefinition>,
}

impl TileSetThemes {
    fn from_bytes(
        bytes: Vec<u8>,
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self, io::Error> {
        let tile_set_themes: TileSetThemes =
            from_bytes(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(tile_set_themes)
    }
}

#[derive(Default)]
pub struct TileSetThemesAssetLoader;

impl AssetLoader for TileSetThemesAssetLoader {
    type Asset = TileSetThemes;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &(),
        mut load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, std::io::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(TileSetThemes::from_bytes(bytes, &mut load_context)?)
    }
}

fn deserialize_color_u8<'de, D>(deserializer: D) -> Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let (r, g, b, a) = <(u8, u8, u8, u8)>::deserialize(deserializer)?;
    Ok(Color::srgba_u8(r, g, b, a))
}
