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

use bevy::{input_focus::InputFocus, prelude::*};
use bevy_editor_cam::prelude::EditorCam;
use bevy_ui_text_input::{
    SubmitText, TextInputBuffer, TextInputMode, TextInputNode, TextInputPrompt,
};

use cosmic_text::Edit;

use crate::{
    battlemaps::DUNGEON_FOG_COLOR,
    hexmap::elements::HexMapToolState,
    shared::{
        camera::CameraZoomRestrictor,
        dragging::DraggingMotionDetector,
        vtt::VttData,
        widgets::{
            dial::{
                DialAssets, DialButton, DialMenuCommands, DialMenuOptions, MenuItemSpawner,
            },
            modal::{DiscreteAppState, ModalWindow},
        },
    },
    vtt::sync::EventContext,
};

use super::{
    DespawnToken, SelectedToken, TOKEN_SIZE_SCALING_ZOOM_LIMIT,
    TOKEN_TORCH_SCALING_ZOOM_LIMIT, Token, TokenMeshEntity, TokenMessage, TokenUpdateMessage,
    control::{
        TokenInteractionMode, TorchInteractionGizmo, UpdateTokenLabel, update_token_material,
    },
    tokens::{TokenIsLocked, TokensAssets},
};

pub struct TokenDial;
impl Plugin for TokenDial {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, submit_token_label)
            .add_observer(on_spawn_token_dial);
    }
}

#[derive(Hash, Clone, PartialEq, Eq, Copy)]
pub enum TokenDialIcon {
    Color,
    Brush,
    Bulb,
    Opacity,
    Torch,
    Lock,
    Scale,
    Trash,
    Title,
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let mut dial_assets: DialAssets<TokenDialIcon> = DialAssets::new(
        meshes.add(Plane3d::new(Vec3::NEG_Z, Vec2::splat(80.0))),
        meshes.add(Circle::new(60.0)),
    );
    dial_assets
        .add_item(
            TokenDialIcon::Color,
            "icons/icon-color.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            TokenDialIcon::Brush,
            "icons/icon-brush.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            TokenDialIcon::Bulb,
            "icons/icon-bulb.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            TokenDialIcon::Lock,
            "icons/icon-move-lock.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            TokenDialIcon::Opacity,
            "icons/icon-opacity.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            TokenDialIcon::Torch,
            "icons/icon-torch.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            TokenDialIcon::Title,
            "icons/icon-title.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            TokenDialIcon::Trash,
            "icons/icon-trash.ktx2",
            &mut materials,
            &asset_server,
        )
        .add_item(
            TokenDialIcon::Scale,
            "icons/icon-scale.ktx2",
            &mut materials,
            &asset_server,
        );
    commands.insert_resource(dial_assets);
}

#[derive(Event)]
pub struct SpawnTokenDial {
    pub token: Entity,
}

fn token_set_interaction<F>(
    selected_tokens: &Query<Entity, With<SelectedToken>>,
    token: Entity,
    mut action: F,
) where
    F: FnMut(Entity),
{
    if selected_tokens.contains(token) {
        for token in selected_tokens.iter() {
            action(token);
        }
    } else {
        action(token);
    }
}

fn on_spawn_token_dial(
    trigger: On<SpawnTokenDial>,
    mut commands: Commands,
    vtt_data: Res<VttData>,
    global_transforms: Query<&GlobalTransform>,
    mut dial_menu_commands: DialMenuCommands,
    dial_assets: Res<DialAssets<TokenDialIcon>>,
    mut dmd: ResMut<DraggingMotionDetector>,
    app_state: Res<State<DiscreteAppState>>,
) {
    if vtt_data.mode.is_player() || *app_state != DiscreteAppState::Normal {
        return;
    }
    dmd.set_detected();

    let calc_scale = |v: f32| -> f32 {
        if v > 0.01 {
            0.01 + ((v * 100.0 - 1.00).ln_1p() * 0.01)
        } else {
            v
        }
    };
    let is_visible = |v: f32| v < 0.1;

    let pos = global_transforms
        .get(trigger.event().token)
        .unwrap()
        .translation()
        .xz();

    let token = trigger.token;
    if let Some(menu_entity) = dial_menu_commands.spawn_menu(DialMenuOptions {
        pos,
        calc_scale,
        is_visible,
    }) {
        commands.entity(menu_entity).with_children(|c| {
            c.spawn_empty().spawn_menu_item(
                7,
                7,
                TokenDialIcon::Trash,
                move |_: On<Pointer<Click>>,
                      mut commands: Commands,
                      selected_tokens: Query<Entity, With<SelectedToken>>,
                      mut next_state: ResMut<NextState<HexMapToolState>>| {
                    token_set_interaction(&selected_tokens, token, |token| {
                        commands.trigger(DespawnToken {
                            token_entity: token,
                        });
                    });
                    next_state.set(HexMapToolState::Selection);
                },
                &dial_assets,
                "Remove token",
            );
            c.spawn_empty().spawn_menu_item(
                2,
                7,
                TokenDialIcon::Scale,
                move |trigger: On<Pointer<Click>>,
                      selected_tokens: Query<Entity, With<SelectedToken>>,
                      transforms: Query<&Transform>,
                      mut commands: Commands,
                      mut editor_cam: Single<&mut EditorCam>,
                      mut next_state: ResMut<NextState<HexMapToolState>>| {
                    token_set_interaction(&selected_tokens, token, |token| {
                        editor_cam.restrict_camera_zoom(TOKEN_SIZE_SCALING_ZOOM_LIMIT);
                        if let Ok(t) = transforms.get(token) {
                            commands.entity(token).insert(TokenInteractionMode::Scale(
                                trigger.hit.position.unwrap(),
                                t.scale.x,
                            ));
                            next_state.set(HexMapToolState::Selection);
                        }
                    });
                },
                &dial_assets,
                "Scale token",
            );
            c.spawn_empty().spawn_menu_item(
                6,
                7,
                TokenDialIcon::Lock,
                move |_: On<Pointer<Click>>,
                      mut commands: Commands,
                      selected_tokens: Query<Entity, With<SelectedToken>>,
                      mut tokens_data: Query<&mut Token>,
                      mut next_state: ResMut<NextState<HexMapToolState>>| {
                    token_set_interaction(&selected_tokens, token, |token| {
                        if let Ok(mut token_data) = tokens_data.get_mut(token) {
                            token_data.mobility = match token_data.mobility {
                                super::tokens::TokenMobility::Unrestricted(locked) => {
                                    super::tokens::TokenMobility::Unrestricted(!locked)
                                }
                                super::tokens::TokenMobility::RefereeOnly(locked) => {
                                    super::tokens::TokenMobility::RefereeOnly(!locked)
                                }
                            };
                            if token_data.mobility.is_locked_now() {
                                commands.entity(token).try_insert(TokenIsLocked);
                            } else {
                                commands.entity(token).try_remove::<TokenIsLocked>();
                            }
                            commands.trigger(EventContext::from(TokenMessage::Update(
                                TokenUpdateMessage::from_token(&token_data),
                            )));
                            next_state.set(HexMapToolState::Selection);
                        }
                    });
                },
                &dial_assets,
                "Lock token",
            );
            c.spawn_empty().spawn_menu_item(
                3,
                7,
                TokenDialIcon::Torch,
                move |_: On<Pointer<Click>>,
                      mut commands: Commands,
                      mut editor_cam: Single<&mut EditorCam>,
                      assets: Res<TokensAssets>,
                      q_token_data: Query<&Token>,
                      mut next_state: ResMut<NextState<HexMapToolState>>| {
                    if let Ok(token_data) = q_token_data.get(token) {
                        let tc = commands
                            .spawn((
                                TorchInteractionGizmo,
                                Mesh3d(assets.torch_bubble.clone()),
                                bevy_mod_outline::OutlineVolume {
                                    visible: true,
                                    width: 4.0,
                                    colour: Color::srgb(1.0, 0.0, 0.0),
                                },
                                bevy_mod_outline::OutlineMode::ExtrudeFlat,
                                Transform::from_xyz(0.0, -5.0, 0.0)
                                    .with_scale(Vec3::splat(token_data.light * 0.1)),
                            ))
                            .id();
                        commands.entity(token).add_child(tc);
                        editor_cam.restrict_camera_zoom(TOKEN_TORCH_SCALING_ZOOM_LIMIT);
                        commands.entity(token).insert(TokenInteractionMode::Torch);
                        next_state.set(HexMapToolState::Selection);
                    }
                },
                &dial_assets,
                "Change light radius",
            );
            c.spawn_empty().spawn_menu_item(
                5,
                7,
                TokenDialIcon::Opacity,
                move |_: On<Pointer<Click>>,
                      mut commands: Commands,
                      selected_tokens: Query<Entity, With<SelectedToken>>,
                      finder: Query<&TokenMeshEntity>,
                      mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
                      mut data: Query<(&Transform, &mut Token)>,
                      mut materials: ResMut<Assets<StandardMaterial>>| {
                    token_set_interaction(&selected_tokens, token, |token| {
                        let mesh_entity = finder.get(token).unwrap().0;
                        let material_asset = mesh_materials.get(mesh_entity).unwrap();
                        let material = materials.get_mut(material_asset.id()).unwrap();
                        let new_alpha = if material.base_color.alpha().is_fully_opaque() {
                            0.3
                        } else {
                            1.0
                        };
                        material.base_color.set_alpha(new_alpha);
                        let (transform, mut token_data) = data.get_mut(token).unwrap();
                        token_data.color.set_alpha(new_alpha);
                        commands.trigger(EventContext::from(TokenMessage::Update(
                            TokenUpdateMessage::from_token(&token_data)
                                .with_transform(*transform)
                                .with_color(token_data.color),
                        )));
                    });
                },
                &dial_assets,
                "Set semi-translucent",
            );
            c.spawn_empty().spawn_menu_item(
                4,
                7,
                TokenDialIcon::Color,
                dial_menu_color_target(token),
                &dial_assets,
                "Change color",
            );
            c.spawn_empty().spawn_menu_item(
                1,
                7,
                TokenDialIcon::Title,
                set_token_label(token),
                &dial_assets,
                "Change label",
            );
        });
    }
}

#[allow(clippy::type_complexity)]
fn dial_menu_color_target(
    token: Entity,
) -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<&ChildOf>,
    Query<Entity, With<DialButton>>,
    Res<DialAssets<TokenDialIcon>>,
    ResMut<Assets<StandardMaterial>>,
) {
    move |trigger, mut commands, parents, prev, dial_assets, mut materials| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let parent = parents.get(trigger.entity).unwrap();

        commands.entity(parent.parent()).with_children(|c| {
            const HOURS_IN_DIAL: i32 = 12;
            c.spawn_empty().spawn_menu_item(
                3,
                HOURS_IN_DIAL,
                TokenDialIcon::Brush,
                dial_menu_colors(token),
                &dial_assets,
                "Change token color",
            );

            c.spawn_empty().spawn_menu_color(
                12,
                &DUNGEON_FOG_COLOR,
                set_token_color(token, DUNGEON_FOG_COLOR),
                &dial_assets,
                &mut materials,
            );

            c.spawn_empty().spawn_menu_item(
                9,
                HOURS_IN_DIAL,
                TokenDialIcon::Bulb,
                dial_menu_torch_colors(token),
                &dial_assets,
                "Change torch light color",
            );
        });
    }
}

#[allow(clippy::type_complexity)]
fn dial_menu_colors(
    token: Entity,
) -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<&ChildOf>,
    Query<Entity, With<DialButton>>,
    Res<DialAssets<TokenDialIcon>>,
    ResMut<Assets<StandardMaterial>>,
) {
    move |trigger, mut commands, parents, prev, dial_assets, mut materials| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let parent = parents.get(trigger.entity).unwrap();
        let colors = vec![
            (
                Color::srgb_u8(255, 0, 0),
                vec![
                    Color::srgb_u8(255, 102, 102),
                    Color::srgb_u8(255, 51, 51),
                    Color::srgb_u8(204, 0, 0),
                    Color::srgb_u8(153, 0, 0),
                    Color::srgb_u8(102, 0, 0),
                    Color::srgb_u8(51, 0, 0),
                ],
            ),
            (
                Color::srgb_u8(0, 255, 0),
                vec![
                    Color::srgb_u8(102, 255, 102),
                    Color::srgb_u8(51, 255, 51),
                    Color::srgb_u8(0, 204, 0),
                    Color::srgb_u8(0, 153, 0),
                    Color::srgb_u8(0, 102, 0),
                    Color::srgb_u8(0, 51, 0),
                ],
            ),
            (
                Color::srgb_u8(0, 0, 255),
                vec![
                    Color::srgb_u8(102, 102, 255),
                    Color::srgb_u8(51, 51, 255),
                    Color::srgb_u8(0, 0, 204),
                    Color::srgb_u8(0, 0, 153),
                    Color::srgb_u8(0, 0, 102),
                    Color::srgb_u8(0, 0, 51),
                ],
            ),
            (
                Color::srgb_u8(255, 165, 0),
                vec![
                    Color::srgb_u8(255, 191, 102),
                    Color::srgb_u8(255, 178, 51),
                    Color::srgb_u8(204, 132, 0),
                    Color::srgb_u8(153, 100, 0),
                    Color::srgb_u8(102, 66, 0),
                    Color::srgb_u8(51, 33, 0),
                ],
            ),
            (
                Color::srgb_u8(128, 0, 128),
                vec![
                    Color::srgb_u8(178, 102, 178),
                    Color::srgb_u8(153, 51, 153),
                    Color::srgb_u8(102, 0, 102),
                    Color::srgb_u8(76, 0, 76),
                    Color::srgb_u8(51, 0, 51),
                    Color::srgb_u8(25, 0, 25),
                ],
            ),
            (
                Color::srgb_u8(100, 100, 100),
                vec![
                    Color::srgb_u8(150, 150, 150),
                    Color::srgb_u8(125, 125, 125),
                    Color::srgb_u8(75, 75, 75),
                    Color::srgb_u8(50, 50, 50),
                    Color::srgb_u8(25, 25, 25),
                    Color::srgb_u8(10, 10, 10),
                ],
            ),
        ];

        for (i, (col, other)) in colors.iter().enumerate() {
            commands.entity(parent.parent()).with_children(|c| {
                c.spawn_empty().spawn_menu_color(
                    i as i32 * 2 + 1,
                    col,
                    dial_menu_more_colors(token, other.clone()),
                    &dial_assets,
                    &mut materials,
                );
            });
        }
    }
}

#[allow(clippy::type_complexity)]
fn dial_menu_torch_colors(
    token: Entity,
) -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<&ChildOf>,
    Query<Entity, With<DialButton>>,
    Res<DialAssets<TokenDialIcon>>,
    ResMut<Assets<StandardMaterial>>,
) {
    move |trigger, mut commands, parents, prev, dial_assets, mut materials| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let parent = parents.get(trigger.entity).unwrap();

        let colors = vec![
            Color::srgb_u8(255, 0, 0),
            Color::srgb_u8(0, 255, 0),
            Color::srgb_u8(0, 0, 255),
            Color::srgb_u8(255, 255, 0),
            Color::srgb_u8(255, 0, 255),
            Color::srgb_u8(0, 255, 255),
        ];

        for (i, col) in colors.iter().enumerate() {
            let cc = col.clone();
            commands.entity(parent.parent()).with_children(|c| {
                c.spawn_empty().spawn_menu_color(
                    i as i32 * 2 + 1,
                    col,
                    move |_: On<Pointer<Click>>,
                          mut commands: Commands,
                      selected_tokens: Query<Entity, With<SelectedToken>>,
                          mut data: Query<&mut Token>,
                          mut next_state: ResMut<NextState<HexMapToolState>>| {
                              token_set_interaction(&selected_tokens, token, |token| {
                                  let mut token_data = data.get_mut(token).unwrap();
                                  token_data.light_color = cc;
                                  commands.trigger(EventContext::from(TokenMessage::Update(
                                      TokenUpdateMessage::from_token(&token_data)
                                          .with_light_color(cc),
                                  )));
                                  next_state.set(HexMapToolState::Selection);
                              });
                          },
                    &dial_assets,
                    &mut materials,
                );
            });
        }
    }
}

#[allow(clippy::type_complexity)]
fn dial_menu_more_colors(
    token: Entity,
    colors: Vec<Color>,
) -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<&ChildOf>,
    Query<Entity, With<DialButton>>,
    Res<DialAssets<TokenDialIcon>>,
    ResMut<Assets<StandardMaterial>>,
) {
    move |trigger, mut commands, parents, prev, dial_assets, mut materials| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let parent = parents.get(trigger.entity).unwrap();

        for (i, col) in colors.iter().enumerate() {
            let cc = col.clone();
            commands.entity(parent.parent()).with_children(|c| {
                c.spawn_empty().spawn_menu_color(
                    i as i32 * 2 + 1,
                    col,
                    set_token_color(token, cc),
                    &dial_assets,
                    &mut materials,
                );
            });
        }
    }
}

fn set_token_label(
    token: Entity,
) -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<Entity, With<DialButton>>,
    ResMut<NextState<DiscreteAppState>>,
    ResMut<NextState<HexMapToolState>>,
    Query<&Token>,
) {
    move |_trigger,
          mut commands,
          prev,
          mut next_app_state,
          mut next_tool_state,
          tokens_data| {
        for e in prev.iter() {
            commands.entity(e).despawn();
        }
        let Ok(token_data) = tokens_data.get(token) else {
            return;
        };
        next_app_state.set(DiscreteAppState::Modal);
        next_tool_state.set(HexMapToolState::Selection);

        let mut input_buffer = TextInputBuffer::default();

        input_buffer.editor.insert_string(&token_data.label, None);

        let input_field = commands
            .spawn((
                TokenLabelEntry(token),
                TextInputPrompt {
                    text: "Token Label".to_string(),
                    color: Some(Color::srgba(1.0, 1.0, 1.0, 0.1)),
                    ..Default::default()
                },
                TextInputNode {
                    mode: TextInputMode::SingleLine,
                    clear_on_submit: false,
                    ..default()
                },
                input_buffer,
                Node {
                    left: Val::Px(20.),
                    width: Val::Px(280.),
                    top: Val::Px(20.),
                    ..default()
                },
                Pickable {
                    should_block_lower: false,
                    is_hoverable: true,
                },
            ))
            .id();
        commands.insert_resource(InputFocus::from_entity(input_field));
        commands
            .spawn((
                TokenLabelEntryUi,
                ModalWindow,
                Node {
                    position_type: PositionType::Relative,
                    border: UiRect::all(Val::Px(4.)),
                    width: Val::Px(480.),
                    height: Val::Px(75.),
                    right: Val::Auto,
                    top: Val::Auto,
                    align_self: AlignSelf::Center,
                    justify_self: JustifySelf::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                Transform::from_xyz(0., 30., 0.),
                BorderRadius::all(Val::Percent(50.)),
                BackgroundColor(Color::srgba_u8(0, 0, 0, 240)),
                ZIndex(999),
            ))
            .add_child(input_field);
    }
}

#[derive(Component)]
struct TokenLabelEntry(Entity);

#[derive(Component)]
struct TokenLabelEntryUi;

fn submit_token_label(
    mut events: MessageReader<SubmitText>,
    mut commands: Commands,
    entry_ui: Query<Entity, With<TokenLabelEntryUi>>,
    entry: Query<&TokenLabelEntry>,
    mut tokens_data: Query<(Entity, &mut Transform, &mut Token)>,
    mut next_app_state: ResMut<NextState<DiscreteAppState>>,
) {
    if !entry.is_empty() {
        if let Some(tle) = entry.iter().next() {
            for event in events.read() {
                let new_label = event.text.clone();
                next_app_state.set(DiscreteAppState::Normal);

                commands.trigger(UpdateTokenLabel {
                    token_entity: tle.0,
                    label: new_label.clone(),
                });

                if let Ok((_, _, mut token_data)) = tokens_data.get_mut(tle.0) {
                    token_data.label = new_label.clone();
                    commands.trigger(EventContext::from(TokenMessage::Update(
                        TokenUpdateMessage::from_token(&token_data),
                    )));
                }

                if let Some(entry_ui_entity) = entry_ui.iter().next() {
                    commands.entity(entry_ui_entity).despawn();
                }
            }
        }
    }
}

fn set_token_color(
    token: Entity,
    color: Color,
) -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<Entity, With<SelectedToken>>,
    Query<&TokenMeshEntity>,
    Query<(&Transform, &mut Token)>,
    Query<&MeshMaterial3d<StandardMaterial>>,
    ResMut<Assets<StandardMaterial>>,
    ResMut<NextState<HexMapToolState>>,
) {
    move |_,
          mut commands,
          selected_tokens,
          finder,
          mut data,
          mesh_materials,
          mut materials,
          mut next_state| {
        token_set_interaction(&selected_tokens, token, |token| {
            update_token_material(
                &mut commands,
                &finder,
                &mesh_materials,
                &mut materials,
                token,
                color,
            );
            let (transform, mut token_data) = data.get_mut(token).unwrap();
            token_data.color = color;
            commands.trigger(EventContext::from(TokenMessage::Update(
                TokenUpdateMessage::from_token(&token_data)
                    .with_transform(*transform)
                    .with_color(color),
            )));
            next_state.set(HexMapToolState::Selection);
        });
    }
}
