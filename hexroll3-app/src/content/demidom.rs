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

use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

use bevy::text::LineHeight;
use bevy::{prelude::*, window::SystemCursorIcon};

use cosmic_text::Edit;

use bevy_ui_text_input::{TextInputBuffer, TextInputMode, TextInputNode};
use html5ever::{
    Attribute, ExpandedName, QualName, parse_document,
    tendril::*,
    tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink},
};

use crate::{
    clients::model::{FetchEntityReason, RerollEntity},
    content::ContentMode,
    dice::RollDice,
    hexmap::elements::{
        AppendSandboxEntity, AppendSubject, FetchEntityFromStorage, HexMapData,
    },
    shared::{
        camera::MapCoords,
        layers::HEIGHT_OF_TOKENS,
        tweens::{UiNodeSizeLens, UiNodeSizeLensMode, UiTransformRotationLens},
        widgets::{cursor::PointerOnHover, link::ContentHoverLink},
    },
    tokens::SpawnTokenFromLibrary,
};

use super::{
    EditableAttributeParams, EditableProxy, NpcAnchor, ThemeBackgroundColor,
    clipboard::CopyOnRightClick, header::EditableTitleInput, spoiler::ContentIsSpoiler,
};

#[derive(Clone, Debug)]
pub enum DemidomContextAttachment {
    EditableAttribute(String, Option<String>, Option<String>),
    DataSettlement(String),
    DataMapLabel,
    Rerollable(bool),
    Link,
}

#[derive(Clone, Debug)]
pub struct ElementAttributes {
    id: Option<String>,
    class: Option<String>,
    hidden: Option<bool>,
    attachments: Option<Vec<DemidomContextAttachment>>,
}

impl ElementAttributes {
    fn new() -> Self {
        ElementAttributes {
            id: None,
            class: None,
            hidden: None,
            attachments: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LinkAttributes {
    href: String,
}

#[derive(Clone, Debug, Default, Component)]
pub struct RollerAttributes {
    uid: String,
    class_override: String,
    attr: String,
    is_map_reload_needed: bool,
    is_appender: bool,
}

#[derive(Clone, Debug)]
pub enum TableType {
    Balanced,
    MaxLastColumn,
}

#[derive(Clone, Debug)]
pub enum ElementType {
    Div(ElementAttributes),
    Header(i32),
    Paragraph,
    Link(LinkAttributes),
    Icon(LinkAttributes),
    DiceRoller(LinkAttributes),
    EntityRoller(RollerAttributes),
    Spawner(String, MapCoords),
    Table(TableType),
    TableRow,
    TableCell,
    TableHeader,
    List,
    ListItem,
    LineBreak,
    HorizontalLine,
    Strong,
    Small,
    Em,
    Text(String),
    Blockquote,
    Coords(MapCoords),
    Anchor(String),
    Bundle,
    NoOp,
}

impl ElementType {
    fn is_link(&self) -> bool {
        match self {
            ElementType::Link(_)
            | ElementType::Icon(_)
            | ElementType::DiceRoller(_)
            | ElementType::EntityRoller(_) => true,
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct Element {
    element: ElementType,
    parent_id: usize,
    children: Vec<usize>,
}

pub struct DemidomResponse {
    pub coords: Option<MapCoords>,
    pub text: String,
    pub rerollable: bool,
}

impl DemidomResponse {
    fn propagate(&mut self, v: Self) {
        self.rerollable = v.rerollable && self.rerollable;
        self.coords = v.coords.or(self.coords.take());
        self.text.push_str(&v.text);
    }
}

#[derive(Component)]
pub struct AccordionOf(Entity);

#[derive(Component, PartialEq)]
pub enum AccordionVisibility {
    Inherited,
    Hidden,
}

#[derive(Component)]
pub struct AccordionChevron(Entity);

#[derive(Component)]
pub struct DemidomLink {
    pub url: String,
}

#[derive(Component, Reflect)]
pub struct DemidomClipboardText {
    pub text: String,
}

#[derive(Default)]
pub struct DemidomElements {
    pub elements: Rc<RefCell<HashMap<usize, Element>>>,
}

impl DemidomElements {
    pub fn parse_entity_html(&mut self, html_content: &str) {
        self.elements.as_ref().borrow_mut().clear();
        let sink = Sink {
            next_id: Cell::new(1),
            names: RefCell::new(HashMap::new()),
            elements: self.elements.clone(),
            last_added: RefCell::new(0),
            last_text_element: Cell::new(0),
        };
        parse_document(sink, Default::default())
            .from_utf8()
            .one(html_content.as_bytes());
    }
}

#[derive(Clone)]
pub struct DemidomIcons {
    pub toc_icon: Handle<Image>,
    pub map_icon: Handle<Image>,
    pub dice_icon: Handle<Image>,
    pub chevron_icon: Handle<Image>,
    pub skull_icon: Handle<Image>,
}

#[derive(Clone)]
pub struct DemidomTheme {
    pub regular_text_font: Handle<Font>,
    pub bold_text_font: Handle<Font>,
    pub regular_title_font: Handle<Font>,
    pub sigils_font: Handle<Font>,
    pub icons: DemidomIcons,
    pub text_color: Color,
    pub link_background: Color,
    pub ruler_color: Color,
    pub table_row_odd: Color,
    pub table_row_even: Color,
    pub table_header: Color,
}

#[derive(Clone)]
pub struct TableContext {
    pub max_cols: u16,
    pub curr_col: u16,
}

pub struct DemidomRenderContext {
    pub parent: Entity,
    pub theme: DemidomTheme,
    pub table: Option<TableContext>,
    pub space_if_needed: u32,
    pub text_node_params: TextNodeParams,
    pub unlocked: bool,
    pub spoilers: bool,
    pub attachments: Option<Vec<DemidomContextAttachment>>,
}

impl DemidomRenderContext {
    pub fn cascade(&mut self, g: Entity) -> Self {
        DemidomRenderContext {
            parent: g,
            theme: self.theme.clone(),
            table: self.table.clone(),
            space_if_needed: self.space_if_needed,
            text_node_params: self.text_node_params.clone(),
            unlocked: self.unlocked,
            spoilers: self.spoilers,
            attachments: self.attachments.clone(),
        }
    }
    pub fn scope(&mut self) -> Self {
        DemidomRenderContext {
            parent: self.parent,
            theme: self.theme.clone(),
            table: self.table.clone(),
            space_if_needed: self.space_if_needed,
            text_node_params: self.text_node_params.clone(),
            unlocked: self.unlocked,
            spoilers: self.spoilers,
            attachments: self.attachments.clone(),
        }
    }
}

#[derive(Clone)]
pub struct DemidomContextFont {
    pub face: Handle<Font>,
    pub size: f32,
    pub background: Option<Color>,
}

impl DemidomContextFont {
    pub fn with_font(&self, font: Handle<Font>) -> Self {
        let mut ret = self.clone();
        ret.face = font;
        ret
    }
    pub fn with_size(&self, size: f32) -> Self {
        let mut ret = self.clone();
        ret.size = size;
        ret
    }
    pub fn with_background(&self, color: Color) -> Self {
        let mut ret = self.clone();
        ret.background = Some(color);
        ret
    }
}

fn make_link_node_bundle(
    commands: &mut DemidomRenderContext,
    font_size: f32,
    is_stretched: bool,
) -> impl Bundle {
    let left_padding = font_size * (5.0 / 24.0);
    let vertical_padding_fix = font_size * (1.0 / 7.0);
    let height = font_size * (32.0 / 26.0);
    (
        Node {
            position_type: PositionType::Relative,
            display: Display::Flex,
            flex_wrap: FlexWrap::NoWrap,
            flex_shrink: 0.0,
            margin: UiRect {
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(vertical_padding_fix),
                bottom: Val::Px(0.0),
            },
            padding: UiRect {
                left: Val::Px(left_padding),
                right: Val::Px(0.0),
                top: Val::Px(-vertical_padding_fix),
                bottom: Val::Px(0.0),
            },
            height: Val::Px(height),
            width: if is_stretched {
                Val::Percent(100.0)
            } else {
                Val::Auto
            },
            justify_content: if is_stretched {
                JustifyContent::Center
            } else {
                JustifyContent::Default
            },
            ..default()
        },
        Pickable {
            should_block_lower: true,
            ..default()
        },
        BorderRadius::all(Val::Px(left_padding)),
        BackgroundColor(commands.theme.link_background),
        ThemeBackgroundColor(commands.theme.link_background),
    )
}

#[derive(Default, Clone)]
pub struct TextNodeParams {
    align_self: AlignSelf,
}

#[derive(Component)]
pub enum RollerIcon {
    Visible,
    Hidden,
}

impl DemidomRenderContext {
    pub fn spawn_horizontal_line(&mut self, commands: &mut Commands, font_size: f32) {
        let height = font_size * (10.0 / 24.0);
        commands
            .spawn((
                Pickable {
                    should_block_lower: false,
                    ..default()
                },
                Node {
                    position_type: PositionType::Relative,
                    width: Val::Percent(100.0),
                    height: Val::Px(height),
                    border: UiRect {
                        left: Val::Px(0.0),
                        right: Val::Px(0.0),
                        top: Val::Px(1.0),
                        bottom: Val::Px(0.0),
                    },
                    ..default()
                },
                BorderColor::all(self.theme.ruler_color),
            ))
            .insert(ChildOf(self.parent));
    }
    pub fn spawn_spacer(&mut self, commands: &mut Commands, font_size: f32) {
        let height = font_size * (10.0 / 24.0);
        commands
            .spawn((
                Pickable {
                    should_block_lower: false,
                    ..default()
                },
                Node {
                    position_type: PositionType::Relative,
                    width: Val::Percent(100.0),
                    height: Val::Px(height),
                    ..default()
                },
            ))
            .insert(ChildOf(self.parent));
    }
    pub fn spawn_linked_icon(&mut self, commands: &mut Commands, href: &str, font_size: f32) {
        let icon = if href.ends_with("toc") {
            self.theme.icons.toc_icon.clone()
        } else {
            self.theme.icons.map_icon.clone()
        };
        let size = font_size * (40.0 / 24.0);
        let _link_box = commands
            .spawn((
                ImageNode {
                    color: Color::srgba_u8(255, 255, 255, 255),
                    image: icon,
                    ..default()
                },
                Node {
                    width: Val::Px(size),
                    height: Val::Px(size),
                    align_self: AlignSelf::FlexEnd,
                    margin: UiRect {
                        left: Val::Px(0.0),
                        right: Val::Px(size / 8.0),
                        top: Val::Px(-size / 8.0),
                        bottom: Val::Px(0.0),
                    },
                    ..default()
                },
                Pickable {
                    should_block_lower: true,
                    ..default()
                },
                BorderRadius::all(Val::Px(size / 8.0)),
                BackgroundColor(self.theme.link_background),
                ThemeBackgroundColor(self.theme.link_background),
                DemidomLink {
                    url: href.to_string(),
                },
            ))
            .insert(ChildOf(self.parent))
            .hover_effect()
            .observe(link_click())
            .id();
    }

    pub fn spawn_roller_icon(
        &mut self,
        commands: &mut Commands,
        attrs: RollerAttributes,
        font_size: f32,
        visible: bool,
    ) {
        let size = font_size * (30.0 / 24.0);
        let _link_box = commands
            .spawn((
                ImageNode {
                    color: Color::srgba_u8(255, 255, 255, 255),
                    image: self.theme.icons.dice_icon.clone(),
                    ..default()
                },
                if visible {
                    RollerIcon::Visible
                } else {
                    RollerIcon::Hidden
                },
                Node {
                    display: if visible {
                        Display::DEFAULT
                    } else {
                        Display::None
                    },
                    width: Val::Px(size),
                    height: Val::Px(size),
                    align_self: AlignSelf::Center,
                    margin: UiRect {
                        left: Val::Px(0.0),
                        right: Val::Px(size / 6.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                    },
                    ..default()
                },
                Pickable {
                    should_block_lower: true,
                    ..default()
                },
                BorderRadius::all(Val::Px(size / 6.0)),
                BackgroundColor(self.theme.link_background),
                ThemeBackgroundColor(self.theme.link_background),
                attrs,
            ))
            .insert(ChildOf(self.parent))
            .hover_effect()
            .observe(roller_click())
            .id();
    }

    pub fn spawn_token_spawner(
        &mut self,
        commands: &mut Commands,
        _attrs: String,
        font_size: f32,
        coords: MapCoords,
    ) {
        let size = font_size * (30.0 / 24.0);
        let _link_box = commands
            .spawn((
                ImageNode {
                    color: Color::srgba_u8(255, 255, 255, 255),
                    image: self.theme.icons.skull_icon.clone(),
                    ..default()
                },
                Node {
                    width: Val::Px(size),
                    height: Val::Px(size),
                    align_self: AlignSelf::Center,
                    margin: UiRect {
                        left: Val::Px(0.0),
                        right: Val::Px(size / 6.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                    },
                    ..default()
                },
                Pickable {
                    should_block_lower: true,
                    ..default()
                },
                BorderRadius::all(Val::Px(size / 6.0)),
                BackgroundColor(self.theme.link_background),
                ThemeBackgroundColor(self.theme.link_background),
            ))
            .insert(ChildOf(self.parent))
            .hover_effect()
            .observe(spawner_click(coords))
            .id();
    }

    pub fn spawn_dice_link(
        &mut self,
        commands: &mut Commands,
        text: &str,
        href: &str,
        _color: Color,
        font: DemidomContextFont,
    ) {
        let bundle = make_link_node_bundle(self, font.size, false);
        let link_box = commands
            .spawn(bundle)
            .insert(DemidomLink {
                url: href.to_string(),
            })
            .insert(ChildOf(self.parent))
            .hover_effect()
            .observe(dice_link_click())
            .id();
        self.cascade(link_box)
            .spawn_text(commands, text, self.theme.text_color, font.clone());
    }

    pub fn spawn_link(
        &mut self,
        commands: &mut Commands,
        href: &str,
        _color: Color,
        font: DemidomContextFont,
    ) -> Entity {
        let bundle = make_link_node_bundle(self, font.size, false);
        commands
            .spawn(bundle)
            .insert(DemidomLink {
                url: href.to_string(),
            })
            .insert(ChildOf(self.parent))
            .hover_effect()
            .observe(link_click())
            .id()
    }

    pub fn spawn_icon(&mut self, commands: &mut Commands, font_size: f32) -> Entity {
        let size = font_size * (30.0 / 24.0);
        commands
            .spawn((
                ImageNode {
                    color: Color::srgba_u8(155, 155, 155, 255),
                    image: self.theme.icons.chevron_icon.clone(),
                    ..default()
                },
                Node {
                    width: Val::Px(size),
                    height: Val::Px(size),
                    ..default()
                },
                Pickable {
                    should_block_lower: true,
                    ..default()
                },
            ))
            .insert(ChildOf(self.parent))
            .id()
    }

    pub fn spawn_text(
        &mut self,
        commands: &mut Commands,
        text: &str,
        color: Color,
        font: DemidomContextFont,
    ) {
        let mut words: Vec<String> = text
            .split_whitespace()
            .enumerate()
            .map(|(i, word)| {
                if i < text.split_whitespace().count() - 1 {
                    format!("{} ", word)
                } else {
                    word.to_string()
                }
            })
            .collect();

        if text.ends_with(" ") {
            words.push(" ".to_string());
        }

        // Fuse word sequence starting with GLUE_FORWARD or ending with GLUE_BACKWARDS
        // to prevent intra-element wrapping.
        if words.len() > 1 {
            let mut merged: Vec<String> = Vec::with_capacity(words.len());
            for w in words.into_iter() {
                if first_char_in(&w, GLUE_BACKWARD)
                    || merged
                        .last()
                        .is_some_and(|p| last_non_ws_in(p, GLUE_FORWARD))
                {
                    if let Some(last) = merged.last_mut() {
                        last.push_str(&w);
                        continue;
                    }
                }
                merged.push(w);
            }
            words = merged;
        }

        if self.space_if_needed > 0
            && !words
                .first()
                .is_some_and(|w| first_char_in(w, GLUE_BACKWARD))
        {
            words.insert(0, " ".to_string());
            self.space_if_needed = self.space_if_needed.saturating_sub(1);
        }

        // NOTE: use CHUNK_SIZE = 2 or 3 to optimize performance by reducing
        // the number of Node entities spawned - but with a slight reduction
        // in text formatting quality.
        const CHUNK_SIZE: usize = 1;
        // TODO: Another potential optimization strategy is to start with a small number of chunk size
        // and then increase it as the number of words grow.
        // Alternatively, have large chunks for smaller words etc..
        // But this is working okay-ish for now.
        for chunk in words.chunks(CHUNK_SIZE) {
            commands
                .spawn((
                    Name::new(chunk.join("").clone()),
                    TextLayout::new_with_justify(Justify::Left),
                    Text::new(""),
                    // NOTE: The following is needed to prevent double-triggering dice rolls
                    Pickable {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                    Node {
                        position_type: PositionType::Relative,
                        align_self: self.text_node_params.align_self,
                        ..default()
                    },
                ))
                .with_children(|c| {
                    for w in chunk {
                        c.spawn((
                            TextSpan::new(w),
                            TextFont {
                                font: font.face.clone(),
                                font_size: font.size,
                                line_height: LineHeight::RelativeToFont(1.333333),
                                ..default()
                            },
                            TextColor(color),
                        ));
                    }
                })
                .insert(ChildOf(self.parent));
        }
    }
}

pub fn render_demidom(
    mut commands: &mut Commands,
    demidom: HashMap<usize, Element>,
    context: &mut DemidomRenderContext,
    font: DemidomContextFont,
    n: usize,
) -> Option<DemidomResponse> {
    let mut ret = DemidomResponse {
        coords: None,
        text: String::new(),
        rerollable: true,
    };
    if let Some(element_to_render) = demidom.get(&n) {
        let children_to_render = element_to_render.children.clone();
        match &element_to_render.element {
            ElementType::Coords(coords) => {
                debug!("Map coords element detected {:?}", coords);
                ret.coords = Some(coords.clone());
            }
            ElementType::Anchor(id) => {
                commands
                    .spawn((
                        Name::new("NpcAnchor"),
                        NpcAnchor(id.clone()),
                        Node {
                            display: Display::Flex,
                            position_type: PositionType::Relative,
                            ..default()
                        },
                        Visibility::Hidden,
                    ))
                    .insert(ChildOf(context.parent));
            }
            ElementType::Header(level) => {
                let header_font_size = font.size
                    * match level {
                        6 => 1.2,
                        5 => 1.4,
                        4 => 1.6,
                        3 => 1.8,
                        2 => 2.0,
                        1 => 3.0,
                        _ => 1.0,
                    };
                ret.text.push_str("\n\n## ");
                context.spawn_spacer(&mut commands, font.size);
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        context,
                        font.with_font(context.theme.regular_title_font.clone())
                            .with_size(header_font_size),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
                ret.text.push_str("\n");
                context.spawn_spacer(&mut commands, font.size);
            }
            ElementType::Paragraph => {
                let g = commands
                    .spawn_empty()
                    .make_clipboard_container(context)
                    .id();
                context.spawn_spacer(&mut commands, font.size);
                ret.text.push_str("\n");

                // NOTE: There's no point in cascading the context per child, so instead
                // let's create one subcontext and reuse it.
                // (This might be the case in other uses of cascade to pay attention)
                let mut subcontext = context.cascade(g);
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        &mut subcontext,
                        font.clone(),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
                commands.entity(g).insert(DemidomClipboardText {
                    text: ret.text.clone(),
                });
                ret.text.push_str("\n");
                context.spawn_spacer(&mut commands, font.size);
            }
            ElementType::Blockquote => {
                let g = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Relative,
                            display: Display::Flex,
                            flex_wrap: FlexWrap::Wrap,
                            padding: UiRect::all(Val::Px(font.size)),
                            border: UiRect::left(Val::Px(3.0)),
                            ..default()
                        },
                        BorderColor::all(context.theme.text_color),
                        BackgroundColor(context.theme.table_row_odd),
                    ))
                    .copy_on_right_click(context)
                    .insert(ChildOf(context.parent))
                    .id();
                context.spawn_spacer(&mut commands, font.size);
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        // TODO: Do we really need to cascade the context here?
                        &mut context.cascade(g),
                        font.clone().with_size(font.size * 1.2),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
                commands.entity(g).insert(DemidomClipboardText {
                    text: ret.text.clone(),
                });
                context.spawn_spacer(&mut commands, font.size);
            }
            ElementType::TableRow => {
                ret.text.push_str("\n|");
                context.table.as_mut().unwrap().curr_col = 0;
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        context,
                        font.clone(),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
            }
            ElementType::TableHeader => {
                context.table.as_mut().unwrap().curr_col += 1;
                let _is_last_col = context.table.as_ref().unwrap().curr_col
                    == context.table.as_ref().unwrap().max_cols;
                let g = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Relative,
                            display: Display::Flex,
                            flex_wrap: FlexWrap::Wrap,
                            padding: UiRect {
                                left: Val::Px(5.0),
                                right: Val::Px(5.0),
                                top: Val::Px(5.0),
                                bottom: Val::Px(5.0),
                            },
                            // max_width: if is_last_col {
                            //     Val::Percent(100.0)
                            // } else {
                            //     Val::Auto
                            // },
                            ..default()
                        },
                        BackgroundColor(context.theme.table_header),
                        Pickable {
                            should_block_lower: false,
                            is_hoverable: false,
                        },
                    ))
                    .insert(ChildOf(context.parent))
                    .id();
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        &mut context.cascade(g),
                        font.with_font(context.theme.regular_title_font.clone())
                            .with_size(font.size * 0.75),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
                ret.text.push_str("|");
            }
            ElementType::Bundle => {
                let g = commands
                    .spawn((
                        Name::new("bundle"),
                        Node {
                            position_type: PositionType::Relative,
                            display: Display::Flex,
                            flex_wrap: FlexWrap::NoWrap,
                            align_content: AlignContent::Start,
                            align_self: context.text_node_params.align_self,
                            ..default()
                        },
                    ))
                    .insert(ChildOf(context.parent))
                    .id();
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        // TODO: Do we really need to cascade the context here?
                        &mut context.cascade(g),
                        font.clone(),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
            }
            ElementType::TableCell => {
                context.table.as_mut().unwrap().curr_col += 1;
                let _is_last_col = context.table.as_ref().unwrap().curr_col
                    == context.table.as_ref().unwrap().max_cols;
                let g = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Relative,
                            display: Display::Flex,
                            flex_wrap: FlexWrap::Wrap,
                            padding: UiRect {
                                left: Val::Px(5.0),
                                right: Val::Px(5.0),
                                top: Val::Px(0.0),
                                bottom: Val::Px(5.0),
                            },
                            // max_width: if is_last_col {
                            //     Val::Percent(100.0)
                            // } else {
                            //     Val::Auto
                            // },
                            align_content: AlignContent::Start,
                            ..default()
                        },
                        BackgroundColor(font.background.unwrap()),
                        Pickable {
                            should_block_lower: false,
                            is_hoverable: false,
                        },
                    ))
                    .insert(ChildOf(context.parent))
                    .id();
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        &mut context.cascade(g),
                        font.clone(),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
                ret.text.push_str("|");
            }
            ElementType::Table(table_type) => {
                ret.text.push_str("\n");
                context.spawn_spacer(&mut commands, font.size);
                // Pre-count the columns so we can give it to the grid
                let mut cols = 0;
                if let Some(c) = children_to_render.clone().into_iter().next() {
                    let demidom_borrowed = &demidom;
                    let child_element = demidom_borrowed.get(&c).unwrap();
                    for _ in &child_element.children {
                        cols += 1;
                    }
                }

                let tracks = match table_type {
                    TableType::Balanced => {
                        let mut tracks: Vec<RepeatedGridTrack> = Vec::new();
                        tracks.push(RepeatedGridTrack::auto(cols));
                        tracks
                    }
                    TableType::MaxLastColumn => {
                        let mut tracks: Vec<RepeatedGridTrack> = Vec::new();
                        for _ in 0..cols - 1 {
                            tracks.push(RepeatedGridTrack::minmax(
                                1,
                                MinTrackSizingFunction::MinContent,
                                MaxTrackSizingFunction::MinContent,
                            ));
                        }
                        tracks.push(RepeatedGridTrack::minmax(
                            1,
                            MinTrackSizingFunction::MinContent,
                            MaxTrackSizingFunction::MaxContent,
                        ));
                        tracks
                    }
                };

                let g = commands
                    .spawn((Node {
                        display: Display::Grid,
                        width: Val::Percent(100.0),
                        grid_template_columns: tracks,
                        column_gap: Val::Px(-1.0),
                        ..default()
                    },))
                    .copy_on_right_click(context)
                    .insert(ChildOf(context.parent))
                    .id();

                for (idx, c) in children_to_render.iter().enumerate() {
                    let font = if idx % 2 != 0 {
                        font.with_background(context.theme.table_row_odd)
                    } else {
                        font.with_background(context.theme.table_row_even)
                    };
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        &mut DemidomRenderContext {
                            theme: context.theme.clone(),
                            parent: g,
                            table: Some(TableContext {
                                max_cols: cols,
                                curr_col: 0,
                            }),
                            space_if_needed: context.space_if_needed,
                            text_node_params: TextNodeParams::default(),
                            unlocked: context.unlocked,
                            spoilers: context.spoilers,
                            attachments: context.attachments.clone(),
                        },
                        font.clone(),
                        *c,
                    ) {
                        ret.propagate(v);
                    }
                }
                commands.entity(g).insert(DemidomClipboardText {
                    text: ret.text.clone(),
                });
                context.spawn_spacer(&mut commands, font.size);
            }
            ElementType::LineBreak => {
                context.spawn_spacer(&mut commands, font.size);
            }
            ElementType::HorizontalLine => {
                context.spawn_horizontal_line(&mut commands, font.size);
            }
            ElementType::List => {
                let g = commands
                    .spawn((Node {
                        position_type: PositionType::Relative,
                        display: Display::Flex,
                        flex_wrap: FlexWrap::Wrap,
                        width: Val::Percent(100.0),
                        padding: UiRect {
                            left: Val::Px(30.0),
                            right: Val::Px(0.0),
                            top: Val::Px(10.0),
                            bottom: Val::Px(0.0),
                        },
                        ..default()
                    },))
                    .copy_on_right_click(&context)
                    .insert(ChildOf(context.parent))
                    .id();
                ret.text.push_str("\n");
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        &mut context.cascade(g),
                        font.clone(),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
                commands.entity(g).insert(DemidomClipboardText {
                    text: ret.text.clone(),
                });
            }
            ElementType::ListItem => {
                context.spawn_text(&mut commands, "•", context.theme.text_color, font.clone());
                let g = commands
                    .spawn((
                        Name::new("ListItem"),
                        Node {
                            position_type: PositionType::Relative,
                            display: Display::Flex,
                            flex_wrap: FlexWrap::Wrap,
                            width: Val::Percent(100.0),
                            padding: UiRect {
                                left: Val::Px(font.size * (40.0 / 24.0)),
                                right: Val::Px(0.0),
                                top: Val::Px(0.0),
                                bottom: Val::Px(0.0),
                            },
                            left: Val::Px(-30.0),
                            flex_grow: 1.0,
                            flex_basis: Val::Px(-5.0),
                            ..default()
                        },
                    ))
                    .insert(ChildOf(context.parent))
                    .id();
                ret.text.push_str("- ");
                let mut item_context = context.cascade(g);
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        &mut item_context,
                        font.clone(),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
                commands.entity(g).insert(DemidomClipboardText {
                    text: ret.text.clone(),
                });
                ret.text.push_str("\n");
                context.spawn_spacer(&mut commands, font.size);
            }
            ElementType::Icon(attributes) => {
                let href = &attributes.href;
                context.spawn_linked_icon(&mut commands, href, font.size);
            }
            ElementType::EntityRoller(attributes) => {
                context.spawn_roller_icon(
                    &mut commands,
                    attributes.clone(),
                    font.size,
                    context.unlocked,
                );
            }
            ElementType::Spawner(attributes, coords) => {
                context.spawn_token_spawner(
                    &mut commands,
                    attributes.clone(),
                    font.size,
                    coords.clone(),
                );
            }
            ElementType::Small => {
                let mut scoped_context = context.scope();
                scoped_context.text_node_params = TextNodeParams {
                    align_self: AlignSelf::Center,
                };
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        &mut scoped_context,
                        font.with_size(font.size * 0.75),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
            }
            ElementType::Link(attributes) => {
                let href = &attributes.href;
                let g = context.spawn_link(
                    &mut commands,
                    href,
                    Color::srgb_u8(0, 0, 0),
                    font.clone(),
                );
                let mut cascaded_context = context.cascade(g);
                cascaded_context
                    .attachments
                    .get_or_insert_with(Vec::new)
                    .push(DemidomContextAttachment::Link);

                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        &mut cascaded_context,
                        font.clone(),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
                context.space_if_needed += 1;
            }
            ElementType::Text(text) => {
                let elements_borrowed = &demidom;
                let parent_element =
                    elements_borrowed.get(&element_to_render.parent_id).unwrap();
                match &parent_element.element {
                    ElementType::DiceRoller(attributes) => {
                        let href = &attributes.href;
                        ret.text.push_str(&text);
                        context.spawn_dice_link(
                            &mut commands,
                            text,
                            href,
                            Color::srgb_u8(0, 0, 0),
                            font.clone(),
                        );
                        context.space_if_needed += 1;
                    }
                    ElementType::Strong => {
                        let grandparent =
                            elements_borrowed.get(&parent_element.parent_id).unwrap();
                        match &grandparent.element {
                            ElementType::DiceRoller(attributes) => {
                                ret.text.push_str(&text);
                                context.spawn_dice_link(
                                    &mut commands,
                                    text,
                                    &attributes.href,
                                    Color::srgb_u8(0, 0, 0),
                                    font.with_font(context.theme.bold_text_font.clone()),
                                );
                                context.space_if_needed += 1;
                            }
                            _ => {
                                ret.text.push_str(&text);
                                context.spawn_text(
                                    &mut commands,
                                    &adjust_text_to_app_fonts(&text),
                                    context.theme.text_color,
                                    font.with_font(context.theme.bold_text_font.clone()),
                                );
                            }
                        }
                    }
                    _ => {
                        ret.text.push_str(&text);
                        context.spawn_text(
                            &mut commands,
                            &adjust_text_to_app_fonts(&text),
                            context.theme.text_color,
                            font.clone(),
                        );
                        let cloned_text = text.clone();
                        if let Some(attachments) = &context.attachments {
                            let mut params = None;
                            let mut is_a_map_label = false;
                            let mut in_settlement = None;
                            let mut in_link = false;

                            for attachment in attachments {
                                match attachment {
                                    DemidomContextAttachment::EditableAttribute(
                                        attr,
                                        entity,
                                        cache_entity,
                                    ) => {
                                        params.get_or_insert((
                                            attr.clone(),
                                            entity.clone(),
                                            cache_entity.clone(),
                                        ));
                                    }
                                    DemidomContextAttachment::DataMapLabel => {
                                        is_a_map_label = true;
                                    }
                                    DemidomContextAttachment::DataSettlement(id) => {
                                        in_settlement.get_or_insert(id.clone());
                                    }
                                    DemidomContextAttachment::Rerollable(rerollable) => {
                                        ret.rerollable = *rerollable;
                                    }
                                    DemidomContextAttachment::Link => {
                                        in_link = true;
                                    }
                                }
                            }

                            let font = font.clone();
                            let text_color = context.theme.text_color;

                            if let Some(params) = params
                                && !in_link
                            {
                                commands.entity(context.parent)
                                    .custom_pointer_on_hover(SystemCursorIcon::Text)
                                    .observe(
                                        move |trigger: On<Pointer<Click>>,
                                            mut commands: Commands,
                                            parents: Query<&ChildOf>,
                                            children: Query<&Children>,
                                            existing: Query<Entity, (With<EditableTitleInput>,
                                                                     Without<EditableProxy>)>,
                                            proxies: Query<Entity, With<EditableProxy>>,
                                            computed: Query<&ComputedNode>| {
                                    if !existing.is_empty() {
                                        for e in existing.iter() {
                                            if parents.get(e).unwrap().0 == trigger.entity {
                                                return;
                                            }
                                            commands.entity(e).try_despawn();
                                        }
                                        for p in proxies.iter() {
                                            commands
                                                .entity(p)
                                                .insert(Visibility::Inherited)
                                                .remove::<EditableProxy>();
                                        }
                                    }

                                    let mut input_buffer = TextInputBuffer::default();

                                    input_buffer
                                        .editor
                                        .insert_string(&cloned_text.trim(), None);

                                    let mut width = 0.0;
                                    for child in children.iter_descendants(trigger.entity) {
                                        if let Ok(computed_node) = computed.get(child) {
                                            width += computed_node.content_size.x;
                                        }
                                        commands.entity(child).insert(Visibility::Hidden).insert(EditableProxy);
                                    }

                                    commands.spawn((
                                        EditableTitleInput(EditableAttributeParams {
                                            attr_name: params.0.clone(),
                                            attr_entity: params.1.clone(),
                                            cache_entity: params.2.clone(),
                                            is_a_map_label,
                                            in_settlement: in_settlement.clone(),
                                        }),
                                        TextColor(text_color.clone()),
                                        TextInputNode {
                                            mode: TextInputMode::SingleLine,
                                            clear_on_submit: false,
                                            ..default()
                                        },
                                        TextFont {
                                            font: font.face.clone(),
                                            font_size: font.size,
                                            line_height: LineHeight::RelativeToFont(1.333333),
                                            ..default()
                                        },
                                        input_buffer,
                                        BorderColor {
                                            bottom: text_color,
                                            ..default()
                                        },
                                        Node {
                                            position_type: PositionType::Absolute,
                                            top: Val::Px(0.0),
                                            left: Val::Px(0.0),
                                            width: Val::Px(width),
                                            height: Val::Px(font.size * 1.333333+2.0),
                                            border: UiRect::bottom(Val::Px(1.0)),
                                            ..default()
                                        },
                                        ChildOf(trigger.entity),
                                        Pickable {
                                            should_block_lower: true,
                                            is_hoverable: true,
                                        },
                                    ));
                                },
                            );
                            }
                        }
                    }
                }
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        context,
                        font.clone(),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
            }
            ElementType::Div(attributes) => {
                let mut spoiler = false;

                if let Some(attachments) = &attributes.attachments {
                    for attachment in attachments {
                        match attachment {
                            DemidomContextAttachment::Rerollable(rerollable) => {
                                ret.rerollable = *rerollable;
                                return Some(ret);
                            }
                            _ => {}
                        }
                    }
                }

                if let Some(id) = &attributes.id {
                    if id == "doc-title" {
                        return Some(ret);
                    }
                    if id.starts_with("editable-title") {
                        let g = commands
                            .spawn((
                                Name::new("EditableTitle"),
                                if id == "editable-title-container" {
                                    Node {
                                        position_type: PositionType::Absolute,
                                        bottom: Val::Px(5.0),
                                        display: Display::Flex,
                                        ..default()
                                    }
                                } else {
                                    Node {
                                        position_type: PositionType::Relative,
                                        display: Display::Flex,
                                        ..default()
                                    }
                                },
                            ))
                            .insert(ChildOf(context.parent))
                            .id();
                        let mut sub_context = context.cascade(g);
                        sub_context
                            .attachments
                            .get_or_insert_with(Vec::new)
                            .extend_from_slice(
                                &attributes.attachments.as_ref().unwrap_or(&Vec::new()),
                            );
                        for c in children_to_render {
                            if let Some(v) = render_demidom(
                                &mut commands,
                                demidom.clone(),
                                &mut sub_context,
                                font.clone(),
                                c,
                            ) {
                                ret.propagate(v);
                            }
                        }
                        context.spawn_spacer(&mut commands, font.size);
                        return Some(ret);
                    }
                }
                if let Some(id) = &attributes.class {
                    if id == "editable" {
                        let uid_for_mock_anchor =
                            attributes.attachments.as_ref().and_then(|attachments| {
                                attachments
                                    .iter()
                                    .filter_map(|attachment| {
                                        if let DemidomContextAttachment::EditableAttribute(
                                            _,
                                            maybe_uid,
                                            _,
                                        ) = attachment
                                        {
                                            maybe_uid.clone()
                                        } else {
                                            None
                                        }
                                    })
                                    .last()
                            });
                        let mut g = commands.spawn((
                            Name::new("Editable"),
                            Node {
                                position_type: PositionType::Relative,
                                display: Display::Flex,
                                ..default()
                            },
                            BorderRadius::all(Val::Px(2.0)),
                            BackgroundColor(Color::srgba(0.1, 0.0, 0.05, 0.1)),
                            ChildOf(context.parent),
                        ));
                        if let Some(d) = uid_for_mock_anchor {
                            g.insert(NpcAnchor(d));
                        }
                        let g = g.id();

                        let mut sub_context = context.cascade(g);
                        sub_context
                            .attachments
                            .get_or_insert_with(Vec::new)
                            .extend_from_slice(
                                &attributes.attachments.as_ref().unwrap_or(&Vec::new()),
                            );
                        for c in children_to_render {
                            if let Some(v) = render_demidom(
                                &mut commands,
                                demidom.clone(),
                                &mut sub_context,
                                font.clone(),
                                c,
                            ) {
                                ret.propagate(v);
                            }
                        }
                        return Some(ret);
                    }

                    if id == "hpmarks" {
                        return Some(ret);
                    }
                    if id == "statblock-container" {
                        let gb = commands
                            .spawn((Node {
                                position_type: PositionType::Relative,
                                display: Display::Flex,
                                flex_wrap: FlexWrap::Wrap,
                                width: Val::Percent(100.0),
                                ..default()
                            },))
                            .insert(ChildOf(context.parent))
                            .id();
                        let bundle = make_link_node_bundle(context, font.size, true);
                        let link_box = commands
                            .spawn(bundle)
                            .insert(ChildOf(gb))
                            .hover_effect()
                            .observe(accordion_toggle())
                            .id();

                        let e = context
                            .cascade(link_box)
                            .spawn_icon(&mut commands, font.size);
                        commands.entity(link_box).insert(AccordionChevron(e));
                        let g = commands
                            .spawn((
                                AccordionOf(link_box),
                                AccordionVisibility::Hidden,
                                Node {
                                    position_type: PositionType::Relative,
                                    overflow: Overflow {
                                        y: OverflowAxis::Hidden,
                                        ..default()
                                    },
                                    display: Display::Flex,
                                    flex_wrap: FlexWrap::Wrap,
                                    width: Val::Percent(100.0),
                                    height: Val::Px(0.0),
                                    ..default()
                                },
                            ))
                            .insert(ChildOf(gb))
                            .id();
                        let mut subcontext = context.cascade(g);
                        for c in children_to_render {
                            if let Some(v) = render_demidom(
                                &mut commands,
                                demidom.clone(),
                                &mut subcontext,
                                font.clone(),
                                c,
                            ) {
                                ret.propagate(v);
                            }
                        }
                        return Some(ret);
                    }
                    if id == "alchemy" {
                        for c in children_to_render {
                            if let Some(v) = render_demidom(
                                &mut commands,
                                demidom.clone(),
                                context,
                                font.with_font(context.theme.sigils_font.clone())
                                    .with_size(font.size * 1.2),
                                c,
                            ) {
                                ret.propagate(v);
                            }
                        }
                        return Some(ret);
                    }
                    if id == "breadcrumbs" {
                        let g = commands
                            .spawn((
                                Name::new("HeaderBreadcrumbs"),
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_wrap: FlexWrap::NoWrap,

                                    ..default()
                                },
                            ))
                            .insert(ChildOf(context.parent))
                            .id();

                        let mut subcontext = context.cascade(g);
                        for c in children_to_render {
                            if let Some(v) = render_demidom(
                                &mut commands,
                                demidom.clone(),
                                &mut subcontext,
                                font.with_size(font.size * 0.55),
                                c,
                            ) {
                                ret.propagate(v);
                            }
                        }
                        return Some(ret);
                    }
                    if id == "spoiler" {
                        spoiler = true;
                    }
                }
                let g = commands
                    .spawn_empty()
                    .make_clipboard_container(context)
                    .id();
                let mut subcontext = context.cascade(g);
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        &mut subcontext,
                        font.clone(),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
                commands.entity(g).insert(DemidomClipboardText {
                    text: ret.text.clone(),
                });
                if spoiler {
                    commands.entity(g).content_is_spoiler(context.spoilers);
                }
                if let Some(id) = &attributes.id {
                    if id == "editable-title" {}
                }
            }
            _ => {
                for c in children_to_render {
                    if let Some(v) = render_demidom(
                        &mut commands,
                        demidom.clone(),
                        context,
                        font.clone(),
                        c,
                    ) {
                        ret.propagate(v);
                    }
                }
            }
        };
    }
    Some(ret)
}

// ---------------------------------------------------------------------------
// Inseparable-character glue
//
// Some glyphs must never line-break away from their adjacent word. We enforce
// that at parse time by wrapping the glyph + its anchored neighbour in a
// `Bundle` element (which renders with `FlexWrap::NoWrap`).
//
//   GLUE_BACKWARD  — must stay with the *previous* sibling: `)`, `,`, `.`, …
//   GLUE_FORWARD   — must stay with the *next*     sibling: `(`, `[`, `“`, …
// ---------------------------------------------------------------------------

const GLUE_BACKWARD: &[char] = &[
    ')', ',', '.', '”', '?', '!', ';', ':', ']', '}', '»', '’', '%', '°', '…',
];
const GLUE_FORWARD: &[char] = &['(', '[', '{', '«', '“', '‘', '¿', '¡'];

fn first_char_in(text: &str, set: &[char]) -> bool {
    text.chars().next().is_some_and(|c| set.contains(&c))
}

fn last_non_ws_in(text: &str, set: &[char]) -> bool {
    text.trim_end()
        .chars()
        .last()
        .is_some_and(|c| set.contains(&c))
}

/// Returns `(remainder, peeled)` where `peeled` is the leading run of `text`
/// whose chars are all in `set`.
fn split_leading_glue(text: &str, set: &[char]) -> (String, String) {
    let mut bytes = 0;
    for c in text.chars() {
        if set.contains(&c) {
            bytes += c.len_utf8();
        } else {
            break;
        }
    }
    (text[bytes..].to_string(), text[..bytes].to_string())
}

/// Returns `(remainder, peeled)` where `peeled` is the trailing run of
/// `text.trim_end()` whose chars are all in `set`. The remainder preserves
/// any trailing whitespace from the original input.
fn split_trailing_glue(text: &str, set: &[char]) -> (String, String) {
    let trimmed = text.trim_end();
    let trailing_ws = &text[trimmed.len()..];
    let mut bytes = 0;
    for c in trimmed.chars().rev() {
        if set.contains(&c) {
            bytes += c.len_utf8();
        } else {
            break;
        }
    }
    let kept_len = trimmed.len() - bytes;
    let kept = format!("{}{}", &trimmed[..kept_len], trailing_ws);
    let glue = trimmed[kept_len..].to_string();
    (kept, glue)
}

pub struct Sink {
    pub next_id: Cell<usize>,
    pub names: RefCell<HashMap<usize, &'static QualName>>,
    pub elements: Rc<RefCell<HashMap<usize, Element>>>,
    pub last_added: RefCell<usize>,
    pub last_text_element: Cell<usize>,
}

impl Sink {
    fn get_id(&self) -> usize {
        let id = self.next_id.get();
        self.next_id.set(id + 2);
        id
    }

    fn is_in_link(&self, eid: usize) -> bool {
        let mut inner_is_link = false;
        if let Some(e) = self.elements.as_ref().borrow().get(&eid) {
            let mut check_parent = e.parent_id;
            let mut depth = 0;
            while check_parent != 0 {
                if let Some(e) = self.elements.as_ref().borrow().get(&check_parent) {
                    inner_is_link = e.element.is_link();
                    if inner_is_link || depth > 3 {
                        break;
                    }
                    check_parent = e.parent_id;
                    depth += 1;
                } else {
                    break;
                }
            }
        }
        inner_is_link
    }

    fn is_following_a_line_break(&self, parent: &usize) -> bool {
        let elements = self.elements.as_ref().borrow();
        let last_added_parent = &elements.get(&self.last_added.borrow()).unwrap().parent_id;
        last_added_parent != parent
            && matches!(
                &elements.get(last_added_parent).unwrap().element,
                ElementType::Paragraph
                    | ElementType::Table(_)
                    | ElementType::TableRow
                    | ElementType::LineBreak
                    | ElementType::HorizontalLine
                    | ElementType::Header(_)
            )
    }

    fn is_following_a_whitespace(&self) -> bool {
        let elements = self.elements.as_ref().borrow();
        match &elements.get(&self.last_added.borrow()).unwrap().element {
            ElementType::Text(t) => t.ends_with(" "),
            ElementType::DiceRoller(_) => true,
            _ => false,
        }
    }

    fn filter_unneeded_whitespaces(&self, t: &str, _parent: &usize) -> String {
        // FIXME: do we even need the next line?
        let mut text = t.replace("\n", " ");
        let elements = self.elements.as_ref().borrow();
        let is_first_child = elements.get(_parent).unwrap().children.is_empty();

        let text_is_whitespace = if text.trim().is_empty() {
            text.clear();
            true
        } else {
            // clean up a non whitespace string
            if text.starts_with(" ") {
                // We clean up any leading whitespace given the following three
                // conditions:
                if self.is_following_a_line_break(_parent)
                    || self.is_following_a_whitespace()
                    || is_first_child
                {
                    text = text.trim_start().to_string();
                // Otherwise, we leave one single leading whitespace
                } else {
                    // FIXME : do we even need this behavior?
                    text = format!(" {}", text.trim_start());
                }
            }
            false
        };

        if text_is_whitespace {
            let elements = self.elements.as_ref().borrow();

            match elements.get(_parent).unwrap().element {
                ElementType::Table(_) | ElementType::TableRow | ElementType::TableHeader => {}
                _ => {
                    // If the text is not part of a table, we assume the whitespace might
                    // be needed.
                    // FIXME: do we even need this??
                    text = " ".to_string();
                }
            };

            if *self.last_added.borrow() != 0 {
                match &elements.get(&self.last_added.borrow()).unwrap().element {
                    ElementType::Text(t) => {
                        // No need in consecutive whitespaces
                        if t.ends_with(" ") {
                            text.clear();
                        }
                    }
                    // A whitespace after a STRONG tag is allowed
                    ElementType::Strong => {}
                    _ => {
                        // No need in a leading whitespace after a tag that is not a STRONG
                        // (or other text formatting tags, but we don't have these yet)
                        text.clear();
                    }
                }
            }
            if self.is_following_a_line_break(_parent) || is_first_child {
                text.clear();
            }
        }
        text
    }

    /// Insert a fresh `Bundle` whose children are exactly `[first, second]`,
    /// re-parenting both. Returns the bundle id. The caller is responsible for
    /// inserting the bundle id into its enclosing parent's children vector.
    fn make_bundle_pair(
        &self,
        elements: &mut HashMap<usize, Element>,
        parent: &usize,
        first: usize,
        second: usize,
    ) -> usize {
        let bid = self.get_id();
        elements.insert(
            bid,
            Element {
                element: ElementType::Bundle,
                parent_id: *parent,
                children: vec![first, second],
            },
        );
        if let Some(e) = elements.get_mut(&first) {
            e.parent_id = bid;
        }
        if let Some(e) = elements.get_mut(&second) {
            e.parent_id = bid;
        }
        bid
    }

    /// Forward-glue: when an inline element `pivot` is being appended, peel any
    /// trailing GLUE_FORWARD run from the previous text sibling and bundle it
    /// with `pivot`. Returns `true` if bundling occurred — in that case the
    /// caller MUST NOT also push `pivot` into `parent` itself.
    fn glue_forward(&self, parent: &usize, pivot: usize) -> bool {
        let mut elements = self.elements.as_ref().borrow_mut();

        // Only worth gluing inline-renderable pivots.
        if !matches!(
            elements.get(&pivot).map(|e| &e.element),
            Some(ElementType::Strong)
                | Some(ElementType::Small)
                | Some(ElementType::Em)
                | Some(ElementType::Link(_))
                | Some(ElementType::Icon(_))
                | Some(ElementType::DiceRoller(_))
                | Some(ElementType::EntityRoller(_))
        ) {
            return false;
        }

        let Some(last_id) = elements
            .get(parent)
            .and_then(|p| p.children.last().copied())
        else {
            return false;
        };

        // Peel the trailing forward-glue from the previous text sibling.
        let glue = match elements.get_mut(&last_id).map(|e| &mut e.element) {
            Some(ElementType::Text(t)) => {
                let (kept, peeled) = split_trailing_glue(t, GLUE_FORWARD);
                if peeled.is_empty() {
                    return false;
                }
                *t = kept;
                peeled
            }
            _ => return false,
        };

        // If the previous text is now whitespace-only, drop it from `parent`.
        let became_empty = matches!(
            elements.get(&last_id).map(|e| &e.element),
            Some(ElementType::Text(t)) if t.trim().is_empty()
        );
        if became_empty {
            if let Some(p) = elements.get_mut(parent) {
                p.children.retain(|c| *c != last_id);
            }
        }

        // Bundle: [Text(glue), pivot].
        let glue_id = self.get_id();
        elements.insert(
            glue_id,
            Element {
                element: ElementType::Text(glue),
                parent_id: 0,
                children: Vec::new(),
            },
        );
        let bid = self.make_bundle_pair(&mut elements, parent, glue_id, pivot);
        if let Some(p) = elements.get_mut(parent) {
            p.children.push(bid);
        }

        self.last_text_element.set(glue_id);
        true
    }

    /// Backward-glue: a freshly-arrived text starts with a GLUE_BACKWARD run
    /// (`glue`). Move it into a bundle with the previous sibling so it cannot
    /// wrap to the next line.
    ///
    /// Special case for hyperlinks: we normally let trailing whitespace inside
    /// a link survive (so the link reads as `[ text ]`). But when the glue
    /// starts with `.` and we're inside a link, we strip that whitespace so we
    /// render `[ text. ]` rather than `[ text . ]`.
    fn glue_backward(&self, parent: &usize, glue: String) {
        if glue.is_empty() {
            return;
        }

        // Read these before holding a mutable borrow — `is_in_link` recurses
        // through `self.elements` itself.
        let last_text_id = self.last_text_element.get();
        let is_in_link = last_text_id != 0 && self.is_in_link(last_text_id);

        let mut elements = self.elements.as_ref().borrow_mut();

        let parent_is_link = elements
            .get(parent)
            .map(|e| e.element.is_link())
            .unwrap_or(false);

        let Some(last_sibling) = elements.get_mut(parent).and_then(|p| p.children.pop())
        else {
            return;
        };

        if last_text_id != 0
            && (!is_in_link || (glue.starts_with('.') && parent_is_link))
            && let Some(ElementType::Text(text)) =
                elements.get_mut(&last_text_id).map(|e| &mut e.element)
        {
            *text = text.trim_end().to_string();
        }

        let glue_id = self.get_id();
        elements.insert(
            glue_id,
            Element {
                element: ElementType::Text(glue),
                parent_id: 0,
                children: Vec::new(),
            },
        );
        let bid = self.make_bundle_pair(&mut elements, parent, last_sibling, glue_id);
        if let Some(p) = elements.get_mut(parent) {
            p.children.push(bid);
        }

        self.last_added.replace(glue_id);
    }
}

impl TreeSink for Sink {
    type Handle = usize;
    type Output = Self;
    type ElemName<'a> = ExpandedName<'a>;

    fn finish(self) -> Self {
        self
    }

    fn get_document(&self) -> usize {
        0
    }

    fn get_template_contents(&self, target: &usize) -> usize {
        target + 1
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn elem_name(&self, target: &usize) -> ExpandedName<'_> {
        self.names
            .borrow()
            .get(target)
            .expect("not an element")
            .expanded()
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, _: ElementFlags) -> usize {
        let id = self.get_id();
        let v = (*name.local).to_string().clone();
        self.names
            .borrow_mut()
            .insert(id, Box::leak(Box::new(name)));
        let element = match v.as_str() {
            "a" => {
                let mut href = String::new();
                let mut id = String::new();
                let mut roller_attrs = RollerAttributes::default();
                let mut coords = MapCoords::default();
                #[derive(Eq, PartialEq)]
                enum LinkType {
                    Href,
                    Coords,
                    Icon,
                    Dice,
                    Roller,
                    NpcAnchor,
                    Other,
                    Spawner,
                }
                let mut link_type = LinkType::Other;
                for attr in attrs {
                    if &*attr.name.local == "hex" {
                        coords.hex = attr.value.to_string();
                    }
                    if &*attr.name.local == "x" {
                        coords.x = attr.value.to_string().parse::<f32>().unwrap_or_default();
                    }
                    if &*attr.name.local == "y" {
                        coords.y = attr.value.to_string().parse::<f32>().unwrap_or_default();
                    }
                    if &*attr.name.local == "zoom" {
                        coords.zoom =
                            attr.value.to_string().parse::<i32>().unwrap_or_default();
                    }
                    if &*attr.name.local == "data-level" {
                        coords.layer =
                            attr.value.to_string().parse::<usize>().unwrap_or_default();
                    }
                    if &*attr.name.local == "class" && attr.value.to_string() == "map-coords" {
                        link_type = LinkType::Coords;
                    }
                    if &*attr.name.local == "class"
                        && attr.value.to_string() == "breadcrumbs-icon"
                    {
                        link_type = LinkType::Icon;
                        href = attr.value.to_string();
                    }
                    if &*attr.name.local == "class" && attr.value.to_string() == "btn-icon" {
                        link_type = LinkType::Roller;
                    }
                    if &*attr.name.local == "class" && attr.value.to_string() == "token" {
                        link_type = LinkType::Spawner;
                    }
                    if &*attr.name.local == "class" && attr.value.to_string() == "npc-anchor" {
                        link_type = LinkType::NpcAnchor;
                    }
                    if &*attr.name.local == "class"
                        && attr.value.to_string() == "btn-spawn-dice"
                    {
                        link_type = LinkType::Dice;
                    }
                    if &*attr.name.local == "data-dice" {
                        href = attr.value.to_string();
                    }
                    if &*attr.name.local == "data-uuid" {
                        roller_attrs.uid = attr.value.to_string();
                    }
                    if &*attr.name.local == "data-override" {
                        roller_attrs.class_override = attr.value.to_string();
                    }
                    if &*attr.name.local == "data-type" {
                        roller_attrs.class_override = attr.value.to_string();
                        roller_attrs.is_map_reload_needed = true;
                        roller_attrs.is_appender = true;
                    }
                    if &*attr.name.local == "data-attr" {
                        roller_attrs.attr = attr.value.to_string();
                        roller_attrs.is_map_reload_needed = true;
                        roller_attrs.is_appender = true;
                    }
                    if &*attr.name.local == "data-reload" {
                        roller_attrs.is_map_reload_needed = true;
                    }
                    if &*attr.name.local == "href" {
                        if link_type == LinkType::Other {
                            link_type = LinkType::Href;
                        }
                        href = attr.value.to_string();
                    }
                    if &*attr.name.local == "id" {
                        id = attr.value.to_string();
                    }
                }

                match link_type {
                    LinkType::Href => ElementType::Link(LinkAttributes { href }),
                    LinkType::Icon => ElementType::Icon(LinkAttributes { href }),
                    LinkType::Dice => ElementType::DiceRoller(LinkAttributes { href }),
                    LinkType::Roller => ElementType::EntityRoller(roller_attrs),
                    LinkType::Spawner => ElementType::Spawner("".to_string(), coords),
                    LinkType::Coords => ElementType::Coords(coords),
                    LinkType::NpcAnchor => ElementType::Anchor(id),
                    LinkType::Other => ElementType::NoOp,
                }
            }
            "div" | "span" => {
                let mut attributes = ElementAttributes::new();
                let mut attachments: Vec<DemidomContextAttachment> = Vec::new();
                let mut maybe_editable_attr = None;
                let mut maybe_editable_attr_entity = None;
                let mut maybe_editable_cache_entity = None;
                for attr in attrs {
                    if &*attr.name.local == "id" {
                        attributes.id = Some(attr.value.to_string());
                    }
                    if &*attr.name.local == "class" {
                        attributes.class = Some(attr.value.to_string());
                    }
                    if &*attr.name.local == "hidden" {
                        attributes.hidden = Some(true);
                    }
                    if &*attr.name.local == "data-reroll" {
                        attachments.push(DemidomContextAttachment::Rerollable(false));
                    }
                    if &*attr.name.local == "data-attr" {
                        maybe_editable_attr = Some(attr.value.to_string());
                    }
                    if &*attr.name.local == "data-entity" {
                        maybe_editable_attr_entity = Some(attr.value.to_string());
                    }
                    if &*attr.name.local == "data-cache-entity" {
                        maybe_editable_cache_entity = Some(attr.value.to_string());
                    }
                    if &*attr.name.local == "data-settlement" {
                        attachments.push(DemidomContextAttachment::DataSettlement(
                            attr.value.to_string(),
                        ));
                    }
                    if &*attr.name.local == "data-map-label" {
                        attachments.push(DemidomContextAttachment::DataMapLabel);
                    }
                }
                if let Some(editable_attr) = maybe_editable_attr {
                    attachments.push(DemidomContextAttachment::EditableAttribute(
                        editable_attr,
                        maybe_editable_attr_entity,
                        maybe_editable_cache_entity,
                    ));
                }
                attributes.attachments = if attachments.is_empty() {
                    None
                } else {
                    Some(attachments)
                };
                ElementType::Div(attributes)
            }
            "blockquote" => ElementType::Blockquote,
            "h1" => ElementType::Header(1),
            "h2" => ElementType::Header(2),
            "h3" => ElementType::Header(3),
            "h4" => ElementType::Header(4),
            "h5" => ElementType::Header(5),
            "h6" => ElementType::Header(6),
            "p" => ElementType::Paragraph,
            "tbody" => {
                let mut table_type = TableType::Balanced;
                for attr in attrs {
                    if &*attr.name.local == "id" {
                        if attr.value.to_string() == "random-encounters" {
                            table_type = TableType::MaxLastColumn;
                        }
                    }
                }

                ElementType::Table(table_type)
            }
            "tr" => ElementType::TableRow,
            "td" => ElementType::TableCell,
            "th" => ElementType::TableHeader,
            "ul" => ElementType::List,
            "li" => ElementType::ListItem,
            "br" => ElementType::LineBreak,
            "hr" => ElementType::HorizontalLine,
            "strong" => ElementType::Strong,
            "em" => ElementType::Em,
            "small" => ElementType::Small,
            _ => ElementType::NoOp,
        };

        self.elements.as_ref().borrow_mut().insert(
            id,
            Element {
                element,
                parent_id: 0,
                children: Vec::new(),
            },
        );
        self.last_added.replace(id);

        id
    }

    fn create_comment(&self, _text: StrTendril) -> usize {
        self.get_id()
    }

    #[allow(unused_variables)]
    fn create_pi(&self, target: StrTendril, value: StrTendril) -> usize {
        unimplemented!()
    }

    fn append_before_sibling(&self, _sibling: &usize, _new_node: NodeOrText<usize>) {}

    fn append_based_on_parent_node(
        &self,
        _element: &usize,
        _prev_element: &usize,
        _new_node: NodeOrText<usize>,
    ) {
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}
    fn set_quirks_mode(&self, _mode: QuirksMode) {}
    fn append(&self, _parent: &usize, child: NodeOrText<usize>) {
        match child {
            NodeOrText::AppendNode(n) => {
                // Glue any trailing forward-glue chars (e.g. "(") from the
                // previous sibling onto this new inline element.
                if !self.glue_forward(_parent, n) {
                    let mut elements = self.elements.as_ref().borrow_mut();
                    if let Some(e) = elements.get_mut(_parent) {
                        e.children.push(n);
                    }
                    if let Some(e) = elements.get_mut(&n) {
                        e.parent_id = *_parent;
                    }
                }
            }
            NodeOrText::AppendText(t) => {
                let text = self.filter_unneeded_whitespaces(&t, _parent);
                if !text.is_empty() {
                    // Coalesce with the previous text sibling under the same
                    // parent if there is one.
                    if *self.last_added.borrow() != 0 {
                        let mut elements = self.elements.as_ref().borrow_mut();
                        let last_id = *self.last_added.borrow();
                        if let Some(last_node) = elements.get_mut(&last_id) {
                            if let ElementType::Text(last_text) = &mut last_node.element {
                                if last_node.parent_id == *_parent {
                                    if text != " " {
                                        last_text.push_str(&text);
                                    }
                                    return;
                                }
                            }
                        }
                    }

                    // From here on the text is following some other element
                    // (Link, Strong, Div, …). Peel any leading backward-glue
                    // run and attach it to that previous sibling via a bundle.
                    let (next_text, mut glue) = split_leading_glue(&text, GLUE_BACKWARD);
                    if !glue.is_empty() && next_text.starts_with(' ') {
                        glue.push(' ');
                    }
                    self.glue_backward(_parent, glue);

                    let mut next_text = next_text
                        .split_whitespace()
                        .collect::<Vec<&str>>()
                        .join(" ");

                    // Append a trailing space unless this text ends with a
                    // forward-glue char (it'll be picked up by `glue_forward`
                    // when the next inline element arrives).
                    if !last_non_ws_in(&next_text, GLUE_FORWARD) {
                        next_text.push(' ');
                    }

                    let id = self.get_id();
                    self.last_text_element.set(id);
                    let mut elements = self.elements.as_ref().borrow_mut();
                    elements.insert(
                        id,
                        Element {
                            element: ElementType::Text(next_text),
                            parent_id: *_parent,
                            children: Vec::new(),
                        },
                    );
                    self.last_added.replace(id);
                    if let Some(e) = elements.get_mut(_parent) {
                        e.children.push(id);
                    }
                }
            }
        }
    }

    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}
    fn add_attrs_if_missing(&self, _target: &usize, _attrs: Vec<Attribute>) {}
    fn remove_from_parent(&self, _target: &usize) {}
    fn reparent_children(&self, _node: &usize, _new_parent: &usize) {}
    fn mark_script_already_started(&self, _node: &usize) {}
}

pub fn accordion_toggle() -> impl Fn(
    On<Pointer<Click>>,
    Commands,
    Query<&AccordionChevron>,
    Query<(
        Entity,
        &AccordionOf,
        &mut Node,
        &mut AccordionVisibility,
        &ComputedNode,
    )>,
) {
    move |mut trigger, mut commands, chevrons, mut query| {
        trigger.propagate(false);
        for (_e, a, mut n, mut v, cn) in query.iter_mut() {
            if a.0 == trigger.entity {
                let (start, end) = if *v == AccordionVisibility::Hidden {
                    *v = AccordionVisibility::Inherited;
                    n.position_type = PositionType::Relative;
                    n.overflow.y = OverflowAxis::Hidden;
                    commands.entity(_e).insert(bevy_tweening::Animator::new(
                        bevy_tweening::Tween::new(
                            EaseFunction::QuadraticIn,
                            Duration::from_millis(300),
                            UiNodeSizeLens {
                                mode: UiNodeSizeLensMode::Height,
                                start: Vec2::new(0.0, 0.0),
                                end: Vec2::new(0.0, cn.content_size.y),
                            },
                        ),
                    ));
                    (0.0, f32::consts::PI)
                } else {
                    *v = AccordionVisibility::Hidden;
                    n.overflow.y = OverflowAxis::Hidden;
                    commands.entity(_e).insert(bevy_tweening::Animator::new(
                        bevy_tweening::Tween::new(
                            EaseFunction::QuadraticOut,
                            Duration::from_millis(300),
                            UiNodeSizeLens {
                                mode: UiNodeSizeLensMode::Height,
                                start: Vec2::new(0.0, cn.content_size.y),
                                end: Vec2::new(0.0, 0.0),
                            },
                        ),
                    ));
                    (f32::consts::PI, 0.0)
                };
                if let Ok(chevron) = chevrons.get(trigger.entity) {
                    commands
                        .entity(chevron.0)
                        .insert(bevy_tweening::Animator::new(bevy_tweening::Tween::new(
                            EaseFunction::QuadraticIn,
                            Duration::from_millis(200),
                            UiTransformRotationLens {
                                start: Rot2::radians(start),
                                end: Rot2::radians(end),
                            },
                        )));
                }
                break;
            }
        }
    }
}

fn extract_ids(url: &str) -> Option<(String, Option<String>)> {
    let parts: Vec<&str> = url.split('/').skip(1).collect();

    match parts.len() {
        2 => return None,
        3 => return Some((parts[1].to_string(), None)),
        4 => return Some((parts[3].to_string(), None)),
        6 => return Some((parts[3].to_string(), Some(parts[5].to_string()))),
        _ => return None,
    }
}

pub fn link_click()
-> impl Fn(On<Pointer<Click>>, Commands, Query<&DemidomLink>, ResMut<NextState<ContentMode>>) {
    move |trigger, mut commands, links, mut next_content_mode| {
        if let Ok(link) = links.get(trigger.entity) {
            let url = link.url.clone();
            debug!("Link url is {}", url);

            if let Some(ids) = extract_ids(&url) {
                commands.trigger(FetchEntityFromStorage {
                    uid: ids.0,
                    anchor: ids.1,
                    why: FetchEntityReason::SandboxLink,
                });
            } else {
                next_content_mode.set(ContentMode::MapOnly);
            }
        }
    }
}

impl RerollEntity {
    pub fn from_roller_attributes(attrs: &RollerAttributes) -> Self {
        Self {
            uid: attrs.uid.clone(),
            coords: None,
            class_override: attrs.class_override.clone(),
            is_map_reload_needed: attrs.is_map_reload_needed,
        }
    }
}

pub fn roller_click()
-> impl Fn(On<Pointer<Click>>, Commands, Query<&RollerAttributes>, Res<HexMapData>) {
    move |trigger, mut commands, links, map_data| {
        if let Ok(link) = links.get(trigger.entity) {
            if link.is_appender {
                commands.trigger(AppendSandboxEntity {
                    target: AppendSubject::Hex {
                        uid: link.uid.clone(),
                        coords: map_data.coords.get(&link.uid).cloned(),
                    },
                    attr: link.attr.clone(),
                    what: link.class_override.clone(),
                });
            } else {
                commands.trigger(RerollEntity::from_roller_attributes(link));
            }
        }
    }
}

pub fn spawner_click(
    coords: MapCoords,
) -> impl Fn(On<Pointer<Click>>, Commands, Res<HexMapData>) {
    move |_, mut commands, hex_map| {
        if let Some(pos) =
            hex_map.get_canonical_pos(&coords.hex, Vec2::new(coords.x, coords.y))
        {
            commands.trigger(SpawnTokenFromLibrary {
                pos: Vec3::new(pos.x + 0.5, HEIGHT_OF_TOKENS, pos.y + 0.5),
            });
        }
    }
}

pub fn dice_link_click() -> impl Fn(On<Pointer<Click>>, Commands, Query<&DemidomLink>) {
    move |trigger, mut commands, links| {
        if let Ok(link) = links.get(trigger.entity) {
            let url = link.url.clone();
            commands.trigger(RollDice { dice: url });
        }
    }
}

fn adjust_text_to_app_fonts(source: &String) -> String {
    source
        .replace("◾", "#")
        .replace("⬝", "#")
        .replace("✦", "®")
}
