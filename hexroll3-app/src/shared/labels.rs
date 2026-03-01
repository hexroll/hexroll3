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

// Labels spawning and despawning
use bevy::{anti_alias::fxaa::Fxaa, prelude::*};

use crate::{
    hexmap::elements::{HexMapData, HexMapResources, HexMapSpawnerState, MainCamera},
    shared::layers::{HEIGHT_OF_REALM_LABELS, HEIGHT_OF_REGION_LABELS},
};

use bevy_rich_text3d::*;

use super::{
    settings::AppSettings,
    spawnq::{MustBeChildOf, SpawnQueue},
    vtt::VttData,
};
pub struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LoadFonts {
            font_embedded: vec![include_bytes!("../../assets/fonts/eczar.ttf")],
            ..Default::default()
        });
        app.add_plugins(Text3dPlugin {
            default_atlas_dimension: (512, 512),
            ..default()
        })
        .add_observer(despawn_labels)
        .add_systems(Update, labels_visibility)
        .add_systems(Update, labels_position)
        .add_systems(
            Update,
            spawn_labels.run_if(in_state(HexMapSpawnerState::Enabled)),
        )
        .add_systems(PostUpdate, show_hidden_labels);
    }
}

#[derive(Event)]
pub struct SpawnLabels;

#[derive(Event)]
pub struct DespawnLabels;

#[derive(Component)]
pub enum MapLabel {
    // TODO: Settlement,
    Region,
    Realm,
}

fn despawn_labels(
    _trigger: On<DespawnLabels>,
    mut commands: Commands,
    query: Query<Entity, With<MapLabel>>,
    mut map_data: ResMut<HexMapData>,
) {
    for e in query.iter() {
        commands.entity(e).despawn();
    }
    map_data.realm_labels.iter_mut().for_each(|l| l.reset());
    map_data.region_labels.iter_mut().for_each(|l| l.reset());
}

fn spawn_labels(
    settings: Res<AppSettings>,
    mut commands: Commands,
    mut map_data: ResMut<HexMapData>,
    assets: Res<HexMapResources>,
    vtt_data: Res<VttData>,
) {
    if !settings.labels_mode.labels_visible() {
        return;
    }
    if vtt_data.is_player() {
        return;
    }
    let collection: Vec<_> = map_data
        .region_labels
        .iter_mut()
        .filter(|lazy_spawner| lazy_spawner.is_not_spawned())
        .take(15)
        .flat_map(|lazy_spawner| {
            lazy_spawner.set_spawned();
            let (label, pos, size_ratio) = &lazy_spawner.value;
            let label: String = label.split_whitespace().collect::<Vec<&str>>().join("\n");
            vec![(
                MapLabel::Region,
                Name::new(label.clone()),
                Text3d::new(label.clone()),
                Text3dBounds { width: 800.0 },
                Text3dStyling {
                    font: "Eczar".into(),
                    size: 128.,
                    align: TextAlign::Center,
                    color: Srgba::new(0.95, 0.92, 0.87, 1.),
                    stroke: Some(std::num::NonZero::new(20).unwrap()),
                    stroke_color: Srgba::new(0.2, 0.2, 0.2, 0.8),
                    stroke_join: StrokeJoin::Round,
                    ..Default::default()
                },
                Mesh3d::default(),
                MeshMaterial3d(assets.region_labels_material.clone()),
                Transform::from_xyz(pos.x, HEIGHT_OF_REALM_LABELS, pos.y)
                    .with_scale(Vec3::splat(4.0 + size_ratio * 2.0) * 0.15)
                    .looking_at(Vec3::new(pos.x, 0.0, pos.y), Dir3::NEG_Z),
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
                ChildOf(assets.labels_parent.clone()),
            )]
        })
        .collect();

    commands.spawn_batch(collection);
    let collection: Vec<_> = map_data
        .realm_labels
        .iter_mut()
        .filter(|lazy_spawner| lazy_spawner.is_not_spawned())
        .take(15)
        .flat_map(|lazy_spawner| {
            lazy_spawner.set_spawned();
            let (label, pos, size_ratio) = &lazy_spawner.value;
            let label: String = label.split_whitespace().collect::<Vec<&str>>().join("\n");
            vec![(
                MapLabel::Realm,
                Name::new(label.clone()),
                Text3d::new(label.clone()),
                Text3dBounds { width: 800.0 },
                Text3dStyling {
                    size: 128.,
                    font: "Eczar".into(),
                    align: TextAlign::Center,
                    color: Srgba::new(0.95, 0.92, 0.87, 1.),
                    stroke: Some(std::num::NonZero::new(20).unwrap()),
                    stroke_color: Srgba::new(0.2, 0.2, 0.2, 0.8),
                    stroke_join: StrokeJoin::Round,
                    ..Default::default()
                },
                Mesh3d::default(),
                MeshMaterial3d(assets.realm_labels_material.clone()),
                Transform::from_xyz(pos.x, HEIGHT_OF_REGION_LABELS, pos.y)
                    .with_scale(Vec3::splat(12.0 + size_ratio * (20.0 - 12.0)) * 0.15)
                    .looking_at(Vec3::new(pos.x, 0.0, pos.y), Dir3::NEG_Z),
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
                ChildOf(assets.labels_parent.clone()),
            )]
        })
        .collect();
    commands.spawn_batch(collection);
}

const REGION_LABEL_MIN_RANGE: f32 = 0.416;
const REGION_LABEL_MIN_START: f32 = 0.928;
const REGION_LABEL_MAX_START: f32 = 2.083;
const REGION_LABEL_MAX_RANGE: f32 = 3.472;

const REALM_LABEL_MIN_RANGE: f32 = 2.777;
const REALM_LABEL_MIN_START: f32 = 4.166;
const REALM_LABEL_MAX_START: f32 = 5.555;
const REALM_LABEL_MAX_RANGE: f32 = 10.416;

fn labels_visibility(
    camera_projection: Single<&Projection, With<MainCamera>>,
    mut camera_fxaa: Single<&mut Fxaa, With<MainCamera>>,
    assets: Res<HexMapResources>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if let Projection::Orthographic(proj) = *camera_projection {
        camera_fxaa.enabled = proj.scale > REGION_LABEL_MIN_RANGE;

        let region_labels_opacity = if proj.scale < REGION_LABEL_MIN_START {
            ((REGION_LABEL_MIN_START - proj.scale) / REGION_LABEL_MIN_RANGE).clamp(0.0, 1.0)
        } else {
            ((proj.scale - REGION_LABEL_MAX_START) / REGION_LABEL_MAX_RANGE).clamp(0.0, 1.0)
        };

        let realm_labels_opacity = if proj.scale < REALM_LABEL_MIN_START {
            ((REALM_LABEL_MIN_START - proj.scale) / REALM_LABEL_MIN_RANGE).clamp(0.0, 1.0)
        } else {
            ((proj.scale - REALM_LABEL_MAX_START) / REALM_LABEL_MAX_RANGE).clamp(0.0, 1.0)
        };

        materials
            .get_mut(&assets.region_labels_material)
            .unwrap()
            .base_color
            .set_alpha(1.0 - region_labels_opacity);
        materials
            .get_mut(&assets.realm_labels_material)
            .unwrap()
            .base_color
            .set_alpha(1.0 - realm_labels_opacity);
    }
}

// NOTE: This component and system are needed to prevent a glitch when
// revealing dungeons - where labels are showing too big for a single frame.
// Using this system, we create the labels hidden, wait 0.2 seconds
// and then unhide them.
#[derive(Component)]
struct HiddenAreaNumber(f32);

fn show_hidden_labels(
    mut commands: Commands,
    numbers: Query<(Entity, &HiddenAreaNumber)>,
    time: Res<Time>,
) {
    for (e, t) in numbers.iter() {
        if t.0 > 0.0 {
            commands
                .entity(e)
                .try_remove::<HiddenAreaNumber>()
                .try_insert(HiddenAreaNumber(t.0 - time.delta_secs()));
        } else {
            commands
                .entity(e)
                .try_insert(Visibility::Inherited)
                .try_remove::<HiddenAreaNumber>();
        }
    }
}

pub fn spawn_area_labels(
    q: &mut ResMut<SpawnQueue>,
    parent_node: Entity,
    is_player: bool,
    l: String,
    scale: f32,
    m: Handle<StandardMaterial>,
    x: f32,
    y: f32,
) {
    q.queue(move |c| {
        if is_player {
            return;
        }
        if let Ok(mut e) = c.get_entity(parent_node) {
            e.with_children(|c| {
                c.spawn((
                    MustBeChildOf,
                    crate::battlemaps::RefereeBattlemapEntity,
                    AreaNumbersMarker { size: 128., scale },
                    bevy_rich_text3d::Text3d::new(l.clone()),
                    bevy_rich_text3d::Text3dBounds { width: 800.0 },
                    bevy_rich_text3d::Text3dStyling {
                        size: 128. / 12.0,
                        font: "Eczar".into(),
                        align: bevy_rich_text3d::TextAlign::Center,
                        color: Srgba::new(0.95, 0.92, 0.87, 1.),
                        stroke: Some(std::num::NonZero::new(40).unwrap()),
                        stroke_color: Srgba::new(0.2, 0.2, 0.2, 1.0),
                        stroke_join: StrokeJoin::Round,
                        ..Default::default()
                    },
                    Mesh3d::default(),
                    MeshMaterial3d(m.clone()),
                    Visibility::Hidden,
                    HiddenAreaNumber(0.2),
                    Transform::from_xyz(
                        x,
                        crate::shared::layers::HEIGHT_OF_AREA_NUMBERS_ON_BATTLEMAP,
                        y,
                    )
                    .with_scale(Vec3::splat(scale * 12.0))
                    .looking_at(Vec3::new(x, 0.0, y), Dir3::NEG_Z),
                    Pickable {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                ));
            });
        }
    });
}

#[derive(Component)]
pub struct LabelToken(Entity);

#[derive(Component)]
pub struct TokenLabelOffset(pub f32);

#[derive(Component)]
pub struct TokenLabel {
    pub label_entity: Entity,
}

pub trait FadableLabel {
    fn size(&self) -> f32;
    fn scale(&self) -> f32;
}

macro_rules! define_fadable_label {
    ($name:ident) => {
        #[derive(Component)]
        pub struct $name {
            pub size: f32,
            pub scale: f32,
        }

        impl FadableLabel for $name {
            fn size(&self) -> f32 {
                self.size
            }

            fn scale(&self) -> f32 {
                self.scale
            }
        }
    };
}

define_fadable_label!(TokenLabelMarker);
define_fadable_label!(RulerLabelMarker);
define_fadable_label!(AreaNumbersMarker);

pub fn spawn_token_labels(
    spawn_queue: &mut ResMut<SpawnQueue>,
    token_entity: Entity,
    label_text: String,
    scale: f32,
    labels_material: Handle<StandardMaterial>,
    x: f32,
    y: f32,
) {
    spawn_queue.queue(move |c| {
        let label_entity = c
            .spawn((
                Name::new(label_text.clone()),
                LabelToken(token_entity),
                Transform::from_xyz(
                    x,
                    crate::shared::layers::HEIGHT_OF_AREA_NUMBERS_ON_BATTLEMAP,
                    y,
                )
                .looking_at(Vec3::new(x, 0.0, y), Dir3::NEG_Z),
            ))
            .with_children(|c| {
                c.spawn((
                    TokenLabelMarker {
                        size: 128.,
                        scale: scale * 0.0015,
                    },
                    bevy_rich_text3d::Text3d::new(label_text.clone()),
                    bevy_rich_text3d::Text3dBounds { width: 800.0 },
                    bevy_rich_text3d::Text3dStyling {
                        size: 1., // Using 1 will force a size fader recalc
                        font: "Eczar".into(),
                        align: bevy_rich_text3d::TextAlign::Center,
                        color: Srgba::new(0.2, 0.2, 0.2, 1.0),
                        ..Default::default()
                    },
                    Mesh3d::default(),
                    MeshMaterial3d(labels_material),
                    Transform::from_xyz(0.0, -0.0, 3.0).with_scale(Vec3::splat(scale)),
                ));
            })
            .id();
        c.entity(token_entity).insert(TokenLabel { label_entity });
    });
}

fn labels_position(
    mut labels: Query<(&mut Transform, &LabelToken)>,
    other: Query<(&GlobalTransform, &TokenLabelOffset), Without<LabelToken>>,
) {
    for (mut gt, label) in labels.iter_mut() {
        let token_entity = label.0;
        if let Ok((token_pos, offset_data)) = other.get(token_entity) {
            gt.translation.x = token_pos.translation().x;
            gt.translation.z =
                token_pos.translation().z + (offset_data.0 * token_pos.scale().x) + 0.1;
        }
    }
}
