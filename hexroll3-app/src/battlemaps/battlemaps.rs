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

// Battlemaps plugin
//
// This modules manages battlemaps spawning, despawning, and other visibility
// parameters of battlemap entities such as labels and grids.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use hexx::Hex;

use crate::battlemaps::BattlemapsRuler;
use crate::hexmap::elements::DungeonUnderlayer;
use crate::shared::AppState;
use crate::shared::labels::RulerLabelMarker;
use crate::shared::settings::Config;
use crate::shared::widgets::buttons::ToggleResourceWrapper;
use crate::{
    audio::PlayBattlemapSound,
    battlemaps::drawing::BattlemapDrawingPlugin,
    hexmap::{
        HexMapTileMaterials,
        elements::{
            HexCoordsForFeature, HexMapData, HexUid, MainCamera, MapVisibilityController,
        },
        {ExpensiveHex, update_hex_map_tiles}, {HexFeature, TerrainType},
    },
    shared::{
        camera::CameraTweenTarget,
        labels::{AreaNumbersMarker, FadableLabel, TokenLabelMarker},
        vtt::{HexRevealState, VttData},
    },
};

use super::BattlemapFeatureUtils;
use super::{
    BattlemapDialProvider, BattlemapRequest, RequestCityOrTownFromBackend,
    RequestDungeonFromBackend, RequestVillageFromBackend, ruler::draw_ruler,
};

pub const DEFAULT_BATTLEMAP_COLOR: [f32; 4] = [0.5, 0.47, 0.45, 1.0];
pub const DUNGEON_FOG_COLOR: Color = Color::linear_rgb(0.016807375, 0.015208514, 0.015208514);

pub struct BattlemapsPlugin;

impl Plugin for BattlemapsPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            BATTLEMAP_SHADER_HANDLE,
            "battlemap.wgsl",
            Shader::from_wgsl
        );
        app
            // -->
            .add_plugins(super::dungeon::DungeonsPlugin)
            .add_plugins(super::caves::CavesPlugin)
            .add_plugins(super::settlement::SettlementPlugin)
            .add_plugins(super::city::CityPlugin)
            .add_plugins(super::village::VillagePlugin)
            .add_plugins(MaterialPlugin::<BattlemapMaterial>::default())
            .add_plugins(BattlemapDrawingPlugin)
            .add_plugins(super::effects::BattlemapEffectsPlugin) // Battlemaps
            .add_plugins(super::battlemap_dial::BattlemapDialPlugin)
            .register_type::<BattlemapMaterial>()
            .insert_resource(ToggleResourceWrapper { value:BattlemapsRuler::default() } )
            .add_systems(Update, draw_ruler.before(ruler_label_zoom_fader))
            .add_systems(Update, (area_labels_zoom_fader, token_labels_zoom_fader, ruler_label_zoom_fader))
            .add_systems(Update, (
                trigger_battlemaps_requests_when_visible.run_if(in_state(AppState::Live)),
                despawn_battlemaps_when_timer_expire).after(update_hex_map_tiles))
            .add_systems(Update, schedule_despawn_battlemaps_when_out_of_range.after(trigger_battlemaps_requests_when_visible))
            .add_systems(Update, battlemap_grid_aliasing_fader)
            .add_systems(PostUpdate, elevate_underlayers)
            // <--
        ;
    }
}

#[derive(bevy::render::render_resource::ShaderType, Debug, Clone, Reflect)]
pub struct BattlemapMaterialControls {
    pub zoom_factor: f32,
    pub grid_mix: f32,
    pub blend: f32,
    pub scale: f32,
}

impl Default for BattlemapMaterialControls {
    fn default() -> Self {
        Self {
            zoom_factor: 1.0,
            grid_mix: 1.0,
            blend: 0.0,
            scale: 120.0,
        }
    }
}

const BATTLEMAP_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("d3b07404-75f1-4f0d-b45b-f26d52bbf063");

#[derive(Asset, Reflect, bevy::render::render_resource::AsBindGroup, Debug, Clone)]
pub struct BattlemapMaterial {
    #[uniform(0)]
    pub controls: BattlemapMaterialControls,
    #[uniform(1)]
    pub color: Vec4,
    #[uniform(2)]
    pub offset: Vec4,
}

impl Default for BattlemapMaterial {
    fn default() -> Self {
        Self {
            controls: BattlemapMaterialControls::default(),
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            offset: Vec4::ZERO,
        }
    }
}

impl Material for BattlemapMaterial {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        BATTLEMAP_SHADER_HANDLE.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

fn trigger_battlemaps_requests_when_visible(
    // TODO: we can potential optimize this query with a marker indicating hexes with battlemaps?
    hexes: Query<
        (
            Entity,
            &HexFeature,
            &HexUid,
            &HexCoordsForFeature,
            &TerrainType,
            &ChildOf,
            Option<&BattlemapDespawnTimer>,
        ),
        Without<SubMapMarker>,
    >,
    mut commands: Commands,
    visibility_controller: Res<MapVisibilityController>,
    map_resources: Res<crate::hexmap::elements::HexMapResources>,
    vtt_data: Res<VttData>,
    tiles: Res<HexMapTileMaterials>,
    panorbit: Single<(&bevy_editor_cam::prelude::EditorCam, &CameraTweenTarget)>,
    map_data: Res<HexMapData>,
) {
    let (cam, cam_tween_target) = panorbit.into_inner();
    // let cam_moving = panorbit.into_inner().is_actively_controlled();
    let cam_moving = cam.current_motion.is_moving();
    if visibility_controller.are_dungeons_and_settlements_visible() && !cam_moving {
        for (hex, hex_type, hex_uid, hex_coords, hex_terrain, child_of, maybe_timer) in
            hexes.iter()
        {
            // NOTE: When the camera is automatically tweening from place to place,
            // loading random battlemaps along the way will create a stutter,
            // so we only allow loading battlemaps related to the target hex.
            if let Some(cam_tween) = &cam_tween_target.target_hex_uid
                && cam_tween != &hex_uid.uid
            {
                continue;
            }
            if !(hex_coords.hex.x > map_data.cmin.x
                && hex_coords.hex.x < map_data.cmax.x
                && hex_coords.hex.y < map_data.cmin.y
                && hex_coords.hex.y > map_data.cmax.y)
                // NOTE: this check is needed to ensure dungeons will spawn
                // when revealed when the map is zoomed in closer than the
                // above check will allow.
                && map_data.cmax.x - map_data.cmin.x > 2
            {
                continue;
            }

            if let Ok(mut entity) = commands.get_entity(hex) {
                if maybe_timer.is_some() {
                    entity.try_insert(SubMapMarker);
                    entity.try_remove::<BattlemapDespawnTimer>();
                    continue;
                }
            }
            let is_revealed =
                vtt_data.revealed.get(&hex_coords.hex) == Some(&HexRevealState::Full);
            let is_unrevealed_dungeon = *hex_type == HexFeature::Dungeon && !is_revealed;
            let mut spawn_empty_battlemap = false;
            let mut is_expensive_hex = false;
            let mut height_offset = 0.0;
            // NOTE: the following check is to ensure partially revealed hexes show a wilderness
            // battlemap for players
            if vtt_data.is_player() && !is_revealed {
                spawn_empty_battlemap = true;
            } else {
                if *hex_type == HexFeature::Dungeon {
                    if let Ok(mut entity) = commands.get_entity(hex) {
                        entity.try_insert(SubMapMarker);
                        entity.try_insert(DungeonFeatureContainer);
                    }
                    let uid = hex_uid.uid.clone();
                    is_expensive_hex = true;
                    commands.trigger(RequestDungeonFromBackend(BattlemapRequest { uid, hex }))
                } else if *hex_type == HexFeature::City || *hex_type == HexFeature::Town {
                    if let Ok(mut entity) = commands.get_entity(hex) {
                        entity.try_insert(SubMapMarker);
                    }
                    let uid = hex_uid.uid.clone();
                    spawn_empty_battlemap = true;
                    height_offset = 11.0;
                    is_expensive_hex = true;
                    commands
                        .trigger(RequestCityOrTownFromBackend(BattlemapRequest { uid, hex }))
                } else if *hex_type == HexFeature::Village {
                    if let Ok(mut entity) = commands.get_entity(hex) {
                        entity.try_insert(SubMapMarker);
                    }
                    let uid = hex_uid.uid.clone();
                    spawn_empty_battlemap = true;
                    height_offset = 11.0;
                    is_expensive_hex = true;
                    commands.trigger(RequestVillageFromBackend(BattlemapRequest { uid, hex }))
                } else if visibility_controller.are_battlemaps_visible()
                    || is_unrevealed_dungeon
                {
                    spawn_empty_battlemap = true;
                    height_offset = 5.0;
                }
            }
            if spawn_empty_battlemap || is_unrevealed_dungeon {
                // NOTE: Spawn empty battlemap
                let mut parent_node = commands.spawn_empty();
                let parent_node_id = parent_node.id();
                parent_node
                    .insert(Mesh3d(map_resources.mesh.clone()))
                    .insert(MeshMaterial3d(map_resources.battlemap_material.clone()))
                    // NOTE: this will block the dial from showing - consider a different approach!
                    // .insert(Pickable {
                    //     should_block_lower: false,
                    //     is_hoverable: false,
                    // })
                    .insert(Name::new("TerrainBattleMap"))
                    .insert(Visibility::default())
                    .insert(Transform::from_xyz(
                        0.0,
                        if is_unrevealed_dungeon {
                            // -3.1
                            -4.2
                        } else {
                            crate::shared::layers::HEIGHT_OF_BATTLEMAP_ON_FEATURE
                                - height_offset
                        },
                        0.0,
                    ))
                    .battlemap_dial_provider();

                if is_unrevealed_dungeon {
                    parent_node.with_child((
                        Mesh3d(map_resources.mesh.clone()),
                        MeshMaterial3d(
                            tiles
                                .terrain_background_materials
                                .get(hex_terrain)
                                .unwrap()
                                .empty
                                .clone(),
                        ),
                        Transform::from_xyz(
                            0.0,
                            if is_unrevealed_dungeon { -1.0 } else { -10.0 },
                            0.0,
                        ),
                    ));
                }
                // FIXME: if this is a battlemap on top of a non-revealed dungeon,
                // do we need to insert another HexFeatureMap??
                if let Ok(mut entity) = commands.get_entity(hex) {
                    entity.try_insert(SubMapMarker);
                    entity.try_remove::<BattlemapDespawnTimer>();
                    entity.add_child(parent_node_id);
                    if is_expensive_hex {
                        commands.entity(child_of.0).try_insert(ExpensiveHex);
                    }
                } else {
                    commands.entity(parent_node_id).despawn();
                }

                commands.trigger(PlayBattlemapSound {
                    hex_entity: hex,
                    biome: hex_terrain.clone(),
                    feature: hex_type.clone(),
                    is_revealed,
                });
            }
        }
    }
}

#[derive(Component)]
pub struct PlayerBattlemapEntity;

#[derive(Component)]
pub struct RefereeBattlemapEntity;

#[derive(Component)]
pub struct DungeonFeatureContainer;

#[derive(Component)]
pub struct BattlemapDespawnTimer(f32);

impl BattlemapDespawnTimer {
    pub fn tick(&self, delta: f32) -> Self {
        BattlemapDespawnTimer(self.0 - delta)
    }
    pub fn is_done(&self) -> bool {
        self.0 <= 0.0
    }
}

fn schedule_despawn_battlemaps_when_out_of_range(
    mut commands: Commands,
    visibility_controller: Res<MapVisibilityController>,
    live_maps: Query<Entity, With<SubMapMarker>>,
) {
    // TODO: Consider adding another check to despawn when out of view bounds
    if visibility_controller.are_dungeons_hidden() {
        for m in live_maps.iter() {
            if let Ok(mut e) = commands.get_entity(m) {
                e.remove::<SubMapMarker>();
                e.insert(BattlemapDespawnTimer(rand::Rng::gen_range(
                    &mut rand::thread_rng(),
                    1.0..=20.0,
                )));
            }
        }
    }
}

fn despawn_battlemaps_when_timer_expire(
    mut commands: Commands,
    live_maps: Query<(Entity, &BattlemapDespawnTimer, &ChildOf), Without<SubMapMarker>>,
    time: Res<Time>,
) {
    for (m, timer, child_of) in live_maps.iter() {
        if timer.is_done() {
            commands.entity(child_of.0).try_remove::<ExpensiveHex>();
        }
        if let Ok(mut e) = commands.get_entity(m) {
            if timer.is_done() {
                e.remove::<BattlemapDespawnTimer>();
                e.remove::<BattlemapLoadedAndConstructed>();
                // FIXME: should be try_...
                // e.despawn_related::<Children>();
                e.queue_handled(
                    |mut entity: EntityWorldMut| {
                        entity.despawn_related::<Children>();
                    },
                    bevy::ecs::error::ignore,
                );
                return;
            } else {
                e.insert(timer.tick(time.delta_secs()));
            }
        }
    }
}

struct LabelFaderThresholds {
    opacity_distance_zero: f32,
    opacity_distance_one: f32,
    opacity_distance_max: f32,
    mipmap_distance_max: f32,
    mipmap_distance_near_max: f32,
}

fn generic_labels_zoom_fader<T: Component + FadableLabel>(
    labels_material: &Handle<StandardMaterial>,
    thresholds: LabelFaderThresholds,
    cam: Query<&Projection, With<MainCamera>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut labels: Query<(&mut bevy_rich_text3d::Text3dStyling, &mut Transform, &T)>,
) {
    for cam in cam.iter() {
        if let Projection::Orthographic(proj) = cam {
            let alpha_value = if proj.scale > thresholds.opacity_distance_zero {
                0.0
            } else if proj.scale < thresholds.opacity_distance_max {
                0.0
            } else if proj.scale < thresholds.opacity_distance_one {
                1.0
            } else {
                1.0 - (proj.scale - thresholds.opacity_distance_one)
                    / (thresholds.opacity_distance_zero - thresholds.opacity_distance_one)
            };
            if let Some(mat) = materials.get_mut(labels_material) {
                mat.base_color.set_alpha(alpha_value);
            }

            let scale = if proj.scale > thresholds.mipmap_distance_max {
                12.0
            } else if proj.scale > thresholds.mipmap_distance_near_max {
                4.0
            } else {
                1.0
            };

            for (mut s, mut t, m) in labels.iter_mut() {
                // NOTE: not checking for redundant assignment causes bevy_rich_text3d
                // to run a costly layout code in each rendered frame!
                if s.size != (m.size() / scale) {
                    s.size = m.size() / scale;
                    t.scale = Vec3::splat(m.scale() * scale);
                }
            }
        }
    }
}

fn area_labels_zoom_fader(
    cam: Query<&Projection, With<MainCamera>>,
    materials: ResMut<Assets<StandardMaterial>>,
    map_resources: Res<crate::hexmap::elements::HexMapResources>,
    labels: Query<(
        &mut bevy_rich_text3d::Text3dStyling,
        &mut Transform,
        &AreaNumbersMarker,
    )>,
) {
    let thresholds = LabelFaderThresholds {
        opacity_distance_zero: 0.3,
        opacity_distance_one: 0.1,
        opacity_distance_max: 0.01,
        mipmap_distance_max: 0.1,
        mipmap_distance_near_max: 0.03,
    };
    generic_labels_zoom_fader(
        &map_resources.dungeon_labels_material,
        thresholds,
        cam,
        materials,
        labels,
    );
}

fn token_labels_zoom_fader(
    cam: Query<&Projection, With<MainCamera>>,
    materials: ResMut<Assets<StandardMaterial>>,
    map_resources: Res<crate::hexmap::elements::HexMapResources>,
    labels: Query<(
        &mut bevy_rich_text3d::Text3dStyling,
        &mut Transform,
        &TokenLabelMarker,
    )>,
    app_config: Res<Config>,
) {
    let labels_scale = app_config.tokens_config.label_font_scale;
    let thresholds = LabelFaderThresholds {
        opacity_distance_zero: 0.03 * labels_scale,
        opacity_distance_one: 0.01 * labels_scale,
        opacity_distance_max: 0.0,
        mipmap_distance_max: 0.015 * labels_scale,
        mipmap_distance_near_max: 0.006 * labels_scale,
    };
    generic_labels_zoom_fader(
        &map_resources.token_labels_material,
        thresholds,
        cam,
        materials,
        labels,
    );
}

fn ruler_label_zoom_fader(
    cam: Query<&Projection, With<MainCamera>>,
    materials: ResMut<Assets<StandardMaterial>>,
    map_resources: Res<crate::hexmap::elements::HexMapResources>,
    labels: Query<(
        &mut bevy_rich_text3d::Text3dStyling,
        &mut Transform,
        &RulerLabelMarker,
    )>,
    app_config: Res<Config>,
) {
    let label_scale = app_config.ruler_config.label_font_scale;
    let thresholds = LabelFaderThresholds {
        opacity_distance_zero: 0.03 * label_scale,
        opacity_distance_one: 0.015 * label_scale,
        opacity_distance_max: 0.0,
        mipmap_distance_max: 0.03 * label_scale,
        mipmap_distance_near_max: 0.01 * label_scale,
    };
    generic_labels_zoom_fader(
        &map_resources.token_labels_material,
        thresholds,
        cam,
        materials,
        labels,
    );
}

fn battlemap_grid_aliasing_fader(
    mut mats: ResMut<Assets<BattlemapMaterial>>,
    cam: Query<&Projection, With<MainCamera>>,
) {
    for cam in cam.iter() {
        if let Projection::Orthographic(proj) = cam {
            let blur_level = 0.4;
            let grid_blur_factor = if proj.scale >= 0.13 {
                blur_level
            } else if proj.scale <= 0.015 {
                0.0
            } else {
                let a = (proj.scale - 0.015) / (0.13 - 0.015);
                (1.0 - (1.0 - a).powf(4.0)) * blur_level
            };
            for m in mats.iter_mut() {
                m.1.controls.zoom_factor = grid_blur_factor;
            }
        }
    }
}

#[derive(Component)]
pub struct SubMapMarker;

#[derive(Component)]
pub struct BattlemapLoadedAndConstructed;

impl BattlemapFeatureUtils for EntityCommands<'_> {
    fn invalidate_battlemap_in_hex_feature(&mut self) {
        self.despawn_related::<Children>()
            // NOTE: we now must always remember to remove the despawn
            // timer as well.
            // FIXME: can we encapsulate these two removes into a single
            // logical operation?
            .remove::<BattlemapDespawnTimer>()
            .try_remove::<BattlemapLoadedAndConstructed>()
            .remove::<SubMapMarker>();
    }
    fn mark_battlemap_has_valid_state(&mut self) {
        self.try_insert(SubMapMarker);
    }
    fn mark_battlemap_as_ready(&mut self) {
        self.try_insert(SubMapMarker)
            .try_insert(BattlemapLoadedAndConstructed);
    }
    fn reset_battlemap_loading_state(&mut self) {
        self.try_remove::<SubMapMarker>();
    }
}

pub fn elevate_underlayers(
    mut commands: Commands,
    mut underlayers: Query<(Entity, &mut DungeonUnderlayer)>,
    battlemaps: Query<(Entity, &HexCoordsForFeature), With<DungeonFeatureContainer>>,
    loaded: Query<&BattlemapLoadedAndConstructed>,
) {
    if underlayers.is_empty() {
        return;
    }
    let mut markers: HashMap<Hex, bool> = HashMap::new();
    for (battlemap_entity, battlemap_coords) in battlemaps.iter() {
        markers.insert(battlemap_coords.hex, loaded.contains(battlemap_entity));
    }
    for (underlayer_entity, mut underlayer_coords) in underlayers.iter_mut() {
        if let Some(loaded) = markers.get(&underlayer_coords.hex) {
            if *loaded {
                underlayer_coords.elevation_change_delay_in_frames -= 1;
                if underlayer_coords.elevation_change_delay_in_frames < 0 {
                    underlayer_coords.elevation_change_delay_in_frames = 0;
                    commands
                        .entity(underlayer_entity)
                        .try_insert(Transform::from_xyz(0.0, -15.0, 0.0));
                }
            } else {
                underlayer_coords.elevation_change_delay_in_frames = 5;
                commands
                    .entity(underlayer_entity)
                    .try_insert(Transform::from_xyz(0.0, -1.0, 0.0));
            }
        }
    }
}
