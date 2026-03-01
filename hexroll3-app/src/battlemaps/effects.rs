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
use std::io;
use std::time::Duration;

use bevy::{asset::AssetLoader, platform::collections::HashMap, prelude::*};
use serde::{Deserialize, Serialize};

use crate::shared::{LoadingState, layers::HEIGHT_OF_TOKENS};

use omagari::{EffectComplex, OmagariPlugin};

pub struct BattlemapEffectsPlugin;

impl Plugin for BattlemapEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<VfxLibrary>()
            .init_asset_loader::<VfxLibraryAssetLoader>()
            .add_systems(Startup, setup_library)
            .add_systems(OnEnter(LoadingState::Loading), load_effects)
            .add_plugins(OmagariPlugin)
            .add_observer(spawn_effect)
            .add_systems(Update, invoke_effect);
    }
}

#[derive(Serialize, Deserialize, Event)]
pub struct SpawnVfxBroadcast {
    pub msg: SpawnVfx,
}

#[derive(Serialize, Deserialize, Event, Clone)]
pub struct SpawnVfx {
    pub vfx: String,
    pub pos: Vec3,
}

pub struct VfxRuntimeData {
    effect: Handle<EffectComplex>,
    hotkey: Option<KeyCode>,
}

#[derive(Resource)]
pub struct BattlemapEffects {
    pub vfx_library: Handle<VfxLibrary>,
    pub(crate) vfx_data: HashMap<String, VfxRuntimeData>,
    pub(crate) textures: Vec<Handle<Image>>,
    pub loading_completed: bool,
}

fn load_effects(
    asset_server: Res<AssetServer>,
    vfx_library_assets: Res<Assets<VfxLibrary>>,
    mut battlemap_effects: ResMut<BattlemapEffects>,
) {
    let Some(vfx_library) = vfx_library_assets.get(&battlemap_effects.vfx_library) else {
        return;
    };
    for (effect_name, effect_config) in vfx_library.effects.iter() {
        let effect: Handle<EffectComplex> =
            asset_server.load(format!("vfx/{}.hanabi.ron", effect_name));
        battlemap_effects.vfx_data.insert(
            effect_name.clone(),
            VfxRuntimeData {
                effect,
                hotkey: effect_config.shortcut.clone(),
            },
        );
    }
    battlemap_effects.loading_completed = true;
}

fn setup_library(mut commands: Commands, asset_server: Res<AssetServer>) {
    let vfx_data: HashMap<String, VfxRuntimeData> = HashMap::new();

    let vfx_library: Handle<VfxLibrary> = asset_server.load("vfx.ron");

    commands.insert_resource(BattlemapEffects {
        textures: vec![
            asset_server.load("omagari/cloud.png"),
            asset_server.load("omagari/cloud2.png"),
            asset_server.load("omagari/spark1.png"),
            asset_server.load("omagari/spark2.png"),
            asset_server.load("omagari/spark3.png"),
            asset_server.load("omagari/glow1.png"),
            asset_server.load("omagari/splat1.png"),
        ],
        vfx_library,
        vfx_data,
        loading_completed: false,
    });
}

fn invoke_effect(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::hexmap::elements::MainCamera>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    res: Res<BattlemapEffects>,
) {
    if keys.pressed(KeyCode::AltLeft) {
        let hotkeys: Vec<KeyCode> = res
            .vfx_data
            .values()
            .filter(|vfx| vfx.hotkey.is_some())
            .map(|vfx| vfx.hotkey.unwrap())
            .collect();
        if keys.any_just_pressed(hotkeys) {
            if let Ok(window) = windows.single() {
                if let Ok((camera, cam_transform)) = cameras.single() {
                    let Some(ray) = window
                        .cursor_position()
                        .and_then(|p| camera.viewport_to_world(cam_transform, p).ok())
                    else {
                        return;
                    };
                    let Some(distance) =
                        ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Dir3::Y))
                    else {
                        return;
                    };
                    let point = ray.origin + ray.direction * distance;
                    for (vfx, vfx_data) in res.vfx_data.iter() {
                        if let Some(hotkey) = vfx_data.hotkey {
                            if keys.just_pressed(hotkey) {
                                commands.trigger(SpawnVfxBroadcast {
                                    msg: SpawnVfx {
                                        vfx: vfx.clone(),
                                        pos: Vec3::new(
                                            point.x,
                                            HEIGHT_OF_TOKENS + 0.99,
                                            point.z,
                                        ),
                                    },
                                });
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn spawn_effect(
    trigger: On<SpawnVfx>,
    mut commands: Commands,
    res: Res<BattlemapEffects>,
    effects: Res<Assets<EffectComplex>>,
) {
    effects
        .get(&res.vfx_data.get(&trigger.event().vfx).unwrap().effect)
        .unwrap()
        .spawn(
            &mut commands,
            &res.textures,
            trigger.event().pos,
            Some(Duration::from_secs(10)),
        );
}

#[derive(Clone, Debug, Deserialize)]
pub struct VfxConfig {
    pub shortcut: Option<KeyCode>,
}

#[derive(Asset, TypePath, Debug, Deserialize)]
pub struct VfxLibrary {
    pub effects: HashMap<String, VfxConfig>,
}

impl VfxLibrary {
    fn from_bytes(
        bytes: Vec<u8>,
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self, io::Error> {
        let vfx_library: VfxLibrary =
            from_bytes(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(vfx_library)
    }
}

#[derive(Default)]
struct VfxLibraryAssetLoader;

impl AssetLoader for VfxLibraryAssetLoader {
    type Asset = VfxLibrary;
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
        Ok(VfxLibrary::from_bytes(bytes, &mut load_context)?)
    }
}
