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
use crate::instance::*;
use crate::repository::*;
use crate::semantics::Class;
use anyhow::{Ok, Result};

/// Creates a new entity frame and subscribes it to the classes it collects.
///
/// # Arguments
///
/// * `tx` - A mutable read/write transaction to load and save frames.
/// * `parent_uid` - The uid of the parent entity.
/// * `uid` - The uid of the entity for which to create a frame.
/// * `class` - The class name of the entity for which to create a frame.
///
/// # Returns
///
/// * `Result<()>` - Indicates success or failure of the frame creation process.
pub fn create_entity_frame(
    tx: &mut ReadWriteTransaction,
    parent_uid: &str,
    uid: &str,
    class: &Class,
) -> Result<()> {
    let mut frame = Frame::init(tx, uid, parent_uid)?;
    for spec in class.collects.iter() {
        subscribe(&mut frame, &spec.class_name);
    }
    let frame_uid = frame.uid;
    tx.save(&frame_uid)
}

/// Removes an entity frame from the repository transaction, effectively undoing the
/// operation performed by `create_entity_frame`.
///
/// # Arguments
///
/// * `tx` - A mutable read/write transaction to remove the frame.
/// * `uid` - The uid of the entity frame to be removed.
///
/// # Returns
///
/// A `Result` indicating success or failure of the removal operation.
pub fn remove_entity_frame(
    tx: &mut ReadWriteTransaction,
    uid: &str,
) -> Result<()> {
    let user_frame = tx.load(&format!("{}_frame", uid)).unwrap().as_frame();

    let refs = &user_frame.obj["$refs"].clone();
    for r in refs.as_array().unwrap() {
        let ref_frame_uid = r["$frame"].as_str().unwrap();
        let ref_class_name = r["$class"].as_str().unwrap();
        let ref_frame = tx
            .load(&format!("{}_frame", ref_frame_uid))
            .unwrap()
            .as_frame();
        let demands =
            &mut ref_frame.obj["$collections"]["$demand"][ref_class_name];
        demands
            .as_array_mut()
            .unwrap()
            .retain(|v| v.as_str().unwrap() != uid);
        tx.save(&format!("{}_frame", ref_frame_uid))?;
    }
    tx.remove(&format!("{}_frame", uid))
}

/// Collect an entity into any subscriber in its frames hierarchy.
///
/// This function traverses the frames hierarchy of an entity to collect it
/// into a subscriber that matches the class name of the entity. The function
/// continues this process up the hierarchy until it reaches the root frame.
///
/// Used when rolling an entity.
///
/// # Arguments
///
/// * `instance` - A reference to the `SandboxBuilder` instance, providing
///   necessary context for the sandbox environment.
/// * `tx` - A mutable read/write transaction to load and save frames.
/// * `origin_owner_uid` - Uid of the parent of the entity being collected.
/// * `class_name` - The class name of the entity to be collected.
/// * `uid` - Uid of the entity being collection.
///
/// In the following example, any Wolf rolled from within a Forest
/// will be collected in the Forest entity frame:
/// ```text
/// Wolf {
///
/// }
///
/// Forest {
///     << Wolf
///     wolves @ Wolf
/// }
/// ```
pub fn collect(
    instance: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    tx: &mut ReadWriteTransaction,
    parent_uid: &str,
    uid: &str,
    class_name: &str,
) -> anyhow::Result<()> {
    let mut frame_owner_uid: String = parent_uid.to_string();
    let mut waitinglist = Waitinglist::new();
    struct DerefTask {
        user_uid: String,
        ref_uid: String,
        class_name: String,
    }
    let mut deref_backlog: Vec<DerefTask> = Vec::new();
    while frame_owner_uid != "root" {
        let parent_owner_uid = {
            let mut frame = tx
                .load(&format!("{}_frame", frame_owner_uid))
                .unwrap()
                .as_frame();
            for parent in instance
                .sandbox
                .resolve_class(blueprint, class_name)
                .unwrap()
                .hierarchy
                .iter()
            {
                let unused = &frame.obj["$collections"]["$unused"];
                if unused.as_object().unwrap().contains_key(parent) {
                    if let Some(uid) = waitinglist.stage(&mut frame, parent) {
                        deref_backlog.push(DerefTask {
                            user_uid: uid,
                            ref_uid: frame_owner_uid.clone(),
                            class_name: parent.clone(),
                        })
                    }
                    let unused = &mut frame.obj["$collections"]["$unused"];
                    unused[parent]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::to_value(uid).unwrap());
                    break;
                }
            }
            frame.obj["$parent"].clone()
        };
        tx.save(&format!("{}_frame", frame_owner_uid))?;
        frame_owner_uid = parent_owner_uid.as_str().unwrap().to_string();
    }
    waitinglist.apply(tx)?;

    for task in deref_backlog {
        let user_frame = tx
            .load(&format!("{}_frame", task.user_uid))
            .unwrap()
            .as_frame();

        let refs = &mut user_frame.obj["$refs"];
        refs.as_array_mut().unwrap().retain(|v| {
            let ref_frame_uid = v["$frame"].as_str().unwrap();
            let ref_class_name = v["$class"].as_str().unwrap();
            ref_frame_uid != task.ref_uid || ref_class_name != task.class_name
        });
        tx.save(&format!("{}_frame", task.user_uid))?;
    }
    Ok(())
}

/// Remove an entity from any collections in its frames hierarchy.
///
/// This function traverses the frames hierarchy of an entity to remove
/// it from any collections that match the class name hierarchy of the entity.
/// The function continues this process up the hierarchy until it reaches the
/// root frame. This is the opposite operation of `collect`, providing a means
/// to 'withdraw' or remove an entity from its frame context.
///
/// Used when unrolling an entity.
///
/// # Arguments
///
/// * `instance` - A reference to the `SandboxBuilder` instance, providing
///   necessary overview of the sandbox environment.
/// * `tx` - A mutable read/write transaction to load and save frames.
/// * `origin_owner_uid` - Uid of the parent of the entity being withdrawn,
///   previously specified during collection.
/// * `class_name` - The class name of the entity to be removed from
///   collections.
pub fn withdraw(
    instance: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    tx: &mut ReadWriteTransaction,
    origin_owner_uid: &str,
    class_name: &str,
) -> anyhow::Result<()> {
    let mut frame_owner_uid: String = origin_owner_uid.to_string();
    while frame_owner_uid != "root" {
        let parent_owner_uid = {
            let frame = tx
                .load(&format!("{}_frame", frame_owner_uid))
                .unwrap()
                .as_frame();
            for parent in instance
                .sandbox
                .resolve_class(blueprint, class_name)
                .unwrap()
                .hierarchy
                .iter()
            {
                let unused = &mut frame.obj["$collections"]["$unused"];
                if unused.as_object().unwrap().contains_key(parent) {
                    unused[parent]
                        .as_array_mut()
                        .unwrap()
                        .retain(|v| v != origin_owner_uid);
                }
            }
            for parent in instance
                .sandbox
                .resolve_class(blueprint, class_name)
                .unwrap()
                .hierarchy
                .iter()
            {
                let used = &mut frame.obj["$collections"]["$unused"];
                if used.as_object().unwrap().contains_key(parent) {
                    used[parent]
                        .as_array_mut()
                        .unwrap()
                        .retain(|v| v != origin_owner_uid);
                }
            }
            frame.obj["$parent"].clone()
        };
        tx.save(&format!("{}_frame", frame_owner_uid))?;
        frame_owner_uid = parent_owner_uid.as_str().unwrap().to_string();
    }
    Ok(())
}

/// Attempts to select a random unused entity of the specified class from the frame hierarchy
/// associated with the given owner. If available, the selected entity is marked as used and
/// returned. The search traverses up the hierarchy until an entity is found or the root is reached.
/// If no entity is found, `None` is returned.
/// The selected entity will not be available for other use requests
/// until it will get recycled (through a `recycle` call).
///
/// # Arguments
///
/// * `instance` - Provides access to the sandbox environment, including randomization.
/// * `tx` - A mutable read/write transaction to load and save frames.
/// * `origin_owner_uid` - The UID of the initial frame owner where the search begins.
/// * `class_name` - The class name of the entity to be selected.
///
/// # Returns
///
/// `Ok(Some(String))` containing the UID of the selected entity if one is found,
/// `Ok(None)` if no entity is available, or an error if the transaction fails.
pub fn use_collected(
    instance: &SandboxBuilder,
    tx: &mut ReadWriteTransaction,
    origin_owner_uid: &str,
    class_name: &str,
) -> Result<Option<String>> {
    let mut ret: Option<String> = None;

    let mut frame_owner_uid: String = origin_owner_uid.to_string();
    while frame_owner_uid != "root" && ret.is_none() {
        let parent_owner_uid = {
            let mut frame = tx
                .load(&format!("{}_frame", frame_owner_uid))
                .unwrap()
                .as_frame();
            let unused = &mut frame.obj["$collections"]["$unused"];
            if unused.as_object().unwrap().contains_key(class_name) {
                let unused_list = unused[class_name].as_array().unwrap();
                if unused_list.is_empty() {
                    if add_pending_entity(
                        &mut frame,
                        class_name,
                        origin_owner_uid,
                    )? {
                        tx.save(&format!("{}_frame", frame_owner_uid))?;
                        let frame = tx
                            .load(&format!("{}_frame", origin_owner_uid))
                            .unwrap()
                            .as_frame();
                        frame.obj["$refs"].as_array_mut().unwrap().push(
                            serde_json::json!({
                                "$frame": frame_owner_uid,
                                "$class": class_name,
                            }),
                        );
                        tx.save(&format!("{}_frame", origin_owner_uid))?;
                    }
                    return Ok(None);
                }
                let selected = instance
                    .randomizer
                    .in_range(0, unused_list.len() as i32 - 1);

                let selected_uid = unused_list[selected as usize].clone();
                unused[class_name]
                    .as_array_mut()
                    .unwrap()
                    .remove(selected as usize);
                frame.obj["$collections"]["$used"][class_name]
                    .as_array_mut()
                    .unwrap()
                    .push(selected_uid.clone());
                ret = Some(selected_uid.as_str().unwrap().to_string());
            }
            frame.obj["$parent"].clone()
        };
        tx.save(&format!("{}_frame", frame_owner_uid))?;
        frame_owner_uid = parent_owner_uid.as_str().unwrap().to_string();
    }
    Ok(ret)
}

/// Recycle a used entity and make it available again.
/// This is the inverse operation to `use_collected`.
///
/// # Arguments
///
/// * `tx` - A mutable read/write transaction to load and save frames.
/// * `origin_owner_uid` - The UID of the initial frame owner starting the recycle process.
/// * `uid_to_recycle` - The UID of the entity to be recycled.
/// * `class_name` - The class name of the entity to be recycled.
///
/// # Returns
///
/// A `Result` indicating success or failure of the operation.
pub fn recycle(
    tx: &mut ReadWriteTransaction,
    origin_owner_uid: &str,
    uid_to_recycle: &str,
    class_name: &str,
) -> Result<()> {
    let mut frame_owner_uid: String = origin_owner_uid.to_string();
    while frame_owner_uid != "root" {
        let parent_owner_uid = {
            let frame = tx
                .load(&format!("{}_frame", frame_owner_uid))
                .unwrap()
                .as_frame();
            let unused = &mut frame.obj["$collections"]["$unused"];
            if unused.as_object().unwrap().contains_key(class_name) {
                unused[class_name]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::Value::from(uid_to_recycle));
                frame.obj["$collections"]["$used"][class_name]
                    .as_array_mut()
                    .unwrap()
                    .retain(|uid| uid != uid_to_recycle);
            }
            frame.obj["$parent"].clone()
        };
        tx.save(&format!("{}_frame", frame_owner_uid))?;
        frame_owner_uid = parent_owner_uid.as_str().unwrap().to_string();
    }
    Ok(())
}

/// Attempts to select a random entity of the specified class from the frame hierarchy
/// associated with the given owner.
/// Picking a collected entity is different from **using** a collected entity in that
/// the picked entity can be selected again by other callers.
///
/// # Arguments
///
/// * `instance` - Provides access to the sandbox environment, including randomization.
/// * `tx` - A mutable read/write transaction to load and save frames.
/// * `origin_owner_uid` - The UID of the initial frame owner where the search begins.
/// * `class_name` - The class name of the entity to be selected.
///
/// # Returns
///
/// `Ok(Some(String))` containing the UID of the selected entity if one is found,
/// `Ok(None)` if no entity is available, or an error if the transaction fails.
pub fn pick_collected(
    instance: &SandboxBuilder,
    tx: &mut ReadWriteTransaction,
    origin_owner_uid: &str,
    class_name: &str,
) -> Result<Option<String>> {
    let mut ret: Option<String> = None;

    let mut frame_owner_uid: String = origin_owner_uid.to_string();
    while frame_owner_uid != "root" {
        let parent_owner_uid = {
            let mut frame = tx
                .load(&format!("{}_frame", frame_owner_uid))
                .unwrap()
                .as_frame();
            let unused = &mut frame.obj["$collections"]["$unused"];
            if unused.as_object().unwrap().contains_key(class_name) {
                let unused_list = unused[class_name].as_array().unwrap();
                if unused_list.is_empty() {
                    if add_pending_entity(
                        &mut frame,
                        class_name,
                        origin_owner_uid,
                    )? {
                        tx.save(&format!("{}_frame", frame_owner_uid))?;
                        let frame = tx
                            .load(&format!("{}_frame", origin_owner_uid))
                            .unwrap()
                            .as_frame();
                        frame.obj["$refs"].as_array_mut().unwrap().push(
                            serde_json::json!({
                                "$frame": frame_owner_uid,
                                "$class": class_name,
                            }),
                        );
                        tx.save(&format!("{}_frame", origin_owner_uid))?;
                    }
                    return Ok(None);
                }
                let selected = instance
                    .randomizer
                    .in_range(0, unused_list.len() as i32 - 1);

                let selected_uid = unused_list[selected as usize].clone();
                ret = Some(selected_uid.as_str().unwrap().to_string());
                break;
            }
            frame.obj["$parent"].clone()
        };
        tx.save(&format!("{}_frame", frame_owner_uid))?;
        frame_owner_uid = parent_owner_uid.as_str().unwrap().to_string();
    }
    Ok(ret)
}

/// Mark an entity demand to a class. This is used when the picking could not
/// satisfy the minimum number of entities required in an array.
///
/// # Arguments
///
/// * `tx` - A mutable read/write transaction to load and save frames.
/// * `origin_owner_uid` - The UID of the initial frame owner where the search begins.
/// * `class_name` - The class name of the entity to be selected.
///
/// # Returns
///
/// `Ok(true)` demand was succesfully registered
/// `Ok(false)` demand was not registered (likely no matching class was found)
pub fn add_demand(
    tx: &mut ReadWriteTransaction,
    origin_owner_uid: &str,
    class_name: &str,
) -> Result<bool> {
    let mut frame_owner_uid: String = origin_owner_uid.to_string();
    while frame_owner_uid != "root" {
        let parent_owner_uid = {
            let mut frame = tx
                .load(&format!("{}_frame", frame_owner_uid))
                .unwrap()
                .as_frame();
            let unused = &mut frame.obj["$collections"]["$unused"];
            if unused.as_object().unwrap().contains_key(class_name) {
                if add_pending_entity(&mut frame, class_name, origin_owner_uid)?
                {
                    tx.save(&format!("{}_frame", frame_owner_uid))?;
                    let frame = tx
                        .load(&format!("{}_frame", origin_owner_uid))
                        .unwrap()
                        .as_frame();
                    frame.obj["$refs"].as_array_mut().unwrap().push(
                        serde_json::json!({
                            "$frame": frame_owner_uid,
                            "$class": class_name,
                        }),
                    );
                    tx.save(&format!("{}_frame", origin_owner_uid))?;
                    return Ok(true);
                }
            }
            frame.obj["$parent"].clone()
        };
        frame_owner_uid = parent_owner_uid.as_str().unwrap().to_string();
    }
    Ok(false)
}

/// Every entity has a Frame record stored in the format of:
/// {}_frame.
///
/// The entity frame is designed to store data only required
/// during modifications (rolling, unrolling, appending etc.)
///
/// This is done for efficiency consdirations.
/// Frame data is almost never used during rendering, with
/// some very rare exceptions.
pub struct Frame<'a> {
    pub uid: String,
    pub obj: &'a mut serde_json::Value,
}

impl<'a> Frame<'a> {
    pub fn from_value(v: &'a mut serde_json::Value) -> Self {
        Frame {
            uid: v["uid"].as_str().unwrap().to_string(),
            obj: v,
        }
    }

    pub fn init(
        tx: &'a mut ReadWriteTransaction,
        uid2: &'a str,
        parent_uid: &'a str,
    ) -> Result<Self> {
        let frame_uid = format!("{}_frame", uid2);
        let frame = tx.create(&frame_uid)?.as_frame();
        frame.obj["$parent"] = serde_json::Value::from(parent_uid);
        frame.obj["$refs"] = serde_json::json!([]);
        let collections = &mut frame.obj["$collections"];
        collections["$unused"] = serde_json::json!({});
        collections["$used"] = serde_json::json!({});
        collections["$demand"] = serde_json::json!({});
        Ok(frame)
    }
}

pub trait FrameConvertor<'a> {
    fn as_frame(&'a mut self) -> Frame<'a>;
}

impl<'b> FrameConvertor<'b> for serde_json::Value {
    fn as_frame(&'b mut self) -> Frame<'b> {
        Frame::from_value(self)
    }
}

/// Subscribe the frame to a class
fn subscribe(frame: &mut Frame, class_name: &str) {
    let collections = &mut frame.obj["$collections"];
    collections["$unused"][class_name] = serde_json::json!([]);
    collections["$used"][class_name] = serde_json::json!([]);
    collections["$demand"][class_name] = serde_json::json!([]);
}

///
#[derive(Default)]
struct Waitinglist {
    appends: Vec<serde_json::Value>,
}

impl Waitinglist {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn stage(&mut self, frame: &mut Frame, parent: &str) -> Option<String> {
        let demand = &frame.obj["$collections"]["$demand"];
        if demand[parent].as_array().unwrap().len() > 0 {
            let demand = &mut frame.obj["$collections"]["$demand"];
            let to_list = demand[parent].as_array_mut().unwrap().pop().unwrap();
            self.appends.push(to_list.clone());
            Some(to_list.as_str().unwrap().to_string())
        } else {
            None
        }
    }
    pub fn apply(&mut self, tx: &mut ReadWriteTransaction) -> Result<()> {
        if !self.appends.is_empty() {
            let rerolls = tx.load("rerolls")?;
            rerolls["entities"]
                .as_array_mut()
                .unwrap()
                .append(&mut self.appends);
            tx.save("rerolls")?;
        }
        Ok(())
    }
}

fn add_pending_entity(
    frame: &mut Frame,
    class_name: &str,
    entity_uid: &str,
) -> Result<bool> {
    let demand = frame.obj["$collections"]["$demand"][class_name]
        .as_array_mut()
        .unwrap();

    let already_present = demand
        .iter()
        .any(|v| v.as_str().is_some_and(|s| s == entity_uid));

    if !already_present {
        demand.push(entity_uid.into());
        Ok(true)
    } else {
        Ok(false)
    }
}
