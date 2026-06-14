use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, SystemCursorIcon};

#[derive(Component)]
struct KnobRing;

#[derive(Component)]
struct KnobGauge;

#[derive(Component)]
struct KnobNotch;

#[derive(Component)]
pub struct KnobMain;

use crate::shared::widgets::cursor::{
    PointerExclusivityIsPreferred, PointerOnHover, TooltipOnHover,
};

use crate::content::ThemeBackgroundColor;

#[derive(EntityEvent)]
pub struct ResetKnob {
    pub entity: Entity,
}

pub trait GeneratorKnob {
    fn spawn_knob<T>(
        &mut self,
        exponential: bool,
        setter: fn(&mut T, i32),
        getter: fn(&T) -> f32,
        resource: &T,
        fraction: f32,
        icon: &str,
        tooltip: &str,
        asset_server: &AssetServer,
        size: f32,
    ) -> &mut Self
    where
        T: Resource;

    fn spawn_knob_ex<T>(
        &mut self,
        exponential: bool,
        setter: fn(&mut T, i32),
        getter: fn(&T) -> f32,
        resource: &T,
        fraction: f32,
        icon: &str,
        tooltip: &str,
        asset_server: &AssetServer,
        size: f32,
        color: Color,
    ) -> &mut Self
    where
        T: Resource;
}
impl GeneratorKnob for EntityCommands<'_> {
    fn spawn_knob<T>(
        &mut self,
        exponential: bool,
        setter: fn(&mut T, i32),
        getter: fn(&T) -> f32,
        resource: &T,
        fraction: f32,
        icon: &str,
        tooltip: &str,
        asset_server: &AssetServer,
        size: f32,
    ) -> &mut Self
    where
        T: Resource,
    {
        self.spawn_knob_ex(
            exponential,
            setter,
            getter,
            resource,
            fraction,
            icon,
            tooltip,
            asset_server,
            size,
            Color::WHITE,
        )
    }
    fn spawn_knob_ex<T>(
        &mut self,
        exponential: bool,
        setter: fn(&mut T, i32),
        getter: fn(&T) -> f32,
        resource: &T,
        fraction: f32,
        icon: &str,
        tooltip: &str,
        asset_server: &AssetServer,
        size: f32,
        color: Color,
    ) -> &mut Self
    where
        T: Resource,
    {
        let initial_value = getter(resource);
        let initial_degs = if exponential {
            let base = ((initial_value / fraction.max(f32::EPSILON)) * 10.0)
                .max(0.0)
                .sqrt();
            (base * 10.0).clamp(0.0, 270.0)
        } else {
            ((initial_value / fraction.max(f32::EPSILON)) * 10.0).clamp(0.0, 270.0)
        };
        let initial_ring_rotation = initial_degs - 135.0;
        let initial_notch_offset = -6.0 * (initial_degs / 360.0);
        self.insert((
            KnobMain,
            Name::new("Knob"),
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                margin: UiRect::right(Val::Px(10.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderRadius::all(Val::Px(size / 2.0)),
            BackgroundColor(Color::srgb_u8(20, 20, 20)),
            ThemeBackgroundColor(Color::srgb_u8(20, 20, 20)),
            Pickable {
                should_block_lower: true,
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(size),
                    height: Val::Px(size),
                    margin: UiRect::all(Val::Px(-1.0)),
                    ..default()
                },
                KnobRing,
                BorderRadius::all(Val::Px(size / 2.0)),
                UiTransform::from_rotation(Rot2::degrees(initial_ring_rotation)),
                Pickable {
                    should_block_lower: false,
                    ..default()
                },
            ))
            .with_child((
                KnobGauge,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(size),
                    height: Val::Px(size),
                    border: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BorderColor {
                    bottom: color.with_alpha((initial_degs > 1.0) as u8 as f32),
                    right: color.with_alpha((initial_degs > 90.0) as u8 as f32),
                    top: color.with_alpha((initial_degs > 180.0) as u8 as f32),
                    left: color.with_alpha((initial_degs > 270.0) as u8 as f32),
                },
                BorderRadius::all(Val::Px(size / 2.0)),
                UiTransform::from_rotation(Rot2::degrees(135.0)),
                Pickable {
                    should_block_lower: false,
                    ..default()
                },
            ))
            .with_child((
                KnobNotch,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((size / 2.0 - 1.0) + initial_notch_offset),
                    width: Val::Px(5.0),
                    height: Val::Px(12.0),
                    ..default()
                },
                BorderRadius::all(Val::Px(2.0)),
                BackgroundColor(color),
                Pickable {
                    should_block_lower: false,
                    ..default()
                },
            ));
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(size),
                    height: Val::Px(size),
                    border: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                BorderColor {
                    bottom: Color::srgb_u8(20, 20, 20),
                    ..default()
                },
                BorderRadius::all(Val::Px(size / 2.0)),
                Pickable {
                    should_block_lower: false,
                    ..default()
                },
            ));
        })
        .with_child(make_large_hud_button_image_bundle(
            &asset_server,
            icon,
            color,
        ))
        .tooltip_on_hover(tooltip, 1.0)
        .observe(move |_: On<Add>, mut resource: ResMut<T>| {
            let value = getter(&*resource);
            setter(&mut resource, value as i32);
        })
        .observe(
            move |trigger: On<ResetKnob>,
                  mut resource: ResMut<T>,
                  children: Query<&Children>,
                  mut knob_ring_transforms: Query<&mut UiTransform, With<KnobRing>>,
                  mut knob_gauge_borders: Query<
                &mut BorderColor,
                (With<KnobGauge>, Without<KnobRing>),
            >,
                  mut knob_notch_nodes: Query<
                &mut Node,
                (With<KnobNotch>, Without<KnobRing>, Without<KnobGauge>),
            >| {
                let value = getter(&*resource);
                setter(&mut resource, value as i32);
                apply_knob_visual(
                    value,
                    exponential,
                    fraction,
                    size,
                    color,
                    trigger.entity,
                    &mut knob_ring_transforms,
                    &mut knob_gauge_borders,
                    &mut knob_notch_nodes,
                    &children,
                );
            },
        )
        .observe(
            move |trigger: On<Pointer<Drag>>,
                  mut knob_ring_transforms: Query<&mut UiTransform, With<KnobRing>>,
                  mut knob_gauge_borders: Query<
                &mut BorderColor,
                (With<KnobGauge>, Without<KnobRing>),
            >,
                  mut knob_notch_nodes: Query<
                &mut Node,
                (With<KnobNotch>, Without<KnobRing>, Without<KnobGauge>),
            >,
                  mut resource: ResMut<T>,
                  children: Query<&Children>,
                  time: Res<Time>| {
                let d = (trigger.delta.x + -trigger.delta.y) * time.delta_secs() * 30.0;
                apply_knob_delta(
                    d,
                    trigger.entity,
                    exponential,
                    fraction,
                    size,
                    setter,
                    &mut knob_ring_transforms,
                    &mut knob_gauge_borders,
                    &mut knob_notch_nodes,
                    &mut resource,
                    &children,
                );
            },
        )
        .observe(
            move |trigger: On<Pointer<Scroll>>,
                  mut knob_ring_transforms: Query<&mut UiTransform, With<KnobRing>>,
                  mut knob_gauge_borders: Query<
                &mut BorderColor,
                (With<KnobGauge>, Without<KnobRing>),
            >,
                  mut knob_notch_nodes: Query<
                &mut Node,
                (With<KnobNotch>, Without<KnobRing>, Without<KnobGauge>),
            >,
                  mut resource: ResMut<T>,
                  children: Query<&Children>,
                  time: Res<Time>,
                  mut last_scroll_time: Local<f32>| {
                let now = time.elapsed_secs();
                let step = if now - *last_scroll_time < 0.15 {
                    15.0
                } else {
                    5.0
                };
                *last_scroll_time = now;
                let d = trigger.event().y * step;
                apply_knob_delta(
                    d,
                    trigger.entity,
                    exponential,
                    fraction,
                    size,
                    setter,
                    &mut knob_ring_transforms,
                    &mut knob_gauge_borders,
                    &mut knob_notch_nodes,
                    &mut resource,
                    &children,
                );
            },
        )
        .custom_pointer_on_hover(SystemCursorIcon::EwResize)
        .observe(
            |trigger: On<Pointer<DragStart>>,
             mut commands: Commands,
             window: Single<(Entity, &Window), With<PrimaryWindow>>| {
                let current_pos: Vec2 = window.1.cursor_position().unwrap();
                commands
                    .entity(trigger.entity)
                    .try_insert(PointerExclusivityIsPreferred)
                    .try_insert(GrabbedMousePosition(current_pos));
                commands.entity(window.0).insert(CursorOptions {
                    visible: false,
                    grab_mode: CursorGrabMode::None,
                    hit_test: true,
                });
            },
        )
        .observe(
            |trigger: On<Pointer<DragEnd>>,
             mut commands: Commands,
             pos: Query<&GrabbedMousePosition>,
             mut window: Single<(Entity, &mut Window), With<PrimaryWindow>>| {
                if let Ok(pos) = pos.get(trigger.entity) {
                    window.1.set_cursor_position(Some(pos.0));
                }
                commands
                    .entity(trigger.entity)
                    .try_remove::<PointerExclusivityIsPreferred>();
                commands.entity(window.0).insert(CursorOptions {
                    visible: true,
                    grab_mode: CursorGrabMode::None,
                    hit_test: true,
                });
            },
        )
    }
}

fn apply_knob_visual(
    value: f32,
    exponential: bool,
    fraction: f32,
    size: f32,
    color: Color,
    entity: Entity,
    knob_ring_transforms: &mut Query<&mut UiTransform, With<KnobRing>>,
    knob_gauge_borders: &mut Query<&mut BorderColor, (With<KnobGauge>, Without<KnobRing>)>,
    knob_notch_nodes: &mut Query<
        &mut Node,
        (With<KnobNotch>, Without<KnobRing>, Without<KnobGauge>),
    >,
    children: &Query<&Children>,
) {
    let degs = if exponential {
        let base = ((value / fraction.max(f32::EPSILON)) * 10.0)
            .max(0.0)
            .sqrt();
        (base * 10.0).clamp(0.0, 270.0)
    } else {
        ((value / fraction.max(f32::EPSILON)) * 10.0).clamp(0.0, 270.0)
    };
    children.iter_descendants(entity).for_each(|e| {
        if let Ok(mut tx) = knob_ring_transforms.get_mut(e) {
            tx.rotation = Rot2::degrees(degs - 135.0);
        }
    });
    children.iter_descendants(entity).for_each(|e| {
        if let Ok(mut border_color) = knob_gauge_borders.get_mut(e) {
            border_color.bottom = color.with_alpha((degs > 1.0) as u8 as f32);
            border_color.right = color.with_alpha((degs > 90.0) as u8 as f32);
            border_color.top = color.with_alpha((degs > 180.0) as u8 as f32);
            border_color.left = color.with_alpha((degs > 270.0) as u8 as f32);
        }
    });
    children.iter_descendants(entity).for_each(|e| {
        if let Ok(mut node) = knob_notch_nodes.get_mut(e) {
            node.left = Val::Px((size / 2.0 - 1.0) + (-6.0 * (degs / 360.0)));
        }
    });
}

fn apply_knob_delta<T: Resource>(
    d: f32,
    entity: Entity,
    exponential: bool,
    fraction: f32,
    size: f32,
    setter: fn(&mut T, i32),
    knob_ring_transforms: &mut Query<&mut UiTransform, With<KnobRing>>,
    knob_gauge_borders: &mut Query<&mut BorderColor, (With<KnobGauge>, Without<KnobRing>)>,
    knob_notch_nodes: &mut Query<
        &mut Node,
        (With<KnobNotch>, Without<KnobRing>, Without<KnobGauge>),
    >,
    resource: &mut T,
    children: &Query<&Children>,
) {
    let mut degs = 0.0;

    children.iter_descendants(entity).for_each(|e| {
        if let Ok(mut tx) = knob_ring_transforms.get_mut(e) {
            let current = tx.rotation.as_degrees();
            let update = (current + d).clamp(-135.0, 135.0);
            tx.rotation = Rot2::degrees(update);
            degs = update + 135.0;
            let base = degs / 10.0;
            let exponential_value = if exponential {
                exponential_graph_value(base)
            } else {
                base
            };
            setter(resource, (exponential_value * fraction) as i32);
        }
    });
    children.iter_descendants(entity).for_each(|e| {
        if let Ok(mut border_color) = knob_gauge_borders.get_mut(e) {
            border_color.bottom.set_alpha((degs > 1.0) as u8 as f32);
            border_color.right.set_alpha((degs > 90.0) as u8 as f32);
            border_color.top.set_alpha((degs > 180.0) as u8 as f32);
            border_color.left.set_alpha((degs > 270.0) as u8 as f32);
        }
    });
    children.iter_descendants(entity).for_each(|e| {
        if let Ok(mut node) = knob_notch_nodes.get_mut(e) {
            let offset = -6.0 * (degs / 360.0);
            node.left = Val::Px((size / 2.0 - 1.0) + offset);
        }
    });
}

#[derive(Component)]
struct GrabbedMousePosition(Vec2);

pub fn exponential_graph_value(x: f32) -> f32 {
    let x = x.clamp(0.0, 27.0);
    x.powi(2) / 10.0
}

pub fn inverse_exponential_graph_value(x: f32) -> f32 {
    let x = x.clamp(0.0, 72.9);
    (x * 10.0).sqrt()
}

fn make_large_hud_button_image_bundle(
    asset_server: &AssetServer,
    image_asset_name: &str,
    color: Color,
) -> impl Bundle {
    (
        Node {
            width: Val::Px(64.0),
            height: Val::Px(64.0),
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
            color,
            ..default()
        },
    )
}
