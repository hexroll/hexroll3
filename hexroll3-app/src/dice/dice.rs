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

use core::f32;
use std::{collections::VecDeque, time::Duration};

use bevy::{
    anti_alias::fxaa::Sensitivity,
    asset::{AssetLoader, RenderAssetUsages, load_internal_asset, uuid_handle},
    camera::visibility::RenderLayers,
    gltf::GltfMaterialName,
    image::ImageLoaderSettings,
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        view::Hdr,
    },
    scene::SceneInstanceReady,
    shader::ShaderRef,
    window::PrimaryWindow,
};

use avian3d::{ancestor_marker::AncestorMarker, prelude::*};

use rand::{Rng, seq::SliceRandom};

use crate::shared::{
    AppState,
    input::InputMode,
    layers::{
        HexrollPhysicsLayer, RENDER_LAYER_DICE, RENDER_LAYER_DICE_SHADOW,
        RENDER_LAYER_TRANSLUCENT_DICE,
    },
    widgets::buttons::{SwitchValue, ToggleResourceWrapper},
};

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::{DiceRollHelpers, DiceRollResolved, RollDice};

const DICE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("123e4567-e89b-12d3-a456-426614174000");

#[derive(Serialize, Deserialize, Event, Clone)]
pub struct DiceRoll {
    pub results: HashMap<DiceType, Vec<i32>>,
}

impl DiceRollHelpers for DiceRoll {
    fn total(&self) -> i32 {
        self.results.iter().flat_map(|(_, values)| values).sum()
    }
    fn to_strings(&self) -> (Vec<String>, Vec<String>) {
        let mut terms = Vec::new();
        let mut details = Vec::new();

        for (dice, results) in &self.results {
            let count = results.len();
            if count > 0 {
                if *dice != DiceType::MODIFIER {
                    terms.push(format!("{}x{:?}", count, dice));
                }
                for &value in results {
                    details.push(value.to_string());
                }
            }
        }
        (terms, details)
    }
}

#[derive(bevy::render::render_resource::ShaderType, Debug, Clone, Reflect)]
struct EmissionSetup {
    emission_factor: f32,
    diffuse_to_emission_factor: f32,
    _padding1: f32,
    _padding2: f32,
}

#[derive(Asset, bevy::render::render_resource::AsBindGroup, Debug, Clone, Reflect)]
struct DiceMaterial {
    #[uniform(100)]
    dice_color: Vec4,
    #[uniform(101)]
    numbers_color: Vec4,
    #[uniform(102)]
    emission_setup: EmissionSetup,
}

impl MaterialExtension for DiceMaterial {
    fn fragment_shader() -> ShaderRef {
        DICE_SHADER_HANDLE.into()
    }
}

const DICE_MAT_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("d4e7e174-3c7b-5b6e-b5c0-4c3b6ebd7a29");
#[derive(Asset, bevy::render::render_resource::AsBindGroup, Debug, Clone, Reflect)]
struct DiceMatMaterial {}

impl MaterialExtension for DiceMatMaterial {
    fn fragment_shader() -> ShaderRef {
        DICE_MAT_SHADER_HANDLE.into()
    }
}

#[derive(Reflect, Component, Default, Debug, Resource)]
pub struct DiceConfig {
    pub linear_force: f32,
    pub angular_force: f32,
}

#[derive(Resource)]
pub struct DiceResources {
    dice_gltf: Handle<Gltf>,
    render_target: Handle<Image>,
    pub dice_sets: Handle<DiceSets>,
}

pub struct DicePlugin;
impl Plugin for DicePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, DICE_SHADER_HANDLE, "dice.wgsl", Shader::from_wgsl);
        load_internal_asset!(
            app,
            DICE_MAT_SHADER_HANDLE,
            "dice_mat.wgsl",
            Shader::from_wgsl
        );
        app.register_type::<DiceSet>()
            .insert_resource(ToggleResourceWrapper {
                value: DiceSet::from_value("plastic"),
            });

        app.init_asset::<DiceSets>()
            .init_asset_loader::<DiceSetAssetLoader>();
        app.insert_resource(DiceConfig {
            linear_force: 0.5,
            angular_force: 0.9,
        });
        app.insert_resource(DiceRollQueue {
            queue: VecDeque::new(),
            timer: 0.0,
        });
        app.insert_resource(TimeToSleep(0.5))
            .insert_resource(Gravity(Vec3::NEG_Y * 19.6 * 2.0))
            .add_plugins(MaterialPlugin::<
                ExtendedMaterial<StandardMaterial, DiceMaterial>,
            >::default())
            .add_plugins(MaterialPlugin::<
                ExtendedMaterial<StandardMaterial, DiceMatMaterial>,
            >::default())
            .insert_resource(DiceTimer {
                timer: Duration::ZERO,
                active: false,
            })
            .add_systems(Startup, setup_dice_assets)
            .add_systems(OnEnter(AppState::Live), setup_dicebox)
            .add_systems(
                Update,
                (
                    reposition_walls,
                    set_d20_color,
                    dice_timer,
                    spawn_from_queue,
                    activate_dice_collisions,
                    roll_d20,
                    apply_force,
                    detect_roll.after(apply_force),
                    resize_offscreen_node,
                )
                    .run_if(in_state(AppState::Live)),
            )
            .add_observer(on_roll_dice);
    }
}

#[derive(Component, Clone, Eq, Hash, PartialEq, Debug, Serialize, Deserialize)]
pub enum DiceType {
    D100,
    D20,
    D12,
    D10,
    D8,
    D6,
    D4,
    MODIFIER,
}

#[derive(Component)]
struct DiceRollModifier {
    modifier: i32,
}

#[derive(Resource)]
struct DiceTimer {
    timer: Duration,
    active: bool,
}

impl DiceTimer {
    fn ping(&mut self) {
        self.active = true;
        self.timer = Duration::from_secs(3);
    }
}

fn dice_timer(
    mut commands: Commands,
    mut timer: ResMut<DiceTimer>,
    time: Res<Time>,
    resolved_dice: Query<(Entity, &DiceType, &DiceIsResolved)>,
    unresolved_dice: Query<Entity, (With<DiceType>, Without<DiceIsResolved>)>,
    modifiers: Query<(Entity, &DiceRollModifier)>,
) {
    if timer.active {
        timer.timer = timer.timer.saturating_sub(time.delta());
        if timer.timer == Duration::ZERO {
            if !resolved_dice.is_empty() && unresolved_dice.is_empty() {
                timer.active = false;
                let mut results: HashMap<DiceType, Vec<i32>> = HashMap::new();
                for (e, n, r) in resolved_dice.iter() {
                    commands.entity(e).despawn();
                    results.entry(n.clone()).or_insert_with(Vec::new).push(r.0);
                }
                for (e, m) in modifiers.iter() {
                    commands.entity(e).despawn();
                    results
                        .entry(DiceType::MODIFIER)
                        .or_insert_with(Vec::new)
                        .push(m.modifier);
                }
                for (k, v) in &results {
                    debug!("Dice results {:?} {:?}", k, v);
                }

                commands.trigger(DiceRollResolved {
                    dice_roll: DiceRoll { results },
                });
            }
        }
    }
}

#[derive(Component)]
struct DiceBoxCamera;

#[derive(Component)]
struct DiceAntialiasingNode;

#[derive(Component)]
struct DiceFloor;

#[derive(Component)]
enum DiceWall {
    Top,
    Bottom,
    Left,
    Right,
}

fn resize_offscreen_node(
    windows: Query<&Window>,
    mut resize_events: MessageReader<bevy::window::WindowResized>,
    mut images: ResMut<Assets<Image>>,
    mut dice_resources: ResMut<DiceResources>,
    mut dice_cam: Single<&mut Camera, With<DiceBoxCamera>>,
    mut antialias_node: Single<&mut ImageNode, With<DiceAntialiasingNode>>,
) {
    for resize_event in resize_events.read() {
        let window = windows.get(resize_event.window).unwrap();
        let window_size = window.physical_size();
        if window_size.x < 128 || window_size.y < 128 {
            return;
        }
        let render_target = images.add(create_render_target(&window_size));
        dice_resources.render_target = render_target.clone();
        dice_cam.target = render_target.clone().into();
        antialias_node.image = render_target.clone().into();
    }
}
fn setup_dice_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
) {
    let window_size = window.physical_size();
    let render_target = images.add(create_render_target(&window_size));
    info!("Loading dicesets");
    let dice_sets: Handle<DiceSets> = asset_server.load("dice/dicesets.ron");
    commands.insert_resource(DiceResources {
        dice_gltf: asset_server.load("dice/d20.glb"),
        render_target: render_target.clone(),
        dice_sets,
    });
}

fn setup_dicebox(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    dice_resources: Res<DiceResources>,
) {
    let layers = CollisionLayers::new(HexrollPhysicsLayer::Dice, HexrollPhysicsLayer::Dice);
    commands.spawn((
        Name::new("DicePlate"),
        DiceFloor,
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(100.0, 100.0)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba_u8(127, 127, 127, 200),
            perceptual_roughness: 1.0,
            alpha_mode: AlphaMode::Multiply,
            ..default()
        })),
        RigidBody::Static,
        Collider::cuboid(50.0, 1.0, 50.0),
        layers,
        Restitution::new(0.1),
        Friction::new(0.8),
        Pickable {
            should_block_lower: false,
            is_hoverable: false,
        },
        RenderLayers::layer(RENDER_LAYER_DICE_SHADOW),
    ));
    commands.spawn((
        Name::new("DiceWallTop"),
        DiceWall::Top,
        RigidBody::Static,
        Collider::cuboid(50.0, 250.0, 1.0),
        layers,
        Transform::from_xyz(0.0, 0.0, 5.0),
    ));
    commands.spawn((
        Name::new("DiceWallBottom"),
        DiceWall::Bottom,
        RigidBody::Static,
        Collider::cuboid(50.0, 250.0, 1.0),
        layers,
        Transform::from_xyz(0.0, 0.0, -5.0),
    ));
    commands.spawn((
        Name::new("DiceWallRight"),
        DiceWall::Right,
        RigidBody::Static,
        Collider::cuboid(1.0, 250.0, 50.0),
        layers,
        Transform::from_xyz(5.0, 0.0, 0.0),
    ));
    commands.spawn((
        Name::new("DiceWallLeft"),
        DiceWall::Left,
        RigidBody::Static,
        Collider::cuboid(1.0, 250.0, 50.0),
        layers,
        Transform::from_xyz(-5.0, 0.0, 0.0),
    ));
    commands.spawn((
        Name::new("Dice Shadow Key Light"),
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 14000.0,
            ..default()
        },
        RenderLayers::layer(RENDER_LAYER_DICE_SHADOW).with(RENDER_LAYER_DICE),
        Transform::from_rotation(Quat::from_rotation_x(-1.2)),
    ));
    // NOTE: Deactivate this camera to use translucent dice materials!
    // Setting Camera::is_active to false works, but it is better to set the target to None
    // and then back - as this should keep icy dice rendered properly during the switch.
    commands.spawn((
        Name::new("DiceBoxCamera"),
        DiceBoxCamera,
        bevy::anti_alias::fxaa::Fxaa {
            enabled: true,
            edge_threshold: Sensitivity::Extreme,
            edge_threshold_min: Sensitivity::Extreme,
        },
        Camera3d { ..default() },
        Projection::from(PerspectiveProjection {
            fov: 0.5,
            ..Default::default()
        }),
        Hdr,
        Camera {
            clear_color: ClearColorConfig::Default,
            target: dice_resources.render_target.clone().into(),
            ..default()
        },
        Transform::from_xyz(0.0, 20.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
        RenderLayers::layer(RENDER_LAYER_DICE),
    ));

    commands.spawn((
        Name::new("DummyDiceBoxCamera"),
        Msaa::default(),
        Camera3d { ..default() },
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Projection::from(PerspectiveProjection {
            fov: 0.5,
            ..Default::default()
        }),
        Hdr,
        Camera {
            order: 3,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Transform::from_xyz(0.0, 20.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
        RenderLayers::layer(RENDER_LAYER_DICE_SHADOW),
    ));

    commands.spawn((
        Name::new("DiceAntialiased"),
        DiceAntialiasingNode,
        ImageNode {
            image: dice_resources.render_target.clone(),
            ..default()
        },
        Pickable {
            should_block_lower: false,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            align_self: AlignSelf::Stretch,
            justify_self: JustifySelf::Stretch,
            ..default()
        },
    ));
}

#[derive(Component)]
struct MaterialMarker;

#[derive(PartialEq, Eq, PartialOrd, Ord, Reflect, Default, Clone, Component)]
pub struct DiceSet {
    pub value: String,
}

fn random_vibrant_color() -> Vec4 {
    let mut rng = rand::thread_rng();
    let h: f32 = rng.gen_range(0.0..=360.0);
    let s: f32 = rng.gen_range(0.99..=1.0);
    let l: f32 = rng.gen_range(0.1..=0.1);

    fn hsl_to_rgb(hsl: (f32, f32, f32)) -> (f32, f32, f32) {
        let (h, s, l) = hsl;
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let h_prime = h / 60.0;
        let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());

        let (r1, g1, b1) = if (0.0..=1.0).contains(&h_prime) {
            (c, x, 0.0)
        } else if (1.0..=2.0).contains(&h_prime) {
            (x, c, 0.0)
        } else if (2.0..=3.0).contains(&h_prime) {
            (0.0, c, x)
        } else if (3.0..=4.0).contains(&h_prime) {
            (0.0, x, c)
        } else if (4.0..=5.0).contains(&h_prime) {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        let m = l - c / 2.0;
        (r1 + m, g1 + m, b1 + m)
    }
    let (r, g, b) = hsl_to_rgb((h, s, l));
    Vec4::new(r, g, b, 1.0)
}

#[derive(Component)]
struct DiceGltfPartMarker;

fn set_d20_color(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &mut MeshMaterial3d<StandardMaterial>,
            &GltfMaterialName,
            &Name,
        ),
        With<DiceGltfPartMarker>,
    >,
    materials2: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, DiceMaterial>>>,
    asset_server: Res<AssetServer>,
    dice_set: Res<ToggleResourceWrapper<DiceSet>>,
    dice_resources: Res<DiceResources>,
    dice_sets: Res<Assets<DiceSets>>,
) {
    for (d, dd, mat, name) in query.iter() {
        if name.to_string().contains("_collider") {
            commands.entity(d).try_insert(Visibility::Hidden);
        }
        let mut base = materials2.get(dd.id()).unwrap().clone();
        base.double_sided = false;
        base.normal_map_texture = Some(asset_server.load_with_settings(
            "dice/dice_normal.ktx2",
            |settings: &mut ImageLoaderSettings| settings.is_srgb = false,
        ));
        base.base_color_texture = Some(asset_server.load("dice/dice_diffuse.ktx2"));

        let mut emission_factor = 0.0;
        let mut diffuse_to_emission_factor = 0.0;

        let dice_set_name = &dice_set.value.value;

        let dice_set_params = dice_sets
            .get(&dice_resources.dice_sets)
            .unwrap()
            .dice_sets
            .get(dice_set_name)
            .unwrap();

        if let Some(ef) = dice_set_params.emission_factor {
            emission_factor = ef;
        }
        if let Some(dtef) = dice_set_params.diffuse_to_emission_factor {
            diffuse_to_emission_factor = dtef;
        }
        if let Some(specular_transmission) = dice_set_params.specular_transmission {
            base.specular_transmission = specular_transmission;
        }
        if let Some(diffuse_transmission) = dice_set_params.diffuse_transmission {
            base.diffuse_transmission = diffuse_transmission;
        }
        if let Some(perceptual_roughness) = dice_set_params.perceptual_roughness {
            base.perceptual_roughness = perceptual_roughness;
        }
        if let Some(metallic) = dice_set_params.metallic {
            base.metallic = metallic;
        }
        if let Some(metallic_roughness_texture) = &dice_set_params.metallic_roughness_texture {
            base.metallic_roughness_texture =
                Some(asset_server.load(metallic_roughness_texture));
        }
        if let Some(reflectance) = dice_set_params.reflectance {
            base.reflectance = reflectance;
        }
        if let Some(ior) = dice_set_params.ior {
            base.ior = ior;
        }
        if let Some(thickness) = dice_set_params.thickness {
            base.thickness = thickness;
        }
        if let Some(emissive_texture) = &dice_set_params.emissive_texture {
            base.emissive_texture = Some(asset_server.load(emissive_texture));
        }
        if let Some(emissive_red) = dice_set_params.emissive_red {
            base.emissive.red = emissive_red;
        }
        if let Some(emissive_green) = dice_set_params.emissive_green {
            base.emissive.green = emissive_green;
        }
        if let Some(emissive_blue) = dice_set_params.emissive_blue {
            base.emissive.blue = emissive_blue;
        }
        if let Some(emissive_exposure_weight) = dice_set_params.emissive_exposure_weight {
            base.emissive_exposure_weight = emissive_exposure_weight;
        }
        if let Some(clearcoat) = dice_set_params.clearcoat {
            base.clearcoat = clearcoat;
        }
        if let Some(clearcoat_perceptual_roughness) =
            dice_set_params.clearcoat_perceptual_roughness
        {
            base.clearcoat_perceptual_roughness = clearcoat_perceptual_roughness;
        }

        let dice_color = match dice_set_params.color_scheme {
            DiceSetColorScheme::Random => {
                Vec4::new(rand::random(), rand::random(), rand::random(), 1.0)
            }
            DiceSetColorScheme::Vibrant => random_vibrant_color(),
            DiceSetColorScheme::Curated => {
                DICE_COLORS[rand::thread_rng().gen_range(0..DICE_COLORS.len())]
            }
        };

        if mat.0.as_str() == "Material.001" {
            let brightness = (dice_color.x + dice_color.y + dice_color.z) / 3.0;
            let numbers_color = if brightness > 0.4 {
                Vec4::new(0.0, 0.0, 0.0, 1.0)
            } else {
                Vec4::new(1.0, 1.0, 1.0, 1.0)
            };

            commands
                .entity(d)
                .try_insert(MaterialMarker)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .try_insert(MeshMaterial3d(materials.add(ExtendedMaterial {
                    base,
                    extension: DiceMaterial {
                        dice_color,
                        numbers_color,
                        emission_setup: EmissionSetup {
                            emission_factor,
                            diffuse_to_emission_factor,
                            _padding1: 0.0,
                            _padding2: 0.0,
                        },
                    },
                })));
        }
    }
}

#[derive(Component)]
struct DiceIsResolved(i32);

struct SetLimits {
    max_n: Option<String>,
    max_y: f32,
}

impl Default for SetLimits {
    fn default() -> Self {
        Self {
            max_n: None,
            max_y: f32::MIN,
        }
    }
}

#[derive(Component)]
struct DieTimer {
    roll_time: f32,
    rest_time: f32,
}

fn activate_dice_collisions(
    mut commands: Commands,
    dice: Query<(Entity, &GlobalTransform, &CollisionLayers)>,
) {
    for (e, t, l) in dice.iter() {
        if t.translation().y < 9.0 && l.filters == HexrollPhysicsLayer::Dice.to_bits() {
            commands
                .entity(e)
                .try_remove::<CollisionLayers>()
                .try_insert(CollisionLayers::new(
                    HexrollPhysicsLayer::Dice,
                    HexrollPhysicsLayer::Dice,
                ));
        }
    }
}

#[allow(clippy::type_complexity)]
fn detect_roll(
    mut commands: Commands,
    children: Query<&Children>,
    names: Query<&Name>,
    all_global_transforms: Query<&GlobalTransform>,
    all_transforms: Query<&Transform>,
    mut dice: Query<(Entity, &RigidBody, &mut DieTimer, Forces), Without<DiceIsResolved>>,
    time: Res<Time>,
) {
    let mut d: HashMap<Entity, SetLimits> = HashMap::new();

    for (e, b, mut dt, mut forces) in dice.iter_mut() {
        if (forces.angular_velocity().length() > 1.0 || dt.roll_time > 0.0)
            && dt.rest_time == 1.0
        {
            dt.roll_time -= time.delta_secs();
            if dt.roll_time < -5.0 {
                // NOTE: when dice fail to resolve after an extra time,
                // we nudge them a bit.
                forces.apply_angular_impulse(Vec3::new(0.1, 0.1, 0.1));
                forces.apply_linear_impulse(Vec3::new(0.0, 0.1, 0.0));
                dt.roll_time = 1.100;
            }
            continue;
        }
        if dt.rest_time > 0.0 {
            dt.rest_time -= time.delta_secs();
            continue;
        }
        if b.is_dynamic() {
            children.iter_descendants(e).for_each(|entity| {
                if let Ok(n) = names.get(entity) {
                    if n.to_string().starts_with('v') {
                        let set_entry = d.entry(e).or_insert(SetLimits::default());
                        let gt = all_global_transforms.get(entity).unwrap();
                        let y = gt.translation().y;
                        let last_y = set_entry.max_y;
                        if y > last_y {
                            d.insert(
                                e,
                                SetLimits {
                                    max_y: y,
                                    max_n: Some(n.to_string()),
                                },
                            );
                        }
                    }
                }
            });
        }
    }
    for (e, v) in d {
        if let Some(n) = v.max_n {
            commands.entity(e).try_insert(ColliderDensity(10000.0));
            commands.entity(e).try_insert(MaxLinearSpeed(0.2));
            commands.entity(e).try_insert(MaxAngularSpeed(0.2));
            commands.entity(e).try_insert(RigidBody::Static);

            // ------------------------------------------------------------------------------
            // FIXME: The following is a hack until avian3d gets
            // https://github.com/avianphysics/avian/issues/911 fixed.
            let mut potentially_confused_transform = *all_transforms.get(e).unwrap();
            potentially_confused_transform.scale = Vec3::new(-5.5, 5.5, 5.5);
            commands
                .entity(e)
                .try_insert(potentially_confused_transform);
            // ------------------------------------------------------------------------------

            if let Some(value) = n
                .split('.')
                .next()
                .and_then(|s| s.trim_start_matches('v').parse::<i32>().ok())
            {
                commands.entity(e).try_insert(DiceIsResolved(value));
            }
        }
    }
}

#[derive(Component)]
struct DiceWaitingForRoll;

#[derive(Component)]
pub struct Dice;

#[allow(clippy::type_complexity)]
fn apply_force(
    mut commands: Commands,
    params: Res<DiceConfig>,
    mut dice: Query<
        (Entity, &GlobalTransform, &DiceParams, Forces),
        (
            With<DiceWaitingForRoll>,
            With<AncestorMarker<ColliderMarker>>,
        ),
    >,
) {
    for (e, t, p, mut forces) in dice.iter_mut() {
        let dir =
            Vec3::ZERO - Vec3::new(t.translation().x, 0.0, -t.translation().z).normalize();
        let cross = dir.cross(Vec3::new(0.0, 1.0, 0.0)) * (params.angular_force);
        let xyz = t.translation().xyz();
        let test = rand::thread_rng().gen_range(0.0..0.5) - 0.3;
        let m = rand::thread_rng().gen_range(p.force_values.0..p.force_values.1)
            * (params.linear_force + test);
        forces.apply_linear_impulse(Vec3::new(xyz.x * m, p.force_values.2, xyz.z * m));
        forces.apply_angular_impulse(cross);
        commands
            .get_entity(e)
            .unwrap()
            .remove::<DiceWaitingForRoll>();
    }
}

#[derive(Component, Clone)]
struct DiceParams {
    dice_type: DiceType,
    dice_name: &'static str,
    scene_index: usize,
    collider_name: &'static str,
    density: f32,
    force_values: (f32, f32, f32),
}

// EXPERIMENNTAL - IS THERE A DIFFERENCE?
// const DEFAULT_DENSITY: f32 = 12.0;
const DEFAULT_DENSITY: f32 = 1200.0;

const DICE_MAP: [(KeyCode, DiceParams, &str); 7] = [
    (
        KeyCode::Digit2,
        DiceParams {
            dice_type: DiceType::D20,
            dice_name: "D20",
            scene_index: 0,
            collider_name: "d20_collider.Material",
            density: DEFAULT_DENSITY,
            force_values: (-1.7, -1.0, -4.0),
        },
        "d20",
    ),
    (
        KeyCode::Digit6,
        DiceParams {
            dice_type: DiceType::D6,
            dice_name: "D6",
            scene_index: 1,
            collider_name: "d6_collider.Material",
            density: DEFAULT_DENSITY,
            force_values: (-1.7, -1.0, -4.0),
        },
        "d6",
    ),
    (
        KeyCode::Digit3,
        DiceParams {
            dice_type: DiceType::D12,
            dice_name: "D12",
            scene_index: 2,
            collider_name: "d12_collider.Material",
            density: DEFAULT_DENSITY,
            force_values: (-1.7, -1.0, -4.0),
        },
        "d12",
    ),
    (
        KeyCode::Digit1,
        DiceParams {
            dice_type: DiceType::D10,
            dice_name: "D10",
            scene_index: 3,
            collider_name: "d10_collider.Material",
            density: DEFAULT_DENSITY,
            force_values: (-1.7, -1.0, -4.0),
        },
        "d10",
    ),
    (
        KeyCode::Digit8,
        DiceParams {
            dice_type: DiceType::D8,
            dice_name: "D8",
            scene_index: 4,
            collider_name: "d8_collider.Material",
            density: DEFAULT_DENSITY,
            force_values: (-1.7 / 2.0, -1.0 / 2.0, -4.0 / 2.0),
        },
        "d8",
    ),
    (
        KeyCode::Digit4,
        DiceParams {
            dice_type: DiceType::D4,
            dice_name: "D4",
            scene_index: 5,
            collider_name: "d4_collider.Material",
            density: DEFAULT_DENSITY,
            force_values: (-0.7, -0.3, -1.0),
        },
        "d4",
    ),
    (
        KeyCode::Digit0,
        DiceParams {
            dice_type: DiceType::D100,
            dice_name: "D100",
            scene_index: 6,
            collider_name: "d100_collider.Material",
            density: DEFAULT_DENSITY,
            force_values: (-1.7, -1.0, -4.0),
        },
        "d100",
    ),
];

struct DiceRollSpec {
    dice_count: u32,
    dice_type: String,
    roll_modifier: i32,
}

fn parse_dice_notation(notation: &str) -> DiceRollSpec {
    let re = Regex::new(r"(?P<num>\d*)d(?P<sides>\d+)(?P<modifier>[+-]\d+)?").unwrap();
    let notation = notation.to_string();
    let notation = notation.replace(" ", "");
    let caps = re.captures(&notation).unwrap();
    let dice_count: u32 = caps["num"].parse().unwrap_or(1);
    let dice_type: String = format!("d{}", caps["sides"].to_string());
    let roll_modifier: i32 = caps
        .name("modifier")
        .map_or(0, |m| m.as_str().parse().unwrap());
    DiceRollSpec {
        dice_count,
        dice_type,
        roll_modifier,
    }
}

struct DiceRollParams {
    dice_params: DiceParams,
    origin: Transform,
}

#[derive(Resource, Default)]
struct DiceRollQueue {
    queue: VecDeque<DiceRollParams>,
    timer: f32,
}

fn spawn_from_queue(
    mut commands: Commands,
    time: Res<Time>,
    mut queue: ResMut<DiceRollQueue>,
    resources: Res<DiceResources>,
    gltfs: Res<Assets<Gltf>>,
    dice_set: Res<ToggleResourceWrapper<DiceSet>>,
    dice_resources: Res<DiceResources>,
    dice_sets: Res<Assets<DiceSets>>,
) {
    let Some(dice_gltf) = gltfs.get(&resources.dice_gltf) else {
        return;
    };
    let Some(dice_sets_asset) = dice_sets.get(&dice_resources.dice_sets) else {
        return;
    };
    let Some(dice_set_params) = dice_sets_asset.dice_sets.get(&dice_set.value.value) else {
        return;
    };

    queue.timer -= time.delta_secs();
    if !queue.queue.is_empty() && queue.timer < 0.0 {
        let render_layers = match dice_set_params.renderer {
            DiceSetRenderer::Direct => RenderLayers::from_layers(&[
                RENDER_LAYER_DICE_SHADOW,
                RENDER_LAYER_TRANSLUCENT_DICE,
            ]),
            DiceSetRenderer::Indirect => {
                RenderLayers::from_layers(&[RENDER_LAYER_DICE_SHADOW, RENDER_LAYER_DICE])
            }
        };

        queue.timer = 0.05;
        let item = queue.queue.pop_front().unwrap();
        let dice = item.dice_params;
        let start_point = item.origin;
        let layers =
            CollisionLayers::new(HexrollPhysicsLayer::Battlemaps, HexrollPhysicsLayer::Dice);
        let mut rng = rand::thread_rng();
        commands
            .spawn((
                DiceWaitingForRoll,
                dice.dice_type.clone(),
                Name::new(dice.dice_name),
                dice.clone(),
                SceneRoot(dice_gltf.scenes[dice.scene_index].clone()),
                render_layers,
                start_point.with_scale(Vec3::new(-5.5, 5.5, 5.5)),
                RigidBody::Dynamic,
                SleepThreshold {
                    linear: 2.0,
                    angular: 2.0,
                },
                #[cfg(not(target_arch = "wasm32"))]
                SweptCcd::default(),
                ColliderDensity(dice.density),
                ColliderConstructorHierarchy::new(None)
                    .with_constructor_for_name(
                        dice.collider_name,
                        ColliderConstructor::TrimeshFromMesh,
                    )
                    .with_default_layers(layers),
                MaxAngularSpeed(rng.gen_range(18.0..22.0)),
                DieTimer {
                    roll_time: 1.100,
                    rest_time: 1.0,
                },
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
            ))
            .insert(Dice)
            .insert(Friction::new(0.8))
            .insert(Restitution::new(0.1))
            .insert(LinearDamping(0.5))
            .insert(AngularDamping(0.4))
            .observe(apply_render_layers_to_children);
    }
}

fn on_roll_dice(
    trigger: On<RollDice>,
    mut commands: Commands,
    _touches: Res<Touches>,
    mut timer: ResMut<DiceTimer>,
    mut queue: ResMut<DiceRollQueue>,
) {
    let x_range = [-4.0, 4.0];
    let z_range = [-4.0, 4.0];

    let start = Vec3::new(
        *x_range.choose(&mut rand::thread_rng()).unwrap(),
        0.0,
        *z_range.choose(&mut rand::thread_rng()).unwrap(),
    );
    let dir = (Vec3::ZERO - (start * Vec3::new(1.0, 1.0, 1.0))).normalize();

    let _axis = dir.cross(Vec3::new(0.0, 1.0, 0.0));
    let mut cross = dir.cross(Vec3::new(0.0, 1.0, 0.0)) * (0.9);
    cross *= -1.0;

    let dice_spec = parse_dice_notation(&trigger.event().dice);
    let mut dice: Option<DiceParams> = None;
    for (_key, d, n) in DICE_MAP {
        if dice_spec.dice_type == n {
            dice = Some(d);
        }
    }
    if let Some(dice) = dice {
        if dice_spec.roll_modifier != 0 {
            commands.spawn_empty().insert(DiceRollModifier {
                modifier: dice_spec.roll_modifier,
            });
        }

        let offsets: [(f32, f32); 8] = [
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, -1.0),
        ];

        for offset in 0..dice_spec.dice_count {
            timer.ping();

            let mut rng = rand::thread_rng();
            let h: f32 = rng.gen_range(0.0..0.4) - 0.2;
            let (x_offset, z_offset) = offsets[offset as usize % offsets.len()];
            let start_point =
                Transform::from_xyz(start.x + x_offset, 10.0, start.z + z_offset)
                    .with_rotation(Quat::from_rotation_y(h));

            queue.queue.push_back(DiceRollParams {
                dice_params: dice.clone(),
                origin: start_point,
            });
        }
    }
}

fn roll_d20(
    mut commands: Commands,
    input_mode: Res<InputMode>,
    _touches: Res<Touches>,
    key_input: Res<ButtonInput<KeyCode>>,
) {
    for (key, _d, n) in DICE_MAP {
        if key_input.just_pressed(key)
            && !key_input.pressed(KeyCode::AltLeft)
            && input_mode.keyboard_available()
        {
            commands.trigger(RollDice {
                dice: n.to_string(),
            });
        }
    }
}

fn apply_render_layers_to_children(
    trigger: On<SceneInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    transforms: Query<&Transform, Without<RenderLayers>>,
    query: Query<(Entity, &RenderLayers)>,
) {
    let Ok((parent, render_layers)) = query.get(trigger.entity) else {
        return;
    };
    children.iter_descendants(parent).for_each(|entity| {
        if transforms.contains(entity) {
            commands
                .entity(entity)
                .try_insert(render_layers.clone())
                .try_insert(DiceGltfPartMarker);
        }
    });
}

fn reposition_walls(
    mut resize_events: MessageReader<bevy::window::WindowResized>,
    cam: Single<&Projection, With<DiceBoxCamera>>,
    mut walls: Query<(&DiceWall, &mut Position)>,
) {
    if let Projection::Perspective(proj) = *cam {
        for _resize_event in resize_events.read() {
            for (wall, mut pos) in &mut walls {
                match wall {
                    DiceWall::Top => pos.y = 6.0,
                    DiceWall::Bottom => pos.y = -6.0,
                    DiceWall::Left => pos.x = -6.0 * proj.aspect_ratio,
                    DiceWall::Right => pos.x = 6.0 * proj.aspect_ratio,
                }
            }
        }
    }
    if let Projection::Orthographic(proj) = *cam {
        for _resize_event in resize_events.read() {
            for (wall, mut pos) in &mut walls {
                match wall {
                    DiceWall::Top => pos.y = proj.area.max.y,
                    DiceWall::Bottom => pos.y = proj.area.min.y,
                    DiceWall::Left => pos.x = proj.area.min.x,
                    DiceWall::Right => pos.x = proj.area.max.x,
                }
            }
        }
    }
}

fn create_render_target(content_page_size: &UVec2) -> Image {
    let size = Extent3d {
        width: content_page_size.x,
        height: content_page_size.y,
        ..default()
    };

    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::RENDER_ATTACHMENT;

    image
}

const DICE_COLORS: [Vec4; 14] = [
    Vec4::new(0.1, 0.1, 0.1, 1.0),
    Vec4::new(0.0, 0.0, 0.0, 1.0),
    Vec4::new(0.2, 0.0, 0.0, 1.0),
    Vec4::new(0.0, 0.2, 0.0, 1.0),
    Vec4::new(0.0, 0.0, 0.2, 1.0),
    Vec4::new(0.7, 0.0, 0.0, 1.0),
    Vec4::new(0.0, 0.7, 0.0, 1.0),
    Vec4::new(0.0, 0.0, 0.7, 1.0),
    Vec4::new(0.2, 0.2, 0.0, 1.0),
    Vec4::new(0.2, 0.0, 0.2, 1.0),
    Vec4::new(0.0, 0.2, 0.2, 1.0),
    Vec4::new(0.7, 0.0, 0.7, 1.0),
    Vec4::new(0.0, 0.7, 0.7, 1.0),
    Vec4::new(0.7, 0.7, 0.0, 1.0),
];

use ron::de::from_bytes;
use std::io;

#[derive(Clone, Debug, Deserialize)]
pub enum DiceSetColorScheme {
    Random,
    Vibrant,
    Curated,
}

#[derive(Clone, Debug, Deserialize)]
pub enum DiceSetRenderer {
    Direct,
    Indirect,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiceSetDefinition {
    pub name: String,
    renderer: DiceSetRenderer,
    emission_factor: Option<f32>,
    diffuse_to_emission_factor: Option<f32>,
    specular_transmission: Option<f32>,
    diffuse_transmission: Option<f32>,
    perceptual_roughness: Option<f32>,
    metallic: Option<f32>,
    metallic_roughness_texture: Option<String>,
    reflectance: Option<f32>,
    ior: Option<f32>,
    thickness: Option<f32>,
    emissive_texture: Option<String>,
    emissive_red: Option<f32>,
    emissive_green: Option<f32>,
    emissive_blue: Option<f32>,
    emissive_exposure_weight: Option<f32>,
    clearcoat: Option<f32>,
    clearcoat_perceptual_roughness: Option<f32>,
    color_scheme: DiceSetColorScheme,
}

#[derive(Asset, TypePath, Debug, Deserialize)]
pub struct DiceSets {
    pub dice_sets: HashMap<String, DiceSetDefinition>,
}

impl DiceSets {
    fn from_bytes(
        bytes: Vec<u8>,
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self, io::Error> {
        let dice_sets: DiceSets =
            from_bytes(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(dice_sets)
    }
}

#[derive(Default)]
struct DiceSetAssetLoader;

impl AssetLoader for DiceSetAssetLoader {
    type Asset = DiceSets;
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
        Ok(DiceSets::from_bytes(bytes, &mut load_context)?)
    }
}
