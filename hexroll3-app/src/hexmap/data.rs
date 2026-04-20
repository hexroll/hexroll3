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

use bevy::{platform::collections::HashMap, prelude::Component};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct HexMapJson {
    pub map: Vec<MapHex>,
    pub realms: HashMap<String, Realm>,
    pub regions: HashMap<String, String>,
    pub borders: HashMap<String, Vec<Border>>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash, Component)]
pub enum TerrainType {
    ForestHex,
    DesertHex,
    MountainsHex,
    SwampsHex,
    TundraHex,
    PlainsHex,
    JungleHex,
    OceanHex,
}

impl TerrainType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TerrainType::ForestHex => "ForestHex",
            TerrainType::DesertHex => "DesertHex",
            TerrainType::MountainsHex => "MountainsHex",
            TerrainType::SwampsHex => "SwampsHex",
            TerrainType::TundraHex => "TundraHex",
            TerrainType::PlainsHex => "PlainsHex",
            TerrainType::JungleHex => "JungleHex",
            TerrainType::OceanHex => "OceanHex",
        }
    }
    pub fn as_region_str(&self) -> &'static str {
        match self {
            TerrainType::ForestHex => "ForestRegion",
            TerrainType::DesertHex => "DesertRegion",
            TerrainType::MountainsHex => "MountainsRegion",
            TerrainType::SwampsHex => "SwampsRegion",
            TerrainType::TundraHex => "TundraRegion",
            TerrainType::PlainsHex => "PlainsRegion",
            TerrainType::JungleHex => "JungleRegion",
            TerrainType::OceanHex => "OceanRegion",
        }
    }
}

#[derive(Component, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum HexFeature {
    None,
    Other,
    Dungeon,
    Inn,
    Residency,
    City,
    Town,
    Village,
}

impl HexFeature {
    pub fn can_have_a_harbor(&self) -> bool {
        match self {
            Self::City | Self::Town | Self::Village => true,
            _ => false,
        }
    }

    pub fn harbor_offset(&self) -> f32 {
        match self {
            Self::City | Self::Town => -42.0,
            Self::Village => -58.0,
            _ => 0.0,
        }
    }

    pub fn feature_scale(&self) -> f32 {
        match self {
            Self::Village => 5.0,
            _ => 10.0,
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct HexMetadata {
    pub harbor: Option<u8>,
    pub river_dir: f32,
    pub offset: f32,
    pub is_rim: bool,
}

impl HexMetadata {
    pub fn feature_angle_and_offset(&self) -> (f32, f32) {
        if let Some(dir) = self.harbor {
            let hour = dir as f32;
            (std::f32::consts::PI * 2.0 * (hour / -6.0), self.offset)
        } else {
            (0.0 + self.river_dir, 0.0)
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct MapHex {
    pub x: i32,
    pub y: i32,
    #[serde(rename = "type")]
    pub hex_type: TerrainType,
    pub uuid: String,
    pub feature: Option<HexFeature>,
    pub feature_uuid: Option<String>,
    pub trails: Option<Vec<u8>>,
    pub rivers: Option<Vec<u8>>,
    pub river_dir: Option<f32>,
    pub region: Option<String>,
    pub realm: Option<String>,
    pub harbor: Option<u8>,
    pub label: Option<String>,
    pub borderline: Option<bool>,
}

#[derive(Deserialize)]
pub struct Realm {
    pub name: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct Border {
    pub hex_x: i32,
    pub hex_y: i32,
    pub borders: Vec<u8>,
}
