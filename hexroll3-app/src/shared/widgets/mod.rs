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

pub mod buttons;
pub mod cursor;
pub mod dial;
pub mod link;
pub mod list;
pub mod modal;

use crate::{content::ThemeBackgroundColor, shared::widgets::link::ContentHoverLink};
use bevy::prelude::*;
use bevy_simple_scroll_view::{ScrollView, ScrollableContent};
use bevy_ui_text_input::*;

pub struct Button {
    image: Handle<Image>,
    image_size: Val,
    button_size: Val,
    border_radius: Val,
    margins: UiRect,
    color: Color,
}

impl Button {
    pub fn from_image(image: Handle<Image>) -> Self {
        Self {
            image,
            image_size: Val::Px(32.0),
            button_size: Val::Px(48.0),
            border_radius: Val::Px(5.0),
            margins: UiRect::right(Val::Px(10.0)),
            color: Color::WHITE,
        }
    }

    pub fn image_size(mut self, size: Val) -> Self {
        self.image_size = size;
        self
    }

    pub fn button_size(mut self, size: Val) -> Self {
        self.button_size = size;
        self
    }

    pub fn border_radius(mut self, radius: Val) -> Self {
        self.border_radius = radius;
        self
    }
    pub fn margins(mut self, margins: UiRect) -> Self {
        self.margins = margins;
        self
    }
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

pub struct TextButton<'a> {
    text: &'a str,
    width: Val,
    button: Button,
}

impl<'a> TextButton<'a> {
    pub fn from_text(text: &'a str, button: Button) -> Self {
        Self {
            text,
            width: Val::Auto,
            button,
        }
    }
    pub fn width(mut self, width: Val) -> Self {
        self.width = width;
        self
    }
}

pub trait ButtonSpawner {
    fn spawn_list_item(&mut self, text: &str, width: Val, radius: Val) -> EntityCommands<'_>;
    fn spawn_text_button(&mut self, button: TextButton) -> EntityCommands<'_>;
    fn spawn_button(&mut self, button: Button) -> EntityCommands<'_>;
}

impl ButtonSpawner for bevy::ecs::relationship::RelatedSpawnerCommands<'_, ChildOf> {
    fn spawn_list_item(&mut self, text: &str, width: Val, radius: Val) -> EntityCommands<'_> {
        let mut list_item = self.spawn((
            Node {
                width,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.0).into()),
            ThemeBackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.0).into()),
            BorderRadius::all(radius),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
        ));
        list_item
            .with_children(|c| {
                c.spawn(text_centered(text));
            })
            .hover_effect();
        list_item
    }
    fn spawn_text_button(&mut self, button: TextButton) -> EntityCommands<'_> {
        let mut ret = self.spawn((
            Node {
                width: button.width,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.9).into()),
            ThemeBackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.9).into()),
            BorderRadius::all(button.button.border_radius),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
        ));
        ret.with_children(|c| {
            c.spawn((Node {
                height: button.button.button_size,
                align_items: AlignItems::Center,
                margin: UiRect::right(Val::Px(10.0)),
                ..default()
            },))
                .with_children(|c| {
                    c.spawn((
                        ImageNode {
                            color: button.button.color,
                            image: button.button.image,
                            ..default()
                        },
                        Node {
                            width: button.button.image_size,
                            height: button.button.image_size,
                            ..default()
                        },
                        Pickable {
                            should_block_lower: false,
                            is_hoverable: true,
                        },
                    ));
                });
            c.spawn(text_centered_with_color(button.text, button.button.color));
        })
        .hover_effect();
        ret
    }

    fn spawn_button(&mut self, button: Button) -> EntityCommands<'_> {
        let mut ret = self.spawn((
            Node {
                width: button.button_size,
                height: button.button_size,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                margin: button.margins,
                ..default()
            },
            BorderRadius::all(button.border_radius),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            BackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.9).into()),
            ThemeBackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.9).into()),
        ));
        ret.with_children(|c| {
            c.spawn((
                ImageNode {
                    color: Color::srgba_u8(255, 255, 255, 255),
                    image: button.image,
                    ..default()
                },
                Node {
                    width: button.image_size,
                    height: button.image_size,
                    ..default()
                },
                Pickable {
                    should_block_lower: false,
                    is_hoverable: false,
                },
            ));
        })
        .hover_effect();
        ret
    }
}

pub trait InputSpawner<T>
where
    T: Component,
{
    fn spawn_input(&mut self, width: Val, font_size: f32, marker: T) -> EntityCommands<'_>;
}

impl<T> InputSpawner<T> for bevy::ecs::relationship::RelatedSpawnerCommands<'_, ChildOf>
where
    T: Component,
{
    fn spawn_input(&mut self, width: Val, font_size: f32, marker: T) -> EntityCommands<'_> {
        let mut input = self.spawn((
            BackgroundColor(Srgba::new(0.0, 0.0, 0.0, 0.7).into()),
            border_radius_pct(12.0),
            Node {
                width,
                height: Val::Px(font_size * 2.2),
                padding: UiRect::all(Val::Percent(5.0)),
                ..default()
            },
            Pickable {
                is_hoverable: true,
                should_block_lower: false,
            },
        ));
        input.with_children(|c| {
            c.spawn((
                TextInputNode {
                    mode: TextInputMode::SingleLine,
                    max_chars: Some(8 + 32),
                    ..default()
                },
                TextFont {
                    font_size,
                    ..default()
                },
                Node {
                    width,
                    height: Val::Px(font_size * 1.2),
                    align_self: AlignSelf::Center,
                    ..default()
                },
                Pickable {
                    is_hoverable: true,
                    should_block_lower: false,
                },
            ))
            .insert(marker);
        });
        input
    }
}

pub trait LayoutSpawner {
    fn spawn_spacer(&mut self);
    fn spawn_row(
        &mut self,
        align_items: AlignItems,
        func: impl FnOnce(&mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>),
    );
    fn spawn_row_with_wrap(
        &mut self,
        align_items: AlignItems,
        func: impl FnOnce(&mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>),
    );
    fn spawn_col(
        &mut self,
        func: impl FnOnce(&mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>),
    );
    fn spawn_list(
        &mut self,
        func: impl FnOnce(&mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>),
    );
}

impl LayoutSpawner for bevy::ecs::relationship::RelatedSpawnerCommands<'_, ChildOf> {
    fn spawn_spacer(&mut self) {
        self.spawn(Node {
            width: Val::Px(20.0),
            height: Val::Px(20.0),
            ..default()
        });
    }
    fn spawn_col(
        &mut self,
        func: impl FnOnce(&mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>),
    ) {
        self.spawn((Node {
            flex_direction: FlexDirection::Column,
            ..default()
        },))
            .with_children(func);
    }
    fn spawn_row(
        &mut self,
        align_items: AlignItems,
        func: impl FnOnce(&mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>),
    ) {
        self.spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items,
            ..default()
        },))
            .with_children(func);
    }
    fn spawn_row_with_wrap(
        &mut self,
        align_items: AlignItems,
        func: impl FnOnce(&mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>),
    ) {
        self.spawn((Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            align_items,
            row_gap: Val::Px(10.0),
            column_gap: Val::Px(10.0),
            ..default()
        },))
            .with_children(func);
    }
    fn spawn_list(
        &mut self,
        func: impl FnOnce(&mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>),
    ) {
        self.spawn((
            ScrollView {
                scroll_speed: 9000.0,
            },
            Node {
                border: UiRect::all(Val::Px(2.)),
                width: Val::Px(400.),
                max_height: Val::Px(250.0),
                overflow: Overflow::scroll_y(),
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            BorderColor::all(Color::linear_rgba(1.0, 0.0, 0.0, 1.0)),
            BorderRadius::all(Val::Px(20.)),
            BackgroundColor(Color::srgba_u8(0, 0, 0, 200)),
        ))
        .with_children(|c| {
            c.spawn((
                ScrollableContent::default(),
                Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Percent(100.0),
                    ..default()
                },
                Pickable {
                    is_hoverable: true,
                    should_block_lower: false,
                },
            ))
            .with_children(func);
        });
    }
}

pub fn border_radius_pct(pct: f32) -> BorderRadius {
    BorderRadius::all(Val::Percent(pct))
}

pub fn text_centered_with_color(text: &str, color: Color) -> impl Bundle {
    (
        TextSpan::default(),
        TextColor(color),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextLayout {
            justify: Justify::Center,
            ..default()
        },
        Text::new(text),
    )
}

pub fn text_centered(text: &str) -> impl Bundle {
    text_centered_with_color(text, Color::WHITE)
}
