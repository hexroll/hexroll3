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
#![cfg_attr(
    all(target_os = "windows", not(feature = "dev")),
    windows_subsystem = "windows"
)]

mod help;
mod intro;
mod loading;
mod version;

use bevy::{
    input::common_conditions::input_toggle_active,
    input_focus::InputFocus,
    prelude::*,
    window::{CursorIcon, PrimaryWindow, SystemCursorIcon, WindowResolution},
};

use avian3d::PhysicsPlugins;

use bevy_hanabi::HanabiPlugin;

#[cfg(feature = "dev")]
use bevy_inspector_egui::quick::WorldInspectorPlugin;

#[cfg(target_arch = "wasm32")]
use bevy_inspector_egui::bevy_egui::input::EguiWantsInput;

use bevy_inspector_egui::bevy_egui::{EguiGlobalSettings, EguiPlugin};
use bevy_mod_outline::{AutoGenerateOutlineNormalsPlugin, OutlinePlugin};
use bevy_simple_scroll_view::ScrollViewPlugin;
use bevy_tweening::TweeningPlugin;

use hexroll3_app::{
    audio::SoundtrackPlugin,
    battlemaps::BattlemapMaterial,
    clients::{
        controller::{ApiControllerPlugin, RequestSandboxFromBackend},
        http::ApiClientPlugin,
        standalone::StandaloneClientPlugin,
    },
    content::ContentPlugin,
    dialogs::{DialogsPlugin, OpenSandboxOptionsModal},
    dice::{DiceConfig, DicePlugin},
    hexmap::{TileMaterial, plugin::HexMap},
    hud::OverlayPlugin,
    shared::{
        AppState, LoadingState,
        gltf::GltfProcessorPlugin,
        input::InputMode,
        settings::{self, UserSettings},
        spawnq::SpawnQueuePlugin,
        tweens::SharedTweensPlugin,
        widgets::{list::ListPlugin, modal::ModalPlugin},
    },
    tokens::TokensPlugin,
    vtt::VttPlugin,
};

use loading::update_loading_state;
use settings::AppSettings;

// NOTE: This function is needed to allow bevy_hanabi use
// serde in wasm, and is required by `typetag` crate as documented
// here: https://docs.rs/typetag/latest/typetag/#so-many-questions and
// here: https://docs.rs/inventory/0.3.21/inventory/index.html#webassembly-and-constructors
#[cfg(target_family = "wasm")]
unsafe extern "C" {
    fn __wasm_call_ctors();
}

fn main() {
    #[cfg(target_family = "wasm")]
    unsafe {
        __wasm_call_ctors();
    }

    let user_settings = UserSettings::read_or_init();

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgba(0.0, 0.0, 0.0, 0.0)))
        .insert_resource(AppSettings::default())
        .insert_resource(InputMode::KeyboardAvailable);
    // TODO: Make eco-mode part of the user config
    #[cfg(not(feature = "dev"))]
    app.insert_resource(bevy::winit::WinitSettings {
        focused_mode: bevy::winit::UpdateMode::Continuous,
        unfocused_mode: bevy::winit::UpdateMode::reactive_low_power(
            std::time::Duration::from_millis(200),
        ),
    });
    app.insert_resource(user_settings)
        .insert_resource(settings::Config::default())
        .register_type::<AppSettings>()
        .add_plugins(TweeningPlugin)
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: Some("#bevy".into()),
                        fit_canvas_to_parent: true,
                        resolution: WindowResolution::default(),
                        ..default()
                    }),
                    ..default()
                })
                .set({
                    #[cfg(target_os = "macos")]
                    {
                        AssetPlugin {
                            file_path: "../Resources/assets/".to_string(),
                            ..default()
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        AssetPlugin {
                            file_path: "https://0.0.0.0:5173/assets/".to_string(),
                            meta_check: bevy::asset::AssetMetaCheck::Never,
                            ..default()
                        }
                    }
                    #[cfg(all(not(target_os = "macos"), not(target_arch = "wasm32")))]
                    {
                        AssetPlugin::default()
                    }
                }),
        )
        .insert_state(hexroll3_app::shared::AppState::Intro)
        .insert_state(hexroll3_app::shared::LoadingState::default())
        .add_plugins(intro::IntroPlugin)
        .add_plugins(help::HelpPlugin)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(DialogsPlugin)
        .add_plugins(ScrollViewPlugin)
        .add_plugins(HanabiPlugin)
        .add_plugins(SharedTweensPlugin)
        .add_plugins(GltfProcessorPlugin)
        .add_plugins(SpawnQueuePlugin)
        .add_plugins(ModalPlugin)
        .add_plugins(ListPlugin)
        .add_plugins(OverlayPlugin)
        .add_plugins(ApiControllerPlugin)
        .add_plugins(ApiClientPlugin)
        .add_plugins(StandaloneClientPlugin)
        .add_plugins(MeshPickingPlugin)
        .add_plugins((OutlinePlugin, AutoGenerateOutlineNormalsPlugin::default()))
        .add_plugins(VttPlugin)
        .add_plugins(DicePlugin);

    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins((
        EguiPlugin::default(),
        #[cfg(feature = "dev")]
        WorldInspectorPlugin::default().run_if(input_toggle_active(false, KeyCode::F11)),
        #[cfg(feature = "dev")]
        bevy_inspector_egui::quick::AssetInspectorPlugin::<StandardMaterial>::default()
            .run_if(input_toggle_active(false, KeyCode::F10)),
        bevy_inspector_egui::quick::AssetInspectorPlugin::<BattlemapMaterial>::default()
            .run_if(input_toggle_active(false, KeyCode::F10)),
        bevy_inspector_egui::quick::AssetInspectorPlugin::<TileMaterial>::default()
            .run_if(input_toggle_active(false, KeyCode::F10)),
        bevy_inspector_egui::quick::ResourceInspectorPlugin::<DiceConfig>::default()
            .run_if(input_toggle_active(false, KeyCode::F10)),
    ))
    .add_systems(OnEnter(hexroll3_app::shared::AppState::Live), spawn_version);

    #[cfg(target_arch = "wasm32")]
    app.insert_resource(EguiWantsInput::default());
    app.add_systems(OnEnter(LoadingState::Ready), load_last_sandbox)
        .add_systems(PostStartup, init_cursor)
        .add_systems(
            OnEnter(AppState::Live),
            ask_for_sandbox_when_none_recently_used,
        )
        .add_plugins(SoundtrackPlugin)
        .add_plugins(HexMap)
        .add_plugins(ContentPlugin)
        .add_plugins(TokensPlugin)
        .add_systems(Update, update_input_mode)
        .add_systems(
            Update,
            hexroll3_app::shared::widgets::cursor::tooltips_system,
        )
        .add_systems(
            Update,
            update_loading_state.run_if(in_state(AppState::Intro)),
        )
        .run();
}

fn load_last_sandbox(mut commands: Commands, user_settings: Res<UserSettings>) {
    if user_settings.sandbox.is_some() {
        commands.trigger(RequestSandboxFromBackend {
            sandbox_uid: user_settings.sandbox.clone().unwrap(),
            pairing_key: user_settings.key.clone(),
        });
    }
}

fn init_cursor(mut commands: Commands, window: Single<Entity, With<PrimaryWindow>>) {
    commands
        .entity(*window)
        .insert(CursorIcon::System(SystemCursorIcon::Default));
}

fn ask_for_sandbox_when_none_recently_used(
    mut commands: Commands,
    user_settings: Res<UserSettings>,
) {
    if user_settings.sandbox.is_none() {
        commands.trigger(OpenSandboxOptionsModal);
    }
}

fn spawn_version(
    mut commands: Commands,
    mut egui_global_settings: ResMut<EguiGlobalSettings>,
) {
    egui_global_settings.enable_cursor_icon_updates = true;
    commands.spawn((
        Name::new("VersionHud"),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            bottom: Val::Px(10.0),
            ..default()
        },
        TextFont::default().with_font_size(10.0),
        Text::new(version::APP_VERSION),
    ));
}

fn update_input_mode(
    mut input_mode: ResMut<InputMode>,
    focused_input: Res<InputFocus>,
    text_inputs: Query<&bevy_ui_text_input::TextInputNode>,
) {
    if let Some(focused_entity) = focused_input.0 {
        if text_inputs.contains(focused_entity) {
            *input_mode = InputMode::KeyboardFocusNeeded;
        } else {
            *input_mode = InputMode::KeyboardAvailable;
        }
    } else {
        *input_mode = InputMode::KeyboardAvailable;
    }
}
