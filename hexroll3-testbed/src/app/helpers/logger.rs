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
use std::sync::{Arc, Mutex};

use chrono::Local;
use log::LevelFilter;

struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
}

pub struct LogStorage {
    messages: Arc<Mutex<Vec<LogEntry>>>,
}

impl LogStorage {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub fn init_logger(&self) {
        let messages = self.messages.clone();
        log::set_boxed_logger(Box::new(SimpleLogger { messages })).unwrap();
        log::set_max_level(LevelFilter::Trace);
    }

    pub fn show(&self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("log_messages")
                .num_columns(3)
                .striped(true)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for msg in self.messages.lock().unwrap().iter().rev() {
                        ui.style_mut().visuals.override_text_color = match msg
                            .level
                            .as_str()
                        {
                            "TRACE" => Some(
                                ctx.theme().default_visuals().weak_text_color(),
                            ),
                            "ERROR" => Some(
                                ctx.theme().default_visuals().error_fg_color,
                            ),
                            "WARN" => Some(
                                ctx.theme().default_visuals().warn_fg_color,
                            ),
                            _ => None,
                        };
                        ui.label(&msg.timestamp);
                        ui.label(&msg.level);
                        ui.label(&msg.message);
                        ui.end_row();
                    }
                });
        });
    }
}

struct SimpleLogger {
    messages: Arc<Mutex<Vec<LogEntry>>>,
}

impl log::Log for SimpleLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        if !record.module_path().unwrap().contains("hexroll") {
            return;
        }
        let mut msgs = self.messages.lock().unwrap();
        if msgs.len() > 10000 {
            msgs.remove(0);
        }

        let timestamp = Local::now().format("%H:%M:%S%.6f").to_string();
        msgs.push(LogEntry {
            timestamp,
            level: format!("{}", record.level()),
            message: format!("{}", record.args()),
        });
    }
    fn flush(&self) {}
}
