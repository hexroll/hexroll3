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
use std::mem::take;
use std::sync::Arc;

use anyhow::Result;
use indexmap::IndexMap;
use std::marker::Send;
use std::marker::Sync;

use crate::{instance::*, repository::*};

/// Scroll class definition data:
///
/// ```text
/// ClassName(ParentClassName) {
///   attribute = value
///   ...
/// }
/// ```
/// Classes are used to generate random entities.
#[derive(Clone)]
pub struct Class {
    pub name: String,
    pub attrs: IndexMap<String, Attr>,
    pub subclasses: SubclassesSpecifier,
    pub hierarchy: Vec<String>,
    pub collects: Vec<CollectionSpecifier>,
    pub html_body: Option<String>,
    pub html_header: Option<String>,
    pub html_metadata: Option<String>,
}

/// Provides the class subclasses for instantiation, either using a List:
/// ```text
/// Class {
///   ^ [
///       * Subclass1
///       * Subclass2
///     ]
/// }
/// ```
///  or using a variable:
/// ```text
/// list_of_subclasses = [
///   * Subclass1
///   * Subclass2
/// ]
///
/// Class { ^ $list_of_subclasses }
/// ```
#[derive(Clone, PartialEq)]
pub enum SubclassesSpecifier {
    List(Vec<String>),
    Var(String),
    Empty(),
}

/// Class collection specification with an optional attribute:
///
/// ```text
/// EntityClass {
///     << ClassNameToCollect
///     attribute! << AnotherClassNameToCollect
/// }
/// ```
#[derive(Clone, PartialEq)]
pub struct CollectionSpecifier {
    pub class_name: String,
    pub virtual_attribute: Option<CollectionAttribute>,
}

/// An attribute specification for collections:
///
/// ```text
/// EntityClass {
///     attribute! << AnotherClassNameToCollect
/// }
/// ```
#[derive(Clone, PartialEq)]
pub struct CollectionAttribute {
    pub attr_name: String,
    pub is_public: bool,
    pub is_optional: bool,
    pub is_array: bool,
}

/// Attr holds the generation command used to generate an attribute
/// in an entity.
///
/// AttrCommand provides the command pattern interface to `apply` and
/// `revert` the attribute logic, supporting rolling, re-rolling,
/// and unrolling entities.
///
/// Attr is cloneable so it can be shared throughout the class hierarchy.
/// (See the method `expand` in ClassBuilder).
/// The `cmd` inside Attr is using std::rc::Rc for that purpose.
#[derive(Clone)]
pub struct Attr {
    pub cmd: std::sync::Arc<dyn AttrCommand + Sync + Send>,
    pub is_public: bool,
    pub is_optional: bool,
    pub is_array: bool,
}

/// AttrCommand can apply or revert attribute value generation of various kinds.
/// Refer to `commands.rs` to learn more about the different types of AttrCommands.
pub trait AttrCommand {
    fn apply(
        &self,
        ctx: &mut Context,
        instance: &SandboxBuilder,
        blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        entity: &str,
    ) -> Result<()>;
    fn revert(
        &self,
        ctx: &mut Context,
        instance: &SandboxBuilder,
        blueprint: &mut SandboxBlueprint,
        tx: &mut ReadWriteTransaction,
        entity: &str,
    ) -> Result<()>;
    fn value(&self) -> Option<String> {
        None
    }
}

/// InjectCommand can inject or eject attributes or attribute overrides to entities
/// picked, used or pointed-at.
pub trait InjectCommand {
    fn inject(
        &self,
        instance: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        caller: &str,
    ) -> Result<()>;
    fn eject(
        &self,
        instance: &SandboxBuilder,
        tx: &mut ReadWriteTransaction,
        euid: &str,
        caller: &str,
    ) -> Result<()>;
}

#[derive(Clone)]
pub struct Injectors {
    pub prependers: Vec<Arc<dyn InjectCommand + Send + Sync>>,
    pub appenders: Vec<Arc<dyn InjectCommand + Send + Sync>>,
}

/// Provides the toolset required to properly define a Scroll class and
/// is primarily used by the Scroll parser.
pub struct ClassBuilder {
    pub name: String,
    pub parent: String,
    pub attrs: IndexMap<String, Attr>,
    pub subclasses: SubclassesSpecifier,
    pub hierarchy: Vec<String>,
    pub collects: Vec<CollectionSpecifier>,
    pub html_body: Option<String>,
    pub html_header: Option<String>,
    pub html_metadata: Option<String>,
    expanded: bool,
}

impl ClassBuilder {
    /// Creates a new `ClassBuilder` instance.
    pub fn new() -> Self {
        ClassBuilder {
            name: String::new(),
            parent: String::new(),
            attrs: indexmap::IndexMap::new(),
            subclasses: SubclassesSpecifier::Empty(),
            hierarchy: vec![],
            collects: vec![],
            expanded: false,
            html_body: None,
            html_header: None,
            html_metadata: None,
        }
    }

    /// Sets the name of the class. Adds the name to the hierarchy if not present.
    pub fn name(&mut self, name: &str) -> &mut Self {
        self.name = name.to_string();
        if self.hierarchy.contains(&self.name) {
            log::error!("Invalid hierarchy detected for class {}", name)
        } else {
            self.hierarchy.push(self.name.clone());
        }
        self
    }

    /// Adds or overrides an attribute for the class.
    pub fn add_attr(&mut self, key: String, attr: Attr) -> &mut Self {
        if self.attrs.contains_key(&key) {
            let existing_attr = &self.attrs[&key];
            if (
                existing_attr.is_array,
                existing_attr.is_public,
                existing_attr.is_optional,
            ) != (attr.is_array, attr.is_public, attr.is_optional)
            {
                log::warn!(
                    "Overriding attribute {} in {} has conflicting metadata.",
                    key,
                    self.name
                );
            }
            self.attrs[&key] = attr;
        } else {
            self.attrs.insert(key.to_owned(), attr);
        }
        self
    }

    /// Sets the HTML body content for the class.
    pub fn html_body(&mut self, body: String) -> &mut Self {
        self.html_body = Some(body);
        self
    }

    /// Sets the HTML body content for the class.
    pub fn html_header(&mut self, header: String) -> &mut Self {
        self.html_header = Some(header);
        self
    }

    /// Sets the HTML metadata content for the class.
    pub fn html_metadata(&mut self, metadata: String) -> &mut Self {
        self.html_metadata = Some(metadata);
        self
    }

    /// Specifies the class names to collect as a list of subclasses.
    pub fn subclass_item(&mut self, class_name_to_collect: &str) {
        if let SubclassesSpecifier::List(subclasses_list) = &mut self.subclasses
        {
            subclasses_list.push(class_name_to_collect.to_string());
        } else {
            self.subclasses = SubclassesSpecifier::List(vec![
                class_name_to_collect.to_string(),
            ]);
        }
    }

    /// Specifies a single subclass to collect using a variable.
    pub fn subclass_var(&mut self, class_name_to_collect: &str) {
        self.subclasses =
            SubclassesSpecifier::Var(class_name_to_collect.to_string());
    }

    /// Collects the specified class name.
    pub fn collect(&mut self, spec: CollectionSpecifier) {
        self.collects.push(spec);
    }

    /// Expands the class with attributes from another class using its name.
    pub fn expand(
        &mut self,
        instance: &SandboxBlueprint,
        expand_with_class_name: &str,
    ) -> &mut Self {
        let expand_class =
            &instance.classes.get(expand_with_class_name).unwrap();
        for (k, v) in &expand_class.attrs {
            self.attrs.insert(k.to_string(), v.clone());
        }
        self.expanded = true;
        self
    }

    /// Extends this class with attributes and properties of a parent class.
    pub fn extends(
        &mut self,
        instance: &SandboxBlueprint,
        parent_class_name: &str,
    ) -> &mut Self {
        self.parent = parent_class_name.to_string();
        let mut parent_class_name_mut = parent_class_name;

        while !parent_class_name_mut.is_empty() {
            self.hierarchy.push(parent_class_name_mut.to_string());
            let parent_class =
                instance.classes.get(parent_class_name_mut).unwrap();
            parent_class_name_mut = if parent_class.hierarchy.len() < 2 {
                ""
            } else {
                &parent_class.hierarchy[1]
            };
            if let Some(parent_html_body) = &parent_class.html_body {
                self.html_body = Some(parent_html_body.clone());
            }
            if let Some(parent_html_header) = &parent_class.html_header {
                self.html_header = Some(parent_html_header.clone());
            }
            if let Some(parent_html_metadata) = &parent_class.html_metadata {
                self.html_metadata = Some(parent_html_metadata.clone());
            }
            self.collects = parent_class.collects.clone();
        }
        self
    }

    /// Applies pending expansions or parent relationships before finalizing the class.
    pub fn conclude(&mut self, instance: &SandboxBlueprint) -> &mut Self {
        if !self.expanded && !self.parent.is_empty() {
            let my_attrs = take(&mut self.attrs);
            self.expand(instance, &self.parent.clone());
            self.attrs.extend(my_attrs);
        }
        self
    }

    /// Finalizes the construction of the class.
    pub fn build(self) -> Class {
        Class {
            name: self.name,
            attrs: self.attrs,
            subclasses: self.subclasses,
            hierarchy: self.hierarchy,
            collects: self.collects,
            html_body: self.html_body,
            html_header: self.html_header,
            html_metadata: self.html_metadata,
        }
    }
}

impl Default for ClassBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq)]
pub struct AppendPayload<'a> {
    pub class_override: Option<&'a str>,
    pub appended_uid: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct RerollPayload<'a> {
    pub existing_uid: String,
    pub class_override: Option<&'a str>,
    pub new_uid: Option<String>,
}

/// Used when calling the generators to state which type of user
/// operation are we trying to do. Can provide and store ephemeral
/// state throughout the generation call.
#[derive(Debug, PartialEq)]
pub enum Context<'a> {
    Rolling,
    Appending(AppendPayload<'a>),
    Rerolling(RerollPayload<'a>),
    Unrolling,
    Restoring,
}
