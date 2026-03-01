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

use std::io;

use ron::de::from_bytes;

use bevy::{asset::AssetLoader, platform::collections::HashMap, prelude::*};
use bevy_simple_scroll_view::{ScrollView, ScrollableContent};
use serde::Deserialize;

use crate::{
    shared::widgets::{
        buttons::MenuButtonEffects,
        cursor::TooltipOnHover,
        modal::{DiscreteAppState, ModalWindow},
    },
    vtt::sync::EventContext,
};

use super::{SpawnToken, SpawnTokenFromLibrary, Token};

pub struct TokensLibraryPlugin;
impl Plugin for TokensLibraryPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<TokenTemplates>()
            .init_asset_loader::<TokenTemplatesAssetLoader>()
            .add_systems(Startup, setup)
            .add_observer(on_spawn_token_from_library);
    }
}

#[derive(Resource)]
pub struct TokenLibrary {
    pub last_spawned: Option<TokenTemplate>,
    pub token_templates_handle: Handle<TokenTemplates>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TokenTemplate {
    pub token_name: String,
    pub light_radius: f32,
    pub token_size: f32,
    pub player_enabled: bool,
}

#[derive(Asset, TypePath, Debug, Deserialize)]
pub struct TokenTemplates {
    pub templates: HashMap<String, TokenTemplate>,
}

impl TokenTemplates {
    pub fn tokens(&self) -> Vec<String> {
        let mut tokens = self
            .templates
            .iter()
            .map(|(k, _)| k.to_string())
            .collect::<Vec<_>>();
        tokens.sort_by(|a, b| {
            let ta = self.templates.get(a).unwrap();
            let tb = self.templates.get(b).unwrap();

            match ta.player_enabled.cmp(&tb.player_enabled) {
                std::cmp::Ordering::Equal => ta
                    .light_radius
                    .partial_cmp(&tb.light_radius)
                    .unwrap_or(std::cmp::Ordering::Equal),
                ord => ord,
            }
        });
        tokens.reverse();
        tokens
    }
    fn from_bytes(
        bytes: Vec<u8>,
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self, io::Error> {
        let tokens_library: TokenTemplates =
            from_bytes(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        // TODO: Do we need an asset label here?
        // let h =
        //     load_context.add_labeled_asset("AssetLabel".to_string(), tokens_library.clone());
        Ok(tokens_library)
    }
}

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let token_templates_handle: Handle<TokenTemplates> = asset_server.load("tokens.ron");
    commands.insert_resource(TokenLibrary {
        last_spawned: None,
        token_templates_handle,
    });
}

pub fn on_spawn_token_from_library(
    trigger: On<SpawnTokenFromLibrary>,
    mut next_state: ResMut<NextState<DiscreteAppState>>,
    mut commands: Commands,
    library: Res<TokenLibrary>,
    asset_server: Res<AssetServer>,
    templates: Res<Assets<TokenTemplates>>,
) {
    let Some(templates) = templates.get(&library.token_templates_handle) else {
        return;
    };
    next_state.set(DiscreteAppState::Modal);
    commands
        .spawn((
            Name::new("TokensLibraryWindow"),
            ModalWindow,
            Node {
                position_type: PositionType::Absolute,
                justify_self: JustifySelf::Center,
                align_self: AlignSelf::Center,
                width: Val::Percent(80.0),
                height: Val::Percent(80.0),
                padding: UiRect::new(
                    Val::Px(30.0),
                    Val::Px(30.0),
                    Val::Px(30.0),
                    Val::Px(30.0),
                ),
                ..default()
            },
            BorderRadius::new(
                Val::Percent(3.0),
                Val::Percent(3.0),
                Val::Percent(3.0),
                Val::Percent(3.0),
            ),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            ScrollView {
                scroll_speed: 3000.0,
            },
            BackgroundColor(Srgba::new(1.0, 1.0, 1.0, 0.5).into()),
            ZIndex(999),
        ))
        .with_children(|c| {
            let tokens = templates.tokens();
            c.spawn((
                ScrollableContent::default(),
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
            ))
            .with_children(|c| {
                for token in tokens.iter() {
                    let pos = trigger.event().pos;
                    let token = token.to_string();
                    c.spawn((
                        Node {
                            width: Val::Px(150.0),
                            height: Val::Px(150.0),
                            ..Default::default()
                        },
                    )).with_child((
                        ImageNode {
                            image: asset_server.load(format!("tokens/thumbnails/{}.png", token)),
                            ..default()
                        },
                        Pickable {
                            should_block_lower: false,
                            is_hoverable: false,
                        },
                    ))
                    .menu_button_hover_effect()
                    .tooltip_on_hover(&token, 1.0)
                    .observe(
                        move |_trigger: On<Pointer<Click>>,
                            mut commands: Commands,
                            templates: Res<Assets<TokenTemplates>>,
                            mut library: ResMut<TokenLibrary>,
                            mut next_state: ResMut<NextState<DiscreteAppState>>| {
                                let Some(templates_library) = templates.get(&library.token_templates_handle) else {
                                    return;
                                };
                                if let Some(token_template) = templates_library.templates.get(&token) {
                                    let template_clone =  token_template.clone();
                                    library.last_spawned = Some(template_clone);
                                    next_state.set(DiscreteAppState::Normal);
                                    commands.trigger(EventContext::from(SpawnToken {
                                        token: Token::from_template(&token_template),
                                        transform: Transform::from_scale(Vec3::splat(1.0)).with_translation(pos),
                                    }));
                                }
                            });
                }
            });
        });
}

#[derive(Default)]
struct TokenTemplatesAssetLoader;

impl AssetLoader for TokenTemplatesAssetLoader {
    type Asset = TokenTemplates;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &(),
        mut load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, std::io::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(TokenTemplates::from_bytes(bytes, &mut load_context)?)
    }
}
