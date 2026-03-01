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
    prelude::*,
    window::CursorIcon,
    window::{PrimaryWindow, SystemCursorIcon},
};
use bevy_vector_shapes::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    content::{ContentMode, ThemeBackgroundColor},
    hexmap::elements::{MainCamera, MapVisibilityController},
    shared::{
        layers::HEIGHT_OF_TOKENS,
        widgets::{
            cursor::{PointerExclusivityIsPreferred, pointer_world_position},
            link::ContentHoverLink,
        },
    },
    vtt::{network::NetworkContext, sync::broadcast_drawing_message},
};

use crate::hexmap::elements::HexMapToolState;

pub struct BattlemapDrawingPlugin;
impl Plugin for BattlemapDrawingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_battlemap_user_drawn)
            .add_systems(
                Update,
                battlemap_drawing_tool.run_if(not(in_state(HexMapDrawHudState::Active))),
            )
            .add_systems(OnEnter(HexMapToolState::Draw), create_drawing_hud)
            .add_systems(OnExit(HexMapToolState::Draw), destroy_drawing_hud)
            .insert_state(HexMapDrawHudState::Inactive)
            .add_observer(on_drawing_message);
    }
}

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct BattlemapUserDrawing {
    drawing_id: u32,
    drawing_data: BattlemapUserDrawingData,
}
impl BattlemapUserDrawing {
    pub fn from_start_pos(start: Vec3) -> Self {
        BattlemapUserDrawing {
            drawing_id: rand::random::<u32>(),
            drawing_data: BattlemapUserDrawingData::from_start_pos(start),
        }
    }
}

#[derive(Component)]
pub struct BattlemapUserDrawingInProgress {}

#[derive(Component, Serialize, Deserialize, Clone)]
struct BattlemapUserDrawingData {
    pull: bool,
    stop: Vec3,
    lines: Vec<(Vec<Vec3>, f32)>,
    bounds: Rect,
}

fn x3l_multidimentional_compression(points: &[Vec3], e: f32) -> Vec<Vec3> {
    if points.len() < 3 {
        return Vec::new();
    }

    let mut result = vec![points[0]];
    let mut e_factor = 10.0;
    let mut last_added = points[0];

    for window in points.windows(3) {
        let (a, m, b) = (window[0], window[1], window[2]);
        let d = last_added.distance(m);
        let pd = perpendicular_distance(&m, &last_added, &b);

        if pd > e.powf(e_factor) || d > e.sqrt().sqrt() {
            if result.last() != Some(&a) {
                result.push(a);
            }
            result.push(m);
            last_added = m;
            e_factor = e_factor.max(2.0) - 1.0;
        } else {
            e_factor *= 2.0;
        }
    }

    result.push(points[points.len() - 1]);
    result
}

impl BattlemapUserDrawingData {
    fn from_start_pos(start: Vec3) -> Self {
        BattlemapUserDrawingData {
            pull: true,
            lines: Vec::new(),
            stop: start,
            bounds: Rect::default(),
        }
    }

    fn calculate_bounds(&mut self) {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;

        for (line, _) in &self.lines {
            for pos in line {
                if pos.x < min_x {
                    min_x = pos.x;
                }
                if pos.x > max_x {
                    max_x = pos.x;
                }
                if pos.z < min_z {
                    min_z = pos.z;
                }
                if pos.z > max_z {
                    max_z = pos.z;
                }
            }
        }

        self.bounds = Rect::new(min_x, min_z, max_x, max_z);
    }

    fn clear(&mut self) {
        self.lines.clear();
        self.bounds = Rect::default();
    }

    fn add_corner(&mut self, pos: Vec3) {
        if let Some((line, _)) = self.lines.iter_mut().last() {
            line.push(pos);
            self.stop = pos;
        }
        self.calculate_bounds();
    }
    fn new_line(&mut self, pos: Vec3, scale: f32) {
        self.lines.push((vec![pos], scale));
        self.stop = pos;
    }

    fn undo_line(&mut self) {
        self.lines.pop();
    }

    fn bake_line(&mut self, scale: f32) {
        if let Some((prev, _)) = self.lines.iter_mut().last() {
            let epsilon = scale;
            *prev = x3l_multidimentional_compression(prev, epsilon);
        }
    }
}

fn setup_painter(painter: &mut ShapePainter) {
    painter.set_3d();
    painter.color = Color::BLACK; //.with_alpha(0.3);
    painter.thickness_type = ThicknessType::Pixels;
    painter.origin = Some(Vec3::Y * 10.0);
    painter.hollow = true;
    painter.rotate_x(-std::f32::consts::PI / 2.0);
    painter.set_translation(Vec3::ZERO);
}

fn draw_line(painter: &mut ShapePainter, from: Vec3, to: Vec3) {
    painter.line(Vec3::new(from.x, -from.z, 2.0), Vec3::new(to.x, -to.z, 2.0));
}

fn draw_battlemap_user_drawn(
    gizmo: Query<&BattlemapUserDrawing>,
    mut painter: ShapePainter,
    visibility: Res<MapVisibilityController>,
) {
    setup_painter(&mut painter);
    for gizmo in gizmo.iter() {
        if !visibility
            .rect
            .inflate(10.0)
            .intersect(gizmo.drawing_data.bounds)
            .is_empty()
            && visibility.scale < 0.7
        {
            for (l, scale) in gizmo.drawing_data.lines.iter() {
                painter.thickness = scale / visibility.scale * 10.0;
                painter.color = Color::BLACK;
                painter.cap = Cap::Round;
                if l.len() > 1 {
                    let mut last = l.get(0).unwrap();
                    for c in &l[1..] {
                        draw_line(&mut painter, *last, *c);
                        last = c;
                    }
                }
            }
        }
    }
}

fn battlemap_drawing_tool(
    mut commands: Commands,
    mut gizmo: Query<
        (Entity, &mut BattlemapUserDrawing),
        With<BattlemapUserDrawingInProgress>,
    >,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mouse: Res<ButtonInput<MouseButton>>,
    key: Res<ButtonInput<KeyCode>>,
    visibility: Res<MapVisibilityController>,
    mut next_tool_state: ResMut<NextState<HexMapToolState>>,
    window: Single<Entity, With<PrimaryWindow>>,
) {
    if let Some((entity, mut gizmo)) = gizmo.iter_mut().next() {
        commands
            .entity(*window)
            .insert(CursorIcon::System(SystemCursorIcon::Crosshair));
        commands
            .entity(entity)
            .try_insert(PointerExclusivityIsPreferred);
        next_tool_state.set(HexMapToolState::Draw);
        if let Some(pos) = pointer_world_position(q_window, q_camera) {
            if key.just_pressed(KeyCode::Escape) {
                commands.trigger(FinalizeActiveDryeraseDrawing);
            }
            if key.just_pressed(KeyCode::KeyZ) {
                commands.trigger(UndoLastDryeraseStroke);
            }
            if mouse.just_pressed(MouseButton::Left) {
                gizmo
                    .drawing_data
                    .new_line(pos.with_y(HEIGHT_OF_TOKENS), visibility.scale);
                gizmo.drawing_data.pull = true;
            }
            if mouse.just_released(MouseButton::Left) {
                gizmo.drawing_data.bake_line(visibility.scale);
                gizmo.drawing_data.pull = false;
            }
            if mouse.pressed(MouseButton::Left) && gizmo.drawing_data.pull {
                if pos
                    .with_y(HEIGHT_OF_TOKENS)
                    .distance(gizmo.drawing_data.stop)
                    > visibility.scale
                {
                    gizmo.drawing_data.add_corner(pos.with_y(HEIGHT_OF_TOKENS));
                }
            }
        }
    }
}

fn perpendicular_distance(pt: &Vec3, start: &Vec3, end: &Vec3) -> f32 {
    let area = (end.z - start.z) * pt.x - (end.x - start.x) * pt.z + end.x * start.z
        - end.z * start.x;
    let base = start.distance_squared(*end).sqrt();
    (area.abs() / base).max(0.0)
}

#[derive(Serialize, Deserialize, Clone, Event)]
pub enum DryeraseDrawingMessage {
    AddDrawing(BattlemapUserDrawing),
    RemoveDrawing(u32),
}

fn on_drawing_message(
    message: On<DryeraseDrawingMessage>,
    mut commands: Commands,
    existing_drawings: Query<(Entity, &BattlemapUserDrawing)>,
) {
    match message.event() {
        DryeraseDrawingMessage::AddDrawing(battlemap_user_drawing) => {
            commands.spawn(battlemap_user_drawing.clone());
        }
        DryeraseDrawingMessage::RemoveDrawing(id_to_remove) => {
            existing_drawings.iter().for_each(|(e, d)| {
                if d.drawing_id == *id_to_remove {
                    commands.entity(e).try_despawn();
                }
            });
        }
    };
}

#[derive(Component)]
struct DrawingHud;

fn create_drawing_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut next_content_mode: ResMut<NextState<ContentMode>>,
) {
    next_content_mode.set(ContentMode::MapOnly);

    commands
        .spawn((
            DrawingHud,
            Name::new("DrawingHud"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                justify_self: JustifySelf::Center,
                ..default()
            },
            Pickable {
                should_block_lower: true,
                ..default()
            },
        ))
        .observe(
            |_: On<Pointer<Over>>,
             mut next_hud_state: ResMut<NextState<HexMapDrawHudState>>| {
                next_hud_state.set(HexMapDrawHudState::Active)
            },
        )
        .observe(
            |_: On<Pointer<Out>>,
             mut next_hud_state: ResMut<NextState<HexMapDrawHudState>>| {
                next_hud_state.set(HexMapDrawHudState::Inactive)
            },
        )
        .with_children(|c| {
            c.commands().add_observer(on_undo_last_stroke);
            c.commands().add_observer(on_finalize_current_drawing);
            c.commands().add_observer(on_erase_visible_drawings);
            c.spawn(make_hud_button_bundle("Exit"))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-exit-256.ktx2",
                ))
                .hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(FinalizeActiveDryeraseDrawing);
                });
            c.spawn(make_hud_button_bundle("ClearDrawing"))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-trash.ktx2",
                ))
                .hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(EraseVisibleDryeraseDrawings);
                });
            c.spawn(make_hud_button_bundle("UndoLastDrawn"))
                .with_child(make_hud_button_image_bundle(
                    &asset_server,
                    "icons/icon-undo-256.ktx2",
                ))
                .hover_effect()
                .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(UndoLastDryeraseStroke);
                });
        });
}

fn destroy_drawing_hud(mut commands: Commands, hud: Query<Entity, With<DrawingHud>>) {
    hud.iter().for_each(|e| commands.entity(e).try_despawn());
}

fn make_hud_button_bundle(name: &str) -> impl Bundle {
    (
        Name::new(name.to_string()),
        Node {
            width: Val::Px(64.0),
            height: Val::Px(64.0),
            margin: UiRect::right(Val::Px(10.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BorderRadius::all(Val::Px(20.0)),
        BackgroundColor(Color::srgb_u8(20, 20, 20)),
        ThemeBackgroundColor(Color::srgb_u8(20, 20, 20)),
        Pickable {
            should_block_lower: true,
            ..default()
        },
    )
}

fn make_hud_button_image_bundle(
    asset_server: &Res<AssetServer>,
    image_asset_name: &str,
) -> impl Bundle {
    (
        Node {
            width: Val::Px(48.0),
            height: Val::Px(48.0),
            padding: UiRect::all(Val::Px(10.0)),
            align_self: AlignSelf::Center,
            ..default()
        },
        Pickable {
            should_block_lower: true,
            ..default()
        },
        ImageNode {
            image: asset_server.load(image_asset_name.to_string()),
            ..default()
        },
    )
}

#[derive(SubStates, Debug, Default, Hash, PartialEq, Eq, Clone)]
#[source(HexMapToolState = HexMapToolState::Draw)]
enum HexMapDrawHudState {
    #[default]
    Inactive,
    Active,
}

#[derive(Event)]
struct FinalizeActiveDryeraseDrawing;

#[derive(Event)]
struct UndoLastDryeraseStroke;

#[derive(Event)]
struct EraseVisibleDryeraseDrawings;

fn on_finalize_current_drawing(
    _: On<FinalizeActiveDryeraseDrawing>,
    mut commands: Commands,
    drawings: Query<(Entity, &BattlemapUserDrawing), With<BattlemapUserDrawingInProgress>>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut next_tool_state: ResMut<NextState<HexMapToolState>>,
    mut next_hud_state: ResMut<NextState<HexMapDrawHudState>>,
    mut socket: Option<ResMut<NetworkContext>>,
) {
    drawings.iter().for_each(|(e, d)| {
        commands
            .entity(e)
            .try_remove::<PointerExclusivityIsPreferred>();
        commands
            .entity(e)
            .try_remove::<BattlemapUserDrawingInProgress>();
        let socket = if socket.is_some() {
            socket.as_mut().unwrap()
        } else {
            return;
        };
        broadcast_drawing_message(socket, DryeraseDrawingMessage::AddDrawing(d.clone()));
    });
    next_tool_state.set(HexMapToolState::Selection);
    commands
        .entity(*window)
        .insert(CursorIcon::System(SystemCursorIcon::Default));
    next_hud_state.set(HexMapDrawHudState::Inactive);
}

fn on_undo_last_stroke(
    _: On<UndoLastDryeraseStroke>,
    mut gizmo: Query<&mut BattlemapUserDrawing, With<BattlemapUserDrawingInProgress>>,
) {
    gizmo.iter_mut().for_each(|mut g| {
        g.drawing_data.undo_line();
        g.drawing_data.pull = false;
    });
}

fn on_erase_visible_drawings(
    _: On<EraseVisibleDryeraseDrawings>,
    mut commands: Commands,
    existing_drawings: Query<
        (Entity, &BattlemapUserDrawing),
        Without<BattlemapUserDrawingInProgress>,
    >,
    mut current_drawings: Query<
        &mut BattlemapUserDrawing,
        With<BattlemapUserDrawingInProgress>,
    >,
    visibility: Res<MapVisibilityController>,
    mut socket: Option<ResMut<NetworkContext>>,
) {
    existing_drawings.iter().for_each(|(e, d)| {
        if !visibility
            .rect
            .inflate(10.0)
            .intersect(d.drawing_data.bounds)
            .is_empty()
        {
            commands.entity(e).try_despawn();
            if let Some(socket) = socket.as_mut() {
                broadcast_drawing_message(
                    socket,
                    DryeraseDrawingMessage::RemoveDrawing(d.drawing_id),
                );
            }
        }
    });
    current_drawings.iter_mut().for_each(|mut d| {
        if !visibility
            .rect
            .inflate(10.0)
            .intersect(d.drawing_data.bounds)
            .is_empty()
        {
            d.drawing_data.clear();
            if let Some(socket) = socket.as_mut() {
                broadcast_drawing_message(
                    socket,
                    DryeraseDrawingMessage::RemoveDrawing(d.drawing_id),
                );
            }
        }
    });
}
