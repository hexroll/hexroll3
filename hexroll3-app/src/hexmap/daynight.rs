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

use bevy::post_process::bloom::Bloom;
use bevy_tweening::component_animator_system;
use serde::{Deserialize, Serialize};

use crate::{
    battlemaps::BattlemapMaterial,
    clients::model::FetchEntityReason,
    content::{ContentDarkMode, ContentMode, context::ContentContext},
    hexmap::{elements::MainCamera, grid::HexMaterial, tiles::*},
    shared::{AppState, tweens::CameraBloomLens, vtt::VttData},
};

use super::{elements::FetchEntityFromStorage, themes::HexmapTheme};

pub struct DayNightPlugin;
impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<HexMapTime>()
            .add_systems(Update, component_animator_system::<HexMapTime>)
            .add_observer(on_toggle_day_night)
            .add_systems(Update, hex_map_day_night.run_if(in_state(AppState::Live)));
    }
}
#[derive(PartialEq, Reflect, Component, Deserialize, Serialize, Default, Clone, Debug)]
pub enum DayNight {
    #[default]
    Day,
    Night,
}

#[derive(Reflect, Component, Default, Debug)]
#[reflect(Component)]
pub struct HexMapTime {
    pub day_night: DayNight,
    pub day_night_analog: f32,
}

impl HexMapTime {
    pub fn toggle(&self) -> DayNight {
        match self.day_night {
            DayNight::Day => DayNight::Night,
            DayNight::Night => DayNight::Day,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct HexMapTimeLens {
    pub start: f32,
    pub end: f32,
}

impl bevy_tweening::Lens<HexMapTime> for HexMapTimeLens {
    fn lerp(&mut self, target: &mut dyn bevy_tweening::Targetable<HexMapTime>, ratio: f32) {
        target.day_night_analog = self.start + (self.end - self.start) * ratio;
    }
}

#[derive(Event, Default)]
pub struct ToggleDayNight {
    pub value: DayNight,
}

fn on_toggle_day_night(
    trigger: On<ToggleDayNight>,
    mut commands: Commands,
    mut time: Single<(Entity, &mut HexMapTime)>,
    camera: Single<(Entity, &Bloom), With<MainCamera>>,
    context: Res<ContentContext>,
    mut content_dark_mode: ResMut<ContentDarkMode>,
    content_mode: Res<State<ContentMode>>,
) {
    let (time_entity, time) = &mut *time;
    let (camera_entity, bloom) = *camera;
    time.day_night = trigger.value.clone();

    commands
        .entity(*time_entity)
        .insert(bevy_tweening::Animator::new(bevy_tweening::Tween::new(
            EaseFunction::Linear,
            Duration::from_secs(3),
            if time.day_night == DayNight::Day {
                HexMapTimeLens {
                    start: time.day_night_analog,
                    end: 0.0,
                }
            } else {
                HexMapTimeLens {
                    start: time.day_night_analog,
                    end: 1.0,
                }
            },
        )));
    commands
        .entity(camera_entity)
        .insert(bevy_tweening::Animator::new(bevy_tweening::Tween::new(
            EaseFunction::Linear,
            Duration::from_secs(3),
            if time.day_night == DayNight::Day {
                CameraBloomLens {
                    start: bloom.intensity,
                    end: 0.0,
                }
            } else {
                CameraBloomLens {
                    start: bloom.intensity,
                    end: 0.5,
                }
            },
        )));
    content_dark_mode.toggle();
    if *content_mode == ContentMode::SplitScreen
        && let Some(current_uid) = &context.current_entity_uid
    {
        commands.trigger(FetchEntityFromStorage {
            uid: current_uid.clone(),
            anchor: None,
            why: FetchEntityReason::Refresh,
        });
    }
}

fn hex_map_day_night(
    theme: Res<HexmapTheme>,
    mut hex_materials: ResMut<Assets<HexMaterial>>,
    mut tile_materials: ResMut<Assets<TileMaterial>>,
    mut river_materials: ResMut<Assets<RiverMaterial>>,
    mut background_materials: ResMut<Assets<BackgroundMaterial>>,
    mut camera: Single<&mut Camera, With<MainCamera>>,
    tiles: Res<HexMapTileMaterials>,
    map_time: Single<&HexMapTime>,
    vtt_data: Res<VttData>,
    map_resources: Res<crate::hexmap::elements::HexMapResources>,
    mut battlemap_materials: ResMut<Assets<BattlemapMaterial>>,
) {
    for (t, m) in tiles.terrain_background_materials.iter() {
        let empty_alpha = background_materials
            .get_mut(&m.empty)
            .unwrap()
            .base_color
            .alpha;
        background_materials.get_mut(&m.empty).unwrap().base_color = tiles
            .unified_terrain_colors
            .get(t)
            .unwrap()
            .mix(map_time.day_night_analog)
            .with_alpha(empty_alpha)
            .into();
        let dungeon_alpha = background_materials
            .get_mut(&m.overlayer)
            .unwrap()
            .base_color
            .alpha;
        background_materials
            .get_mut(&m.overlayer)
            .unwrap()
            .base_color = tiles
            .unified_terrain_colors
            .get(t)
            .unwrap()
            .mix(map_time.day_night_analog)
            .with_alpha(dungeon_alpha)
            .into();
        background_materials
            .get_mut(&m.overlayer)
            .unwrap()
            .layer_color = tiles
            .unified_terrain_colors
            .get(t)
            .unwrap()
            .mix(map_time.day_night_analog)
            .with_alpha(dungeon_alpha)
            .into();
    }
    for (_, m) in tiles.terrain_rim_materials.iter() {
        tile_materials.get_mut(m).unwrap().mixer.x = map_time.day_night_analog;
    }
    for (_, m) in tiles.terrain_materials.iter() {
        tile_materials.get_mut(m).unwrap().mixer.x = map_time.day_night_analog;
    }
    for (_, m) in tiles.terrain_feature_materials.iter() {
        for (_, m) in m.iter() {
            tile_materials.get_mut(m).unwrap().mixer.x = map_time.day_night_analog;
        }
    }
    camera.clear_color = theme.clear_color_by_mode(&vtt_data.mode, map_time.day_night_analog);
    for hex_material in hex_materials.iter_mut() {
        hex_material.1.color =
            LinearRgba::from_vec3(Vec3::splat(1.0 - map_time.day_night_analog))
                .with_alpha(hex_material.1.color.alpha);
    }
    // NOTE: Change non-dungeon battlemap instensity
    if let Some(battlemap_grid) =
        battlemap_materials.get_mut(&map_resources.battlemap_material)
    {
        let color_value = (1.1 - map_time.day_night_analog).clamp(0.1, 1.0);
        battlemap_grid.color = Vec4::new(color_value, color_value, color_value, 1.0);
    }

    // Rivers color
    if let Some(river_material) =
        river_materials.get_mut(&map_resources.river_tile_materials.river_tile_material)
    {
        let current_river_fader_value = river_material.color.alpha;
        river_material.color = theme
            .def
            .river_color
            .day
            .mix(&theme.def.river_color.night, map_time.day_night_analog)
            .with_alpha(current_river_fader_value)
            .into();
    }
    if let Some(river_material) =
        river_materials.get_mut(&map_resources.river_tile_materials.river_battlemap_material)
    {
        let current_river_fader_value = river_material.color.alpha;
        river_material.color = theme
            .def
            .river_color
            .day
            .mix(&theme.def.river_color.night, map_time.day_night_analog)
            .with_alpha(current_river_fader_value)
            .into();
    }
}
