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

// This module provides a framework for managing asynchronous HTTP requests within a Bevy application.
// It is used to drive all client-side requests in hexroll's http.rs code and helps significantly reduce
// boilerplate code.

#![allow(dead_code)]
use std::hash::Hash;

#[derive(Resource)]
pub struct HttpAgent {
    agent: ureq::Agent,
}

impl Default for HttpAgent {
    fn default() -> Self {
        Self {
            agent: ureq::agent(),
        }
    }
}

use bevy::prelude::*;

use bevy::{
    ecs::system::ScheduleSystem,
    platform::collections::HashMap,
    tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future},
};
use ureq::http::StatusCode;

#[derive(Resource)]
pub struct AsyncBackendTasks<K, P>
where
    K: Hash + Eq + PartialEq,
    P: Sync + Send + 'static,
{
    generating_chunks: HashMap<K, Task<Option<P>>>,
}

impl<K, P> Default for AsyncBackendTasks<K, P>
where
    K: Hash + Eq + PartialEq,
    P: Sync + Send + 'static,
{
    fn default() -> Self {
        AsyncBackendTasks {
            generating_chunks: HashMap::new(),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum AsyncHttpResult {
    Okay(String),
    ErrorCode(u16),
    Failure,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum AsyncHttpSpawnFailure {
    TaskKeyExists,
}

impl<K, P> AsyncBackendTasks<K, P>
where
    K: Hash + Eq + PartialEq,
    P: Sync + Send + 'static,
{
    pub fn spawn_standalone<F>(
        &mut self,
        key: K,
        mut callback: F,
    ) -> core::result::Result<(), AsyncHttpSpawnFailure>
    where
        F: FnMut() -> Option<P> + Send + Sync + 'static,
    {
        if self.generating_chunks.contains_key(&key) {
            return Err(AsyncHttpSpawnFailure::TaskKeyExists);
        }
        let task_pool = AsyncComputeTaskPool::get();
        let task = task_pool.spawn(async move { callback() });
        self.generating_chunks.insert(key, task);
        return Ok(());
    }

    pub fn spawn_cached<F>(
        &mut self,
        data: String,
        key: K,
        mut callback: F,
    ) -> core::result::Result<(), AsyncHttpSpawnFailure>
    where
        F: FnMut(String) -> P + Send + Sync + 'static,
    {
        if self.generating_chunks.contains_key(&key) {
            return Err(AsyncHttpSpawnFailure::TaskKeyExists);
        }
        let task_pool = AsyncComputeTaskPool::get();
        let task = task_pool.spawn(async move { Some(callback(data)) });
        self.generating_chunks.insert(key, task);
        return Ok(());
    }

    pub fn spawn_request<F>(
        &mut self,
        http_agent: &mut HttpAgent,
        url: String,
        api_key: Option<String>,
        key: K,
        mut callback: F,
    ) -> core::result::Result<(), AsyncHttpSpawnFailure>
    where
        F: FnMut(String) -> P + Send + Sync + 'static,
    {
        if self.generating_chunks.contains_key(&key) {
            return Err(AsyncHttpSpawnFailure::TaskKeyExists);
        }
        let task_pool = AsyncComputeTaskPool::get();
        let agent = http_agent.agent.clone();
        let task = task_pool.spawn(async move {
            let request = agent
                .get(&url)
                .header("X-API-KEY".to_string(), api_key.unwrap());
            debug!("GET request sent to {}", url);
            let data = if let Ok(mut response) = request.call() {
                match response.status() {
                    StatusCode::OK => {
                        debug!("Received response from {}", url);
                        if let Ok(text) = response.body_mut().read_to_string() {
                            if text.is_empty() {
                                debug!("response text is empty from {}", url);
                            }
                            AsyncHttpResult::Okay(text)
                        } else {
                            debug!("Response text cannot be read from {}", url);
                            AsyncHttpResult::Failure
                        }
                    }
                    _ => AsyncHttpResult::Failure,
                }
            } else {
                AsyncHttpResult::Failure
            };
            match data {
                AsyncHttpResult::Okay(data) => Some(callback(data)),
                AsyncHttpResult::ErrorCode(code) => {
                    error!("Received HTTP error {}", code);
                    None
                }
                AsyncHttpResult::Failure => {
                    error!("HTTP request failed {}", url);
                    None
                }
            }
        });
        self.generating_chunks.insert(key, task);
        return Ok(());
    }

    pub fn spawn_post<F>(
        &mut self,
        http_agent: &mut HttpAgent,
        url: String,
        api_key: Option<String>,
        body: serde_json::Value,
        key: K,
        mut callback: F,
    ) where
        F: FnMut(String) -> P + Send + Sync + 'static,
    {
        if self.generating_chunks.contains_key(&key) {
            return;
        }
        let task_pool = AsyncComputeTaskPool::get();
        let agent = http_agent.agent.clone();
        let task = task_pool.spawn(async move {
            let request = agent
                .post(&url)
                .header("X-API-KEY".to_string(), api_key.unwrap());
            let data = if let Ok(mut response) = request.send_json(body) {
                match response.status() {
                    StatusCode::OK => {
                        if let Ok(text) = response.body_mut().read_to_string() {
                            AsyncHttpResult::Okay(text)
                        } else {
                            AsyncHttpResult::Failure
                        }
                    }
                    _ => AsyncHttpResult::Failure,
                }
            } else {
                AsyncHttpResult::Failure
            };
            match data {
                AsyncHttpResult::Okay(data) => Some(callback(data)),
                AsyncHttpResult::ErrorCode(code) => {
                    error!("Received HTTP error {}", code);
                    None
                }
                AsyncHttpResult::Failure => None,
            }
        });
        self.generating_chunks.insert(key, task);
    }

    pub fn poll_responses<F>(&mut self, mut callback: F)
    where
        F: FnMut(&K, Option<P>),
    {
        self.generating_chunks.retain(|key, task| {
            let status = block_on(future::poll_once(task));
            let retain = status.is_none();
            if let Some(task_completed) = status {
                callback(key, task_completed);
            }
            retain
        });
    }
}

pub trait ApiHandler {
    fn register_api_callback<M, I, R>(
        &mut self,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self
    where
        R: std::marker::Send + std::marker::Sync + 'static,
        I: std::marker::Send + std::marker::Sync + Hash + Eq + 'static;
}

impl ApiHandler for App {
    fn register_api_callback<M, I, R>(
        &mut self,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self
    where
        R: std::marker::Send + std::marker::Sync + 'static,
        I: std::marker::Send + std::marker::Sync + Hash + Eq + 'static,
    {
        self.insert_resource(AsyncBackendTasks::<I, R>::default())
            .add_systems(Update, system);
        self
    }
}
