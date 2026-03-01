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

use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    camera::visibility::RenderLayers,
    core_pipeline::{oit::OrderIndependentTransparencySettings, tonemapping::Tonemapping},
    gltf::GltfMaterialName,
    post_process::bloom::{Bloom, BloomCompositeMode},
    prelude::*,
    render::view::Hdr,
    scene::SceneInstanceReady,
    text::LineHeight,
};
use hexroll3_app::shared::{AppState, LoadingState};

const GLTF_PATH: &str = "hexroll.glb";

pub struct IntroPlugin;

impl Plugin for IntroPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Intro), setup_mesh_and_animation)
            .add_systems(OnEnter(AppState::Intro), setup_camera_and_environment)
            .add_systems(OnExit(AppState::Intro), despawn_stuff)
            .add_systems(Update, bloom_knob.run_if(in_state(AppState::Intro)))
            .add_systems(Update, fader_knob.run_if(in_state(AppState::Intro)))
            .add_systems(Update, fader_to_app.run_if(in_state(AppState::Live)))
            .add_systems(
                Update,
                captions_fader_knob.run_if(in_state(AppState::Intro)),
            );
    }
}

#[derive(Component)]
struct AnimationToPlay {
    graph_handle: Handle<AnimationGraph>,
    index: AnimationNodeIndex,
}

fn setup_mesh_and_animation(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let (graph, index) = AnimationGraph::from_clip(
        asset_server.load(GltfAssetLabel::Animation(0).from_asset(GLTF_PATH)),
    );

    let graph_handle = graphs.add(graph);

    let animation_to_play = AnimationToPlay {
        graph_handle,
        index,
    };
    let mesh_scene =
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(GLTF_PATH)));
    commands
        .spawn((
            Name::new("Intro"),
            animation_to_play,
            mesh_scene,
            RenderLayers::layer(9),
            IntroStuff,
        ))
        .observe(on_intro_scene_spawned);
}
#[derive(Component)]
struct IntroStuff;

#[derive(Component)]
struct ControllerBone;

#[derive(Component)]
struct FaderBone;

#[derive(Component)]
struct IntroFaderNode;

#[derive(Component)]
struct CaptionFaderBone;

#[derive(Component)]
struct BasePlate;

#[derive(Component)]
struct Captions;

fn bloom_knob(
    mut bloom: Query<&mut Bloom>,
    controller: Query<&Transform, With<ControllerBone>>,
    base_plates: Query<&MeshMaterial3d<StandardMaterial>, With<BasePlate>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut bloom) = bloom.iter_mut().next() else {
        return;
    };
    let Some(controller) = controller.iter().next() else {
        return;
    };
    for bpm in base_plates.iter() {
        if let Some(mat) = materials.get_mut(&bpm.0) {
            mat.emissive = LinearRgba::new(
                1.0,
                2.0 * controller.scale.x + 1.0,
                10.0 * controller.scale.x + 0.1,
                1.0,
            );
        }
    }
    bloom.intensity = 0.5 * controller.scale.x + 0.05;
}

fn captions_fader_knob(
    mut caption: Query<&mut TextColor, With<Captions>>,
    caption_fader: Query<&Transform, With<CaptionFaderBone>>,
) {
    let Some(controller) = caption_fader.iter().next() else {
        return;
    };
    for mut panel in caption.iter_mut() {
        panel.0.set_alpha(1.0 - controller.scale.x);
    }
}

fn fader_knob(
    mut panel: Query<&mut BackgroundColor, With<IntroFaderNode>>,
    controller: Query<&Transform, With<FaderBone>>,
    mut next_state: ResMut<NextState<AppState>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    loading_state: Res<State<LoadingState>>,
) {
    let Some(mut panel) = panel.iter_mut().next() else {
        return;
    };
    let Some(controller) = controller.iter().next() else {
        return;
    };
    panel.0.set_alpha(1.0 - controller.scale.x);
    if keyboard.get_just_pressed().len() > 0 || controller.scale.x < 0.0 {
        if *loading_state == LoadingState::Ready {
            next_state.set(AppState::Live);
        }
    }
}

fn fader_to_app(
    mut commands: Commands,
    mut panel: Query<(Entity, &mut BackgroundColor), With<IntroFaderNode>>,
) {
    let Some((e, mut panel)) = panel.iter_mut().next() else {
        return;
    };
    let new_alpha = panel.0.alpha() - 0.01;
    if panel.0.alpha() < 0.0001 {
        commands.entity(e).despawn();
    } else {
        panel.0.set_alpha(new_alpha);
    }
}

fn on_intro_scene_spawned(
    trigger: On<SceneInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    bones: Query<(Entity, &Name)>,
    animations_to_play: Query<&AnimationToPlay>,
    mut players: Query<&mut AnimationPlayer>,
    asset_server: Res<AssetServer>,
    mat_names: Query<(&GltfMaterialName, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let font = asset_server.load("fonts/FiraSans-Regular.ttf");
    commands
        .spawn((
            IntroStuff,
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                top: Val::Percent(75.0),
                width: Val::Percent(100.0),
                align_self: AlignSelf::Center,
                ..default()
            },
        ))
        .with_child((
            Captions,
            TextSpan::default(),
            TextColor::WHITE,
            TextFont {
                font: font.clone(),
                font_size: 18.0,
                ..default()
            },
            Text::new("created by Pen, Dice & Paper"),
            TextLayout::new_with_justify(Justify::Center),
        ))
        .with_child((
            Captions,
            TextSpan::default(),
            TextColor(LinearRgba::new(0.5,0.5,0.5,1.0).into()),
            TextFont {
                font: font.clone(),
                font_size: 15.0,
                ..default()
            },
            Text::new("Additional content by Alex Doyla (WATABOU), Cille Rosenørn Abildhauge (Penzilla), Andrew Sheppard, Moreno and Angela Paissan & Alex Gray"),
            TextLayout::new_with_justify(Justify::Center),
        ))
        .with_child((
            Captions,
            TextSpan::default(),
            TextColor(LinearRgba::new(0.5,0.5,0.5,1.0).into()),
            TextFont {
                font: font.clone(),
                font_size: 15.0,
                ..default()
            },
            Text::new("with special thanks to our Patreon community, to the Bevy community and contributors, and to Aron Clark"),
            TextLayout::new_with_justify(Justify::Center),
        ))
        .with_child((
            Captions,
            TextSpan::default(),
            TextColor(LinearRgba::new(0.5,0.5,0.5,1.0).into()),
            TextFont {
                font: font.clone(),
                font_size: 10.0,
                line_height: LineHeight::Px(15.0),
                ..default()
            },
            Text::new("Ennies Silver Award winner for best digital aid, 2024. Copyright (c) 2021-2025 All Rights Reserved. Handmade with love by humans."),
            TextLayout::new_with_justify(Justify::Center),
        ));

    commands.spawn((
        AudioPlayer::new(asset_server.load("soundtrack.ogg")),
        IntroStuff,
    ));
    if let Ok(animation_to_play) = animations_to_play.get(trigger.entity) {
        for child in children.iter_descendants(trigger.entity) {
            if let Ok((mat_name, mat)) = mat_names.get(child) {
                if mat_name.0 == "Base" {
                    commands.entity(child).insert(BasePlate);
                }
                if mat_name.0 == "Particles" {
                    if let Some(mat) = materials.get_mut(&mat.0) {
                        mat.emissive = LinearRgba::new(300.0, 300.0, 300.0, 300.0);
                    }
                }
            }
            commands.entity(child).insert(RenderLayers::layer(9));
            if let Ok((b, n)) = bones.get(child) {
                if n.as_str() == "Controller" {
                    commands.entity(b).insert(ControllerBone);
                }
                if n.as_str() == "Fader" {
                    commands.entity(b).insert(FaderBone);
                }
                if n.as_str() == "CaptionFader" {
                    commands.entity(b).insert(CaptionFaderBone);
                }
            }
            if let Ok(mut player) = players.get_mut(child) {
                player.play(animation_to_play.index).repeat();
                commands
                    .entity(child)
                    .insert(AnimationGraphHandle(animation_to_play.graph_handle.clone()));
            }
        }
    }
}

fn despawn_stuff(mut commands: Commands, stuff: Query<Entity, With<IntroStuff>>) {
    for e in stuff.iter() {
        commands.entity(e).despawn();
    }
}

fn setup_camera_and_environment(mut commands: Commands) {
    commands.spawn((
        IntroFaderNode,
        Node {
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::BLACK),
    ));

    commands.spawn((
        IntroStuff,
        Camera3d::default(),
        Hdr,
        Camera {
            order: 10,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        RenderLayers::from_layers(&[9]),
        Tonemapping::None,
        Bloom {
            intensity: 0.5,
            composite_mode: BloomCompositeMode::Additive,
            ..default()
        },
        Projection::from(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 5.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Msaa::Off,
        TemporalAntiAliasing::default(),
        OrderIndependentTransparencySettings::default(),
        Transform::from_xyz(0.0, 10.0, 0.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::NEG_Z),
    ));
}
