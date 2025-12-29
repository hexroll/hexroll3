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
use std::collections::HashMap;

use egui::{Context, CursorIcon, FontId, Ui};
use path_tree::PathTree;

use crate::{
    app::helpers::html::render_demidom,
    app::{HexrollTestbedApp, RouteHandler},
};

impl HexrollTestbedApp {
    pub fn preview_panel(&mut self, ctx: &Context, ui: &mut Ui) {
        ui.scope(|ui| {
            // Some styling before we render
            ui.style_mut().visuals.button_frame = true;
            ui.style_mut().visuals.widgets.inactive.rounding = egui::Rounding::same(5.0);
            ui.style_mut().visuals.widgets.active.rounding = egui::Rounding::same(5.0);
            ui.style_mut().visuals.widgets.hovered.rounding = egui::Rounding::same(5.0);

            // This controls the overall scale of the rendered HTML
            let font_size = (ui.available_width() / 50.0) * self.config.zoom_level;

            ui.style_mut().override_font_id = Some(FontId::proportional(font_size));
            ui.style_mut().text_styles.insert(
                egui::TextStyle::Button,
                egui::FontId::new(font_size * 0.8, eframe::epaint::FontFamily::Proportional),
            );

            // Render and process any DemidomResponse we get, which is mostly click
            // events on links or buttons.
            let ret = render_demidom(self.current_entity.html_demidom.clone(), ui, font_size, 1);
            if let Some(ret) = ret {
                if ret.url != self.current_url {
                    self.current_url = ret.url.clone();
                    ctx.output_mut(|o| {
                        o.cursor_icon = CursorIcon::Wait;
                    });
                    // Response URLs are passed to the router for in exchange
                    // for a callback for things like navigating to another entity,
                    // rerolling or appending.
                    let mut tree = PathTree::<RouteHandler>::new();
                    HexrollTestbedApp::routes(&mut tree);
                    [&ret].iter().for_each(|v| {
                        if let Some(route) = tree.find(&v.url).clone() {
                            let param_map: HashMap<String, String> = route
                                .1
                                .params()
                                .iter()
                                .map(|&(k, v)| (k.to_string(), v.to_string()))
                                .collect();
                            route.0(self, &param_map);
                        }
                    });
                }
            }
        });
    }
}
