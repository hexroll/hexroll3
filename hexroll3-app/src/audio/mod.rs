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

use bevy::{platform::collections::HashMap, prelude::*};
use bevy_seedling::prelude::*;

use crate::{
    hexmap::{HexFeature, TerrainType},
    shared::{settings::UserSettings, widgets::buttons::ToggleEventWrapper},
};

pub struct SoundtrackPlugin;

impl Plugin for SoundtrackPlugin {
    fn build(&self, app: &mut App) {
        if app.world().get_resource::<UserSettings>().unwrap().audio {
            app.add_plugins(bevy_seedling::SeedlingPlugin::default())
                .add_systems(Startup, setup)
                .add_systems(Update, fade_in_new_players)
                .add_systems(Update, set_listener_distance)
                .add_observer(on_toggle_audio)
                .add_observer(on_dungeon_sound)
                .add_observer(on_battlemap_sound);
        }
    }
}

#[derive(Resource)]
pub struct HexMapSoundscapes {
    pub biomes: HashMap<TerrainType, Handle<AudioSample>>,
    pub features: HashMap<HexFeature, Handle<AudioSample>>,
    pub dungeons: HashMap<DungeonAudioSample, Handle<AudioSample>>,
}

#[derive(Event)]
pub struct PlayBattlemapSound {
    pub hex_entity: Entity,
    pub biome: TerrainType,
    pub feature: HexFeature,
    pub is_revealed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DungeonAudioSample {
    Dungeon,
    Cave,
}

#[derive(Event)]
pub struct PlayDungeonSound {
    pub hex_entity: Entity,
    pub sample: DungeonAudioSample,
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut biomes: HashMap<TerrainType, Handle<AudioSample>> = HashMap::new();
    biomes.insert(
        TerrainType::ForestHex,
        asset_server.load("audio/forest.ogg"),
    );
    biomes.insert(
        TerrainType::MountainsHex,
        asset_server.load("audio/mountains.ogg"),
    );
    biomes.insert(
        TerrainType::PlainsHex,
        asset_server.load("audio/forest.ogg"),
    );
    biomes.insert(
        TerrainType::SwampsHex,
        asset_server.load("audio/swamps.ogg"),
    );
    biomes.insert(
        TerrainType::TundraHex,
        asset_server.load("audio/tundra.ogg"),
    );
    biomes.insert(
        TerrainType::DesertHex,
        asset_server.load("audio/desert.ogg"),
    );
    biomes.insert(
        TerrainType::JungleHex,
        asset_server.load("audio/jungle.ogg"),
    );
    biomes.insert(TerrainType::OceanHex, asset_server.load("audio/ocean.ogg"));

    let mut features: HashMap<HexFeature, Handle<AudioSample>> = HashMap::new();
    features.insert(HexFeature::City, asset_server.load("audio/city.ogg"));
    features.insert(HexFeature::Town, asset_server.load("audio/city.ogg"));
    features.insert(HexFeature::Village, asset_server.load("audio/village.ogg"));

    let mut dungeons: HashMap<DungeonAudioSample, Handle<AudioSample>> = HashMap::new();

    dungeons.insert(
        DungeonAudioSample::Cave,
        asset_server.load("audio/cave.ogg"),
    );
    dungeons.insert(
        DungeonAudioSample::Dungeon,
        asset_server.load("audio/crypt.ogg"),
    );

    commands.insert_resource(HexMapSoundscapes {
        biomes,
        features,
        dungeons,
    });
}

#[derive(Component, Default, PartialEq, Clone, Event)]
pub enum AudioToggle {
    #[default]
    On,
    Off,
}

fn on_toggle_audio(
    trigger: On<ToggleEventWrapper<AudioToggle>>,
    mut main_bus: Query<
        &mut bevy_seedling::prelude::VolumeNode,
        With<bevy_seedling::prelude::MainBus>,
    >,
) {
    match trigger.event().value {
        AudioToggle::On => {
            for mut volume_node in main_bus.iter_mut() {
                volume_node.volume = bevy_seedling::prelude::Volume::Linear(1.0);
            }
        }
        AudioToggle::Off => {
            for mut volume_node in main_bus.iter_mut() {
                volume_node.volume = bevy_seedling::prelude::Volume::Linear(0.0);
            }
        }
    }
}

fn on_battlemap_sound(
    trigger: On<PlayBattlemapSound>,
    mut commands: Commands,
    soundscapes: Res<crate::audio::HexMapSoundscapes>,
    players: Query<&SamplePlayer>,
) {
    setup_player_on_entity(
        trigger.hex_entity,
        &mut commands,
        &players,
        if trigger.is_revealed {
            soundscapes
                .features
                .get(&trigger.feature)
                .unwrap_or(soundscapes.biomes.get(&trigger.biome).unwrap())
                .clone()
        } else {
            soundscapes.biomes.get(&trigger.biome).unwrap().clone()
        },
    );
}

fn on_dungeon_sound(
    trigger: On<PlayDungeonSound>,
    mut commands: Commands,
    soundscapes: Res<crate::audio::HexMapSoundscapes>,
    players: Query<&SamplePlayer>,
) {
    if let Some(sample) = soundscapes.dungeons.get(&trigger.sample) {
        setup_player_on_entity(trigger.hex_entity, &mut commands, &players, sample.clone());
    }
}

#[derive(Component)]
struct FadeInCompleted;

fn fade_in_new_players(
    mut commands: Commands,
    mut new_players: Query<
        (Entity, &mut bevy_seedling::prelude::SpatialBasicNode),
        Without<FadeInCompleted>,
    >,
    time: Res<Time>,
) {
    for (e, mut player) in new_players.iter_mut() {
        if let Volume::Linear(v) = player.volume {
            if v < 1.0 {
                player.volume =
                    bevy_seedling::prelude::Volume::Linear(v + (0.5 * time.delta_secs()));
            } else {
                commands.entity(e).try_insert(FadeInCompleted);
            }
        }
    }
}

pub fn set_listener_distance(
    mvc: Res<crate::hexmap::elements::MapVisibilityController>,
    mut listener: Query<&mut Transform, With<bevy_seedling::spatial::SpatialListener3D>>,
) {
    if let Some(mut audio_listener_transform) = listener.iter_mut().next() {
        audio_listener_transform.translation.z = {
            let range = 500.0;
            let normalized = {
                let min_value = 0.1;
                let max_value = 1.0;
                if mvc.scale < min_value {
                    0.0
                } else if mvc.scale > max_value {
                    1.0
                } else {
                    (mvc.scale - min_value) / (max_value - min_value)
                }
            };
            -500.0 - range * normalized
        };
    }
}

fn setup_player_on_entity(
    entity: Entity,
    commands: &mut Commands,
    players: &Query<&SamplePlayer>,
    sample: Handle<AudioSample>,
) {
    if players.contains(entity) {
        if let Ok(p) = players.get(entity) {
            if p.sample == sample {
                return;
            }
        }
    }
    if let Ok(mut entity) = commands.get_entity(entity) {
        let mut distance_attenuation =
            bevy_seedling::firewheel::dsp::distance_attenuation::DistanceAttenuation::default(
            );
        distance_attenuation.distance_gain_factor = 0.1;
        entity.try_insert((
            bevy_seedling::sample::SamplePlayer::new(sample.clone())
                .looping()
                .with_volume(bevy_seedling::prelude::Volume::Linear(2.0)),
            bevy_seedling::prelude::PlaybackSettings {
                play_from: bevy_seedling::prelude::PlayFrom::Seconds(rand::Rng::gen_range(
                    &mut rand::thread_rng(),
                    0.0..=10.0,
                )),
                ..default()
            }
            // NOTE: This is critical to ensure the battlemap will not vanish
            .preserve(),
            bevy_seedling::sample_effects![bevy_seedling::prelude::SpatialBasicNode {
                panning_threshold: 0.3,
                volume: bevy_seedling::prelude::Volume::Linear(0.0),
                distance_attenuation,
                ..default()
            }],
        ));
    }
}
