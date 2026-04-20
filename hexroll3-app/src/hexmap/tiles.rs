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

use crate::clients::controller::PostMapLoadedOp;
use crate::hexmap::data::*;
use crate::shared::LoadingState;
use crate::shared::labels::MapLabel;

use bevy::asset::uuid_handle;
use bevy::platform::collections::hash_map::HashMap;

use bevy::{
    asset::{RenderAssetUsages, load_internal_asset},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use hexx::HexLayout;
use hexx::PlaneMeshBuilder;

use super::elements::HexMapSpawnerState;
use super::themes::*;

pub struct TilesPlugin;
impl Plugin for TilesPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            TILE_SHADER_HANDLE,
            "shaders/tile_material.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            BG_SHADER_HANDLE,
            "shaders/background_material.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            MASKED_BG_SHADER_HANDLE,
            "shaders/simple_background_material.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            RIVER_SHADER_HANDLE,
            "shaders/river_material.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            TRAIL_SHADER_HANDLE,
            "shaders/trail_material.wgsl",
            Shader::from_wgsl
        );
        app.add_plugins(MaterialPlugin::<TileMaterial>::default())
            .add_plugins(MaterialPlugin::<BackgroundMaterial>::default())
            .add_plugins(MaterialPlugin::<SimpleBackgroundMaterial>::default())
            .add_plugins(MaterialPlugin::<RiverMaterial>::default())
            .add_plugins(MaterialPlugin::<TrailMaterial>::default())
            .register_type::<TileMaterial>()
            .register_type::<RiverMaterial>()
            .add_observer(load_hexmap_theme_resources)
            .add_systems(PreStartup, setup)
            .add_systems(OnEnter(LoadingState::Loading), load_theme_on_startup);
    }
}

#[derive(Debug, Resource, Clone)]
pub struct HexMapTileMaterials {
    pub unified_terrain_colors: HashMap<TerrainType, TerrainColors>,
    pub terrain_background_materials: HashMap<TerrainType, TerrainBackgroudOptions>,
    pub terrain_materials: HashMap<TerrainType, Handle<TileMaterial>>,
    pub terrain_rim_materials: HashMap<TerrainType, Handle<TileMaterial>>,
    pub terrain_feature_materials:
        HashMap<TerrainType, HashMap<HexFeature, Handle<TileMaterial>>>,
    pub tiles_z_offset: f32,
    pub use_rim_for_rivers: bool,
}

#[derive(Clone, Debug)]
pub struct TerrainBackgroudOptions {
    pub empty: Handle<BackgroundMaterial>,
    pub overlayer: Handle<BackgroundMaterial>,
}

#[derive(Resource)]
pub struct TileSetThemesMetadata {
    pub theme_names: Option<Vec<String>>,
    pub themes: Handle<TileSetThemes>,
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let tile_set_themes: Handle<TileSetThemes> = asset_server.load("terrain/tilesets.ron");
    commands.insert_resource(TileSetThemesMetadata {
        theme_names: None,
        themes: tile_set_themes,
    });
}

fn load_theme_on_startup(
    mut commands: Commands,
    mut themes_metadata: ResMut<TileSetThemesMetadata>,
    themes: Res<Assets<TileSetThemes>>,
) {
    let Some(themes) = themes.get(&themes_metadata.themes) else {
        return;
    };
    themes_metadata.theme_names = Some(themes.themes.keys().map(|v| v.to_string()).collect());
    commands.trigger(LoadHexmapTheme {
        theme: "oldschool".to_string(),
    });
}

#[derive(Event)]
pub struct LoadHexmapTheme {
    pub theme: String,
}

fn load_hexmap_theme_resources(
    trigger: On<LoadHexmapTheme>,
    mut commands: Commands,
    mut tile_materials: ResMut<Assets<TileMaterial>>,
    mut background_materials: ResMut<Assets<BackgroundMaterial>>,
    asset_server: Res<AssetServer>,
    all_labels: Query<Entity, With<MapLabel>>,
    curr_hex_map_spawner_state: Res<State<HexMapSpawnerState>>,
    mut next_hex_map_spawner_state: ResMut<NextState<HexMapSpawnerState>>,
    themes_metadata: Res<TileSetThemesMetadata>,
    themes: Res<Assets<TileSetThemes>>,
) {
    if *curr_hex_map_spawner_state == HexMapSpawnerState::Inhibited {
        return;
    }
    if *curr_hex_map_spawner_state == HexMapSpawnerState::Enabled {
        next_hex_map_spawner_state.set(HexMapSpawnerState::Inhibited);
    }
    let Some(themes) = themes.get(&themes_metadata.themes) else {
        return;
    };
    let Some(theme) = themes.themes.get(&trigger.event().theme) else {
        return;
    };
    commands.insert_resource(HexmapTheme {
        name: trigger.theme.clone(),
        def: theme.clone(),
    });
    // We despawn any existing tiles so not to preseve dangling material
    // handlers.
    all_labels.iter().for_each(|e| commands.entity(e).despawn());
    let unified_terrain_colors = theme.terrain_colors.clone();

    let terrain_background_materials = {
        let mut make_background = |color: Color| -> TerrainBackgroudOptions {
            TerrainBackgroudOptions {
                empty: background_materials.add(BackgroundMaterial {
                    base_color: color.into(),
                    layer_color: color.into(),
                }),
                overlayer: background_materials.add(BackgroundMaterial {
                    base_color: color.into(),
                    layer_color: color.with_alpha(0.0).into(),
                }),
            }
        };

        let terrain_background_materials: HashMap<_, _> = [
            (
                TerrainType::ForestHex,
                make_background(
                    unified_terrain_colors
                        .get(&TerrainType::ForestHex)
                        .unwrap()
                        .day,
                ),
            ),
            (
                TerrainType::MountainsHex,
                make_background(
                    unified_terrain_colors
                        .get(&TerrainType::MountainsHex)
                        .unwrap()
                        .day,
                ),
            ),
            (
                TerrainType::SwampsHex,
                make_background(
                    unified_terrain_colors
                        .get(&TerrainType::SwampsHex)
                        .unwrap()
                        .day,
                ),
            ),
            (
                TerrainType::DesertHex,
                make_background(
                    unified_terrain_colors
                        .get(&TerrainType::DesertHex)
                        .unwrap()
                        .day,
                ),
            ),
            (
                TerrainType::PlainsHex,
                make_background(
                    unified_terrain_colors
                        .get(&TerrainType::PlainsHex)
                        .unwrap()
                        .day,
                ),
            ),
            (
                TerrainType::JungleHex,
                make_background(
                    unified_terrain_colors
                        .get(&TerrainType::JungleHex)
                        .unwrap()
                        .day,
                ),
            ),
            (
                TerrainType::TundraHex,
                make_background(
                    unified_terrain_colors
                        .get(&TerrainType::TundraHex)
                        .unwrap()
                        .day,
                ),
            ),
            (
                TerrainType::OceanHex,
                make_background(
                    unified_terrain_colors
                        .get(&TerrainType::OceanHex)
                        .unwrap()
                        .day,
                ),
            ),
        ]
        .iter()
        .cloned()
        .collect();

        terrain_background_materials
    };

    let mut make_terrain = |texture_filename| -> Handle<TileMaterial> {
        let texture = asset_server.load(format!(
            "terrain/{}/{}.ktx2",
            trigger.theme, texture_filename
        ));
        tile_materials.add(TileMaterial {
            // base_color_texture: Some(texture.clone()),
            // emissive_texture: Some(texture.clone()),
            // emissive: color.into(),
            // alpha_mode: AlphaMode::Blend,
            array_texture: texture.clone(),
            mixer: Vec4::splat(1.0),
        })
    };

    let terrain_materials: HashMap<_, _> = [
        (TerrainType::ForestHex, make_terrain("forest")),
        (TerrainType::MountainsHex, make_terrain("mountains")),
        (TerrainType::SwampsHex, make_terrain("swamps")),
        (TerrainType::DesertHex, make_terrain("desert")),
        (TerrainType::PlainsHex, make_terrain("plains")),
        (TerrainType::JungleHex, make_terrain("jungle")),
        (TerrainType::TundraHex, make_terrain("tundra")),
        (TerrainType::OceanHex, make_terrain("empty")),
    ]
    .iter()
    .cloned()
    .collect();

    let terrain_rim_materials: HashMap<_, _> = [
        (TerrainType::ForestHex, make_terrain("rim-forest")),
        (TerrainType::MountainsHex, make_terrain("rim-mountains")),
        (TerrainType::SwampsHex, make_terrain("rim-swamps")),
        (TerrainType::DesertHex, make_terrain("rim-desert")),
        (TerrainType::PlainsHex, make_terrain("rim-plains")),
        (TerrainType::JungleHex, make_terrain("rim-jungle")),
        (TerrainType::TundraHex, make_terrain("rim-tundra")),
        (TerrainType::OceanHex, make_terrain("empty")),
    ]
    .iter()
    .cloned()
    .collect();

    let terrain_feature_materials: HashMap<TerrainType, HashMap<HexFeature, _>> = [
        (
            TerrainType::ForestHex,
            [
                (HexFeature::Dungeon, make_terrain("forest-dungeon")),
                (HexFeature::City, make_terrain("forest-city")),
                (HexFeature::Town, make_terrain("forest-town")),
                (HexFeature::Village, make_terrain("forest-village")),
                (HexFeature::Residency, make_terrain("forest-stronghold")),
                (HexFeature::Inn, make_terrain("forest-inn")),
            ]
            .iter()
            .cloned()
            .collect(),
        ),
        (
            TerrainType::MountainsHex,
            [
                (HexFeature::Dungeon, make_terrain("mountains-dungeon")),
                (HexFeature::City, make_terrain("mountains-city")),
                (HexFeature::Town, make_terrain("mountains-town")),
                (HexFeature::Village, make_terrain("mountains-village")),
                (HexFeature::Residency, make_terrain("mountains-stronghold")),
                (HexFeature::Inn, make_terrain("mountains-inn")),
            ]
            .iter()
            .cloned()
            .collect(),
        ),
        (
            TerrainType::SwampsHex,
            [
                (HexFeature::Dungeon, make_terrain("swamps-dungeon")),
                (HexFeature::City, make_terrain("swamps-city")),
                (HexFeature::Town, make_terrain("swamps-town")),
                (HexFeature::Village, make_terrain("swamps-village")),
                (HexFeature::Residency, make_terrain("swamps-stronghold")),
                (HexFeature::Inn, make_terrain("swamps-inn")),
            ]
            .iter()
            .cloned()
            .collect(),
        ),
        (
            TerrainType::DesertHex,
            [
                (HexFeature::Dungeon, make_terrain("desert-dungeon")),
                (HexFeature::City, make_terrain("desert-city")),
                (HexFeature::Town, make_terrain("desert-town")),
                (HexFeature::Village, make_terrain("desert-village")),
                (HexFeature::Residency, make_terrain("desert-stronghold")),
                (HexFeature::Inn, make_terrain("desert-inn")),
            ]
            .iter()
            .cloned()
            .collect(),
        ),
        (
            TerrainType::PlainsHex,
            [
                (HexFeature::Dungeon, make_terrain("plains-dungeon")),
                (HexFeature::City, make_terrain("plains-city")),
                (HexFeature::Town, make_terrain("plains-town")),
                (HexFeature::Village, make_terrain("plains-village")),
                (HexFeature::Residency, make_terrain("plains-stronghold")),
                (HexFeature::Inn, make_terrain("plains-inn")),
            ]
            .iter()
            .cloned()
            .collect(),
        ),
        (
            TerrainType::JungleHex,
            [
                (HexFeature::Dungeon, make_terrain("jungle-dungeon")),
                (HexFeature::City, make_terrain("jungle-city")),
                (HexFeature::Town, make_terrain("jungle-town")),
                (HexFeature::Village, make_terrain("jungle-village")),
                (HexFeature::Residency, make_terrain("jungle-stronghold")),
                (HexFeature::Inn, make_terrain("jungle-inn")),
            ]
            .iter()
            .cloned()
            .collect(),
        ),
        (
            TerrainType::TundraHex,
            [
                (HexFeature::Dungeon, make_terrain("tundra-dungeon")),
                (HexFeature::City, make_terrain("tundra-city")),
                (HexFeature::Town, make_terrain("tundra-town")),
                (HexFeature::Village, make_terrain("tundra-village")),
                (HexFeature::Residency, make_terrain("tundra-stronghold")),
                (HexFeature::Inn, make_terrain("tundra-inn")),
            ]
            .iter()
            .cloned()
            .collect(),
        ),
    ]
    .iter()
    .cloned()
    .collect();

    commands.insert_resource(HexMapTileMaterials {
        unified_terrain_colors,
        terrain_background_materials,
        terrain_materials,
        terrain_rim_materials,
        terrain_feature_materials,
        tiles_z_offset: theme.tile_offset,
        use_rim_for_rivers: theme.use_rim_for_rivers,
    });
    if *curr_hex_map_spawner_state != HexMapSpawnerState::Unready {
        commands.trigger(crate::clients::controller::RequestMapFromBackend {
            post_map_loaded_op: PostMapLoadedOp::InvalidateVisible,
        });
    }
}

pub fn hexagonal_plane(hex_layout: &HexLayout) -> Mesh {
    let mesh_info = PlaneMeshBuilder::new(hex_layout)
        .facing(Vec3::Y)
        .center_aligned()
        .build();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, mesh_info.vertices)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_info.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, mesh_info.uvs)
    .with_inserted_indices(Indices::U16(mesh_info.indices))
}

pub const TILE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("489e4567-e89b-12d3-a456-426614174000");
#[derive(Asset, bevy::render::render_resource::AsBindGroup, Debug, Clone, Reflect)]
pub struct TileMaterial {
    #[texture(0, dimension = "2d_array")]
    #[sampler(1)]
    pub array_texture: Handle<Image>,
    #[uniform(2)]
    pub mixer: Vec4,
}

impl Material for TileMaterial {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        TILE_SHADER_HANDLE.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

pub const BG_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("c2d46c3e-60f9-4c26-9c42-708b7ba0babd");
#[derive(Asset, TypePath, bevy::render::render_resource::AsBindGroup, Debug, Clone)]
pub struct BackgroundMaterial {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[uniform(1)]
    pub layer_color: LinearRgba,
}

impl Material for BackgroundMaterial {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        BG_SHADER_HANDLE.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

pub const MASKED_BG_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("e2d16d3e-60f9-2a9f-ac31-819a2caa129d");
#[derive(Asset, TypePath, bevy::render::render_resource::AsBindGroup, Debug, Clone)]
pub struct SimpleBackgroundMaterial {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[uniform(1)]
    pub layer_color: LinearRgba,
}

impl Material for SimpleBackgroundMaterial {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        MASKED_BG_SHADER_HANDLE.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

pub const RIVER_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("b32cdd4e-b6f9-4ac1-b1cf-d23e83f412ea");
#[derive(Asset, bevy::render::render_resource::AsBindGroup, Debug, Clone, Reflect)]
pub struct RiverMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub time: Vec4,
    #[uniform(2)]
    pub res: Vec4,

    pub alpha_mode: AlphaMode,
}

impl Material for RiverMaterial {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        RIVER_SHADER_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}

pub const TRAIL_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("f47ac10b-58cc-4372-a567-0e02b2c3d479");
#[derive(Asset, TypePath, bevy::render::render_resource::AsBindGroup, Debug, Clone)]
pub struct TrailMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub res: Vec4,
    pub alpha_mode: AlphaMode,
}

impl Material for TrailMaterial {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        TRAIL_SHADER_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}
