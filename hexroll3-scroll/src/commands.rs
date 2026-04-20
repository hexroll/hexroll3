use std::collections::HashMap;
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
use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Result, anyhow};

use crate::{
    frame::*,
    generators::*,
    instance::*,
    renderer::{RendererContext, render_entity, render_indirections},
    repository::*,
    semantics::*,
};

/// A trait for creating commands that assign primitive values to
/// an attribute
pub trait ValueAssigner {
    fn new(name: String, value: serde_json::Value) -> Self;
}

/// A trait for creating commands related to entity assignments.
/// Useful for operations such as rolling single entities, handling
/// entity arrays, or choosing from collected entities.
pub trait EntityAssigner {
    /// Constructs a new instance implementing the `EntityAssigner` trait.
    ///
    /// # Parameters
    ///
    /// * `name`: The attribute name to which the entities will be assigned.
    /// * `class_names`: Specifies the classes from which entities will be rolled or selected.
    /// * `min`: Specifies the minimum number of entities to be rolled or selected.
    /// * `max`: Specifies the maximum number of entities to be rolled or selected.
    /// * `injectors`: A set of injectors to enhance entities with additional attributes or
    ///                to override existing attributes.
    fn new(
        name: String,
        class_names: ClassNamesToRoll,
        min: CardinalityValue,
        max: CardinalityValue,
        injectors: Injectors,
    ) -> Self;
}

pub trait RefInjectCommand {
    fn make(
        name: String,
        path: Vec<String>,
    ) -> Arc<dyn InjectCommand + Send + Sync>;
}

#[derive(Clone)]
pub enum CardinalityValue {
    Number(i32),
    Variable(String),
    Undefined,
}

#[derive(Clone)]
pub enum ClassNamesToRoll {
    List(Vec<String>),
    Indirect(String),
    Unset(),
}

/// An attribute command used when assigning simple values to attributes:
///
/// ```text
/// Cat {
///     name = "Garfield"
/// }
/// ```
#[derive(Clone)]
pub struct AttrCommandAssigner {
    pub name: String,
    pub value: serde_json::Value,
}

impl ValueAssigner for AttrCommandAssigner {
    fn new(name: String, value: serde_json::Value) -> Self {
        AttrCommandAssigner { name, value }
    }
}

impl AttrCommand for AttrCommandAssigner {
    fn apply(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        if entity.as_object().unwrap().contains_key(&self.name)
            && self.value.is_boolean()
        {
            log::warn!(
                "Entity class {} has an override with value {} for attribute {} already set to {}",
                entity["class"],
                self.value,
                self.name,
                entity[&self.name]
            );
        }
        entity[&self.name] = self.value.to_owned();
        Ok(())
    }

    fn revert(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity.clear(&self.name);
        Ok(())
    }
    fn value(&self) -> Option<String> {
        Some(self.value.as_str().unwrap().to_string())
    }
}

/// A trait for creating commands that assign global vars values to
/// an attribute
pub trait GlobalVarAssigner {
    fn new(name: String, global_var: String) -> Self;
}

/// An attribute command used when assigning global vars values to attributes:
///
/// ```text
/// Cat {
///     age = $global_cat_age_for_some_reason
/// }
/// ```
#[derive(Clone)]
pub struct AttrCommandVarAssigner {
    pub name: String,
    pub global_var: String,
}

impl GlobalVarAssigner for AttrCommandVarAssigner {
    fn new(name: String, global_var: String) -> Self {
        AttrCommandVarAssigner { name, global_var }
    }
}

impl AttrCommand for AttrCommandVarAssigner {
    fn apply(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;

        entity[&self.name] = blueprint
            .globals
            .get(&self.global_var)
            .ok_or(anyhow::anyhow!(
                "Failed to find global value for {}",
                self.global_var
            ))?
            .clone();

        Ok(())
    }

    fn revert(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity.clear(&self.name);
        Ok(())
    }
}

/// An attribute command for assigning values and templates
/// that are weakly linked with entities.
///
/// These weakly linked values are not stored in the data file,
/// but are instead read from the parsed model at runtime.
///
/// ```text
/// Cat {
///     name ~ "Garfield"
/// }
/// ```
#[derive(Clone)]
pub struct AttrCommandWeakAssigner {
    pub name: String,
    pub value: serde_json::Value,
}

impl AttrCommand for AttrCommandWeakAssigner {
    fn apply(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity[&self.name] = serde_json::Value::Null;
        Ok(())
    }

    fn revert(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity.clear(&self.name);
        Ok(())
    }
    fn value(&self) -> Option<String> {
        Some(self.value.as_str().unwrap().to_string())
    }
}

/// An attribute command for rolling dice values to attributes.
///
/// ```text
/// Dog {
///     age @ 1d6+2
/// }
/// ```
#[derive(Clone)]
pub struct AttrCommandDice {
    pub name: String,
    pub number_of_dice: i32,
    pub dice_type: i32,
    pub dice_modifier: i32,
}

impl AttrCommand for AttrCommandDice {
    fn apply(
        &self,
        _ctx: &mut Context,
        builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let mut total: i32 = 0;
        for _ in 0..self.number_of_dice {
            total += builder.randomizer.in_range(1, self.dice_type);
        }
        total += self.dice_modifier;
        let entity = tx.load(euid)?;
        entity[&self.name] = serde_json::to_value(total).unwrap();
        Ok(())
    }

    fn revert(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity.clear(&self.name);
        Ok(())
    }
}

/// Pre-rendered attribute values command.
///
/// Used to pre-render jinja expressions during generation:
///
/// ```text
/// attribute = `{%if True %}{{"some value}}{% endif %}`
/// ```
#[derive(Clone)]
pub struct AttrCommandPrerenderedAssigner {
    pub name: String,
    pub value: serde_json::Value,
}

impl AttrCommand for AttrCommandPrerenderedAssigner {
    fn apply(
        &self,
        _ctx: &mut Context,
        builder: &SandboxBuilder,
        blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let ro_entity = tx.retrieve(euid)?;
        let rendered = render_entity(
            builder.sandbox,
            blueprint,
            tx,
            &ro_entity.value,
            true,
        )?;
        let prerendered = builder
            .templating_env
            .render_str(self.value.as_str().unwrap(), &rendered)?;
        let entity = tx.load(euid)?;
        entity[&self.name] = serde_json::Value::String(prerendered);
        Ok(())
    }

    fn revert(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity.clear(&self.name);
        Ok(())
    }
    fn value(&self) -> Option<String> {
        Some(self.value.as_str().unwrap().to_string())
    }
}

/// An attribute command for rolling entities into attributes.
///
/// ```text
/// Wolf {
/// }
///
/// Forest {
///     [1..10 wolves] @ Wolf
/// }
///
/// ```
#[derive(Clone)]
pub struct AttrCommandRollEntity {
    pub name: String,
    pub class_names: ClassNamesToRoll,
    pub min: CardinalityValue,
    pub max: CardinalityValue,
    pub injectors: Injectors,
}

impl EntityAssigner for AttrCommandRollEntity {
    fn new(
        name: String,
        class_names: ClassNamesToRoll,
        min: CardinalityValue,
        max: CardinalityValue,
        injectors: Injectors,
    ) -> Self {
        AttrCommandRollEntity {
            name,
            class_names,
            min,
            max,
            injectors,
        }
    }
}

impl AttrCommand for AttrCommandRollEntity {
    fn apply(
        &self,
        ctx: &mut Context,
        builder: &SandboxBuilder,
        blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let (min, max, class_override) = match ctx {
            Context::Appending(payload) => (1, 1, payload.class_override),
            Context::Rerolling(payload) => (1, 1, payload.class_override),
            Context::Rolling => (
                resolve_value(blueprint, &self.min),
                resolve_value(blueprint, &self.max),
                None,
            ),
            _ => {
                return Err(anyhow!(
                    "Invalid context when applying roll: {:#?}",
                    ctx
                ));
            }
        };
        let class_names = if let Some(class_name) = class_override {
            vec![class_name.to_string()]
        } else {
            match &self.class_names {
                ClassNamesToRoll::List(v) => v.clone(),
                ClassNamesToRoll::Indirect(s) => {
                    let entity = tx.load(euid)?;
                    let mut rc = RendererContext {
                        cache: HashMap::new(),
                        env: builder.templating_env.clone(),
                    };
                    let entity_copy = entity.clone();
                    match &entity[s] {
                        serde_json::Value::String(v) => vec![v.to_string()],
                        serde_json::Value::Object(_) => {
                            vec![
                                render_indirections(
                                    &mut rc,
                                    builder.sandbox,
                                    blueprint,
                                    tx,
                                    &entity_copy,
                                    s,
                                )?
                                .as_str()
                                .unwrap()
                                .to_string(),
                            ]
                        }
                        _ => unreachable!(),
                    }
                }
                ClassNamesToRoll::Unset() => unreachable!(),
            }
        };
        let uid = {
            let entity = tx.load(euid)?;
            entity["uid"].as_str().unwrap().to_string()
        };
        let seq = if max == 0 && min == 0 {
            serde_json::json!([])
        } else {
            let n = if max - min > 0 {
                builder.randomizer.in_range(min, max)
            } else {
                min
            };
            let mut ret: serde_json::Value = {
                let entity = tx.load(euid)?;
                match ctx {
                    Context::Appending(_) => entity[&self.name].clone(),
                    Context::Rerolling(_) => entity[&self.name].clone(),
                    Context::Rolling => serde_json::json!([]),
                    _ => {
                        return Err(anyhow!(
                            "Invalid context when applying roll: {:#?}",
                            ctx
                        ));
                    }
                }
            };
            for _ in 0..n {
                let actual_class_name =
                    builder.randomizer.choose::<String>(&class_names);
                let generated_uid = roll(
                    builder,
                    blueprint,
                    tx,
                    actual_class_name,
                    &uid,
                    Some(&self.injectors),
                )?;
                {
                    let entity = tx.load(&generated_uid)?;
                    entity["$parent"] = serde_json::json!({
                        "uid": &uid,
                        "attr": self.name,
                    });
                    tx.save(&generated_uid)?;
                }

                if let Context::Rerolling(payload) = ctx {
                    if let Some(index) =
                        ret.as_array_mut().unwrap().iter().position(|value| {
                            value.as_str().unwrap()
                                == payload.existing_uid.as_str()
                        })
                    {
                        ret.as_array_mut().unwrap()[index] =
                            serde_json::Value::from(generated_uid.clone());
                        payload.new_uid = Some(generated_uid.clone());
                    } else {
                    }
                } else {
                    ret.as_array_mut()
                        .unwrap()
                        .push(serde_json::Value::from(generated_uid.clone()));
                }

                if let Context::Appending(payload) = ctx {
                    payload.appended_uid = Some(generated_uid.clone());
                }
            }
            serde_json::json!(ret)
        };
        {
            let entity = tx.load(euid)?;
            entity[&self.name] = seq;
        }
        Ok(())
    }

    fn revert(
        &self,
        _ctx: &mut Context,
        builder: &SandboxBuilder,
        blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        // Checking if there is an actual array is only needed when dealing
        // with the legacy reference injection. For example:
        //
        // ```text
        // Ruler {
        //      Weapon @ Weapon
        //      Necromancer {
        //          Weapon = &Weapon
        //      }
        // }
        // ```
        //
        // In this case, when unrolling, the first weapon will be cleared,
        // but Necromancer as no way of differentiating between its
        // Weapon attribute and a regular attribute, so it will try
        // to revert it, and will find nothing.
        if let Some(arr) = entity[&self.name].as_array() {
            for euid in arr.clone() {
                unroll(
                    builder,
                    blueprint,
                    tx,
                    euid.as_str().unwrap(),
                    Some(&self.injectors),
                )?;
            }
        }
        // entity.clear(&self.name);
        Ok(())
    }
}

///
/// Roll an attribute value from list.
///
/// ```text
/// Cat {
///     color @ [
///         * Black
///         * Ginger
///         * Gray
///     ]
/// }
/// ```
#[derive(Clone)]
pub struct AttrCommandRollFromList {
    pub name: String,
    pub list: Vec<serde_json::Value>,
}

impl AttrCommand for AttrCommandRollFromList {
    fn apply(
        &self,
        _ctx: &mut Context,
        builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity[&self.name] = builder.randomizer.choose(&self.list).to_owned();
        Ok(())
    }

    fn revert(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity.clear(&self.name);
        Ok(())
    }
}

/// Copy the attribute value of an ancestor.
/// The ancestor attribute is specified using a class name followed
/// by an attribute name:
///
/// ```text
/// attribute = :ClassName.attribute_name
/// ```
#[derive(Clone)]
pub struct AttrCommandContext {
    pub name: String,
    pub context_parent: String,
    pub context_attr: String,
}

impl AttrCommand for AttrCommandContext {
    fn apply(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity[&self.name] = serde_json::json!({
            "type" : "context",
            "spec" : {
            "parent" : self.context_parent,
            "attr" : self.context_attr
        }
        });
        Ok(())
    }

    fn revert(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity.clear(&self.name);
        Ok(())
    }
}

/// Roll an attribute value indirectly using a variable.
///
/// ```text
/// pets = [
///     * dog
///     * cat
///     * fox
/// ]
///
/// attribute @ $pets
///
/// ```
#[derive(Clone)]
pub struct AttrCommandRollFromVariable {
    pub name: String,
    pub var: String,
}

impl AttrCommand for AttrCommandRollFromVariable {
    fn apply(
        &self,
        _ctx: &mut Context,
        builder: &SandboxBuilder,
        blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let value = blueprint.globals[&self.var]
            .as_array()
            .ok_or(anyhow!("Unable to find {}", self.var))?;
        let entity = tx.load(euid)?;
        entity[&self.name] = builder.randomizer.choose(value).to_owned();
        Ok(())
    }

    fn revert(
        &self,
        _ctx: &mut Context,
        _builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity.clear(&self.name);
        Ok(())
    }
}

/// Use a collected entity by class name:
///
/// ```text
/// attribute ? ClassName
/// ```
/// or
///
/// ```text
/// [1..3 attribute] ? ClassName
/// ```
///
/// * The class name can be a concrete or a base class
/// * The used entity will be made unavailable for others until it gets recycled
///
/// Any selected entity can be 'injected' with new attributes or attribute overrides:
///
/// ```text
/// attribute ? ClassName {
///     new_attribute = "some value"
/// }
/// ```
#[derive(Clone)]
pub struct AttrCommandUseEntity {
    pub name: String,
    pub class_names: ClassNamesToRoll,
    pub min: CardinalityValue,
    pub max: CardinalityValue,
    injectors: Injectors,
}

impl EntityAssigner for AttrCommandUseEntity {
    fn new(
        name: String,
        class_names: ClassNamesToRoll,
        min: CardinalityValue,
        max: CardinalityValue,
        injectors: Injectors,
    ) -> Self {
        AttrCommandUseEntity {
            name,
            class_names,
            min,
            max,
            injectors,
        }
    }
}

impl AttrCommand for AttrCommandUseEntity {
    fn apply(
        &self,
        ctx: &mut Context,
        builder: &SandboxBuilder,
        blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        if entity.is_missing(&self.name) {
            entity[&self.name] =
                serde_json::to_value(Vec::new() as Vec<serde_json::Value>)
                    .unwrap();
        }
        let (min, max) = match ctx {
            Context::Appending(_) => (1, 1),
            Context::Rolling => (
                resolve_value(blueprint, &self.min),
                resolve_value(blueprint, &self.max),
            ),
            _ => {
                return Err(anyhow!(
                    "Invalid context when applying use: {:#?}",
                    ctx
                ));
            }
        };
        let class_names = match &self.class_names {
            ClassNamesToRoll::List(l) => l,
            _ => unreachable!(),
        };
        for _ in 0..builder.randomizer.in_range(min, max) {
            let cls = builder.randomizer.choose::<String>(class_names);
            if let Ok(Some(selected_uid)) =
                use_collected(builder, tx, euid, cls)
            {
                for injector in self.injectors.appenders.as_slice() {
                    injector.inject(builder, tx, &selected_uid, euid)?;
                }
                add_user_to_entity(tx, &selected_uid, euid, &self.name)?;
                {
                    let entity = tx.load(euid)?;
                    let list = entity[&self.name].as_array_mut().unwrap();
                    list.push(serde_json::to_value(selected_uid)?);
                }
            }
        }
        //
        Ok(())
    }

    fn revert(
        &self,
        ctx: &mut Context,
        builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        if *ctx != Context::Unrolling {
            return Err(anyhow!(
                "Reverting a use entity command is only allowed with Context::Unrolling"
            ));
        }
        let entity = tx.load(euid)?;
        let uids_in_use = entity[&self.name].as_array().unwrap();
        for uid_in_use in uids_in_use.clone() {
            let uid_in_use = uid_in_use.as_str().unwrap();
            for injector in self.injectors.appenders.as_slice() {
                injector.eject(builder, tx, uid_in_use, euid)?;
            }
            if let Ok(entity_in_use) = tx.load(uid_in_use) {
                entity_in_use["$users"].as_array_mut().unwrap().retain(
                    |user| !(user["uid"] == euid && user["attr"] == self.name),
                );
                tx.save(uid_in_use)?;
                let class_names = match &self.class_names {
                    ClassNamesToRoll::List(l) => l,
                    _ => unreachable!(),
                };
                for cls in class_names {
                    recycle(tx, euid, uid_in_use, &cls)?;
                }
            }
        }
        // entity.clear(&self.name); // This is likely redundant, but kept for clarity
        Ok(())
    }
}

/// Pick a collected entity by class name:
///
/// ```text
/// attribute % ClassName
/// ```
/// or
///
/// ```text
/// [1..3 attribute] % ClassName
/// ```
///
/// * The class name can be a concrete or a base class
/// * Picked entities can be selected again, unlike used entities.
/// * If multiple entities are picked for an attribute, uniqueness is maintained.
///
/// Any selected entity can be 'injected' with new attributes or attribute overrides:
///
/// ```text
/// attribute ? ClassName {
///     new_attribute = "some value"
/// }
/// ```
#[derive(Clone)]
pub struct AttrCommandPickEntity {
    pub name: String,
    pub class_names: ClassNamesToRoll,
    pub min: CardinalityValue,
    pub max: CardinalityValue,
    injectors: Injectors,
}

impl EntityAssigner for AttrCommandPickEntity {
    fn new(
        name: String,
        class_names: ClassNamesToRoll,
        min: CardinalityValue,
        max: CardinalityValue,
        injectors: Injectors,
    ) -> Self {
        AttrCommandPickEntity {
            name,
            class_names,
            min,
            max,
            injectors,
        }
    }
}

impl AttrCommand for AttrCommandPickEntity {
    fn apply(
        &self,
        ctx: &mut Context,
        builder: &SandboxBuilder,
        blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        if entity.is_missing(&self.name) {
            entity[&self.name] =
                serde_json::to_value(Vec::new() as Vec<serde_json::Value>)
                    .unwrap();
        }
        let (min, max) = match ctx {
            Context::Appending(_) => (1, 1),
            Context::Rolling => (
                resolve_value(blueprint, &self.min),
                resolve_value(blueprint, &self.max),
            ),
            _ => {
                return Err(anyhow!(
                    "Invalid context when applying pick: {:#?}",
                    ctx
                ));
            }
        };

        let class_names = match &self.class_names {
            ClassNamesToRoll::List(l) => l,
            _ => unreachable!(),
        };
        let mut uniqueness_check_set: HashSet<String> = HashSet::new();
        for _ in 0..builder.randomizer.in_range(min, max) {
            let cls = builder.randomizer.choose::<String>(class_names);
            if let Ok(Some(selected_uid)) =
                pick_collected(builder, tx, euid, cls)
            {
                if !uniqueness_check_set.insert(selected_uid.clone()) {
                    continue;
                }
                for injector in self.injectors.appenders.as_slice() {
                    injector.inject(builder, tx, &selected_uid, euid)?;
                }

                add_user_to_entity(tx, &selected_uid, euid, &self.name)?;
                let entity = tx.load(euid)?;
                let list = entity[&self.name].as_array_mut().unwrap();
                list.push(serde_json::to_value(selected_uid)?);
            }
        }
        if (uniqueness_check_set.len() as i32) < min {
            let cls = builder.randomizer.choose::<String>(class_names);
            add_demand(tx, euid, cls)?;
        }
        Ok(())
    }

    fn revert(
        &self,
        ctx: &mut Context,
        builder: &SandboxBuilder,
        _blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        euid: &str,
    ) -> Result<()> {
        if *ctx != Context::Unrolling {
            return Err(anyhow!(
                "Reverting a pick entity command is only allowed with Context::Unrolling"
            ));
        }
        let entity = tx.load(euid)?;
        let uids_in_use = entity[&self.name].as_array().unwrap();
        for uid_in_use in uids_in_use.clone() {
            let uid_in_use = uid_in_use.as_str().unwrap();
            for injector in self.injectors.appenders.as_slice() {
                injector.eject(builder, tx, uid_in_use, euid)?;
            }
            if let Ok(entity_in_use) = tx.load(uid_in_use) {
                entity_in_use["$users"].as_array_mut().unwrap().retain(
                    |user| !(user["uid"] == euid && user["attr"] == self.name),
                );
                tx.save(uid_in_use)?;
            }
        }
        Ok(())
    }
}

/// An attribute injection command that sets a simple value:
///
/// ```text
/// attribute ? ClassName {
///     new_attribute = "Just a string"
/// }
/// ```
#[derive(Clone)]
pub struct InjectCommandSetValue {
    pub name: String,
    pub value: serde_json::Value,
}

impl ValueAssigner for InjectCommandSetValue {
    fn new(name: String, value: serde_json::Value) -> Self {
        InjectCommandSetValue { name, value }
    }
}

impl InjectCommand for InjectCommandSetValue {
    fn inject(
        &self,
        _builder: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        _caller: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity[&self.name] = self.value.clone();
        Ok(())
    }
    fn eject(
        &self,
        _builder: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        _caller: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity[&self.name] = serde_json::Value::from(false);
        Ok(())
    }
}

/// An attribute injection command that sets a dice roll value:
///
/// ```text
/// attribute ? ClassName {
///     new_attribute @ 2d20
/// }
/// ```
#[derive(Clone)]
pub struct InjectCommandDiceRoll {
    pub name: String,
    pub number_of_dice: i32,
    pub dice_type: i32,
    pub dice_modifier: i32,
}

impl InjectCommand for InjectCommandDiceRoll {
    fn inject(
        &self,
        builder: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        _caller: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        let mut total: i32 = 0;
        for _ in 0..self.number_of_dice {
            total += builder.randomizer.in_range(1, self.dice_type);
        }
        total += self.dice_modifier;
        entity[&self.name] = serde_json::to_value(total).unwrap();
        Ok(())
    }
    fn eject(
        &self,
        _builder: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        _caller: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity[&self.name] = serde_json::Value::from(false);
        Ok(())
    }
}

/// An attribute injection command that rolls a value from a list:
///
/// ```text
/// attribute % ClassName {
///     new_attribute @ [
///         * foo
///         * bar
///     ]
/// }
/// ```
#[derive(Clone)]
pub struct InjectCommandRollFromList {
    pub name: String,
    pub list: Vec<serde_json::Value>,
}

impl InjectCommand for InjectCommandRollFromList {
    fn inject(
        &self,
        builder: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        _caller: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity[&self.name] = builder.randomizer.choose(&self.list).to_owned();
        Ok(())
    }
    fn eject(
        &self,
        _builder: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        _caller: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity.clear(&self.name);
        Ok(())
    }
}

/// An attribute injection command that copies a value from the parent entity:
///
/// ```text
/// MainClass {
///     main_attribute = "Value"
///     attribute % AnotherClass {
///         # new_attribute will be set to "Value"
///         new_attribute = &main_attribute
///     }
/// }
///```
#[derive(Clone)]
pub struct InjectCommandCopyValue {
    pub name: String,
    pub path: Vec<String>,
}

impl RefInjectCommand for InjectCommandCopyValue {
    fn make(
        name: String,
        path: Vec<String>,
    ) -> Arc<dyn InjectCommand + Send + Sync> {
        Arc::new(InjectCommandCopyValue { name, path })
    }
}

impl InjectCommand for InjectCommandCopyValue {
    fn inject(
        &self,
        _builder: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        caller: &str,
    ) -> Result<()> {
        let value = {
            if let Some((pointer_uid, pointer_attr_name)) =
                walk_path(caller, &self.path, tx)?
            {
                let src = tx.load(pointer_uid.as_str().unwrap())?;
                src[pointer_attr_name.as_str().unwrap()].clone()
            } else {
                serde_json::Value::from(false)
            }
        };
        let entity = tx.load(euid)?;
        entity[&self.name] = value;
        Ok(())
    }
    fn eject(
        &self,
        _builder: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        _caller: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity[&self.name] = serde_json::Value::from(false);
        Ok(())
    }
}

/// An attribute injection command that points to a value in the parent entity.
/// The actual value is resolved during rendering.
///
/// ```text
/// MainClass {
///     main_attribute = "Value"
///     attribute % AnotherClass {
///         # new_attribute will not be statically set and can change
///         # if main_attribute's value is modified.
///         new_attribute = *main_attribute
///     }
/// }
///```
pub struct InjectCommandPtr {
    pub name: String,
    pub path: Vec<String>,
}

impl RefInjectCommand for InjectCommandPtr {
    fn make(
        name: String,
        path: Vec<String>,
    ) -> Arc<dyn InjectCommand + Send + Sync> {
        Arc::new(InjectCommandPtr { name, path })
    }
}

impl InjectCommand for InjectCommandPtr {
    fn inject(
        &self,
        _builder: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        caller: &str,
    ) -> Result<()> {
        if let Some((pointer_uid, pointer_attr_name)) =
            walk_path(caller, &self.path, tx)?
        {
            let value = serde_json::json!({
                "type": "pointer",
                "spec": {
                    "uid": pointer_uid,
                    "attr": pointer_attr_name
                }
            });
            let entity = tx.load(euid)?;
            entity[&self.name] = value;
            add_user_to_entity(
                tx,
                pointer_uid.as_str().unwrap(),
                euid,
                &self.name,
            )?;
            Ok(())
        } else {
            Err(anyhow!(
                "walking an attribute path failed for {}",
                self.name
            ))
        }
    }
    fn eject(
        &self,
        _builder: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        _caller: &str,
    ) -> Result<()> {
        let entity = tx.load(euid)?;
        entity[&self.name] = serde_json::Value::from(false);
        Ok(())
    }
}

/// Adds a user to a used, picked or pointed-to entity.
/// The user spec is added to a `$users` array attribute and is used to
/// inform any users in case the entity in use is unrolled.
/// # Parameters
///
/// - `tx`: A read/write transaction
/// - `uid`: Uid of the entity being used, picked or pointed-to.
/// - `user_uid`: Uid of the user
/// - `user_attr`: The attribute in the user entity storing this entity's uid.
///
/// # Returns
///
/// - `Result<()>`: Returns `Ok(())` if the operation is successful or an error if it fails.
fn add_user_to_entity(
    tx: &mut ReadWriteTransaction,
    uid: &str,
    user_uid: &str,
    user_attr: &str,
) -> Result<()> {
    let selected_entity = tx.load(uid)?;
    if selected_entity.is_missing("$users") {
        selected_entity["$users"] = serde_json::json!([]);
    }
    selected_entity["$users"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "uid": user_uid,
            "attr": user_attr,
        }));
    tx.save(uid)?;
    Ok(())
}

/// Resolves pointer or reference attribute path.
///
/// When injecting pointers or references, the specified attribute
/// can be navigated to using dot-notation path:
///
/// ```text
/// attribute = *parent_attr.its_attribute
/// ```
fn walk_path(
    starting_uid: &str,
    path: &[String],
    tx: &mut ReadWriteTransaction,
) -> Result<Option<(serde_json::Value, serde_json::Value)>> {
    let mut pointer_uid: serde_json::Value = serde_json::json!(starting_uid);
    let mut pointer_attr_name = serde_json::to_value(path.first()).unwrap();
    for (index, path_part) in path.iter().enumerate() {
        let src = tx.load(pointer_uid.as_str().unwrap())?;
        let pointed_value = &src[path_part];
        if index == path.len() - 1 {
            pointer_attr_name = serde_json::to_value(path_part).unwrap();
        } else if let Some(value_as_array) = pointed_value.as_array() {
            if value_as_array.is_empty() {
                return Ok(None);
            }
            pointer_uid = value_as_array.first().unwrap().clone();
        } else {
            pointer_attr_name = serde_json::to_value(path_part).unwrap();
        }
    }
    Ok(Some((pointer_uid, pointer_attr_name)))
}

/// Resolve the actual cardinality value when specifying it in array attributes.
/// Values can be set explicitly as integers, or indirectly from variables.
/// When no cardinality is specified, then we assume a single entity
/// is being generated.
fn resolve_value(blueprint: &SandboxBlueprint, cv: &CardinalityValue) -> i32 {
    match cv {
        CardinalityValue::Number(n) => *n,
        CardinalityValue::Variable(v) => {
            log::info!("{}", v);
            blueprint.globals.get(v).unwrap().as_i64().unwrap() as i32
        }
        CardinalityValue::Undefined => 1,
    }
}
