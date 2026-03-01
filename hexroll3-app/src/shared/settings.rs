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

use std::{fs::File, path::PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::ContentDarkMode;

pub const CONFIG_DIR: &str = "hexroll3";

#[derive(Default, Reflect, PartialEq, Serialize, Deserialize, Clone)]
pub enum RulersMode {
    #[default]
    ShowWhenMoving,
    NoRulers,
}

#[derive(Default, Reflect, PartialEq, Serialize, Deserialize, Clone, Component)]
pub enum LabelsMode {
    #[default]
    RegionsAndAreasOnly,
    HexCoordinatesOnly,
    All,
    None,
}

impl LabelsMode {
    pub fn hex_coords_visible(&self) -> bool {
        *self == Self::All || *self == Self::HexCoordinatesOnly
    }
    pub fn labels_visible(&self) -> bool {
        *self == Self::All || *self == Self::RegionsAndAreasOnly
    }
    pub fn cycle(&mut self) {
        *self = match *self {
            LabelsMode::RegionsAndAreasOnly => LabelsMode::HexCoordinatesOnly,
            LabelsMode::HexCoordinatesOnly => LabelsMode::All,
            LabelsMode::All => LabelsMode::None,
            LabelsMode::None => LabelsMode::RegionsAndAreasOnly,
        };
    }
}

#[derive(Resource, Default, Reflect, Serialize, Deserialize, Clone)]
#[reflect(Resource)]
pub struct AppSettings {
    pub rulers_mode: RulersMode,
    pub labels_mode: LabelsMode,
}

#[derive(Reflect, Serialize, Deserialize, Clone)]
pub struct SandboxRef {
    pub sandbox: Option<String>,
    pub key: Option<String>,
    pub last_used: Option<u64>,
}

#[derive(Resource, Reflect, Serialize, Deserialize, Clone)]
#[reflect(Resource)]
pub struct UserSettings {
    pub server: String,
    pub signaling: String,
    pub sandbox: Option<String>,
    pub key: Option<String>,
    pub sandboxes: Vec<SandboxRef>,
    pub tts_command: Option<String>,
    pub audio: bool,
}

impl UserSettings {
    #[cfg(not(feature = "dev"))]
    const CONFIG_FILENAME: &str = "hexroll.json";

    #[cfg(not(feature = "dev"))]
    pub fn config_path() -> PathBuf {
        let config_dir = dirs::config_dir().expect("Unable to get config dir");
        let config_path = config_dir.join(CONFIG_DIR).join(Self::CONFIG_FILENAME);
        config_path
    }

    #[cfg(feature = "dev")]
    pub fn config_path() -> PathBuf {
        PathBuf::from("./dev.json")
    }

    pub fn read_or_init() -> Self {
        let config_path = Self::config_path();
        if std::fs::metadata(&config_path).is_ok() {
            let file = std::fs::File::open(&config_path).expect("Cannot open hexroll.json");
            let reader = std::io::BufReader::new(file);
            serde_json::from_reader(reader).expect("Cannot parse hexroll.json")
        } else {
            let config_dir = dirs::config_dir().expect("Unable to get config dir");
            std::fs::create_dir_all(config_dir.join("hexroll3"))
                .expect("Failed to create directory");
            let default_config = Self::default();
            default_config.save();
            default_config
        }
    }
    pub fn save(&self) {
        let config_path = Self::config_path();
        let file = File::create(config_path).expect("Failed to create config file");
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self).expect("Failed to save config file");
    }
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            server: "https://hexroll.app".to_string(),
            signaling: "wss://vtt.hexroll.app:4321".to_string(),
            sandbox: None,
            key: None,
            sandboxes: Vec::new(),
            tts_command: None,
            audio: true,
        }
    }
}

#[derive(Resource, Reflect, Serialize, Deserialize, Clone)]
#[reflect(Resource)]
pub struct PageTheme {
    pub text: Color,
    pub bg: Color,
    pub table_header: Color,
    pub table_row_odd: Color,
    pub table_row_even: Color,
    pub link_bg: Color,
}

#[derive(Reflect, Serialize, Deserialize, Clone)]
pub struct TokensConfig {
    pub label_font_scale: f32,
}

impl Default for TokensConfig {
    fn default() -> Self {
        Self {
            // Sets the token labels scale. Optimal range is 0.5 - 2.0.
            label_font_scale: 1.0,
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone)]
pub struct RulerConfig {
    pub label_font_scale: f32,
}

impl Default for RulerConfig {
    fn default() -> Self {
        Self {
            // Sets the ruler label scale. Optimal range is 0.5 - 2.0.
            label_font_scale: 1.0,
        }
    }
}

#[derive(Resource, Reflect, Serialize, Deserialize, Clone)]
#[reflect(Resource)]
pub struct SnackbarConfig {
    pub font_size: f32,
    pub max_dice_results_to_show: usize,
}

impl Default for SnackbarConfig {
    fn default() -> Self {
        Self {
            font_size: 24.0,
            max_dice_results_to_show: 10,
        }
    }
}

#[derive(Resource, Reflect, Serialize, Deserialize, Clone)]
#[reflect(Resource)]
pub struct Config {
    pub daytime_page_theme: PageTheme,
    pub nighttime_page_theme: PageTheme,
    pub tokens_config: TokensConfig,
    pub ruler_config: RulerConfig,
    pub snackbar_config: SnackbarConfig,
}

impl Config {
    pub fn daytime_page_theme(&self, dark_mode: &ContentDarkMode) -> &PageTheme {
        match dark_mode {
            ContentDarkMode::Off => &self.daytime_page_theme,
            ContentDarkMode::On => &self.nighttime_page_theme,
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            tokens_config: TokensConfig::default(),
            ruler_config: RulerConfig::default(),
            snackbar_config: SnackbarConfig::default(),
            daytime_page_theme: PageTheme {
                text: Color::BLACK,
                bg: Color::WHITE,
                table_header: Color::srgb_u8(200, 200, 200),
                table_row_odd: Color::srgb_u8(245, 245, 245),
                table_row_even: Color::srgb_u8(230, 230, 230),
                link_bg: Color::srgb_u8(230, 230, 230),
            },
            nighttime_page_theme: PageTheme {
                text: Color::WHITE,
                bg: Color::srgb_u8(45, 45, 45),
                table_header: Color::srgb_u8(20, 20, 20),
                table_row_odd: Color::srgb_u8(45, 45, 45),
                table_row_even: Color::srgb_u8(30, 30, 30),
                link_bg: Color::srgb_u8(30, 30, 30),
            },
        }
    }
}
