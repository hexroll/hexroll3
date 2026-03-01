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

use serde::{Deserialize, Serialize};
use shortcuts::keyboard_shortcuts;
use std::time::Duration;

use bevy::{
    app::{App, Plugin, Update},
    ecs::{event::Event, schedule::IntoScheduleConfigs},
    state::condition::in_state,
};

use crate::{dice::DiceRoll, shared::AppState};

mod dicebar;
mod menubar;
mod searchbar;
mod shortcuts;
mod snackbar;
mod toggles;
mod vttbar;

mod drawer;
mod menu;

// This plugin must be added by the user for the HUD to work
pub struct OverlayPlugin;
impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            menubar::MenuBarPlugin,
            snackbar::SnackbarPlugin,
            drawer::DrawerPlugin,
            menu::MenuPlugin,
            searchbar::SearchBarPlugin,
            dicebar::DiceBarPlugin,
            vttbar::VttBarPlugin,
        ));
        app.add_systems(Update, keyboard_shortcuts.run_if(in_state(AppState::Live)));
    }
}

#[derive(Event)]
// Users can trigger this event to show a message in the snackbar
pub struct ShowTransientUserMessage {
    pub text: String,
    pub special: Option<String>,
    pub keep_alive: Option<Duration>,
}

#[derive(Serialize, Deserialize, Event)]
// Users can trigger this event to show dice roll results in the snackbar
pub struct DiceMessage {
    pub roller: String,
    pub dice_roll: DiceRoll,
}

#[derive(Event)]
// Users can trigger this event to open the searchbar and focus user input inside it
pub struct OpenSearchBarAndFocus;
