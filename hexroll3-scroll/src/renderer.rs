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
use anyhow::anyhow;
use minijinja::Environment;
use std::collections::HashMap;

use crate::instance::{SandboxBlueprint, SandboxInstance};
use crate::renderer_env::prepare_renderer;
use crate::repository::{ReadOnlyLoader, ReadOnlyTransaction};

pub struct RendererContext<'a> {
    pub cache: HashMap<String, serde_json::value::Value>,
    pub env: Environment<'a>,
}

/// Generates HTML for the given object using a specified template.
///
/// # Arguments
///
/// * `instance` - Reference to the CachedInstance containing class specifications.
/// * `tx` - Read-only transaction for accessing repository data.
/// * `obj` - JSON object representing the data to be rendered.
///
/// # Returns
///
/// A `String` containing the rendered HTML if the class has an HTML body; otherwise, returns an empty string.
///
/// The function sets up a rendering environment, prepares the renderer, and attempts to render the template with the provided data.
pub fn render_entity_html(
    instance: &SandboxInstance,
    blueprint: &mut SandboxBlueprint,
    tx: &ReadOnlyTransaction,
    obj: &serde_json::Value,
) -> anyhow::Result<(String, String)> {
    let mut env = Environment::new();
    prepare_renderer(&mut env, instance);
    if let Some(class_spec) = obj["class"]
        .as_str()
        .and_then(|name| instance.resolve_class(blueprint, &name))
    {
        if let (Some(html_body), Some(html_header)) =
            (&class_spec.html_body, &class_spec.html_header)
        {
            let rendered_entity =
                render_entity(instance, blueprint, tx, obj, true)?;
            let mut rendered_header = env
                .render_str(html_header.as_str(), &rendered_entity)
                .map_err(anyhow::Error::new)?;
            if let Some(html_metadata) = &class_spec.html_metadata {
                let rendered_metadata = env
                    .render_str(html_metadata.as_str(), &rendered_entity)
                    .map_err(anyhow::Error::new)?;
                rendered_header.insert_str(0, &rendered_metadata);
            }
            let rendered_body = env
                .render_str(html_body.as_str(), &rendered_entity)
                .map_err(anyhow::Error::new)?;
            return Ok((rendered_header, rendered_body));
        }
    }
    Ok((String::new(), String::new()))
}

pub fn render_entity<T: ReadOnlyLoader>(
    instance: &SandboxInstance,
    blueprint: &mut SandboxBlueprint,
    tx: &T,
    obj: &serde_json::Value,
    is_root: bool,
) -> anyhow::Result<serde_json::Value> {
    let env = {
        let mut env = Environment::new();
        env.set_undefined_behavior(minijinja::UndefinedBehavior::Chainable);
        prepare_renderer(&mut env, instance);
        env
    };
    recursive_entity_renderer(
        &mut RendererContext {
            cache: HashMap::new(),
            env,
        },
        instance,
        blueprint,
        tx,
        obj,
        is_root,
        None,
    )
}

/// Renders attributes of a given object recursively within a specified context.
///
/// This function processes an object's attributes based on the class specification,
/// handles caching, and renders templates or nested objects as needed.
/// Supports hierarchical rendering with parent and pointer references.
///
/// # Note
/// This function assumes a well-formed context entry from the point it processes specific attributes.
/// If any of the `as_str().unwrap()` calls fail, it indicates a logical bug in the input structure,
/// rather than a recoverable runtime error.
///
/// # Type Parameters
/// - `T`: A loader implementing `ReadOnlyLoader` for data retrieval.
///
/// # Arguments
/// - `context`: The rendering context for managing templates and caching.
/// - `instance`: The cached instance with class specifications.
/// - `tx`: Data loader for retrieving entities.
/// - `obj`: JSON object to be rendered.
/// - `is_root`: Indicates if the object is the root node.
/// - `stopper`: Optional key to halt rendering at a specific point.
///
/// # Returns
/// - `anyhow::Result<serde_json::Value>`: Rendered object or an error if rendering fails.
fn recursive_entity_renderer<T: ReadOnlyLoader>(
    context: &mut RendererContext,
    instance: &SandboxInstance,
    blueprint: &mut SandboxBlueprint,
    tx: &T,
    obj: &serde_json::Value,
    is_root: bool,
    stopper: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let uuid = obj["uid"].as_str().unwrap().to_string();
    if context.cache.contains_key(&uuid) && !is_root {
        return Ok(context.cache.get(&uuid).unwrap().clone());
    }
    let class_name = obj["class"].as_str().unwrap();
    let class_spec = instance.resolve_class(blueprint, class_name).unwrap();

    let mut ctx = serde_json::json!({
        "uuid" : obj["uid"]
    });
    let mut ret = serde_json::json!({
        "class": obj["class"],
        "uuid": obj["uid"]
    });
    for spec in class_spec.collects.iter() {
        if let Some(attr) = &spec.virtual_attribute {
            if attr.is_optional && !is_root {
                continue;
            }
            let frame = tx.retrieve(&format!("{}_frame", uuid))?;
            let unused =
                &frame.value["$collections"]["$unused"][&spec.class_name];
            ctx[&attr.attr_name] = serde_json::Value::from(
                unused
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|unused_id| {
                        let next = tx.retrieve(unused_id.as_str().unwrap())?;
                        recursive_entity_renderer(
                            context,
                            instance,
                            blueprint,
                            tx,
                            &next.value,
                            false,
                            None,
                        )
                    })
                    .collect::<Result<Vec<serde_json::Value>, _>>()?,
            );
            if attr.is_public || is_root {
                ret[&attr.attr_name] = ctx[&attr.attr_name].clone();
            }
        }
    }
    for (attr_name, raw_value) in obj.as_object().unwrap() {
        if attr_name.starts_with('$') {
            continue;
        }
        let (is_optional, is_public, is_array) =
            if let Some(attr_spec) = &class_spec.attrs.get(attr_name) {
                (
                    attr_spec.is_optional,
                    attr_spec.is_public,
                    attr_spec.is_array,
                )
            } else {
                (false, false, false)
            };
        if is_optional && !is_root {
            continue;
        }
        ctx[attr_name] = match raw_value {
            serde_json::Value::Bool(_) | serde_json::Value::Number(_) => obj[attr_name].clone(),
            serde_json::Value::String(_) => serde_json::Value::String(
                context
                    .env
                    .render_str(obj[attr_name].as_str().unwrap(), &ctx)
                    .map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to render string template {} for uid {} attr {} with error {:#}",
                            obj[attr_name].as_str().unwrap(),
                            uuid,
                            attr_name,
                            e
                        )
                    })?,
            ),
            serde_json::Value::Array(_) => {
                if is_array {
                    serde_json::Value::from(
                        raw_value
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|child_uid| {
                                let next = tx.retrieve(child_uid.as_str().unwrap())?;
                                recursive_entity_renderer(
                                    context,
                                    instance,
                                    blueprint,
                                    tx,
                                    &next.value,
                                    false,
                                    None,
                                )
                            })
                            .collect::<Result<Vec<serde_json::Value>, _>>()?,
                    )
                } else if let Some(id) = raw_value.as_array().unwrap().iter().next() {
                    let next = tx.retrieve(id.as_str().unwrap())?;
                    recursive_entity_renderer(context, instance, blueprint, tx, &next.value, false, None)?
                } else {
                    serde_json::json!({})
                }
            }
            serde_json::Value::Object(_) => {
                render_indirections(context, instance, blueprint, tx, obj, attr_name)?
            }
            serde_json::Value::Null => {
                let tmpl_str = class_spec.attrs[attr_name].cmd.value().unwrap();
                serde_json::Value::String(context.env.render_str(&tmpl_str, &ctx).map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to render string template {} for uid {} attr {} with error {:#}",
                            tmpl_str,
                            uuid,
                            attr_name,
                            e
                        )
                    })?,
                )
            }
        };
        if is_public || is_root {
            ret[attr_name] = ctx[attr_name].clone();
        }
        if let Some(stopper) = stopper {
            if stopper == attr_name {
                break;
            }
        } else {
            context.cache.insert(uuid.clone(), ret.clone());
        }
    }
    Ok(ret)
}

/// Renders a specific attribute from a pointed-to entity using its unique identifier.
///
/// Retrieves the entity by its UID, processes it through `render_inner`, and extracts
/// the specified attribute.
///
/// # Type Parameters
/// - `T`: A `ReadOnlyLoader` for retrieving entity data.
///
/// # Arguments
/// - `context`: The rendering context for template and caching operations.
/// - `instance`: Cached instance containing relevant specifications.
/// - `tx`: Data loader for retrieving the pointed-to entity.
/// - `uid`: Unique identifier for the target entity.
/// - `attr`: Attribute to be rendered from the pointed entity.
///
/// # Returns
/// - `anyhow::Result<serde_json::Value>`: Rendered attribute or an error if retrieval/rendering fails.
fn render_pointer_attribute<T: ReadOnlyLoader>(
    context: &mut RendererContext,
    instance: &SandboxInstance,
    blueprint: &mut SandboxBlueprint,
    tx: &T,
    uid: &str,
    attr: &str,
) -> anyhow::Result<serde_json::Value> {
    let pointed_entity = tx.retrieve(uid)?.value;
    let pointed_render = recursive_entity_renderer(
        context,
        instance,
        blueprint,
        tx,
        &pointed_entity,
        true,
        Some(attr),
    )?;
    Ok(pointed_render[attr].clone())
}

/// Renders or retrieves a specified parent attribute from a cached entity instance,
/// navigating up the hierarchy if necessary.
///
/// # Type Parameters
/// - `T`: A loader implementing `ReadOnlyLoader` for data retrieval.
///
/// # Arguments
/// - `context`: Rendering context used for attribute rendering.
/// - `instance`: Cached instance containing class hierarchy information.
/// - `tx`: Data loader for retrieving entities.
/// - `pid`: Parent entity identifier.
/// - `parent_class`: Class name to match in the hierarchy.
/// - `parent_attr`: Attribute to retrieve.
///
/// # Returns
/// - `anyhow::Result<serde_json::Value>`: The rendered or retrieved attribute value.
///
/// # Errors
/// - Returns an error if the parent entity is missing or lacks a valid `class` attribute,
///   or if its class is not found in the cached instance.
fn render_parent_attribute<T: ReadOnlyLoader>(
    context: &mut RendererContext,
    instance: &SandboxInstance,
    blueprint: &mut SandboxBlueprint,
    tx: &T,
    pid: &str,
    parent_class: &str,
    parent_attr: &str,
) -> anyhow::Result<serde_json::Value> {
    let parent = tx.retrieve(pid)?.value;
    let class = parent["class"].as_str().unwrap();
    let Some(class_spec) = instance.resolve_class(blueprint, class) else {
        return Err(anyhow!(
            "Could not find class {} in entity {}",
            class,
            pid
        ));
    };
    if class_spec.hierarchy.contains(&parent_class.to_string()) {
        // Theoretically, we should have done:
        // ```
        // let v = recursive_entity_renderer(context, instance, tx, &parent, true, Some(parent_attr))?;
        // return Ok(v[parent_attr].clone());
        // ```
        // But it is excessive and will result in poor performance, so we care for three cases
        // only:
        //   * child entities
        //   * indirections (pointers and context references)
        //   * value copies
        //
        let v = &parent[parent_attr];
        return if v.is_array() && !v.as_array().unwrap().is_empty() {
            let data_uid =
                v.as_array().unwrap().first().unwrap().as_str().unwrap();
            let data = tx.retrieve(data_uid)?.value;
            recursive_entity_renderer(
                context,
                instance,
                blueprint,
                tx,
                &data,
                false,
                Some(parent_attr),
            )
        } else if v.is_object() {
            render_indirections(
                context,
                instance,
                blueprint,
                tx,
                &parent,
                parent_attr,
            )
        } else {
            Ok(parent[parent_attr].clone())
        };
    } else {
        let my_pid = &parent["parent_uid"];
        if my_pid != "root" {
            return render_parent_attribute(
                context,
                instance,
                blueprint,
                tx,
                my_pid.as_str().unwrap(),
                parent_class,
                parent_attr,
            );
        }
    }
    Ok(serde_json::json!(false))
}

/// Render pointers and context references indicated using a json object
/// containing a `type` entry of either `context` or `pointer` a `spec`
/// entry containing the data needed to indirectly render the value.
///
/// # Arguments
/// - `context`: Rendering context used for attribute rendering.
/// - `instance`: Cached instance containing class hierarchy information.
/// - `tx`: Data loader for retrieving entities.
/// - `obj`: Entity object holding this indirection attribute.
/// - `attr_name`: Attribute name holding this indirection attribute.
///
/// # Returns
/// - `anyhow::Result<serde_json::Value>`: The rendered or retrieved attribute value.
pub fn render_indirections<T: ReadOnlyLoader>(
    context: &mut RendererContext,
    instance: &SandboxInstance,
    blueprint: &mut SandboxBlueprint,
    tx: &T,
    obj: &serde_json::Value,
    attr_name: &str,
) -> Result<serde_json::Value, anyhow::Error> {
    let indirection = &obj[attr_name];
    if indirection["type"] == "context" {
        let pid = obj["parent_uid"].as_str().unwrap();
        let spec = &indirection["spec"];
        let parent_attr = spec["attr"].as_str().unwrap();
        return render_parent_attribute(
            context,
            instance,
            blueprint,
            tx,
            pid,
            spec["parent"].as_str().unwrap(),
            parent_attr,
        );
    } else if indirection["type"] == "pointer" {
        let spec = &indirection["spec"];
        let attr = spec["attr"].as_str().unwrap();
        return render_pointer_attribute(
            context,
            instance,
            blueprint,
            tx,
            spec["uid"].as_str().unwrap(),
            attr,
        );
    } else {
        return Err(anyhow!(
            "Unknown obj detected {}, {}",
            indirection,
            attr_name
        ));
    }
}
