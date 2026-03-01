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
use anyhow::{anyhow, Result};
use std::borrow::BorrowMut;
use std::cell::{RefCell, RefMut};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use pest::iterators::{Pair, Pairs};
use pest::Parser;

use crate::commands::*;
use crate::instance::*;
use crate::semantics::*;

#[derive(Parser)]
#[grammar = "scroll.pest"]
struct ScrollParser;

pub fn parse_file(instance: &mut SandboxInstance, filename: PathBuf) -> Result<()> {
    if let Ok(unparsed_file) = std::fs::read_to_string(filename.to_str().unwrap()) {
        let filepath = match filename.parent() {
            Some(parent) => parent.to_path_buf(),
            None => PathBuf::from("./"),
        };
        parse_buffer(
            instance,
            &unparsed_file,
            Some(filepath.to_str().unwrap()),
            Some(filename.to_str().unwrap()),
        )
    } else {
        Err(anyhow!(
            "Failed reading scroll file {}",
            filename.to_str().unwrap()
        ))
    }
}

pub fn parse_buffer(
    instance: &mut SandboxInstance,
    buffer: &str,
    filepath: Option<&str>,
    filename: Option<&str>,
) -> Result<()> {
    match ScrollParser::parse(Rule::file, buffer) {
        Ok(pairs) => parse_scroll(instance, pairs, filepath.unwrap_or("")),
        Err(e) => Err(anyhow!(
            "Parsing {} failed! {:#}",
            filename.unwrap_or("buffer"),
            e.to_string()
        )),
    }
}

/// Helper structure to manage probability parsing and execution.
///
/// The `ProbabilityHelper` is designed to parse a probability specifier
/// in the form of "(xN)", where N is any integer number indicating the
/// multiplier to be used. It is used in conjunction with a parser that
/// interprets these specifiers and executes a callback function
/// multiple times based on the parsed multiplier.
struct ProbabilityHelper {
    multiplier: i32,
}

impl<'a> ProbabilityHelper {
    pub fn new() -> Self {
        ProbabilityHelper { multiplier: 1 }
    }
    pub fn parse_multiplier(&mut self, inner_pair: &Pair<'_, Rule>) {
        if let Ok(p) = inner_pair.clone().into_inner().as_str().parse::<i32>() {
            self.multiplier = p;
        }
    }
    pub fn multiply<F: FnMut() + 'a>(&mut self, mut callback: F) {
        for _ in 0..self.multiplier {
            callback();
        }
        self.multiplier = 1;
    }
}

fn upcast_string(input: &str) -> serde_json::Value {
    if let Ok(parsed_i32) = input.parse::<i32>() {
        serde_json::json!(parsed_i32)
    } else if let Ok(parsed_f32) = input.parse::<f32>() {
        serde_json::json!(parsed_f32)
    } else if input == "true" || input == "false" {
        serde_json::json!(input.parse::<bool>().unwrap())
    } else {
        serde_json::json!(input)
    }
}

// Helper function to parse attribute specifiers.
//
// Scroll attributes have a name and an optional visibility
// specifier. This function parses the attribute name and
// visibility specifiers and returns a tuple with:
// (attr_name, is_public, is_optional)
fn parse_attribute_spec(pair: Pair<'_, Rule>) -> (&str, bool, bool) {
    let mut is_public: bool = false;
    let mut is_optional: bool = false;
    let mut attr_decl_rule = pair.into_inner();

    let attr_name = attr_decl_rule.next().unwrap().as_str();
    for inner_pair in attr_decl_rule {
        if let Rule::is_public = inner_pair.as_rule() {
            is_public = true;
        }
        if let Rule::is_optional = inner_pair.as_rule() {
            is_optional = true;
        }
    }

    (attr_name, is_public, is_optional)
}

fn parse_injection_roll_from_list(inner_pair: Pair<Rule>) -> Arc<InjectCommandRollFromList> {
    let mut iter = inner_pair.into_inner();
    let (attr, _, _) = parse_attribute_spec(iter.next().unwrap());

    let mut value: Vec<serde_json::Value> = vec![];

    for inner_pair in iter {
        match inner_pair.as_rule() {
            Rule::free_text => value.push(serde_json::to_value(inner_pair.as_str()).expect("x")),
            Rule::list_value => {
                value.push(serde_json::to_value(inner_pair.as_str().trim()).expect("x"))
            }
            Rule::probability_spec => {}
            _ => unreachable!(),
        }
    }

    Arc::new(InjectCommandRollFromList {
        name: attr.to_string(),
        list: value,
    })
}

fn parse_injection_dice_roll(inner_pair: Pair<Rule>) -> Arc<InjectCommandDiceRoll> {
    let mut iter = inner_pair.into_inner();
    let (attr, _, _) = parse_attribute_spec(iter.next().unwrap());

    Arc::new(InjectCommandDiceRoll {
        name: attr.to_string(),
        number_of_dice: iter.next().unwrap().as_str().parse().unwrap(),
        dice_type: iter.next().unwrap().as_str().parse().unwrap(),
        dice_modifier: if let Some(val) = iter.next() {
            val.as_str().parse().unwrap()
        } else {
            0
        },
    })
}

fn parse_injection_assign_by_ref<T: InjectCommand + RefInjectCommand + 'static>(
    inner_pair: Pair<Rule>,
) -> Arc<dyn InjectCommand + Send + Sync> {
    let mut iter = inner_pair.into_inner();
    let (attr, _, _) = parse_attribute_spec(iter.next().unwrap());
    let mut path: Vec<String> = Vec::new();
    let next = iter.next().unwrap();
    match next.as_rule() {
        Rule::attr_spec => path.push(next.as_str().to_string()),
        Rule::attr_attr_spec => {
            let mut attr_attr = next.into_inner();
            path.push(attr_attr.next().unwrap().as_str().to_string());
            path.push(attr_attr.next().unwrap().as_str().to_string());
        }
        _ => unreachable!(),
    }
    T::make(attr.to_string(), path)
}

fn parse_injections(pair: Pair<Rule>) -> anyhow::Result<Injectors> {
    let mut prependers = Vec::<Arc<dyn InjectCommand + Send + Sync>>::new();
    let mut appenders = Vec::<Arc<dyn InjectCommand + Send + Sync>>::new();
    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::prepend_copy_value => {
                prependers.push(parse_injection_assign_by_ref::<InjectCommandCopyValue>(
                    inner_pair,
                ));
            }
            Rule::append_copy_value => {
                appenders.push(parse_injection_assign_by_ref::<InjectCommandCopyValue>(
                    inner_pair,
                ));
            }
            Rule::prepend_ptr => {
                prependers.push(parse_injection_assign_by_ref::<InjectCommandPtr>(
                    inner_pair,
                ));
            }
            Rule::append_ptr => {
                appenders.push(parse_injection_assign_by_ref::<InjectCommandPtr>(
                    inner_pair,
                ));
            }
            Rule::prepend_assignment => {
                prependers.push(parse_injection_assignment(inner_pair));
            }
            Rule::assignment => {
                appenders.push(parse_injection_assignment(inner_pair));
            }
            Rule::roll_from_list => {
                appenders.push(parse_injection_roll_from_list(inner_pair));
            }
            Rule::roll_a_dice => {
                appenders.push(parse_injection_dice_roll(inner_pair));
            }
            _ => unreachable!(),
        }
    }
    Ok(Injectors {
        prependers,
        appenders,
    })
}

fn parse_dice_notation(name: String, mut pair: Pairs<Rule>) -> Arc<dyn AttrCommand + Send + Sync> {
    Arc::new(AttrCommandDice {
        name,
        number_of_dice: pair.next().unwrap().as_str().parse().unwrap(),
        dice_type: pair.next().unwrap().as_str().parse().unwrap(),
        dice_modifier: if let Some(val) = pair.next() {
            val.as_str().parse().unwrap()
        } else {
            0
        },
    })
}

fn parse_attribute_ex(
    f: fn(String, Pairs<Rule>) -> std::sync::Arc<dyn AttrCommand + Send + Sync>,
    pair: Pair<Rule>,
    mut class: RefMut<ClassBuilder>,
) {
    let mut inner = pair.into_inner();
    let (attr, is_public, is_optional) = parse_attribute_spec(inner.next().unwrap());
    class.add_attr(
        attr.to_string(),
        Attr {
            cmd: f(attr.to_string(), inner),
            is_public,
            is_optional,
            is_array: false,
        },
    );
}

fn parse_subclasses(pair: Pair<Rule>, mut class: RefMut<ClassBuilder>) {
    let inner_pair = pair.into_inner().next().unwrap();
    match inner_pair.as_rule() {
        Rule::global => {
            class.subclass_var(inner_pair.as_str());
        }
        Rule::entities_list => {
            let mut prob = ProbabilityHelper::new();
            for ip in inner_pair.into_inner() {
                if let Rule::probability_spec = ip.as_rule() {
                    prob.parse_multiplier(&ip);
                }
                if let Rule::entity_name = ip.as_rule() {
                    prob.multiply(|| class.subclass_item(ip.as_str().trim()));
                }
            }
        }
        _ => unreachable!(),
    }
}

fn parse_number_or_variable(pair: Pair<Rule>) -> CardinalityValue {
    let inner = pair.into_inner().next().unwrap();
    if inner.as_rule() == Rule::number {
        CardinalityValue::Number(inner.as_str().parse().unwrap())
    } else {
        let mut v = inner.as_str().to_string();
        v.remove(0);
        CardinalityValue::Variable(v)
    }
}

fn parse_entity_attribute<CMD: AttrCommand + Send + Sync + EntityAssigner + 'static>(
    pair: Pair<Rule>,
    mut class: RefMut<ClassBuilder>,
) {
    let mut attr: Option<&str> = None;
    let mut value: ClassNamesToRoll = ClassNamesToRoll::Unset();
    let mut min: CardinalityValue = CardinalityValue::Undefined;
    let mut max: CardinalityValue = CardinalityValue::Undefined;
    let mut is_public: bool = false;
    let mut is_optional: bool = false;
    let mut injectors: Injectors = Injectors {
        prependers: Vec::new(),
        appenders: Vec::new(),
    };

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::property => {
                (attr, is_public, is_optional) = {
                    let (a, b, c) = parse_attribute_spec(inner_pair);
                    (Some(a), b, c)
                }
            }
            Rule::indirect => {
                value = ClassNamesToRoll::Indirect(
                    inner_pair.into_inner().next().unwrap().as_str().to_string(),
                );
            }
            Rule::array => {
                let mut iter = inner_pair.into_inner();
                min = parse_number_or_variable(iter.next().unwrap());
                max = parse_number_or_variable(iter.next().unwrap());
                (attr, is_public, is_optional) = {
                    let (a, b, c) = parse_attribute_spec(iter.next().unwrap());
                    (Some(a), b, c)
                }
            }
            Rule::entity_name => {
                value = ClassNamesToRoll::List(vec![]);
                if let ClassNamesToRoll::List(l) = &mut value {
                    l.push(inner_pair.as_str().to_string())
                }
            }
            Rule::entities_list => {
                value = ClassNamesToRoll::List(vec![]);
                if let ClassNamesToRoll::List(l) = &mut value {
                    for ip in inner_pair.into_inner() {
                        if let Rule::entity_name = ip.as_rule() {
                            l.push(ip.as_str().trim().to_string())
                        }
                    }
                }
            }
            Rule::injections => {
                injectors = parse_injections(inner_pair).unwrap();
            }
            _ => unreachable!(),
        }
    }
    if let Some(attr) = attr {
        if let ClassNamesToRoll::Unset() = value {
            unreachable!()
        }
        let is_array = !matches!(min, CardinalityValue::Undefined)
            && !matches!(max, CardinalityValue::Undefined);
        class.add_attr(
            attr.to_string(),
            Attr {
                cmd: std::sync::Arc::new(CMD::new(attr.to_string(), value, min, max, injectors)),
                is_public,
                is_optional,
                is_array,
            },
        );
    } else {
        unreachable!();
    }
}

fn parse_collection(pair: Pair<Rule>, mut class: RefMut<ClassBuilder>) {
    let mut class_name: Option<String> = None;
    let mut named_collection: Option<String> = None;
    let mut is_optional = false;
    let is_array = true;
    let mut is_public = false;
    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::entity_name => class_name = Some(inner_pair.as_str().to_string()),
            Rule::property => {
                for property_pair in inner_pair.into_inner() {
                    match property_pair.as_rule() {
                        Rule::property_name => {
                            named_collection = Some(property_pair.as_str().to_string())
                        }
                        Rule::is_public => is_public = true,
                        Rule::is_optional => is_optional = true,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(class_name) = class_name {
        let virtual_attribute = named_collection.map(|named_collection| CollectionAttribute {
            attr_name: named_collection,
            is_optional,
            is_array,
            is_public,
        });
        class.collect(CollectionSpecifier {
            class_name,
            virtual_attribute,
        });
    }
}

fn parse_roll_from_list(name: String, pair: Pairs<Rule>) -> Arc<dyn AttrCommand + Send + Sync> {
    let mut value: Vec<serde_json::Value> = vec![];

    let mut prob = ProbabilityHelper::new();
    for inner_pair in pair {
        match inner_pair.as_rule() {
            Rule::probability_spec => {
                prob.parse_multiplier(&inner_pair);
            }
            Rule::list_value => {
                prob.multiply(|| value.push(upcast_string(inner_pair.as_str().trim())));
            }
            _ => unreachable!(),
        }
    }
    Arc::new(AttrCommandRollFromList { name, list: value })
}

fn parse_roll_via_variable(
    name: String,
    mut pair: Pairs<Rule>,
) -> Arc<dyn AttrCommand + Send + Sync> {
    let mut var = pair.next().unwrap().as_str().to_string();
    var.remove(0);
    Arc::new(AttrCommandRollFromVariable { name, var })
}

fn parse_context(name: String, mut pair: Pairs<Rule>) -> Arc<dyn AttrCommand + Send + Sync> {
    let next = pair.next().unwrap();
    match next.as_rule() {
        Rule::context | Rule::context_ptr => {
            let mut context_rule = next.into_inner();
            Arc::new(AttrCommandContext {
                name,
                context_parent: context_rule.next().unwrap().as_str().to_string(),
                context_attr: context_rule.next().unwrap().as_str().to_string(),
            })
        }
        _ => unreachable!(),
    }
}

fn parse_value_token(token: Pair<Rule>) -> serde_json::Value {
    match token.as_rule() {
        Rule::null => serde_json::json!(false),
        Rule::template_body => serde_json::json!(token.as_str()),
        Rule::slim_template_body => serde_json::json!(token.as_str()),
        Rule::string => serde_json::json!(token.as_str()),
        Rule::free_text => upcast_string(token.as_str().trim()),
        _ => upcast_string(token.as_str().trim()),
    }
}

fn parse_simple_value(name: String, mut pair: Pairs<Rule>) -> Arc<dyn AttrCommand + Send + Sync> {
    let next = pair.next().unwrap();
    match next.as_rule() {
        Rule::value => {
            let token = next.into_inner().next().unwrap();
            let value = parse_value_token(token);
            Arc::new(AttrCommandAssigner { name, value })
        }
        _ => unreachable!(),
    }
}

fn parse_injection_assignment(inner_pair: Pair<Rule>) -> Arc<InjectCommandSetValue> {
    let mut iter = inner_pair.into_inner();
    let (attr, _, _) = parse_attribute_spec(iter.next().unwrap());

    let token = iter.next().unwrap().into_inner().next().unwrap();
    let value = parse_value_token(token);

    Arc::new(InjectCommandSetValue {
        name: attr.to_string(),
        value,
    })
}

fn parse_weak_value(name: String, mut pair: Pairs<Rule>) -> Arc<dyn AttrCommand + Send + Sync> {
    let value: serde_json::Value;
    let next = pair.next().unwrap();
    match next.as_rule() {
        Rule::value => {
            let as_str = next.as_str();
            let token = next.into_inner().next().unwrap();
            value = match token.as_rule() {
                Rule::template_body => serde_json::json!(token.as_str()),
                Rule::slim_template_body => {
                    serde_json::json!(token.as_str())
                }
                Rule::string => serde_json::json!(token.as_str()),
                _ => upcast_string(as_str.trim()),
            };
            Arc::new(AttrCommandWeakAssigner { name, value })
        }
        _ => unreachable!(),
    }
}

fn parse_prerendered_value(
    name: String,
    mut pair: Pairs<Rule>,
) -> Arc<dyn AttrCommand + Send + Sync> {
    let value: serde_json::Value;
    let next = pair.next().unwrap();
    match next.as_rule() {
        Rule::value => {
            let as_str = next.as_str();
            let token = next.into_inner().next().unwrap();
            value = match token.as_rule() {
                Rule::template_body => serde_json::json!(token.as_str()),
                Rule::slim_template_body => {
                    serde_json::json!(token.as_str())
                }
                Rule::string => serde_json::json!(token.as_str()),
                _ => upcast_string(as_str.trim()),
            };
            Arc::new(AttrCommandPrerenderedAssigner { name, value })
        }
        _ => unreachable!(),
    }
}

fn parse_entity_tags(pair: Pairs<Rule>, mut class_builder: RefMut<ClassBuilder>) {
    for inner_pair in pair {
        match inner_pair.as_rule() {
            Rule::tag_body_body => {
                class_builder
                    .borrow_mut()
                    .html_body(inner_pair.as_str().to_string());
            }
            Rule::tag_header_body => {
                class_builder
                    .borrow_mut()
                    .html_header(inner_pair.as_str().to_string());
            }
            Rule::tag_html_body => {}
            // TODO: implement all tags
            _ => {}
        }
    }
}

fn parse_entity(instance: &mut SandboxInstance, pair: Pair<Rule>) -> Result<Class> {
    let class_builder = RefCell::new(ClassBuilder::new());
    pair.into_inner()
        .for_each(|inner_pair| match inner_pair.as_rule() {
            Rule::entity_declaration => {
                let mut inner = inner_pair.into_inner();
                class_builder
                    .borrow_mut()
                    .name(inner.next().unwrap().as_str());
                if let Some(parent) = inner.next() {
                    class_builder
                        .borrow_mut()
                        .extends(instance, parent.as_str());
                }
            }
            Rule::subclasses => parse_subclasses(inner_pair, class_builder.borrow_mut()),
            Rule::pop_an_entity => {
                parse_entity_attribute::<AttrCommandUseEntity>(
                    inner_pair,
                    class_builder.borrow_mut(),
                );
            }
            Rule::pick_an_entity => {
                parse_entity_attribute::<AttrCommandPickEntity>(
                    inner_pair,
                    class_builder.borrow_mut(),
                );
            }
            Rule::roll_an_entity => {
                parse_entity_attribute::<AttrCommandRollEntity>(
                    inner_pair,
                    class_builder.borrow_mut(),
                );
            }
            Rule::roll_one_of => {
                parse_entity_attribute::<AttrCommandRollEntity>(
                    inner_pair,
                    class_builder.borrow_mut(),
                );
            }
            Rule::roll_from_indirect => {
                parse_entity_attribute::<AttrCommandRollEntity>(
                    inner_pair,
                    class_builder.borrow_mut(),
                );
            }
            Rule::roll_a_dice => {
                parse_attribute_ex(parse_dice_notation, inner_pair, class_builder.borrow_mut());
            }
            Rule::roll_from_list => {
                parse_attribute_ex(parse_roll_from_list, inner_pair, class_builder.borrow_mut());
            }
            Rule::roll_from_global => {
                parse_attribute_ex(
                    parse_roll_via_variable,
                    inner_pair,
                    class_builder.borrow_mut(),
                );
            }
            Rule::context_assignment => {
                parse_attribute_ex(parse_context, inner_pair, class_builder.borrow_mut());
            }
            Rule::assignment => {
                parse_attribute_ex(parse_simple_value, inner_pair, class_builder.borrow_mut());
            }
            Rule::weak_assignment => {
                parse_attribute_ex(parse_weak_value, inner_pair, class_builder.borrow_mut());
            }
            Rule::inheritance => {
                class_builder.borrow_mut().expand(
                    instance,
                    inner_pair.into_inner().next().unwrap().as_str().trim(),
                );
            }
            Rule::collect => {
                parse_collection(inner_pair, class_builder.borrow_mut());
            }
            Rule::tags => {
                parse_entity_tags(inner_pair.into_inner(), class_builder.borrow_mut());
            }
            Rule::prerendered_assignment => {
                parse_attribute_ex(
                    parse_prerendered_value,
                    inner_pair,
                    class_builder.borrow_mut(),
                );
            }
            Rule::declaration => {}
            _ => unreachable!(),
        });
    class_builder.borrow_mut().conclude(instance);
    Ok(class_builder.into_inner().build())
}

fn parse_scroll(
    instance: &mut SandboxInstance,
    pairs: Pairs<Rule>,
    include_path: &str,
) -> Result<()> {
    for pair in pairs {
        match pair.as_rule() {
            Rule::variable_definition => {
                let mut inner = pair.into_inner();
                let var = inner.next().unwrap().as_str();
                let mut val: Option<serde_json::Value> = None;

                let mut list: Vec<serde_json::Value> = Vec::new();

                let mut prob = ProbabilityHelper::new();
                for inner_pair in inner {
                    match inner_pair.as_rule() {
                        Rule::value => val = Some(upcast_string(inner_pair.as_str().trim())),
                        Rule::probability_spec => {
                            prob.parse_multiplier(&inner_pair);
                        }
                        Rule::list_value => prob.multiply(|| {
                            list.push(serde_json::to_value(inner_pair.as_str().trim()).expect("x"))
                        }),
                        _ => unreachable!(),
                    }
                }

                if let Some(inner_val) = val {
                    instance.globals.insert(var.to_string(), inner_val);
                } else {
                    instance
                        .globals
                        .insert(var.to_string(), serde_json::json!(list));
                }
            }
            Rule::entity_definition => match parse_entity(instance, pair) {
                Ok(ret) => {
                    instance.classes.insert(ret.name.to_owned(), ret);
                }
                Err(e) => return Err(e),
            },
            Rule::include_stmt => {
                let mut ip = pair.into_inner();
                let what = ip.next().unwrap().as_str();
                let path = String::from_str(include_path).unwrap() + what + ".scroll";
                log::info!("importing {}", path);
                let unparsed_file = std::fs::read_to_string(path.clone())?;
                parse_buffer(instance, &unparsed_file, Some(include_path), Some(&path))?;
            }
            Rule::EOI => {}
            _ => unreachable!(),
        }
    }
    Ok(())
}
