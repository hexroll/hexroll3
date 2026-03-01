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
use bevy::prelude::*;

use bevy_matchbox::prelude::PeerId;

use crate::{
    battlemaps::{
        BattlemapUserDrawing, BattlemapUserDrawingInProgress, DryeraseDrawingMessage,
        SpawnVfx, SpawnVfxBroadcast,
    },
    dice::DiceRollResolved,
    hexmap::{HexMapTime, HexState, HexmapTheme, MapMessage, elements::MainCamera},
    hud::DiceMessage,
    shared::{
        camera::{CameraControl, camera_callback},
        input::InputMode,
        labels::LabelToken,
        vtt::{HexRevealState, StoreVttState, VttData},
    },
    tokens::{Token, TokenMessage, TokenUpdateMessage},
};

use super::network::{
    NetworkContext, NetworkingState, SendFullStateToPeer, broadcast_ephemeral_message,
    broadcast_message, broadcast_state_to_new_peers, detect_first_node,
    receive_channel_messages, receive_control_messages, receive_ephemeral_channel_messages,
    send_message,
};

use super::network::NetworkingConnection;

const TOKENS_CHANNEL_ID: usize = 1;
const HEX_CHANNEL_ID: usize = 2;
const DICE_CHANNEL_ID: usize = 3;
const VFX_CHANNEL_ID: usize = 4;
const CAMERA_CHANNEL_ID: usize = 5;
const DRAWING_CHANNEL_ID: usize = 6;

pub struct SyncPlugin;
impl Plugin for SyncPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_token_message);
        app.add_observer(dice_results);
        app.add_observer(spawn_vfx);
        app.add_observer(on_frame_players_camera);

        let my_receive_control_messages =
            receive_control_messages(app.register_system(invalidate_state_callback));
        let receive_dice_messages = receive_ephemeral_channel_messages(
            DICE_CHANNEL_ID,
            app.register_system(dice_results_callback),
        );
        let receive_drawing_messages = receive_ephemeral_channel_messages(
            DRAWING_CHANNEL_ID,
            app.register_system(drawing_callback),
        );
        let receive_vfx_messages = receive_ephemeral_channel_messages(
            VFX_CHANNEL_ID,
            app.register_system(vfx_callback),
        );
        let receive_hex_messages = receive_channel_messages(
            HEX_CHANNEL_ID,
            app.register_system(hex_message_callback),
        );
        let receive_token_messages = receive_channel_messages(
            TOKENS_CHANNEL_ID,
            app.register_system(token_message_callback),
        );
        let receive_camera_messages = receive_ephemeral_channel_messages(
            CAMERA_CHANNEL_ID,
            app.register_system(camera_callback),
        );

        app.add_systems(
            Update,
            (
                ((
                    my_receive_control_messages.after(detect_first_node),
                    (
                        receive_token_messages,
                        receive_hex_messages,
                        receive_dice_messages,
                        receive_vfx_messages,
                        receive_drawing_messages,
                        receive_camera_messages,
                    ),
                )
                    // NOTE: This is to ensure we do not clear the state after getting init messages
                    .chain())
                // NOTE: this is to ensure we run these systems only if we are connected
                .run_if(in_state(NetworkingConnection::Connected)),
                broadcast_state_to_new_peers.run_if(
                    in_state(NetworkingConnection::Connected)
                        .and(in_state(NetworkingState::Initialized)),
                ),
            )
                .chain(), // This is to ensure we broadcast state only after our state is intact
        );
        app.add_observer(on_broadcast_state_to_peer);
        app.add_observer(on_sync_map_for_peers);
    }
}

fn on_broadcast_state_to_peer(
    trigger: On<SendFullStateToPeer>,
    tokens: Query<(&Transform, &Token)>,
    vtt_data: Res<VttData>,
    drawings: Query<&BattlemapUserDrawing, Without<BattlemapUserDrawingInProgress>>,
    theme: Res<HexmapTheme>,
    hexmap_time: Single<&HexMapTime>,
    mut socket: ResMut<NetworkContext>,
) {
    let peer = trigger.event().peer;
    debug!("Sending state to connecting peer: {}", peer);
    socket.begin_sync(peer);
    for (transform, token) in tokens.iter() {
        let msg = TokenMessage::Update(TokenUpdateMessage {
            token_id: token.token_id,
            token_name: token.token_name.clone(),
            glb_file: token.glb_file.clone(),
            color: Some(token.color),
            light_color: Some(token.light_color),
            label: Some(token.label.clone()),
            light: Some(token.light),
            transform: Some(*transform),
            mobility: Some(token.mobility.clone()),
        });
        send_token_message(&mut socket, peer, msg);
    }
    for (coords, state) in vtt_data.revealed.iter() {
        let msg = MapMessage::HexStateChange(HexState {
            coords: *coords,
            state: Some(*state),
            is_ocean: false,
        });
        send_hex_message(&mut socket, peer, msg);
    }
    for coords in vtt_data.revealed_ocean.iter() {
        let msg = MapMessage::HexStateChange(HexState {
            coords: *coords,
            state: Some(HexRevealState::Full),
            is_ocean: true,
        });
        send_hex_message(&mut socket, peer, msg);
    }
    send_hex_message(
        &mut socket,
        peer,
        MapMessage::OpenedDoors(vtt_data.open_doors.clone()),
    );
    for drawing in drawings.iter() {
        send_drawing_message(
            &mut socket,
            peer,
            DryeraseDrawingMessage::AddDrawing(drawing.clone()),
        );
    }
    send_hex_message(
        &mut socket,
        peer,
        MapMessage::SwitchDayNight(hexmap_time.day_night.clone()),
    );
    send_hex_message(
        &mut socket,
        peer,
        MapMessage::ChangeTheme(theme.name.clone()),
    );
    socket.end_sync(peer);
}

fn invalidate_state_callback(
    mut commands: Commands,
    tokens: Query<Entity, With<Token>>,
    labels: Query<Entity, With<LabelToken>>,
    mut map_data: ResMut<VttData>,
) {
    for e in tokens.iter() {
        commands.entity(e).despawn();
    }
    for e in labels.iter() {
        commands.entity(e).despawn();
    }
    map_data.revealed.clear();
    map_data.revealed_ocean.clear();
}

#[derive(Default, Clone, PartialEq)]
pub enum EventSource {
    #[default]
    User,
    Peer,
    Save,
}

#[derive(Event)]
pub struct EventContext<T> {
    pub event: T,
    pub source: EventSource,
}

impl<T> EventContext<T> {
    pub fn with_source(mut self, source: EventSource) -> Self {
        self.source = source;
        self
    }
}

impl<T> From<T> for EventContext<T> {
    fn from(event: T) -> Self {
        Self {
            event,
            source: EventSource::default(),
        }
    }
}

fn on_token_message(
    trigger: On<EventContext<TokenMessage>>,
    mut socket: Option<ResMut<NetworkContext>>,
    mut commands: Commands,
) {
    if trigger.source != EventSource::Save {
        commands.trigger(StoreVttState);
    }
    let socket = if socket.is_some() {
        socket.as_mut().unwrap()
    } else {
        return;
    };
    broadcast_token_message(socket, trigger.event.clone());
}

#[derive(Event)]
pub struct SyncMapForPeers(pub MapMessage);

pub fn on_sync_map_for_peers(
    trigger: On<SyncMapForPeers>,
    mut socket: Option<ResMut<NetworkContext>>,
) {
    // NOTE: Apply to peers
    let socket = if socket.is_some() {
        socket.as_mut().unwrap()
    } else {
        return;
    };
    broadcast_hex_message(socket, trigger.0.clone());
}

pub fn detect_camera_control(
    keys: Res<ButtonInput<KeyCode>>,
    input_mode: Res<InputMode>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::KeyF) && input_mode.keyboard_available() {
        commands.trigger(FramePlayerCamera);
    }
}

fn spawn_vfx(
    trigger: On<SpawnVfxBroadcast>,
    mut socket: Option<ResMut<NetworkContext>>,
    mut commands: Commands,
) {
    // NOTE: Apply locally
    commands.trigger(trigger.event().msg.clone());

    // NOTE: Apply to peers
    let socket = if socket.is_some() {
        socket.as_mut().unwrap()
    } else {
        return;
    };
    broadcast_vfx_message(socket, trigger.event().msg.clone());
}

fn dice_results(
    trigger: On<DiceRollResolved>,
    mut socket: Option<ResMut<NetworkContext>>,
    mut commands: Commands,
    vtt_data: Res<VttData>,
) {
    // NOTE: Apply locally
    commands.trigger(DiceMessage {
        roller: vtt_data.node_name.clone(),
        dice_roll: trigger.event().dice_roll.clone(),
    });

    // NOTE: Apply to peers
    let socket = if socket.is_some() {
        socket.as_mut().unwrap()
    } else {
        return;
    };
    broadcast_dice_message(
        socket,
        DiceMessage {
            roller: vtt_data.node_name.clone(),
            dice_roll: trigger.event().dice_roll.clone(),
        },
    );
}

//
// Incoming Broadcasts Systems
//
fn dice_results_callback(message: In<DiceMessage>, mut commands: Commands) {
    commands.trigger(message.0);
}

pub fn token_message_callback(message: In<TokenMessage>, mut commands: Commands) {
    commands.trigger(message.0);
}

pub fn hex_message_callback(message: In<MapMessage>, mut commands: Commands) {
    commands.trigger(message.0);
}

pub fn drawing_callback(message: In<DryeraseDrawingMessage>, mut commands: Commands) {
    commands.trigger(message.0);
}

pub fn vfx_callback(message: In<SpawnVfx>, mut commands: Commands) {
    commands.trigger(message.0);
}

//
// Outgoing Broadcast Helpers

pub fn broadcast_hex_message(socket: &mut ResMut<NetworkContext>, msg: MapMessage) {
    broadcast_message(socket, HEX_CHANNEL_ID, msg);
}

pub fn broadcast_token_message(socket: &mut ResMut<NetworkContext>, msg: TokenMessage) {
    broadcast_message(socket, TOKENS_CHANNEL_ID, msg);
}

pub fn broadcast_drawing_message(
    socket: &mut ResMut<NetworkContext>,
    msg: DryeraseDrawingMessage,
) {
    broadcast_ephemeral_message(socket, DRAWING_CHANNEL_ID, msg);
}

pub fn broadcast_vfx_message(socket: &mut ResMut<NetworkContext>, msg: SpawnVfx) {
    broadcast_ephemeral_message(socket, VFX_CHANNEL_ID, msg);
}

pub fn broadcast_camera_message(socket: &mut ResMut<NetworkContext>, msg: CameraControl) {
    broadcast_ephemeral_message(socket, CAMERA_CHANNEL_ID, msg);
}

pub fn broadcast_dice_message(socket: &mut ResMut<NetworkContext>, msg: DiceMessage) {
    broadcast_ephemeral_message(socket, DICE_CHANNEL_ID, msg);
}

//
// Peer-specific Messaging Helpers
//
pub fn send_token_message(
    socket: &mut ResMut<NetworkContext>,
    peer: PeerId,
    msg: TokenMessage,
) {
    send_message(socket, TOKENS_CHANNEL_ID, peer, msg);
}

pub fn send_hex_message(socket: &mut ResMut<NetworkContext>, peer: PeerId, msg: MapMessage) {
    send_message(socket, HEX_CHANNEL_ID, peer, msg);
}

pub fn send_drawing_message(
    socket: &mut ResMut<NetworkContext>,
    peer: PeerId,
    msg: DryeraseDrawingMessage,
) {
    send_message(socket, DRAWING_CHANNEL_ID, peer, msg);
}

#[derive(Event)]
pub struct FramePlayerCamera;

pub fn on_frame_players_camera(
    _: On<FramePlayerCamera>,
    mut socket: Option<ResMut<NetworkContext>>,
    camera: Single<(&Transform, &Projection), With<MainCamera>>,
) {
    let socket = if socket.is_some() {
        socket.as_mut().unwrap()
    } else {
        return;
    };
    let (t, p) = *camera;
    if let Projection::Orthographic(o) = p {
        let msg = CameraControl {
            camera_translation: t.translation,
            camera_scale: o.scale,
        };
        broadcast_camera_message(socket, msg);
    }
}
