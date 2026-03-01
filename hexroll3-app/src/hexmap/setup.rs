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

// Hexmap core setup code
//     |
//     V
// +-------+     +-------+     +-------+
// | setup | --> | stage | --> | spawn |
// +-------+     +-------+     +-------+
//     ^             ^             ^
//     |             |             |
// [Startup]    [Map Loaded]   [Each Frame]
//
use bevy::prelude::*;

use bevy_rich_text3d::*;

use hexx::Hex;

use crate::{
    battlemaps::{BattlemapMaterial, DUNGEON_FOG_COLOR},
    hexmap::{
        curve_tiles::*, daynight::*, elements::*, grid::*, spawn::invalidate_hex, tiles::*,
    },
    shared::{
        geometry::*,
        layers::{HEIGHT_OF_HEX_MAP, HEIGHT_OF_SELECTION_HEX},
        vtt::*,
    },
};

#[derive(Component)]
pub struct HexMapRootMarker;

pub fn setup(
    mut commands: Commands,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut river_materials: ResMut<Assets<RiverMaterial>>,
    mut trail_materials: ResMut<Assets<TrailMaterial>>,
    mut hexgrid_materials: ResMut<Assets<HexMaterial>>,
    mut background_materials: ResMut<Assets<BackgroundMaterial>>,
    mut simple_background_materials: ResMut<Assets<SimpleBackgroundMaterial>>,
    mut battlemap_materials: ResMut<Assets<BattlemapMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut next_hex_map_spawner_state: ResMut<NextState<HexMapSpawnerState>>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Name::new("Hexgrid"),
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(100000.0, 100000.0)))),
        MeshMaterial3d(hexgrid_materials.add(HexMaterial {
            color: LinearRgba::new(0.0, 0.0, 0.0, 1.0),
            res: Vec4::new(0.01, 0.01, 0.04, 0.001),
            alpha_mode: AlphaMode::Blend,
        })),
        Pickable {
            should_block_lower: false,
            is_hoverable: false,
        },
        Transform::from_xyz(26.0, HEIGHT_OF_HEX_MAP + 0.03, 106.0),
    ));
    commands.spawn((
        Name::new("Hexmap"),
        HexMapRootMarker,
        Visibility::default(),
        HexMapTime::default(),
        Transform::from_xyz(0.0, HEIGHT_OF_HEX_MAP, 0.0),
    ));

    let layout = hexmap_layout();

    let curved_mesh_tile_set = build_curved_mesh_tile_set(&layout, &mut meshes);

    let water_material = simple_background_materials.add(SimpleBackgroundMaterial {
        base_color: Color::srgba(0.0, 0.8, 1.0, 1.0).into(),
        layer_color: Color::srgba(0.0, 0.8, 1.0, 1.0).into(),
    });
    let ocean_material = background_materials.add(BackgroundMaterial {
        base_color: Color::srgba(0.0, 0.8, 1.0, 1.0).into(),
        layer_color: Color::srgba(0.0, 0.8, 1.0, 1.0).into(),
    });
    let underworld_material = background_materials.add(BackgroundMaterial {
        base_color: DUNGEON_FOG_COLOR.into(),
        layer_color: DUNGEON_FOG_COLOR.into(),
    });
    let region_labels_material = standard_materials.add(StandardMaterial {
        base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..Default::default()
    });
    let realm_labels_material = standard_materials.add(StandardMaterial {
        base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..Default::default()
    });
    let pins_material = standard_materials.add(StandardMaterial {
        base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..Default::default()
    });
    let dungeon_labels_material = standard_materials.add(StandardMaterial {
        base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..Default::default()
    });
    let token_labels_material = standard_materials.add(StandardMaterial {
        base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..Default::default()
    });

    let river_tile_material = river_materials.add(RiverMaterial {
        color: Color::srgba(0.0, 0.8, 1.0, 1.0).into(),
        time: Vec4::splat(1.0),
        res: Vec4::new(100.0, 100.0, 0.0, 0.0),
        alpha_mode: AlphaMode::Blend,
    });

    let river_battlemap_material = river_materials.add(RiverMaterial {
        color: Color::srgba(0.0, 0.8, 1.0, 1.0).into(),
        time: Vec4::splat(1.0),
        res: Vec4::new(100.0, 100.0, 0.0, 0.0),
        alpha_mode: AlphaMode::Blend,
    });

    let trail_material = trail_materials.add(TrailMaterial {
        color: Color::srgba(0.0, 0.0, 0.0, 0.7).into(),
        res: Vec4::new(0.001, 0.001, 0.001, 0.001),
        alpha_mode: AlphaMode::Blend,
    });

    let selection_visible_material = standard_materials.add(StandardMaterial {
        unlit: true,
        base_color: Color::srgb_u8(255, 0, 0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let selection_hidden_material = standard_materials.add(StandardMaterial {
        unlit: true,
        base_color: Color::srgba_u8(255, 0, 0, 0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let hex_mask_material = standard_materials.add(StandardMaterial {
        base_color: Color::srgba_u8(0, 0, 0, 200),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    let polygon_points = make_polygon(&layout, &vec![Hex::new(0, 0)]);

    let temp: Vec<lyon::math::Point> = polygon_points
        .iter()
        .map(|v| lyon::math::Point::new(v.x, v.y))
        .collect();

    let selection_mesh = meshes.add(make_mesh_from_outline(&temp, 20.0));

    let battlemap_material = battlemap_materials.add(BattlemapMaterial::default());
    commands.spawn((
        Name::new("HexSelector"),
        SelectionEntity,
        Mesh3d(selection_mesh.clone()),
        MeshMaterial3d(selection_visible_material.clone()),
        Transform::from_xyz(0.0, HEIGHT_OF_SELECTION_HEX, 0.0),
    ));

    let coords_font = asset_server.load("fonts/oswald.ttf");

    let labels_parent = commands
        .spawn((
            Name::new("MapLabels"),
            MapLabels,
            Visibility::Inherited,
            Transform::default(),
        ))
        .id();

    commands.insert_resource(HexMapResources {
        mesh: meshes.add(hexagonal_plane(&layout)),
        layer_mesh: meshes.add(Plane3d::new(Vec3::Y, HEX_SIZE)),
        curved_mesh_tile_set,
        river_tile_materials: RiverTileMaterials {
            river_tile_material,
            river_battlemap_material,
        },
        trail_material,

        water_material,
        ocean_material,
        underworld_material,
        region_labels_material,
        realm_labels_material,
        pins_material,
        dungeon_labels_material,
        token_labels_material,

        selection_mesh,
        selection_visible_material,
        selection_hidden_material,

        hex_mask_material,

        battlemap_material,
        coords_font,
        labels_parent,
    });
    commands.insert_resource(VttData {
        node_name: "Referee".to_string(),
        mode: HexMapMode::RefereeViewing,
        ..default()
    });
    commands.insert_resource(HexMapData::default());

    let invalidate = commands.register_system(invalidate_hex);
    commands.insert_resource(HexEntityCallbacks { invalidate });
    next_hex_map_spawner_state.set(HexMapSpawnerState::Enabled);
}
