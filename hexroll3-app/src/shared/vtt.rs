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

// App-wide Virtual Tabletop Data
use std::fmt;

use serde::{
    Deserialize, Serialize,
    de::{Deserializer, Visitor},
    ser::{SerializeMap, Serializer},
};

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use hexx::Hex;

#[derive(Event)]
pub struct StoreVttState;

#[derive(Event)]
pub struct LoadVttState;

#[derive(Debug, Resource, Default, Serialize, Deserialize, Clone)]
pub struct VttData {
    pub node_name: String,
    pub mode: HexMapMode,
    #[serde(
        serialize_with = "serialize_revealed_state",
        deserialize_with = "deserialize_revealed_state"
    )]
    pub revealed: HashMap<Hex, HexRevealState>,
    pub revealed_ocean: HashSet<Hex>,
    pub open_doors: HashSet<String>,
    #[serde(skip)]
    pub invalidate_map: bool,
}

impl VttData {
    pub fn patch_ephemeral_state(&mut self, existing_data: &VttData) {
        self.mode = existing_data.mode.clone();
        self.node_name = existing_data.node_name.clone();
    }

    pub fn is_player(&self) -> bool {
        self.mode.is_player()
    }
}

#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone, Copy)]
pub enum HexRevealState {
    #[default]
    #[serde(rename = "PT")]
    Partial,
    #[serde(rename = "FL")]
    Full,
}

#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone)]
pub enum HexMapMode {
    #[default]
    #[serde(rename = "V")]
    RefereeViewing,
    #[serde(rename = "R")]
    RefereeRevealing,
    #[serde(rename = "P")]
    Player,
    #[serde(rename = "A")]
    RefereeAsPlayer,
}

impl HexMapMode {
    pub fn is_player(&self) -> bool {
        *self == HexMapMode::Player || *self == HexMapMode::RefereeAsPlayer
    }
    pub fn is_referee(&self) -> bool {
        match self {
            HexMapMode::RefereeViewing
            | HexMapMode::RefereeRevealing
            | HexMapMode::RefereeAsPlayer => true,
            _ => false,
        }
    }
    pub fn mask_visibility(&self, revealed: bool) -> Visibility {
        match self {
            HexMapMode::RefereeRevealing => {
                if revealed {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                }
            }
            _ => Visibility::Hidden,
        }
    }
}

#[derive(Event, Component, Default, PartialEq, Clone)]
pub enum PlayerPreview {
    #[default]
    Off,
    On,
}

fn serialize_revealed_state<S>(
    map: &HashMap<Hex, HexRevealState>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut seq = serializer.serialize_map(Some(map.len()))?;
    for (key, value) in map {
        seq.serialize_entry(&format!("{},{}", key.x, key.y), value)?;
    }
    seq.end()
}

fn deserialize_revealed_state<'de, D>(
    deserializer: D,
) -> Result<HashMap<Hex, HexRevealState>, D::Error>
where
    D: Deserializer<'de>,
{
    struct HexMapVisitor;

    impl<'de> Visitor<'de> for HexMapVisitor {
        type Value = HashMap<Hex, HexRevealState>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a map of string keys to HexRevealState")
        }

        fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
        where
            V: serde::de::MapAccess<'de>,
        {
            let mut result = HashMap::new();
            while let Some((key, value)) = map.next_entry::<String, HexRevealState>()? {
                let (x, y) = key.split_once(',').unwrap();
                let hex = Hex {
                    x: x.parse().map_err(serde::de::Error::custom)?,
                    y: y.parse().map_err(serde::de::Error::custom)?,
                };
                result.insert(hex, value);
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(HexMapVisitor)
}
