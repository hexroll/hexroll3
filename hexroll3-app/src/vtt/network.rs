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

use std::{any::type_name, time::Duration};

use bevy::{ecs::system::SystemId, prelude::*};
use bevy_matchbox::{
    MatchboxSocket,
    prelude::{PeerId, PeerState},
};
use serde::{Deserialize, Serialize};

use crate::{
    clients::controller::VttStateApiController,
    hud::ShowTransientUserMessage,
    shared::{settings::UserSettings, vtt::VttData},
};

pub struct NetworkingPlugin;
impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            detect_first_node.run_if(in_state(NetworkingState::Started)),
        )
        .add_systems(
            Update,
            detect_disconnect.run_if(in_state(NetworkingConnection::Connected)),
        )
        .add_observer(on_connect)
        .init_state::<NetworkingConnection>()
        .init_state::<NetworkingState>();
    }
}

#[derive(States, Debug, Hash, Eq, PartialEq, Clone, Default)]
pub enum NetworkingConnection {
    #[default]
    Disconnected,
    Connected,
}

#[derive(States, Debug, Hash, Eq, PartialEq, Clone, Default)]
pub enum NetworkingState {
    #[default]
    Started,
    Initialized,
    IgnoringStaleInitData,
    InitializingFromNode(PeerId),
}

#[derive(Serialize, Deserialize)]
pub enum ControlMessage {
    InitStarted(lamport::Time),
    InitCompleted,
}

#[derive(Resource)]
pub struct NetworkContext {
    pub clock: lamport::Clock,
    pub socket: MatchboxSocket,
}

#[derive(Serialize, Deserialize)]
pub struct ClockedMessage<T> {
    body: T,
    time: Option<lamport::Time>,
}

#[derive(Event)]
pub struct SendFullStateToPeer {
    pub peer: PeerId,
}

pub const CONTROL_CHANNEL_ID: usize = 0;

impl NetworkContext {
    pub fn begin_sync(&mut self, peer: PeerId) {
        debug!("Sending InitStarted message");
        let time = self.clock.time();
        self.send_control_message(peer, &ControlMessage::InitStarted(time));
    }
    pub fn end_sync(&mut self, peer: PeerId) {
        debug!("Sending InitCompleted message");
        self.send_control_message(peer, &ControlMessage::InitCompleted);
    }
    fn send_control_message(&mut self, peer: PeerId, msg: &ControlMessage) {
        let serialized = bincode::serialize(msg).expect("Serialization failed");
        self.socket
            .channel_mut(CONTROL_CHANNEL_ID)
            .send(serialized.clone().into_boxed_slice(), peer);
    }
}

#[derive(Event)]
pub struct ConnectVtt;

fn detect_disconnect(
    socket: Res<NetworkContext>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<NetworkingConnection>>,
) {
    if socket.socket.any_channel_closed() {
        info!("VTT channel closed");
        commands.remove_resource::<NetworkContext>();
        next_state.set(NetworkingConnection::Disconnected);
        commands.trigger(ShowTransientUserMessage {
            text: String::from("VTT Disconnected"),
            special: None,
            keep_alive: None,
        })
    }
}

pub fn on_connect(
    _: On<ConnectVtt>,
    mut commands: Commands,
    user_settings: Res<UserSettings>,
    state: Res<State<NetworkingConnection>>,
    mut next_state: ResMut<NextState<NetworkingConnection>>,
    socket: Option<ResMut<NetworkContext>>,
) {
    if let Some(sandbox_uid) = &user_settings.sandbox {
        if *state == NetworkingConnection::Disconnected {
            let socket = bevy_matchbox::matchbox_socket::WebRtcSocketBuilder::new(&format!(
                "{}/{}",
                user_settings.signaling, sandbox_uid
            ))
            .add_reliable_channel() // CONTROL_CHANNEL_ID
            .add_reliable_channel() // TOKENS_CHANNEL_ID
            .add_reliable_channel() // HEX_CHANNEL_ID
            .add_reliable_channel() // DICE_CHANNEL_ID
            .add_reliable_channel() // VFX_CHANNEL_ID
            .add_reliable_channel() // CAMERA_CHANNEL_ID
            .add_reliable_channel() // DRAWING_CHANNEL_ID
            .build();
            commands.insert_resource(NetworkContext {
                socket: MatchboxSocket::from(socket),
                clock: lamport::Clock::new(),
            });
            next_state.set(NetworkingConnection::Connected);
            commands.trigger(ShowTransientUserMessage {
                text: "VTT session is ready for sandbox: ".to_string(),
                special: Some(sandbox_uid.to_string()),
                keep_alive: Some(Duration::from_secs(30)),
            })
        } else {
            if let Some(mut socket) = socket {
                socket.socket.close();
            }
        }
    }
}

pub fn on_click_vtt() -> impl Fn(On<Pointer<Click>>, Commands) {
    move |_, mut commands| {
        commands.trigger(ConnectVtt);
    }
}

pub fn send_message<T>(
    socket: &mut ResMut<NetworkContext>,
    channel: usize,
    peer: PeerId,
    msg: T,
) where
    T: Serialize + std::marker::Send + 'static,
{
    let msg = ClockedMessage::<T> {
        body: msg,
        time: None,
    };
    let serialized = bincode::serialize(&msg).expect("Serialization failed");
    socket
        .socket
        .channel_mut(channel)
        .send(serialized.clone().into_boxed_slice(), peer);
}

pub fn broadcast_message<T>(socket: &mut ResMut<NetworkContext>, channel: usize, msg: T)
where
    T: Serialize + std::marker::Send + 'static,
{
    let msg = ClockedMessage::<T> {
        body: msg,
        time: Some(socket.clock.increment()),
    };
    let serialized = bincode::serialize(&msg).expect("Serialization failed");
    let peers: Vec<_> = socket.socket.connected_peers().collect();
    for peer in peers {
        debug!("broadcasting message {} to peer {}", type_name::<T>(), peer);
        socket
            .socket
            .channel_mut(channel)
            .send(serialized.clone().into_boxed_slice(), peer);
    }
}

pub fn broadcast_ephemeral_message<T>(
    socket: &mut ResMut<NetworkContext>,
    channel: usize,
    msg: T,
) where
    T: Serialize + std::marker::Send + 'static,
{
    let serialized = bincode::serialize(&msg).expect("Serialization failed");
    let peers: Vec<_> = socket.socket.connected_peers().collect();
    for peer in peers {
        socket
            .socket
            .channel_mut(channel)
            .send(serialized.clone().into_boxed_slice(), peer);
    }
}

pub fn receive_channel_messages<T>(
    channel: usize,
    callback: SystemId<In<T>>,
) -> impl Fn(Commands<'_, '_>, Option<ResMut<'_, NetworkContext>>, Res<'_, State<NetworkingState>>)
where
    T: for<'a> Deserialize<'a> + std::marker::Send + 'static,
{
    move |mut commands: Commands,
          mut socket: Option<ResMut<NetworkContext>>,
          current_state: Res<State<NetworkingState>>| {
        let socket = if socket.is_some() {
            socket.as_mut().unwrap()
        } else {
            return;
        };
        for (id, message) in socket.socket.channel_mut(channel).receive() {
            if *current_state == NetworkingState::IgnoringStaleInitData {
                debug!("Ignoring stale init data");
                continue;
            }
            if let NetworkingState::InitializingFromNode(peer_id) = current_state.get() {
                if *peer_id != id {
                    continue;
                }
            }
            let deserialized: ClockedMessage<T> =
                bincode::deserialize(&message).expect("Serialization failed");
            if let Some(time) = deserialized.time {
                socket.clock.witness(time);
            }
            let deserialized = deserialized.body;
            commands.run_system_with(callback, deserialized);
        }
    }
}

pub fn receive_ephemeral_channel_messages<T>(
    channel: usize,
    callback: SystemId<In<T>>,
) -> impl Fn(Commands<'_, '_>, Option<ResMut<'_, NetworkContext>>)
where
    T: for<'a> Deserialize<'a> + std::marker::Send + 'static,
{
    move |mut commands: Commands, mut socket: Option<ResMut<NetworkContext>>| {
        let socket = if socket.is_some() {
            socket.as_mut().unwrap()
        } else {
            return;
        };
        for (_id, message) in socket.socket.channel_mut(channel).receive() {
            let deserialized = bincode::deserialize(&message).expect("Serialization failed");
            commands.run_system_with(callback, deserialized);
        }
    }
}

pub fn receive_control_messages(
    clear_state_callback: SystemId,
) -> impl Fn(
    Commands<'_, '_>,
    ResMut<'_, NetworkContext>,
    ResMut<'_, NextState<NetworkingState>>,
    Res<'_, State<NetworkingState>>,
    ResMut<'_, VttStateApiController>,
) {
    move |mut commands: Commands,
          mut socket: ResMut<NetworkContext>,
          mut state: ResMut<NextState<NetworkingState>>,
          current_state: Res<State<NetworkingState>>,
          mut controller: ResMut<VttStateApiController>| {
        let mut interim_state = current_state.clone();
        let mut state_changed = false;
        for (id, message) in socket.socket.channel_mut(CONTROL_CHANNEL_ID).receive() {
            let deserialized: ControlMessage =
                bincode::deserialize(&message).expect("Serialization failed");
            match deserialized {
                ControlMessage::InitStarted(time) => {
                    if time < socket.clock.time()
                        || *current_state == NetworkingState::Initialized
                    {
                        interim_state = NetworkingState::IgnoringStaleInitData;
                        state_changed = true;
                        debug!("Ignoring stale data");
                    } else {
                        interim_state = NetworkingState::InitializingFromNode(id);
                        state_changed = true;
                        // TODO: we're about to be initilzed from a more up-to-date node.
                        // delete our state and accept messages from this node only
                        // until initilization completed.
                        commands.run_system(clear_state_callback);
                        // NOTE: if we are being initialized from another node,
                        // it means our remote state controller should not be active.
                        *controller = VttStateApiController::Inhibited;
                    }
                }
                ControlMessage::InitCompleted => {
                    if let NetworkingState::InitializingFromNode(peer_id) = interim_state {
                        if peer_id != id {
                            continue;
                        } else {
                            debug!("Initialization from node completed");
                            interim_state = NetworkingState::Initialized;
                            state_changed = true;
                        }
                    } else if interim_state == NetworkingState::IgnoringStaleInitData {
                        interim_state = NetworkingState::Initialized;
                        state_changed = true;
                    }
                }
            }
        }
        if state_changed {
            state.set(interim_state);
        }
    }
}

pub fn broadcast_state_to_new_peers(
    mut commands: Commands,
    mut socket: Option<ResMut<NetworkContext>>,
    vtt_data: Res<VttData>,
) {
    let socket = if socket.is_some() {
        socket.as_mut().unwrap()
    } else {
        return;
    };
    // NOTE(vtt-sync): the following is_player() exception is tightly coupled with
    // the fact that player nodes are not loading vtt state (LoadVttState)
    // and so we do not want player nodes to accidently initalize a later
    // joining referee node. In such case, a referee node might have its loaded
    // state overwritten with an empty state from the clear player node.
    // Search for the other 'vtt-sync' note in the ui code to read more.
    if vtt_data.is_player() {
        if let Ok(peers) = socket.socket.try_update_peers() {
            for (peer, state) in peers {
                debug!("Player is state: {state:?} to peer: {peer}");
            }
        }
        return;
    }
    if let Ok(peers) = socket.socket.try_update_peers() {
        for (peer, state) in peers {
            debug!("{peer}: {state:?}");
            if state == PeerState::Connected {
                commands.trigger(SendFullStateToPeer { peer });
            }
        }
    }
}

pub fn detect_first_node(
    mut next_state: ResMut<NextState<NetworkingState>>,
    current_state: Res<State<NetworkingState>>,
    mut counter: Local<i32>,
) {
    *counter += 1;
    if *current_state == NetworkingState::Started && *counter > 60 * 10 {
        debug!("Setting state to initialized");
        next_state.set(NetworkingState::Initialized);
    }
}
