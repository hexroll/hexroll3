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

use bevy::ecs::{component::Component, resource::Resource};

use crate::{clients::model::FetchEntityReason, shared::widgets::buttons::Switch};

#[derive(Debug, Resource, Default)]
pub struct ContentContext {
    pub current_entity_uid: Option<String>,
    pub current_hex_uid: Option<String>,
    pub history: Vec<String>,
    pub fistory: Vec<String>,
    pub unlocked: bool,
    pub rerollable: bool,
    pub spoilers: bool,
}

impl ContentContext {
    pub fn set_current_uid(&mut self, uid: String, why: &FetchEntityReason) {
        if let Some(current_uid) = self.current_entity_uid.as_ref()
            && current_uid == &uid
        {
            return;
        }
        if *why != FetchEntityReason::History {
            if let Some(last_uid) = self.current_entity_uid.as_ref() {
                self.history.push(last_uid.clone());
            }
            self.invalidate_forward_navigation();
        }
        self.current_entity_uid = Some(uid);
    }

    pub fn invalidate_last_history_entry(&mut self) {
        if let Some(history_uid) = self.history.last()
            && let Some(current_uid) = &self.current_entity_uid
        {
            if history_uid == current_uid {
                self.history.pop();
            }
        }
        self.current_entity_uid = None;
    }

    pub fn invalidate_forward_navigation(&mut self) {
        self.fistory.clear();
    }

    pub fn go_back(&mut self) -> Option<String> {
        if let Some(uid) = self.history.last() {
            if let Some(curr_uid) = self.current_entity_uid.as_ref() {
                self.fistory.push(curr_uid.clone());
            }
            let ret = uid.clone();
            self.history.pop();
            Some(ret)
        } else {
            None
        }
    }

    pub fn go_forward(&mut self) -> Option<String> {
        if let Some(uid) = self.fistory.last() {
            if let Some(curr_uid) = self.current_entity_uid.as_ref() {
                self.history.push(curr_uid.clone());
            }
            let ret = uid.clone();
            self.fistory.pop();
            Some(ret)
        } else {
            None
        }
    }
}

#[derive(Component, Default, PartialEq, Clone)]
pub enum Spoilers {
    #[default]
    Visible,
    Hidden,
}

impl Switch for Spoilers {
    fn rotate(&self) -> Self {
        match self {
            Spoilers::Visible => Spoilers::Hidden,
            Spoilers::Hidden => Spoilers::Visible,
        }
    }

    fn index(&self) -> usize {
        match self {
            Spoilers::Visible => 0,
            Spoilers::Hidden => 1,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => Spoilers::Visible,
            1 => Spoilers::Hidden,
            _ => unreachable!(),
        }
    }
}
