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

// Page rendering & page swap orchestration
//
// This module is mainly responsible for invoking content rendering and
// managing the its render targets.
// Specifically, it cares for the smooth dissolve between the current and
// any newly fetch content page.
//
// Note that the actual HTML parsing and rendering is done using the
// `demidom` module.
use std::{collections::HashMap, time::Duration};

use regex::Regex;

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::RenderLayers,
    ecs::system::SystemId,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        view::Hdr,
    },
    window::{CursorIcon, PrimaryWindow, SystemCursorIcon},
};
use bevy_simple_scroll_view::{ScrollTarget, ScrollView, ScrollableContent};
use bevy_tweening::lens::UiBackgroundColorLens;

use crate::{
    clients::{controller::RenderEntityContent, model::FetchEntityReason},
    shared::{
        AppState,
        layers::{RENDER_LAYER_CONTENT_OFFSCREEN, RENDER_LAYER_CONTENT_ONSCREEN},
        settings::Config,
        tweens::UiImageNodeAlphaLens,
        widgets::buttons::MenuButtonDisabled,
    },
};

use super::{
    ContentDarkMode, ContentMode, EntityRenderingCompleted, NpcAnchor, ScrollToAnchor,
    clipboard::CopyOnRightClick,
    context::ContentContext,
    demidom::*,
    header::{make_header_bundle, update_header_buttons_state},
    viewport::get_split_content_metrics,
};

pub struct PageRendererPlugin;

impl Plugin for PageRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Live),
            (setup, setup_offscreen_node).chain(),
        );
        app.add_systems(
            Update,
            resize_offscreen_node.run_if(in_state(AppState::Live)),
        );
        app.add_systems(
            Update,
            update_header_buttons_state.run_if(in_state(ContentMode::SplitScreen)),
        );
        app.add_systems(
            Update,
            scroll_to_anchor_continous.run_if(in_state(ContentMode::SplitScreen)),
        );
        app.insert_resource(ContentContext::default());
        app.insert_state(ContentMode::MapOnly);
        app.add_observer(on_render_entity_content);
        app.add_observer(on_scroll_to_anchor);
    }
}

#[derive(Component)]
pub struct ContentCamera;

#[derive(Component)]
pub struct ContentPage;

#[derive(Component)]
pub struct ContentHeader;

#[derive(Component)]
struct ContentText;

#[derive(Component)]
struct ContentViewport;

#[derive(Resource)]
struct ContentAssets {
    main_font: Handle<Font>,
    main_font_bold: Handle<Font>,
    title_font: Handle<Font>,
    sigil_font: Handle<Font>,
    render_target: Handle<Image>,
    swap_system: SystemId,
}

#[derive(Component)]
struct ContentOffScreenNode;

#[derive(Component)]
struct ContentScroll;

#[derive(Component)]
struct PageCamera;

pub struct ContentPageModel {
    demidom: ContentDemidom,
}

impl ContentPageModel {
    pub fn from_entity_html(uid: &str, html: &str) -> Self {
        let data = html.replace("\n", "");
        let re = Regex::new(r#"> +<"#).unwrap();
        let data = re.replace_all(&data, "><").to_string();
        let parts: Vec<&str> = data.split("</h4>").collect();
        let (header_html, body_html) = (
            format!("{}</h4>", parts.get(0).unwrap()),
            parts[1..].join("</h4>"),
        );
        let mut header_demidom = DemidomElements::default();
        let mut body_demidom = DemidomElements::default();
        header_demidom.parse_entity_html(&header_html);
        body_demidom.parse_entity_html(&body_html);
        // TODO: Document this usecase
        ContentPageModel {
            demidom: ContentDemidom {
                uid: uid.to_string(),
                header: header_demidom.elements.take(),
                body: body_demidom.elements.take(),
            },
        }
    }
}

fn create_render_target(content_page_size: &Vec2) -> Image {
    let size = Extent3d {
        width: content_page_size.x as u32,
        height: content_page_size.y as u32,
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

fn resize_offscreen_node(
    windows: Query<&Window>,
    mut resize_events: MessageReader<bevy::window::WindowResized>,
    mut images: ResMut<Assets<Image>>,
    mut content_assets: ResMut<ContentAssets>,
    mut offscreen: Single<&mut Node, With<ContentOffScreenNode>>,
    mut page_cam: Single<&mut Camera, With<PageCamera>>,
    mut viewport: Single<&mut ImageNode, With<ContentViewport>>,
) {
    for resize_event in resize_events.read() {
        if let Ok(window) = windows.get(resize_event.window) {
            let window_size = window.physical_size();
            if window_size.x < 128 || window_size.y < 128 {
                return;
            }
            let (_, _, _, content_page_size) = get_split_content_metrics(window_size);
            let handle = images.add(create_render_target(&content_page_size));
            content_assets.render_target = handle.clone();
            offscreen.width = Val::Px(content_page_size.x as f32 * 0.8);
            page_cam.target = handle.clone().into();
            viewport.image = handle.clone();
        } else {
            warn!("Received a resize event without a window?")
        }
    }
}

fn setup_offscreen_node(
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut content_assets: ResMut<ContentAssets>,
    mut offscreen: Single<&mut Node, With<ContentOffScreenNode>>,
    mut page_cam: Single<&mut Camera, With<PageCamera>>,
    mut viewport: Single<&mut ImageNode, With<ContentViewport>>,
) {
    let window_size = window.physical_size();
    let (_, _, _, content_page_size) = get_split_content_metrics(window_size);
    let handle = images.add(create_render_target(&content_page_size));
    content_assets.render_target = handle.clone();
    offscreen.width = Val::Px(content_page_size.x as f32 * 0.8);
    page_cam.target = handle.clone().into();
    viewport.image = handle.clone();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    let handle = images.add(create_render_target(&Vec2::new(1024.0, 1024.0)));

    let swap_system = commands.register_system(swap_content_text_node);
    commands.insert_resource(ContentAssets {
        main_font: asset_server.load("fonts/crimsonpro.ttf"),
        main_font_bold: asset_server.load("fonts/crimsonpro-bold.ttf"),
        title_font: asset_server.load("fonts/oswald.ttf"),
        sigil_font: asset_server.load("fonts/sigils.ttf"),
        render_target: handle.clone(),
        swap_system,
    });

    let cam2 = commands
        .spawn((
            Name::new("PageCamera"),
            PageCamera,
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(Color::WHITE),
                target: handle.clone().into(),
                ..Default::default()
            },
            RenderLayers::from_layers(&[RENDER_LAYER_CONTENT_OFFSCREEN]),
        ))
        .id();

    let cam = commands
        .spawn((
            Name::new("ContentCamera"),
            ContentCamera,
            Camera3d::default(),
            Msaa::default(),
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            Hdr,
            Camera {
                order: 1,
                viewport: Some(bevy::camera::Viewport::default()),
                clear_color: ClearColorConfig::None,
                ..Default::default()
            },
            RenderLayers::from_layers(&[RENDER_LAYER_CONTENT_ONSCREEN]),
        ))
        .id();
    commands
        .spawn((
            ContentOffScreenNode,
            Name::new("ContentOffScreen"),
            Node {
                position_type: PositionType::Relative,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                align_self: AlignSelf::Stretch,
                justify_self: JustifySelf::Stretch,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::FlexStart,
                align_content: AlignContent::FlexStart,
                overflow: Overflow::scroll_y(), // n.b.
                ..default()
            },
            RenderLayers::layer(RENDER_LAYER_CONTENT_OFFSCREEN),
            UiTargetCamera(cam2),
        ))
        .with_child(make_text_node_bundle());
    commands
        .spawn((
            Name::new("ContentPanel"),
            ContentPage,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(300.0),
                height: Val::Px(300.0),
                ..default()
            },
            RenderLayers::layer(RENDER_LAYER_CONTENT_OFFSCREEN),
            UiTargetCamera(cam),
            BackgroundColor(Color::WHITE),
        ))
        .with_children(|mut c| {
            make_header_bundle(&mut c, &asset_server);
            c.spawn((
                ContentScroll,
                ScrollView {
                    scroll_speed: 9000.0,
                },
                Name::new("ContentPage"),
                Node {
                    position_type: PositionType::Relative,
                    left: Val::Px(0.0),
                    top: Val::Px(100.0),
                    width: Val::Percent(100.0),
                    padding: UiRect {
                        left: Val::Percent(10.),
                        right: Val::Percent(10.),
                        top: Val::Percent(5.),
                        bottom: Val::Percent(15.),
                    },
                    align_self: AlignSelf::Stretch,
                    justify_self: JustifySelf::Stretch,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::FlexStart,
                    align_content: AlignContent::FlexStart,
                    overflow: Overflow::scroll_y(), // n.b.
                    ..default()
                },
                RenderLayers::layer(RENDER_LAYER_CONTENT_ONSCREEN),
                UiTargetCamera(cam),
            ))
            .with_child(make_viewport_bundle(handle.clone()));
        });
}

fn swap_content_text_node(
    mut commands: Commands,
    offscreen_host: Single<Entity, With<ContentOffScreenNode>>,
    mut content_entity: Single<(Entity, &mut ComputedNode), With<ContentText>>,
    viewport_host: Single<Entity, With<ContentScroll>>,
    content_assets: Res<ContentAssets>,
) {
    // Clear any existing viewport content
    commands
        .entity(*viewport_host)
        .despawn_related::<Children>();
    // Move the content rendered offscreen to the viewport
    commands
        .entity(content_entity.0)
        .remove::<ChildOf>()
        .insert(ChildOf(*viewport_host))
        .remove::<ContentText>();
    // NOTE: we have to do this silly thing to make bevy_simple_scroll_view plugin
    // happy - as it will only set max_y after ComputedNode will change.
    content_entity.1.inverse_scale_factor = 1.0;

    // Create a new offscreen bundle
    commands
        .spawn(make_text_node_bundle())
        .insert(ChildOf(*offscreen_host));
    commands
        .spawn(make_viewport_bundle(content_assets.render_target.clone()))
        .insert(ChildOf(*viewport_host));
}

fn make_text_node_bundle() -> impl Bundle {
    (
        ScrollableContent::default(),
        Node {
            position_type: PositionType::Relative,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            align_self: AlignSelf::Stretch,
            justify_self: JustifySelf::Stretch,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::FlexStart,
            align_content: AlignContent::FlexStart,
            ..default()
        },
        Pickable {
            should_block_lower: true,
            is_hoverable: true,
        },
        ContentText,
    )
}

fn make_viewport_bundle(image: Handle<Image>) -> impl Bundle {
    (
        ContentViewport,
        ImageNode {
            image,
            color: Color::WHITE.with_alpha(0.0),
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
    )
}

pub struct ContentDemidom {
    pub uid: String,
    pub header: HashMap<usize, Element>,
    pub body: HashMap<usize, Element>,
}

fn render_entity_page(
    page_model: &ContentPageModel,
    header: Single<Entity, With<ContentHeader>>,
    page: Single<Entity, With<ContentText>>,
    off: Single<(Entity, &BackgroundColor), With<ContentPage>>,
    viewport: Single<Entity, With<ContentViewport>>,
    mut page_camera: Single<&mut Camera, With<PageCamera>>,
    content_assets: &ContentAssets,
    content_stuff: &ContentContext,
    content_dark_mode: &ContentDarkMode,
    asset_server: &AssetServer,
    app_config: &Config,
    mut commands: &mut Commands,
) -> Option<DemidomResponse> {
    let data = &page_model.demidom;
    commands.entity(*page).despawn_related::<Children>();
    commands.entity(*header).despawn_related::<Children>();
    // NOTE: ensure we apply the correct day/night theme in case
    // this has changed and we are refreshing the content page
    let page_theme = app_config.daytime_page_theme(&*content_dark_mode);
    {
        commands.entity(off.0).insert(bevy_tweening::Animator::new(
            bevy_tweening::Tween::new(
                EaseFunction::QuadraticIn,
                Duration::from_millis(200),
                UiBackgroundColorLens {
                    start: off.1.0,
                    end: page_theme.bg,
                },
            ),
        ));
        page_camera.clear_color = ClearColorConfig::Custom(page_theme.bg);
    }
    commands
        .entity(*viewport)
        .try_insert(bevy_tweening::Animator::new(
            bevy_tweening::Tween::new(
                EaseFunction::QuadraticIn,
                Duration::from_millis(400),
                UiImageNodeAlphaLens { from: 0.0, to: 1.0 },
            )
            .with_completed_system(content_assets.swap_system),
        ));
    let theme_icons = DemidomIcons {
        toc_icon: asset_server.load("icons/icon-pin.ktx2"),
        map_icon: asset_server.load("icons/icon-region.ktx2"),
        dice_icon: asset_server.load("icons/icon-dice.ktx2"),
        chevron_icon: asset_server.load("icons/icon-chevron-128.ktx2"),
    };
    let mut body_context = DemidomRenderContext {
        parent: *page,
        theme: DemidomTheme {
            regular_text_font: content_assets.main_font.clone(),
            bold_text_font: content_assets.main_font_bold.clone(),
            regular_title_font: content_assets.title_font.clone(),
            sigils_font: content_assets.sigil_font.clone(),
            icons: theme_icons.clone(),
            text_color: page_theme.text,
            link_background: page_theme.link_bg,
            ruler_color: page_theme.link_bg,
            table_row_odd: page_theme.table_row_odd,
            table_row_even: page_theme.table_row_even,
            table_header: page_theme.table_header,
        },
        table: None,
        space_if_needed: 0,
        text_node_params: crate::content::demidom::TextNodeParams::default(),
        unlocked: content_stuff.unlocked,
        spoilers: content_stuff.spoilers,
        attachments: None,
    };
    let font = DemidomContextFont {
        face: body_context.theme.regular_text_font.clone(),
        size: 21.0,
        background: None,
    };
    let possible_body_rendering_response = render_demidom(
        &mut commands,
        data.body.clone(),
        &mut body_context,
        font.clone(),
        1,
    );
    if let Some(body_rendering_response) = possible_body_rendering_response {
        commands
            .entity(*page)
            .insert(DemidomClipboardText {
                text: body_rendering_response.text.clone(),
            })
            .copy_on_right_click(&body_context);
    } else {
        warn!("That's odd, body rendering did not have a response value.",)
    }

    let mut header_context = DemidomRenderContext {
        parent: *header,
        theme: DemidomTheme {
            regular_text_font: content_assets.main_font.clone(),
            bold_text_font: content_assets.main_font_bold.clone(),
            regular_title_font: content_assets.title_font.clone(),
            sigils_font: content_assets.sigil_font.clone(),
            text_color: Color::WHITE,
            link_background: Color::srgb_u8(5, 5, 5),
            icons: theme_icons.clone(),
            ruler_color: Color::srgb_u8(230, 230, 230),
            table_row_odd: page_theme.table_row_odd,
            table_row_even: page_theme.table_row_even,
            table_header: page_theme.table_header,
        },
        table: None,
        space_if_needed: 0,
        text_node_params: crate::content::demidom::TextNodeParams::default(),
        unlocked: content_stuff.unlocked,
        spoilers: content_stuff.spoilers,
        attachments: None,
    };
    let header_rendering_response = render_demidom(
        &mut commands,
        data.header.clone(),
        &mut header_context,
        font,
        1,
    );
    header_rendering_response
}

fn scroll_to_anchor_continous(
    cmd: Single<(Entity, &ScrollToAnchorCommand)>,
    mut commands: Commands,
    page: Single<(Entity, &ScrollableContent, &ChildOf), With<DemidomClipboardText>>,
    anchors: Query<(&NpcAnchor, &UiGlobalTransform)>,
    global_transforms: Query<&UiGlobalTransform>,
    computed_nodes: Query<&ComputedNode>,
) {
    let (page_entity, page_scrollable, page_child_of) = &*page;

    let (Ok(parent_global_transform), Ok(parent_computed_node)) = (
        global_transforms.get(page_child_of.0),
        computed_nodes.get(page_child_of.0),
    ) else {
        return;
    };

    let gap =
        ((parent_global_transform.translation.y * 2.0) - parent_computed_node.size.y) / 2.0;

    let anchor_entity = anchors.iter().find(|v| v.0.0 == cmd.1.0);
    if let Some((_, tx)) = anchor_entity {
        if tx.translation.y > 0.0 && page_scrollable.max_scroll > 0.0 {
            commands
                .entity(*page_entity)
                .insert(ScrollTarget::from_value(
                    page_scrollable.pos_y - tx.translation.y + gap,
                    page_scrollable.max_scroll,
                ));
            commands.entity(cmd.0).try_despawn();
        }
    }
}

#[derive(Component)]
struct ScrollToAnchorCommand(String);

fn on_scroll_to_anchor(
    trigger: On<ScrollToAnchor>,
    mut commands: Commands,
    existing_commands: Query<Entity, With<ScrollToAnchorCommand>>,
) {
    existing_commands
        .iter()
        .for_each(|e| commands.entity(e).try_despawn());
    commands.spawn(ScrollToAnchorCommand(trigger.anchor.clone()));
}

fn on_render_entity_content(
    trigger: On<RenderEntityContent>,
    mut commands: Commands,
    content_assets: Option<Res<ContentAssets>>,
    header: Single<Entity, With<ContentHeader>>,
    page: Single<Entity, With<ContentText>>,
    off: Single<(Entity, &BackgroundColor), With<ContentPage>>,
    viewport: Single<Entity, With<ContentViewport>>,
    page_camera: Single<&mut Camera, With<PageCamera>>,
    asset_server: Res<AssetServer>,
    mut next_content_mode: ResMut<NextState<ContentMode>>,
    mut content_stuff: ResMut<ContentContext>,
    content_dark_mode: Res<ContentDarkMode>,
    app_config: Res<Config>,
    window: Single<Entity, With<PrimaryWindow>>,
    reroller: Single<Entity, With<super::header::RerollButtonMarker>>,
) {
    let Some(content_assets) = content_assets else {
        return;
    };
    let data = &trigger.data;
    let why = &trigger.why;

    let resp = render_entity_page(
        data,
        header,
        page,
        off,
        viewport,
        page_camera,
        content_assets.as_ref(),
        content_stuff.as_ref(),
        content_dark_mode.as_ref(),
        asset_server.as_ref(),
        app_config.as_ref(),
        &mut commands,
    );

    next_content_mode.set(ContentMode::SplitScreen);

    commands
        .entity(*window)
        .insert(CursorIcon::System(SystemCursorIcon::Default));

    // NOTE: If this is just a content refresh, no need to continue.
    if *why == FetchEntityReason::Refresh {
        return;
    }

    content_stuff.set_current_uid(trigger.uid.to_string(), why);
    if *why != FetchEntityReason::History {
        content_stuff.invalidate_forward_navigation();
    }

    if let Some(resp) = resp {
        if !resp.rerollable {
            if content_stuff.unlocked {
                commands.entity(*reroller).try_insert(MenuButtonDisabled);
            }
            content_stuff.rerollable = false;
        } else {
            if content_stuff.unlocked {
                commands
                    .entity(*reroller)
                    .try_remove::<MenuButtonDisabled>();
            }
            content_stuff.rerollable = true;
        }
        commands.trigger(EntityRenderingCompleted {
            uid: data.demidom.uid.clone(),
            anchor: trigger.anchor.clone(),
            map_coords: resp.coords,
            fetch_reason: why.clone(),
        })
    }
}
