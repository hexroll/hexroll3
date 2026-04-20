/*
// Copyright (C) 2020-2025 Pen, Dice & Paper
//
// This program is dual-licensed under the following terms:
//
// Option 1: (Non-Commercial) GNU Affero General Public License (AGPL)
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
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::{collections::HashMap, rc::Rc};

use egui::{CursorIcon, FontId};
use egui::{Separator, Vec2};

use html5ever::parse_document;
use html5ever::tendril::*;
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{Attribute, ExpandedName, QualName};

use crate::app::HexrollTestbedApp;

#[derive(Clone, Debug)]
pub struct ElementAttributes {
    id: Option<String>,
    class: Option<String>,
    hidden: Option<bool>,
}

impl ElementAttributes {
    fn new() -> Self {
        ElementAttributes {
            id: None,
            class: None,
            hidden: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LinkAttributes {
    href: String,
}

#[derive(Clone, Debug)]
pub enum ElementType {
    Div(ElementAttributes),
    Header(i32),
    Paragraph,
    Link(LinkAttributes),
    Table,
    TableRow,
    TableCell,
    TableHeader,
    ListItem,
    LineBreak,
    HorizontalLine,
    Strong,
    Text(String),
    NoOp,
}

#[derive(Clone)]
pub struct Element {
    element: ElementType,
    parent_id: usize,
    children: Vec<usize>,
}

pub struct DemidomResponse {
    pub url: String,
}

impl HexrollTestbedApp {
    pub fn parse_entity_html(&mut self, html_content: &str) {
        self.current_entity
            .html_demidom
            .as_ref()
            .borrow_mut()
            .clear();
        let sink = Sink {
            next_id: Cell::new(1),
            names: RefCell::new(HashMap::new()),
            elements: self.current_entity.html_demidom.clone(),
            last_added: RefCell::new(0),
        };
        parse_document(sink, Default::default())
            .from_utf8()
            .one(html_content.as_bytes());
    }
}

pub fn render_demidom(
    demidom: Rc<RefCell<HashMap<usize, Element>>>,
    ui: &mut egui::Ui,
    font_size: f32,
    n: usize,
) -> Option<DemidomResponse> {
    let mut ret: Option<DemidomResponse> = None;
    if let Some(element_to_render) = demidom.borrow().get(&n) {
        let children_to_render = element_to_render.children.clone();
        match &element_to_render.element {
            ElementType::Header(level) => {
                let header_font_size = font_size
                    * match level {
                        6 => 1.2,
                        5 => 1.4,
                        4 => 1.6,
                        3 => 1.8,
                        2 => 2.0,
                        1 => 3.0,
                        _ => 1.0,
                    };
                ui.allocate_space(egui::vec2(ui.available_width(), 20.0));
                for c in children_to_render {
                    if let Some(v) =
                        render_demidom(demidom.clone(), ui, header_font_size, c)
                    {
                        ret = Some(v);
                    }
                }
                ui.allocate_space(egui::vec2(ui.available_width(), 00.0));
            }
            ElementType::Paragraph => {
                for c in children_to_render {
                    if let Some(v) =
                        render_demidom(demidom.clone(), ui, font_size, c)
                    {
                        ret = Some(v);
                    }
                }
                ui.allocate_space(egui::vec2(ui.available_width(), 00.0));
            }
            ElementType::TableRow => {
                for c in children_to_render {
                    if let Some(v) =
                        render_demidom(demidom.clone(), ui, font_size, c)
                    {
                        ret = Some(v);
                    }
                }
                ui.end_row();
            }
            ElementType::TableHeader | ElementType::TableCell => {
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        for c in children_to_render {
                            if let Some(v) = render_demidom(
                                demidom.clone(),
                                ui,
                                font_size,
                                c,
                            ) {
                                ret = Some(v);
                            }
                        }
                    });
                });
            }
            ElementType::Table => {
                ui.allocate_space(egui::vec2(ui.available_width(), 0.0));
                ui.allocate_space(egui::vec2(-1.0 * ui.available_width(), 0.0));
                // Pre-count the columns so we can give it to the grid
                let mut cols = 0;
                if let Some(c) = children_to_render.clone().into_iter().next() {
                    let demidom_borrowed = demidom.borrow();
                    let child_element = demidom_borrowed.get(&c).unwrap();
                    for _ in &child_element.children {
                        cols += 1;
                    }
                }
                egui::Grid::new(n)
                    .striped(true)
                    .num_columns(cols)
                    .with_row_color(|i, s| {
                        if i == 0 {
                            Some(s.visuals.extreme_bg_color)
                        } else if i % 2 == 0 {
                            Some(s.visuals.faint_bg_color)
                        } else {
                            None
                        }
                    })
                    .min_col_width(ui.available_width() * 0.01)
                    .max_col_width(1000.0)
                    .spacing(Vec2::new(font_size, font_size / 4.0))
                    .show(ui, |ui| {
                        for c in children_to_render {
                            if let Some(v) = render_demidom(
                                demidom.clone(),
                                ui,
                                font_size,
                                c,
                            ) {
                                ret = Some(v);
                            }
                        }
                    });
                ui.allocate_space(egui::vec2(ui.available_width(), 0.0));
            }
            ElementType::LineBreak => {
                ui.end_row();
            }
            ElementType::HorizontalLine => {
                ui.end_row();
                ui.add(Separator::default().horizontal());
            }
            ElementType::ListItem => {
                ui.end_row();
                for c in children_to_render {
                    if let Some(v) =
                        render_demidom(demidom.clone(), ui, font_size, c)
                    {
                        ret = Some(v);
                    }
                }
            }
            ElementType::Text(text) => {
                let elements_borrowed = demidom.borrow();
                let parent_element = elements_borrowed
                    .get(&element_to_render.parent_id)
                    .unwrap();
                match &parent_element.element {
                    ElementType::Link(attributes) => {
                        let href = &attributes.href;
                        let res = if href.contains("inspect") {
                            let mut j = egui::text::LayoutJob::default();
                            j.append(
                                text,
                                0.0,
                                egui::TextFormat {
                                    font_id: FontId::proportional(font_size),
                                    color: ui.style().visuals.hyperlink_color,
                                    line_height: Some(font_size * 1.5),
                                    ..Default::default()
                                },
                            );

                            ui.style_mut().interaction.selectable_labels =
                                false;
                            let res = ui.link(j).on_hover_text(href);
                            ui.style_mut().interaction.selectable_labels = true;
                            res
                        } else {
                            ui.allocate_space(egui::vec2(font_size / 7.0, 0.0));

                            ui.style_mut().text_styles.insert(
                                egui::TextStyle::Button,
                                egui::FontId::new(
                                    font_size,
                                    eframe::epaint::FontFamily::Proportional,
                                ),
                            );
                            let res = ui.button(text).on_hover_text(href);
                            ui.allocate_space(egui::vec2(font_size / 7.0, 0.0));
                            res
                        };
                        if res.clicked() {
                            ret = Some(DemidomResponse {
                                url: href.to_string(),
                            });
                            res.on_hover_cursor(CursorIcon::Wait);
                        }
                    }
                    ElementType::Strong => {
                        let grandparent = elements_borrowed
                            .get(&parent_element.parent_id)
                            .unwrap();
                        match &grandparent.element {
                            ElementType::Link(attributes) => {
                                let mut j = egui::text::LayoutJob::default();
                                j.append(
                                    text,
                                    0.0,
                                    egui::TextFormat {
                                        font_id: FontId::proportional(
                                            font_size,
                                        ),
                                        color: ui
                                            .style()
                                            .visuals
                                            .hyperlink_color,
                                        line_height: Some(font_size * 1.5),
                                        ..Default::default()
                                    },
                                );
                                let res =
                                    ui.link(j).on_hover_text(&attributes.href);
                                if res.clicked() {
                                    ret = Some(DemidomResponse {
                                        url: attributes.href.to_string(),
                                    });
                                }
                            }
                            _ => {
                                let mut j = egui::text::LayoutJob::default();
                                j.append(
                                    text,
                                    0.0,
                                    egui::TextFormat {
                                        font_id: FontId::proportional(
                                            font_size,
                                        ),
                                        color: ui
                                            .style()
                                            .visuals
                                            .strong_text_color(),
                                        line_height: Some(font_size * 1.5),
                                        ..Default::default()
                                    },
                                );
                                ui.label(j);
                            }
                        }
                    }
                    _ => {
                        let mut j = egui::text::LayoutJob::default();
                        j.append(
                            text,
                            0.0,
                            egui::TextFormat {
                                font_id: FontId::proportional(font_size),
                                color: ui.style().visuals.text_color(),
                                line_height: Some(font_size * 1.5),
                                ..Default::default()
                            },
                        );
                        ui.label(j);
                    }
                }
                for c in children_to_render {
                    if let Some(v) =
                        render_demidom(demidom.clone(), ui, font_size, c)
                    {
                        ret = Some(v);
                    }
                }
            }
            ElementType::Div(attributes) => {
                if let Some(id) = &attributes.id {
                    if id == "doc-title" {
                        return ret;
                    }
                }
                if let Some(id) = &attributes.class {
                    if id == "breadcrumbs" {
                        for c in children_to_render {
                            if let Some(v) = render_demidom(
                                demidom.clone(),
                                ui,
                                font_size * 0.55,
                                c,
                            ) {
                                ret = Some(v);
                            }
                        }
                        ui.end_row();
                        ui.add(Separator::default().horizontal());
                        return ret;
                    }
                }
                for c in children_to_render {
                    if let Some(v) =
                        render_demidom(demidom.clone(), ui, font_size, c)
                    {
                        ret = Some(v);
                    }
                }
                if let Some(id) = &attributes.id {
                    if id == "editable-title" {
                        ui.allocate_space(egui::vec2(
                            ui.available_width(),
                            0.0,
                        ));
                    }
                }
            }
            _ => {
                for c in children_to_render {
                    if let Some(v) =
                        render_demidom(demidom.clone(), ui, font_size, c)
                    {
                        ret = Some(v);
                    }
                }
            }
        };
    }
    ret
}

pub struct Sink {
    pub next_id: Cell<usize>,
    pub names: RefCell<HashMap<usize, &'static QualName>>,
    pub elements: Rc<RefCell<HashMap<usize, Element>>>,
    pub last_added: RefCell<usize>,
}

impl Sink {
    fn get_id(&self) -> usize {
        let id = self.next_id.get();
        self.next_id.set(id + 2);
        id
    }

    fn is_following_a_line_break(&self, parent: &usize) -> bool {
        let elements = self.elements.as_ref().borrow();
        let last_added_parent =
            &elements.get(&self.last_added.borrow()).unwrap().parent_id;
        last_added_parent != parent
            && matches!(
                &elements.get(last_added_parent).unwrap().element,
                ElementType::Paragraph
                    | ElementType::Table
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
            _ => false,
        }
    }

    fn filter_unneeded_whitespaces(&self, t: &str, _parent: &usize) -> String {
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
                    text = format!(" {}", text.trim_start());
                }
            }
            false
        };

        if text_is_whitespace {
            let elements = self.elements.as_ref().borrow();

            match elements.get(_parent).unwrap().element {
                ElementType::Table
                | ElementType::TableRow
                | ElementType::TableHeader => {}
                _ => {
                    // If the text is not part of a table, we assume the whitespace might
                    // be needed.
                    text = " ".to_string();
                }
            };

            if *self.last_added.borrow() != 0 {
                match &elements.get(&self.last_added.borrow()).unwrap().element
                {
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

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        _: ElementFlags,
    ) -> usize {
        let id = self.get_id();
        let v = (*name.local).to_string().clone();
        self.names
            .borrow_mut()
            .insert(id, Box::leak(Box::new(name)));
        let element = match v.as_str() {
            "a" => {
                let mut href = String::new();
                for attr in attrs {
                    if &*attr.name.local == "href" {
                        href = attr.value.to_string();
                    }
                }
                ElementType::Link(LinkAttributes { href })
            }
            "div" | "span" => {
                let mut attributes = ElementAttributes::new();
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
                }
                ElementType::Div(attributes)
            }
            "h1" => ElementType::Header(1),
            "h2" => ElementType::Header(2),
            "h3" => ElementType::Header(3),
            "h4" => ElementType::Header(4),
            "h5" => ElementType::Header(5),
            "h6" => ElementType::Header(6),
            "p" => ElementType::Paragraph,
            "tbody" => ElementType::Table,
            "tr" => ElementType::TableRow,
            "td" => ElementType::TableCell,
            "th" => ElementType::TableHeader,
            "li" => ElementType::ListItem,
            "br" => ElementType::LineBreak,
            "hr" => ElementType::HorizontalLine,
            "strong" => ElementType::Strong,
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

    fn append_before_sibling(
        &self,
        _sibling: &usize,
        _new_node: NodeOrText<usize>,
    ) {
    }

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
                let mut tasks: Vec<usize> = Vec::new();
                if let Some(e) =
                    self.elements.as_ref().borrow_mut().get_mut(_parent)
                {
                    e.children.push(n);
                    tasks.push(n);
                }
                for n in tasks {
                    if let Some(e) =
                        &mut self.elements.as_ref().borrow_mut().get_mut(&n)
                    {
                        e.parent_id = *_parent;
                    }
                }
            }
            NodeOrText::AppendText(t) => {
                let text = self.filter_unneeded_whitespaces(&t, _parent);
                if !text.is_empty() {
                    if *self.last_added.borrow() != 0 {
                        let mut elements = self.elements.as_ref().borrow_mut();
                        let last_node = &mut elements
                            .get_mut(&self.last_added.borrow())
                            .unwrap();
                        if let ElementType::Text(last_text) =
                            &mut last_node.element
                        {
                            if last_node.parent_id == *_parent {
                                if !last_text.ends_with(" ") {
                                    last_text.push(' ');
                                }
                                if text != " " {
                                    last_text.push_str(&text);
                                }
                                return;
                            }
                        }
                    }
                    let id = self.get_id();
                    self.elements.as_ref().borrow_mut().insert(
                        id,
                        Element {
                            element: ElementType::Text(text),
                            parent_id: *_parent,
                            children: Vec::new(),
                        },
                    );
                    self.last_added.replace(id);
                    if let Some(e) =
                        self.elements.as_ref().borrow_mut().get_mut(_parent)
                    {
                        e.children.push(id);
                    }
                }
            }
        }
    }

    fn append_doctype_to_document(
        &self,
        _: StrTendril,
        _: StrTendril,
        _: StrTendril,
    ) {
    }
    fn add_attrs_if_missing(&self, _target: &usize, _attrs: Vec<Attribute>) {}
    fn remove_from_parent(&self, _target: &usize) {}
    fn reparent_children(&self, _node: &usize, _new_parent: &usize) {}
    fn mark_script_already_started(&self, _node: &usize) {}
}
