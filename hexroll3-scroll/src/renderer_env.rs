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
#![allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]

use caith::RollResultType;
use minijinja::Environment;
use rand::SeedableRng;
use std::{
    cmp::max,
    collections::{HashMap, HashSet},
};

use crate::{instance::SandboxInstance, repository::ReadOnlyLoader};

pub fn prepare_renderer(env: &mut Environment, instance: &SandboxInstance) {
    env.add_filter("bulletize", func_bulletize);
    env.add_filter("count_identical", func_count_identical);

    env.add_function("appender", func_appender);
    env.add_function("articlize", func_articlize);
    env.add_function("capitalize", func_capitalize);
    env.add_function("currency", func_currency(/*instance*/));
    env.add_function("first", func_first);
    env.add_function("float", func_float);
    env.add_function("hex_coords", func_hex_coords(instance));
    env.add_function("if_plural_else", func_if_plural_else);
    env.add_function("int", func_int);
    env.add_function("length", func_length);
    env.add_function("list_to_obj", func_list_to_obj);
    env.add_function("max", func_max);
    env.add_function("maybe", func_maybe);
    env.add_function("maybe2", func_maybe2);
    env.add_function("plural", func_plural);
    env.add_function("plural_with_count", func_plural_with_count);
    env.add_function("round", func_round);
    env.add_function("sandbox", func_sandbox(instance));
    env.add_function("sortby", func_sortby);
    env.add_function("dice", func_unstable_dice);
    env.add_function("stable_dice", func_stable_dice);
    env.add_function("sum", func_sum);
    env.add_function("title", func_capitalize);
    env.add_function("trim", func_trim);
    env.add_function("unique", func_unique);
    env.add_function("html_link", func_html_link(instance));
    env.add_function("reroller", func_reroll);
    env.add_function("toc_breadcrumb", func_toc(instance));
    env.add_function("sandbox_breadcrumb", func_map);
    env.add_function("dice_roller", func_dice_roller);
    env.add_function("nobrackets", func_nobrackets);
    env.add_function("opposite", func_opposite);

    // unimplemented
    env.add_function("begin_spoiler", func_nop_0);
    env.add_function("end_spoiler", func_nop_0);
    env.add_function("note_button", func_nop_1);
    env.add_function("note_container", func_nop_1);
}

fn func_bulletize(value: Vec<String>, seperator: &str) -> String {
    value.join(&format!(" &#{}; ", seperator)).to_string()
}

fn func_articlize(
    value: minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<String, minijinja::Error> {
    if let Some(noun) = value.as_str() {
        fn is_plural(noun: &str) -> bool {
            noun.ends_with('s')
                && noun != "bus"
                && noun != "grass"
                && noun != "kiss"
        }
        fn starts_with_vowel_sound(word: &str) -> bool {
            let vowels = ["a", "e", "i", "o", "u"];
            if let Some(first_char) = word.chars().next() {
                vowels.contains(&first_char.to_lowercase().to_string().as_str())
            } else {
                false
            }
        }
        let article = if is_plural(noun) {
            return Ok(String::from(noun));
        } else if starts_with_vowel_sound(noun) {
            "an"
        } else {
            "a"
        };
        Ok(format!("{} {}", article, noun))
    } else {
        Err(minijinja::Error::new(
            minijinja::ErrorKind::UndefinedError,
            "Function articlize received a non-string value",
        ))
    }
}

fn func_capitalize(
    possible_str: minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<minijinja::value::Value, minijinja::Error> {
    if let Some(v) = possible_str.as_str() {
        let mut v = v.to_string();
        if !v.is_empty() {
            v[0..1].make_ascii_uppercase(); // Capitalize the first character
        }
        Ok(minijinja::Value::from(v))
    } else {
        let t: serde_json::Value = possible_str.clone();
        Ok(minijinja::value::Value::from_serialize(&t))
    }
}

fn func_currency(// _instance: &SandboxInstance,
) -> impl Fn(
    minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<String, minijinja::Error> {
    let currency_factor = 1.0;
    move |v: minijinja::value::ViaDeserialize<serde_json::Value>| -> Result<String, minijinja::Error> {
        if let Some(v) = v.as_f64() {
            let v = v * currency_factor;
            if v > 1.0 {
                return Ok(format!("{} gp", format_with_commas(v as i64)));
            }
            if v > 0.1 {
                return Ok(format!("{:.0} sp", (v * 10.0).round() as i64));
            }
            if v > 0.01 {
                return Ok(format!("{:.0} cp", (v * 100.0).round() as i64));
            }
            return Ok(format!("{:.0} gp", v as i64));
        }
        Err(minijinja::Error::new(
            minijinja::ErrorKind::UndefinedError,
            "Currency value is not floating point",
        ))
    }
}

fn func_first(
    possible_str: minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<minijinja::value::Value, minijinja::Error> {
    if let Some(v) = possible_str.as_array() {
        if let Some(first) = v.iter().next() {
            return Ok(minijinja::Value::from_serialize(first.clone()));
        }
    } else if let Some(v) = possible_str.as_str() {
        if let Some(first) = v.chars().next() {
            return Ok(minijinja::Value::from_serialize(first));
        }
    }
    Err(minijinja::Error::new(
        minijinja::ErrorKind::UndefinedError,
        "func_first could not pick the first item from an array",
    ))
}

fn func_float(value: &str) -> Result<f32, minijinja::Error> {
    if let Ok(value) = value.trim().parse::<f32>() {
        Ok(value)
    } else {
        Err(minijinja::Error::new(
            minijinja::ErrorKind::UndefinedError,
            "Unable to convert value in func_float",
        ))
    }
}

fn func_int(value: &str) -> Result<i64, minijinja::Error> {
    if let Ok(value) = value.trim().parse::<i64>() {
        Ok(value)
    } else {
        Err(minijinja::Error::new(
            minijinja::ErrorKind::UndefinedError,
            "Unable to convert value in func_int",
        ))
    }
}

fn func_length(
    c: minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<usize, minijinja::Error> {
    if let Some(r) = c.as_array() {
        Ok(r.len())
    } else {
        Ok(0)
    }
}

fn func_list_to_obj(
    list: minijinja::value::ViaDeserialize<serde_json::Value>,
    attr_name: &str,
) -> Result<minijinja::value::Value, minijinja::Error> {
    let mut map = serde_json::json!({});
    if let Some(list) = list.as_array() {
        for item in list.iter() {
            if let Some(key) = item.as_object().unwrap().get(attr_name) {
                if let Some(key_str) = key.as_str() {
                    let m = &mut map.as_object_mut().unwrap();
                    if !m.contains_key(key_str) {
                        m.insert(key_str.to_string(), serde_json::json!([]));
                    }
                    m[key_str].as_array_mut().unwrap().push(item.clone());
                }
            }
        }
    }
    Ok(minijinja::value::Value::from_serialize(map))
}

fn func_count_identical(list: Vec<String>) -> HashMap<String, i32> {
    let mut counts = HashMap::new();
    for item in list {
        *counts.entry(item).or_insert(0) += 1;
    }
    counts
}

fn func_trim(
    _c: minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<String, minijinja::Error> {
    if let Some(value) = _c.as_str() {
        return Ok(clean_string(value.to_string()));
    }
    Err(minijinja::Error::new(
        minijinja::ErrorKind::UndefinedError,
        "Function trim did not get a string",
    ))
}

fn func_nobrackets(
    _c: minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<String, minijinja::Error> {
    if let Some(value) = _c.as_str() {
        return Ok(value.to_string());
    }
    Err(minijinja::Error::new(
        minijinja::ErrorKind::UndefinedError,
        "Function trim did not get a string",
    ))
}

fn func_sortby(
    list: minijinja::value::ViaDeserialize<serde_json::Value>,
    attr_to_sortby: &str,
) -> Result<minijinja::value::Value, minijinja::Error> {
    let mut ret = serde_json::json!([]);
    if let Some(list) = list.as_array() {
        let mut list_to_sort = list.clone();
        list_to_sort.sort_by(|a, b| {
            let a_value =
                a.get(attr_to_sortby).and_then(|v| v.as_str()).unwrap_or("");
            let b_value =
                b.get(attr_to_sortby).and_then(|v| v.as_str()).unwrap_or("");
            a_value.cmp(b_value)
        });
        ret = serde_json::Value::Array(list_to_sort.to_vec());
    }
    Ok(minijinja::value::Value::from_serialize(ret))
}

fn func_hex_coords(
    instance: &SandboxInstance,
) -> impl Fn(
    minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<String, minijinja::Error>
+ 'static {
    let repo = instance.repo.clone();
    move |uid: minijinja::value::ViaDeserialize<serde_json::Value>| -> Result<String, minijinja::Error> {
        if let Ok(tmpl) = repo.inspect(|tx|{
            // FIXME: There was a crash on the next line - need to investigate

            // protect against the unwrap() panicking
            let Some(uid) = uid.as_str() else {
                return Ok(format!("uid error in {:?}", uid.0).into())
            };
            let obj = tx.retrieve(uid)?;

            let x = obj.value["$coords"]["x"].as_i64().unwrap_or(0) as i32;
            let y = obj.value["$coords"]["y"].as_i64().unwrap_or(0) as i32;

            let x_dir = if x > 0 {
                "E"
            } else if x < 0 {
                "W"
            } else {
                ""
            };
            let y_dir = if y > 0 {
                "S"
            } else if y < 0 {
                "N"
            } else {
                ""
            };

            let abs_x = x.abs();
            let abs_y = y.abs();

            let tmpl = if x != 0 && y != 0 {
                format!("{}{}{}{}", x_dir, abs_x, y_dir, abs_y)
            } else if x != 0 {
                format!("{}{}", x_dir, abs_x)
            } else if y != 0 {
                format!("{}{}", y_dir, abs_y)
            } else {
                "BASE".to_string()
            };
            Ok(tmpl)
        }) {
            Ok(tmpl)
        } else {
            Ok("[unknown]".to_string())
        }
    }
}

fn func_maybe(
    v: minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<String, minijinja::Error> {
    if let Some(s) = v.as_str() {
        return Ok(s.to_string());
    }
    Ok(String::new())
}

fn func_maybe2(
    p: &str,
    v: minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<String, minijinja::Error> {
    if let Some(s) = v.as_str() {
        let mut p = p.to_string();
        p.push_str(s);
        return Ok(p);
    }
    Ok(String::new())
}

fn func_sandbox(
    instance: &SandboxInstance,
) -> impl Fn() -> Result<String, minijinja::Error> + 'static
// where
//     'a: 'static,
{
    let sid = instance.sid.clone().unwrap_or_default();
    move || -> Result<String, minijinja::Error> {
        Ok(format!("/inspect/{}", sid))
    }
}

fn func_round(value: f32, _dec: f32) -> Result<f32, minijinja::Error> {
    let y = (value * 100.0).round() / 100.0;
    if false {
        return Err(minijinja::Error::new(
            minijinja::ErrorKind::UndefinedError,
            "",
        ));
    }
    Ok(y)
}

fn func_max(a: i32, b: i32) -> Result<i32, minijinja::Error> {
    if false {
        return Err(minijinja::Error::new(
            minijinja::ErrorKind::UndefinedError,
            "",
        ));
    }
    Ok(max(a, b))
}

fn func_opposite(v: &str) -> &'static str {
    match v {
        "E" => "W",
        "W" => "E",
        "S" => "N",
        "N" => "S",
        _ => "?",
    }
}

fn func_appender(parent_uid: &str, attr: &str, cls: &str) -> String {
    #[cfg(feature = "testbed")]
    let html = format!(
        r#"
        <a href="/append/{parent_uid}/{attr}/{cls}">⊞</a>
        "#
    );

    #[cfg(not(feature = "testbed"))]
    let html = format!(
        r#"<a class="btn-icon" data-uuid="{parent_uid}" data-attr="{attr}" data-type="{cls}">
           <i class="fa-solid fa-circle-plus"></i></a>"#
    );
    html
}

fn func_dice_roller(dice_str: &str, label: &str) -> String {
    format!(
        r#"<a class="btn-spawn-dice" data-dice='{dice_str}'
           onclick="javascript:window.app.spawn_dice('{dice_str}');">
           <strong>{label}</strong></a>"#
    )
}

fn func_plural(count: f32, v: &str) -> Result<String, minijinja::Error> {
    if count <= 1.0 {
        return Ok(v.to_string());
    }
    let mut plural = v.to_string();
    let c = v.chars().last().unwrap_or('\0');
    let c_minus_1 = v.chars().rev().nth(1).unwrap_or('\0');

    if "sxzh".contains(c) {
        plural.push_str("es");
    } else if c == 'y' {
        if "aeiou".contains(c_minus_1) {
            plural.push('s');
        } else {
            plural.pop();
            plural.push_str("ies");
        }
    } else if v.ends_with("olf") {
        plural.pop();
        plural.push_str("ves");
    } else {
        plural.push('s');
    }
    Ok(plural)
}

fn func_plural_with_count(
    count: f32,
    v: &str,
) -> Result<String, minijinja::Error> {
    if count <= 1.0 {
        return Ok(v.to_string());
    }
    Ok(format!(
        "{} {}",
        count as i32,
        func_plural(count, v).unwrap()
    ))
}

fn func_if_plural_else(
    check: &str,
    ifplural: &str,
    ifnotplural: &str,
) -> Result<String, minijinja::Error> {
    let check = check.to_lowercase();
    if check.ends_with('s') || check == "teeth" || check == "wolves" {
        Ok(ifplural.to_string())
    } else {
        Ok(ifnotplural.to_string())
    }
}

fn func_sum(l: minijinja::value::ViaDeserialize<serde_json::Value>) -> f64 {
    let mut sum = 0.0;
    for v in l.as_array().unwrap() {
        if let Ok(a) = v.as_str().unwrap().trim().parse::<f64>() {
            sum += a;
        }
    }
    sum
}

fn func_unique(
    v: minijinja::value::ViaDeserialize<serde_json::Value>,
    attr: minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<minijinja::value::Value, minijinja::Error> {
    let mut ret = serde_json::json!([]);
    let mut unique_set = HashSet::new();

    if let Some(v) = v.as_array() {
        for e in v.iter() {
            if let Some(value) =
                e.as_object().unwrap().get(attr.as_str().unwrap())
            {
                if !unique_set.contains(value) {
                    ret.as_array_mut().unwrap().push(e.clone());
                    unique_set.insert(value.clone());
                }
            }
        }
        return Ok(minijinja::value::Value::from_serialize(&ret));
    }
    Ok(minijinja::value::Value::from_serialize(serde_json::json!(
        {}
    )))
}

fn func_unstable_dice(roll: &str) -> Result<i32, minijinja::Error> {
    if false {
        return Err(minijinja::Error::new(
            minijinja::ErrorKind::UndefinedError,
            "",
        ));
    }
    let roller = caith::Roller::new(roll).unwrap();
    let mut rng = rand::thread_rng();
    if let RollResultType::Single(value) =
        roller.roll_with(&mut rng).unwrap().get_result()
    {
        return Ok(value.get_total() as i32);
    }
    Ok(0)
}

fn func_stable_dice(
    roll: &str,
    uid: &str,
    index: u64,
) -> Result<i32, minijinja::Error> {
    if false {
        return Err(minijinja::Error::new(
            minijinja::ErrorKind::UndefinedError,
            "",
        ));
    }
    let roller = caith::Roller::new(roll).unwrap();
    let seed = string_to_seed(uid) + index;
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    if let RollResultType::Single(value) =
        roller.roll_with(&mut rng).unwrap().get_result()
    {
        return Ok(value.get_total() as i32);
    }
    Ok(0)
}

fn func_html_link(
    instance: &SandboxInstance,
) -> impl Fn(&str, &str) -> Result<String, minijinja::Error> + 'static {
    let sid = match instance.sid.as_ref() {
        Some(sid) => sid.clone(),
        None => "".to_string(),
    };
    move |uid, text| -> Result<String, minijinja::Error> {
        Ok(format!(
            "<a href='/inspect/{}/entity/{}'>{}</a>",
            sid, uid, text
        ))
    }
}

fn func_map() -> Result<String, minijinja::Error> {
    Ok("<a class='breadcrumbs-icon' href='/sandbox/00000000'></a>".to_string())
}

fn func_toc(
    instance: &SandboxInstance,
) -> impl Fn() -> Result<String, minijinja::Error> + 'static {
    let sid = instance.sid.clone().unwrap_or_default();
    move || -> Result<String, minijinja::Error> {
        Ok(format!(
            "<a class='breadcrumbs-icon' href='/sandbox/{}/toc'></a>",
            sid
        ))
    }
}

fn func_nop_0() -> Result<String, minijinja::Error> {
    Ok(String::new())
}

fn func_nop_1(
    _: minijinja::value::ViaDeserialize<serde_json::Value>,
) -> Result<String, minijinja::Error> {
    Ok(String::new())
}

fn func_reroll(
    uid: minijinja::value::ViaDeserialize<serde_json::Value>,
    class: &str,
    reload: bool,
) -> Result<String, minijinja::Error> {
    let id = if let Some(obj) = uid.get("uuid") {
        obj.to_string().trim_matches('"').to_string()
    } else {
        uid.to_string().trim_matches('"').to_string()
    };
    let class = if class.is_empty() { "default" } else { class };
    #[cfg(feature = "testbed")]
    let html = Ok(format!(
        "<a href='/reroll/{}'>⟳</a><a href='/unroll/{}'>🗑</a>",
        id, id
    )
    .to_string());
    #[cfg(not(feature = "testbed"))]
    let html = Ok(format!(
        "<a class='btn-icon' data-uuid='{}' data-override='{}' {} href='/reroll/{}'>⟳</a>",
        id,
        class,
        if reload {
            "data-reload='true'"
        } else {""},

        id
    ).to_string());
    html
}

fn clean_string(mut s: String) -> String {
    s = s.trim().to_string();
    s.retain(|c| c != '\n' && c != '\r');
    s = s
        .chars()
        .fold((String::new(), None), |(mut acc, prev_char), c| {
            if c == ' ' && prev_char == Some(' ') {
                (acc, prev_char)
            } else {
                acc.push(c);
                (acc, Some(c))
            }
        })
        .0;
    s
}

fn format_with_commas(v: i64) -> String {
    let s = v.to_string();
    let mut formatted = String::new();
    let mut count = 0;

    for c in s.chars().rev() {
        if count == 3 {
            formatted.push(',');
            count = 0;
        }
        formatted.push(c);
        count += 1;
    }
    formatted.chars().rev().collect()
}

fn string_to_seed<S: AsRef<str>>(seed_str: S) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    std::hash::Hash::hash(&seed_str.as_ref(), &mut hasher);
    std::hash::Hasher::finish(&hasher)
}

#[cfg(test)]
mod tests {}
