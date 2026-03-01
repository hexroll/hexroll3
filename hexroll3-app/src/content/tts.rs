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

#![allow(dead_code)]
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use bevy::log::error;

pub struct TtsHandle {
    sender: Sender<String>,
    thread: Option<JoinHandle<()>>,
}

impl TtsHandle {
    pub fn new(cmd: &str) -> Self {
        let (sender, receiver) = mpsc::channel::<String>();
        let cmd = String::from(cmd);
        let thread = thread::spawn(move || {
            for text in receiver.iter() {
                if let Err(e) = run_tts_command(&cmd, &text) {
                    error!("Error running TTS command: {:?}", e);
                }
            }
        });

        Self {
            sender,
            thread: Some(thread),
        }
    }

    pub fn send_text(&self, text: String) -> Result<(), String> {
        self.sender
            .send(text)
            .map_err(|_| "Failed to send text".to_string())
    }

    pub fn abort(self) {
        drop(self.sender);
        if let Some(thread) = self.thread {
            thread.join().unwrap();
        }
    }
}

fn run_tts_command(cmd: &str, text: &str) -> io::Result<()> {
    let mut child = Command::new(cmd).stdin(Stdio::piped()).spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }

    let _ = child.wait()?;
    Ok(())
}
