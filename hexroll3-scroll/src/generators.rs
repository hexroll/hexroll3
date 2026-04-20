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
use anyhow::{Result, anyhow};
use serde_json::Value;

use crate::{frame::*, instance::*, repository::*, semantics::*};

/// Rolls an entity within the given parent, creating a new entity with a unique identifier,
/// Roll an entity.
///
/// Invoking this method is rarely needed and it is usually called once to generate the sandbox
/// root.
///
/// ```rust,ignore
/// roll(&builder, tx, "main", "root", None);
/// ```
///
/// Adding entities to an existing repository is usually done through the `append` function.
///
/// # Arguments
///
/// * `builder` - A reference to the sandbox builder holding the sandbox instance
/// * `tx` - a read/write transaction.
/// * `class_name` - The class name of the entity to roll.
/// * `parent_uid` - The parent uid of the entity to roll.
/// * `injectors` - Optional injectors that add attributes or override attributes in the entity.
///
/// # Returns
///
/// A `Result` containing the unique identifier of the newly created entity, or an error if the process fails.
pub fn roll(
    builder: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    tx: &mut ReadWriteTransaction,
    class_name: &str,
    parent_uid: &str,
    injectors: Option<&Injectors>,
) -> Result<String> {
    let (map_data, class) = {
        let map_data =
            (blueprint.map_data_provider)(builder, blueprint, tx, class_name)?;

        let class = {
            if let Some((class_name, _)) = map_data.as_ref() {
                blueprint.classes.get(class_name).unwrap().clone()
            } else {
                resolve_actual_class_to_roll(builder, blueprint, class_name)?
            }
        };
        (map_data, class)
    };

    let uid = builder.randomizer.uid();
    {
        // Create the entity frame and subscribe to potential child entities
        create_entity_frame(tx, parent_uid, &uid, &class)?;

        // Create and initialize the entity
        let entity = tx.create(&uid)?;
        entity["uid"] = serde_json::Value::from(uid.as_str());
        // TODO: remove legacy `uuid` references
        entity["uuid"] = serde_json::Value::from(uid.as_str());
        entity["parent_uid"] = serde_json::Value::from(parent_uid);

        entity["class"] = serde_json::Value::from(class.name.as_str());
        if let Some((_, map_json)) = map_data {
            entity["map_data"] = Value::String(map_json.to_string());
        }

        // Run the entity commands for injectors and attributes
        if let Some(prependers) = injectors {
            for injector in prependers.prependers.as_slice() {
                injector.inject(builder, tx, &uid, parent_uid)?;
            }
        }

        for (_, attr) in class.attrs.as_slice() {
            attr.cmd.apply(
                &mut Context::Rolling,
                builder,
                blueprint,
                tx,
                &uid,
            )?;
        }

        if let Some(appenders) = injectors {
            for injector in appenders.appenders.as_slice() {
                injector.inject(builder, tx, &uid, parent_uid)?;
            }
        }

        // Save and return the uid
        tx.save(&uid)?;
    }

    collect(builder, blueprint, tx, &uid, uid.as_str(), &class.name)?;

    Ok(uid)
}

/// Unroll an entity while maintaining the entity graph's integrity.
///
/// This function reverses the effects of a previously executed 'roll' operation
/// on an entity within a repository transaction. It does so by:
/// - Removing entity references from all associated frames.
/// - Undoing any injector and attribute modifications that were applied to the entity.
/// - Clearing the entity's parent reference.
/// - Deleting the entity and its associated frame from the transaction.
/// - Re-applying entity modification commands for all users of the entity.
///
/// # Arguments
/// * `builder` - A reference to the sandbox builder holding the sandbox instance
/// * `tx` - A read/write transaction.
/// * `uid` - The uid of the entity to unroll.
/// * `injectors` - Optional injectors to use for ejecting attributes.
///
/// # Returns
/// This function returns a `Result` indicating success or failure of the unroll operation.
///
pub fn unroll(
    builder: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    tx: &mut ReadWriteTransaction,
    uid: &str,
    injectors: Option<&Injectors>,
) -> Result<String> {
    let Ok(entity) = tx.load(uid) else {
        return Ok(String::new());
    };
    let parent_spec = entity["$parent"].clone();
    let class_name = entity["class"].as_str().unwrap().to_string();
    let users = entity["$users"]
        .as_array()
        .cloned()
        .unwrap_or_else(Vec::new);

    // Remove entity references from all frames
    withdraw(builder, blueprint, tx, uid, &class_name)?;

    // Undo injectors and attributes
    let class = builder
        .sandbox
        .resolve_class(blueprint, &class_name)
        .unwrap();
    if let Some(injs) = injectors {
        for injector in injs.appenders.as_slice() {
            injector.eject(builder, tx, uid, "")?;
        }
    }
    for (_, attr) in class.attrs.as_slice() {
        attr.cmd.revert(
            &mut Context::Unrolling,
            builder,
            blueprint,
            tx,
            uid,
        )?;
    }
    if let Some(boots) = injectors {
        for injector in boots.prependers.as_slice() {
            injector.eject(builder, tx, uid, "")?;
        }
    }

    // Clear the $parent reference
    let parent_uid = if !parent_spec.is_null() {
        let parent_uid = parent_spec["uid"].as_str().unwrap();
        let parent_attr = parent_spec["attr"].as_str().unwrap();
        let parent = tx.load(parent_uid)?;
        parent[parent_attr]
            .as_array_mut()
            .unwrap()
            .retain(|v| v != uid);
        tx.save(parent_uid)?;
        parent_uid
    } else {
        "root"
    };

    // Actual deletion of the entity and its frame
    remove_entity_frame(tx, uid)?;
    tx.remove(uid)?;

    // Re-apply entity $users commands
    // Each user_spec in $user is assumed to have two entries:
    // `user_spec["uid"]` holding the uid of the user using this entity
    // `user_spec["attr"]` holder the attribute name storing the unrolled entity uid
    for user_spec in users {
        let user_uid = user_spec["uid"].as_str().unwrap();
        let user_attr = user_spec["attr"].as_str().unwrap();
        if let Ok(user) = tx.load(user_uid) {
            let user_class = builder
                .sandbox
                .resolve_class(blueprint, user["class"].as_str().unwrap())
                .unwrap();
            match user[user_attr] {
                serde_json::Value::Object(_) => {
                    user_class.attrs[user_attr].cmd.revert(
                        &mut Context::Restoring,
                        builder,
                        blueprint,
                        tx,
                        user_uid,
                    )?;
                }
                serde_json::Value::Array(_) => {
                    user[user_attr]
                        .as_array_mut()
                        .unwrap()
                        .retain(|v| v != uid);
                }
                _ => {}
            }
            let mut ctx = Context::Appending(AppendPayload {
                class_override: Some(&class_name),
                appended_uid: None,
            });
            user_class.attrs[user_attr]
                .cmd
                .apply(&mut ctx, builder, blueprint, tx, user_uid)?;
            tx.save(user_uid)?;
        }
    }

    Ok(parent_uid.to_string())
}

/// Reroll an existing entity, with an optional class override, returning the
/// new entity's new uid.
///
/// Rerolling is done via the parent entity's attribute holding the entity
/// to reroll. The attribute command is applied to generate a new entity
/// and only after, the old entity is unrolled.
/// Even if the newly rolled entity uses child entities of the old entity,
/// they will be replaced once the old entity is unrolled.
///
/// # Arguments
///
/// * `builder` - A reference to the sandbox builder holding the sandbox instance
/// * `tx` - A read/write transaction.
/// * `uid` - The uid of the entity to reroll.
/// * `class_override` - An optional string slice that can specify a new class
///   for the entity.
///
/// # Returns
///
/// * `Result<String>` - On success, returns the new uid of the rerolled
///   entity. On failure, returns an error detailing what went wrong.
///
/// # Errors
///
/// This function may return an error if the reroll process encounters issues
/// such as database transaction failures or if the entity with the specified
/// uid does not exist.
pub fn reroll(
    builder: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    tx: &mut ReadWriteTransaction,
    uid: &str,
    class_override: Option<&str>,
) -> Result<String> {
    let (parent_uid, parent_attr) = {
        let entity = tx.load(uid)?;

        let parent_spec = &entity["$parent"];

        (
            parent_spec["uid"].as_str().unwrap().to_string(),
            parent_spec["attr"].as_str().unwrap().to_string(),
        )
    };

    let parent_entity = tx.load(&parent_uid)?;
    let parent_class_name =
        parent_entity["class"].as_str().unwrap().to_string();
    let parent_class = &builder
        .sandbox
        .resolve_class(blueprint, &parent_class_name)
        .unwrap();

    let mut ctx = Context::Rerolling(RerollPayload {
        class_override,
        existing_uid: uid.to_string(),
        new_uid: None,
    });

    parent_class.attrs[&parent_attr].cmd.apply(
        &mut ctx,
        builder,
        blueprint,
        tx,
        &parent_uid,
    )?;

    unroll(builder, blueprint, tx, uid, None)?;

    if let Context::Rerolling(payload) = ctx {
        if let Some(new_uid) = payload.new_uid {
            return Ok(new_uid);
        }
    }
    Err(anyhow!("Reroll failed due to an unknown reason"))
}

/// Appends an entity to a parent entity's attribute within a transaction.
///
/// # Arguments
/// * `instance` - Reference to the current `Instance` with cached classes.
/// * `tx` - Mutable reference to a `RepoTransaction` used for loading and saving entities.
/// * `parent_uid` - String slice that specifies the unique identifier of the parent entity.
/// * `attr_name` - String slice that specifies the attribute name within the parent entity.
/// * `class_override` - Optional string slice for class override.
/// * `count` - Number of entities to generate
///
/// # Returns
/// * `Result<Vec<String>>` - On success, returns a list of appended entities uids.
///
/// # Errors
/// * Returns an error if the parent entity or its class/attribute is not found.
/// * Returns an error if appending the entity fails, or if the UID was not set.
pub fn append(
    builder: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    tx: &mut ReadWriteTransaction,
    parent_uid: &str,
    attr_name: &str,
    class_override: Option<&str>,
    mut count: u32,
) -> Result<Vec<String>> {
    let parent = tx.load(parent_uid)?;
    let parent_class = parent["class"].as_str().ok_or_else(|| {
        anyhow::anyhow!("class key not found or not a string")
    })?;
    let class = builder
        .sandbox
        .resolve_class(blueprint, parent_class)
        .ok_or_else(|| anyhow::anyhow!("class not found in instance"))?;
    let attr = class
        .attrs
        .get(attr_name)
        .ok_or_else(|| anyhow::anyhow!("attribute not found in class"))?;
    let mut ctx = Context::Appending(AppendPayload {
        class_override,
        appended_uid: None,
    });
    let mut ret = Vec::new();
    while count > 0 {
        count -= 1;
        attr.cmd
            .apply(&mut ctx, builder, blueprint, tx, parent_uid)?;
        if let Context::Appending(payload) = &ctx {
            if let Some(added_uid) = &payload.appended_uid {
                tx.save(parent_uid)?;
                ret.push(added_uid.to_string());
            } else {
                return Err(anyhow!(
                    "Appending entity to {} in {} failed",
                    attr_name,
                    parent_uid
                ));
            }
        } else {
            return Err(anyhow!(
                "Appending entity to {} in {} failed",
                attr_name,
                parent_uid
            ));
        }
    }
    Ok(ret)
}

/// Resolve a concrete class to roll using the specified class in a scroll.
/// The specified class could be a parent class, a variable pointing to a class List
/// or already a concrete class.
fn resolve_actual_class_to_roll(
    builder: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    class_name: &str,
) -> Result<Class> {
    let mut class_to_resolve = builder
        .sandbox
        .resolve_class(blueprint, class_name)
        .ok_or(anyhow!("class {} not found", class_name))?;

    while class_to_resolve.subclasses != SubclassesSpecifier::Empty() {
        class_to_resolve = match &class_to_resolve.subclasses {
            SubclassesSpecifier::Var(variable_symbol) => {
                let rolled_class_name = {
                    let variable_name = &variable_symbol[1..]; // removing the $ sign
                    let class_list = blueprint.globals[variable_name]
                        .as_array()
                        .ok_or(anyhow!("Unable to find {}", variable_symbol))?;
                    builder
                        .randomizer
                        .choose(class_list)
                        .as_str()
                        .unwrap()
                        .to_string()
                };
                builder
                    .sandbox
                    .resolve_class(blueprint, rolled_class_name.trim())
                    .ok_or(anyhow!("class {} not found", rolled_class_name))?
            }
            SubclassesSpecifier::List(class_list) => {
                let rolled_class_name = builder.randomizer.choose(class_list);
                builder
                    .sandbox
                    .resolve_class(blueprint, rolled_class_name)
                    .ok_or(anyhow!("class {} not found", rolled_class_name))?
            }
            SubclassesSpecifier::Empty() => class_to_resolve,
        };
    }

    Ok(class_to_resolve)
}
