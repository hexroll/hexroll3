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
mod helpers;
mod views;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::{collections::HashMap, rc::Rc};

use hexroll3_scroll::instance::SandboxInstance;

use helpers::config::load_settings;
use helpers::html::Element;
use helpers::logger::LogStorage;

use self::helpers::config::TestbedConfig;

pub struct HexrollTestbedApp {
    // Main app state
    instance: Option<SandboxInstance>,
    current_entity: EntityData,
    scroll_in_filepath_is_valid: bool,

    // Views state
    uid_to_show: String,
    current_url: String,
    left_sidebar_expanded: bool,
    center_view_mode: CenterViewMode,
    file: egui_file_dialog::FileDialog,
    history: VecDeque<String>,

    // Utilities
    app_logs: LogStorage,
    config: TestbedConfig,
}

#[derive(Default)]
pub struct EntityData {
    uid: String,
    json_stored: serde_json::Value,
    json_rendered: serde_json::Value,
    html_source: String,
    html_demidom: Rc<RefCell<HashMap<usize, Element>>>,
}

#[derive(PartialEq, Clone)]
enum CenterViewMode {
    Preview,
    RawHtml,
    RenderedJson,
}

type RouteHandler =
    Box<dyn Fn(&mut HexrollTestbedApp, &HashMap<String, String>)>;

impl Default for HexrollTestbedApp {
    fn default() -> Self {
        let mut cconfig =
            load_settings().unwrap_or_else(|_| TestbedConfig::default());
        if cconfig.main_scroll_filepath.is_empty() {
            cconfig.main_scroll_filepath =
                "./hexroll3-scroll-data/main.scroll".to_owned();
        }

        let storage = LogStorage::new();
        storage.init_logger();

        Self {
            // Main app state
            instance: None,
            current_entity: EntityData::default(),
            scroll_in_filepath_is_valid:
                HexrollTestbedApp::test_scroll_filepath(
                    &cconfig.main_scroll_filepath,
                ),
            // Views state
            uid_to_show: "root".to_string(),
            current_url: "".to_string(),
            left_sidebar_expanded: true,
            center_view_mode: CenterViewMode::Preview,
            file: egui_file_dialog::FileDialog::new()
                .initial_directory(
                    dirs::data_dir()
                        .or_else(dirs::home_dir)
                        .unwrap_or_else(|| std::env::current_dir().unwrap()),
                )
                .add_file_filter(
                    "H3X",
                    Arc::new(|path| {
                        path.extension().unwrap_or_default() == "h3x"
                    }),
                )
                .default_file_filter("H3"),
            history: VecDeque::new(),
            // Utilities
            app_logs: storage,
            config: cconfig,
        }
    }
}

impl eframe::App for HexrollTestbedApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_pixels_per_point(self.config.zoom_level);
        self.file.update(ctx);
        egui::TopBottomPanel::top("top-panel")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(10.0))
            .show(ctx, |ui| {
                self.top_app_bar(ctx, ui);
            });

        self.trace_panel(ctx, _frame);
        match self.instance {
            Some(_) => self.instance_panels(ctx, _frame),
            None => self.open_or_roll_panel(ctx),
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

impl HexrollTestbedApp {
    pub fn trace_panel(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
    ) {
        egui::TopBottomPanel::bottom("bottom-panel")
            .min_height(100.0)
            .max_height(1000.0)
            .resizable(true)
            .show_animated(ctx, self.left_sidebar_expanded, |ui| {
                self.app_logs.show(ctx, ui);
            });
    }
    pub fn instance_panels(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
    ) {
        let mut navigate_to: Option<String> = None;
        egui::SidePanel::left("left-panel")
            .resizable(true)
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(10.0))
            .show_animated(ctx, self.left_sidebar_expanded, |ui| {
                ctx.style_mut(|s| s.interaction.tooltip_delay = 0.01);
                collapsible_sidebar_button_ui(
                    ui,
                    &mut self.left_sidebar_expanded,
                );
                ui.horizontal(|ui| {
                    if ui.text_edit_singleline(&mut self.uid_to_show).changed()
                    {
                        navigate_to = Some(self.uid_to_show.clone());
                    }
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    self.raw_json_panel(
                        ui,
                        self.current_entity.json_stored.clone(),
                    );
                });
            });

        egui::TopBottomPanel::top("top-panel")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    if !self.left_sidebar_expanded {
                        collapsible_sidebar_button_ui(
                            ui,
                            &mut self.left_sidebar_expanded,
                        );
                    }
                    self.center_panel_selectors(ui);
                    ui.separator();
                    if ui.button("Back").clicked()
                        && self.history.pop_front().is_some()
                    {
                        if let Some(next_item) = self.history.front() {
                            self.navigate(&next_item.clone(), false);
                        }
                    }
                });
            });

        const MARGIN_FACTOR: f32 = 0.8;

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    if self.center_view_mode == CenterViewMode::Preview {
                        ui.set_width(ui.available_width() * MARGIN_FACTOR);
                        ui.set_max_width(ui.available_width() * MARGIN_FACTOR);
                    }
                    ui.with_layout(
                        egui::Layout::left_to_right(egui::Align::TOP)
                            .with_main_wrap(true),
                        |ui| match self.center_view_mode {
                            CenterViewMode::Preview => {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                let row_height = ui
                                    .text_style_height(&egui::TextStyle::Body);
                                ui.set_row_height(row_height);
                                self.preview_panel(ctx, ui);
                            }
                            CenterViewMode::RawHtml => {
                                self.raw_html_panel(ui);
                            }
                            CenterViewMode::RenderedJson => {
                                if self.current_entity.json_rendered.is_null() {
                                    self.load_rendered_json();
                                }
                                self.rendered_json_panel(ui);
                            }
                        },
                    )
                });
            });
        });
        if let Some(uid) = navigate_to {
            self.navigate(&uid, false);
        }
    }
}

fn collapsible_sidebar_button_ui(ui: &mut egui::Ui, open: &mut bool) {
    if ui.button("☰").clicked() {
        *open = !*open;
    }
}
