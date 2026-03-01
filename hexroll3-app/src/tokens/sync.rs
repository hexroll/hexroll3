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

use bevy::{platform::collections::HashSet, prelude::*};

use crate::{
    shared::vtt::{StoreVttState, VttData},
    vtt::sync::{EventContext, EventSource},
};

use super::{
    Token, TokenMeshEntity,
    control::UpdateTokenLabel,
    tokens::{TokenMobility, TokenMobilityActuator},
    update_token_material,
};

#[derive(Serialize, Deserialize, Clone, Event)]
pub enum TokenMessage {
    Update(TokenUpdateMessage),
    Delete(u32),
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TokenUpdateMessage {
    pub token_name: String,
    pub glb_file: String,
    pub token_id: u32,
    pub transform: Option<Transform>,
    pub light: Option<f32>,
    pub color: Option<Color>,
    pub light_color: Option<Color>,
    pub label: Option<String>,
    pub mobility: Option<TokenMobility>,
}

impl TokenUpdateMessage {
    pub fn from_token(token: &Token) -> Self {
        Self {
            token_id: token.token_id,
            token_name: token.token_name.clone(),
            glb_file: token.glb_file.clone(),
            light: Some(token.light),
            color: Some(token.color),
            light_color: Some(token.light_color),
            label: Some(token.label.clone()),
            mobility: Some(token.mobility.clone()),
            transform: None,
        }
    }

    pub fn from_token_id(token_id: u32) -> Self {
        Self {
            token_id,
            ..default()
        }
    }

    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = Some(transform);
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn with_light_color(mut self, color: Color) -> Self {
        self.light_color = Some(color);
        self
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = Some(label.clone());
        self
    }
}

pub fn on_token_message(
    message: On<TokenMessage>,
    mut commands: Commands,
    mut tokens: Query<(Entity, &mut Transform, &mut Token)>,
    vtt_data: Res<VttData>,
    finder: Query<&TokenMeshEntity>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut already_spawned: HashSet<u32> = HashSet::new();
    match message.event() {
        TokenMessage::Delete(token_id) => {
            for (e, _, token) in tokens.iter_mut() {
                if &token.token_id == token_id {
                    commands.entity(e).despawn();
                }
            }
        }
        TokenMessage::Update(update_msg) => {
            let mut found = false;
            for (e, mut transform, mut token) in tokens.iter_mut() {
                if token.token_id == update_msg.token_id {
                    if let Some(msg_transform) = update_msg.transform {
                        transform.scale = msg_transform.scale;
                        transform.translation = msg_transform.translation;
                        transform.rotation = msg_transform.rotation;
                    }
                    if let Some(msg_light) = update_msg.light {
                        token.light = msg_light;
                    }
                    if let Some(msg_color) = update_msg.color {
                        token.color = msg_color;
                        update_token_material(
                            &mut commands,
                            &finder,
                            &mesh_materials,
                            &mut materials,
                            e,
                            msg_color,
                        );
                    }
                    if let Some(msg_torch_color) = update_msg.light_color {
                        token.light_color = msg_torch_color;
                    }
                    if let Some(msg_mobility) = &update_msg.mobility {
                        token.mobility = msg_mobility.clone();
                        commands
                            .entity(e)
                            .apply_mobility_on_token(&token.mobility, &vtt_data);
                    }
                    if let Some(label) = &update_msg.label {
                        token.label = label.clone();
                        commands.trigger(UpdateTokenLabel {
                            token_entity: e,
                            label: label.clone(),
                        });
                    }

                    found = true;
                }
            }

            if !found && !already_spawned.contains(&update_msg.token_id) {
                already_spawned.insert(update_msg.token_id);
                commands.trigger(
                    EventContext::from(crate::tokens::SpawnToken {
                        token: Token {
                            token_name: update_msg.token_name.clone(),
                            glb_file: update_msg.glb_file.clone(),
                            token_id: update_msg.token_id,
                            light: update_msg.light.unwrap(),
                            color: update_msg.color.unwrap(),
                            light_color: update_msg.light_color.unwrap(),
                            label: update_msg.label.clone().unwrap(),
                            mobility: update_msg.mobility.clone().unwrap(),
                        },
                        transform: update_msg.transform.unwrap(),
                    })
                    .with_source(EventSource::Peer),
                );
            } else {
                commands.trigger(StoreVttState);
            }
        }
    }
}
