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
use egui::CursorIcon;
use egui::Ui;
use egui_json_tree::{DefaultExpand, JsonTree, render::DefaultRender};

use crate::app::HexrollTestbedApp;

impl HexrollTestbedApp {
    pub fn raw_json_panel(&mut self, ui: &mut Ui, value: serde_json::Value) {
        let tree = JsonTree::new("", &value);
        ui.style_mut().interaction.selectable_labels = false;
        tree.on_render(|ui, context| {
            match serde_json::to_string_pretty(context.value()) {
                Ok(pretty_str) if pretty_str.trim_matches('"').len() == 8 => {
                    let rendered_tree_item = context.render_default(ui);
                    if rendered_tree_item.hovered() {
                        let rendered_tree_item = rendered_tree_item
                            .highlight()
                            .on_hover_cursor(CursorIcon::Default);
                        if rendered_tree_item.is_pointer_button_down_on() {
                            rendered_tree_item
                                .ctx
                                .set_cursor_icon(CursorIcon::Wait);
                        }
                        if rendered_tree_item.clicked() {
                            rendered_tree_item
                                .ctx
                                .set_cursor_icon(CursorIcon::Wait);
                            self.navigate(
                                pretty_str.to_string().trim_matches('"'),
                                true,
                            );
                            rendered_tree_item
                                .ctx
                                .set_cursor_icon(CursorIcon::Default);
                        }
                    }
                }
                _ => {
                    context.render_default(ui);
                }
            }
        })
        .default_expand(DefaultExpand::All)
        .show(ui);
    }
}
