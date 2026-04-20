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

// Generate dungeon and cave maps, and then generate the needed `scroll`
// model, tailored for each map, to generate rooms, caverns and content.
// The generated scroll model is stored in the sandbox is can be restored
// to correctly re-roll entities.

use anyhow::Result;
use rand::{SeedableRng, rngs::StdRng};
use serde_json::Value;
use std::str::FromStr;

use hexroll3_dungeoneer::{
    cave::{CaveBuilder, Point},
    dungeon::{DungeonBuilder, Randomizers},
};
use hexroll3_scroll::{instance::*, repository::*};

use crate::watabou::ValueUuidExt;

pub fn map_data_providers() -> fn(
    &hexroll3_scroll::instance::SandboxBuilder<'_>,
    &mut hexroll3_scroll::instance::SandboxBlueprint,
    &mut hexroll3_scroll::repository::ReadWriteTransaction<'_>,
    &str,
) -> Result<
    std::option::Option<(std::string::String, Value)>,
    anyhow::Error,
> {
    return |builder, mut blueprint, tx, class_name| {
        match class_name {
            "CaveMap" => Some(prep_cave_map(builder, &mut blueprint, tx)),
            "DungeonMap" => Some(prep_dungeon_map(builder, &mut blueprint, tx)),
            _ => None,
        }
        .transpose()
    };
}

pub fn prep_dungeon_map(
    builder: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    tx: &mut ReadWriteTransaction,
) -> Result<(String, Value)> {
    let rng = builder.randomizer.u64();
    let dungeon_randomizers = Randomizers::new(rng);
    let mut dungeon_builder = DungeonBuilder::new(dungeon_randomizers);
    dungeon_builder.excavate_dungeon(false);
    let json = dungeon_builder.as_json();
    let map_class_name = format!("DungeonMap_{}", builder.randomizer.uid());

    let mut class_def = String::new();

    class_def.push_str(&format!("{map_class_name} (DungeonMap) {{\n"));

    dungeon_builder.for_each_room(|room, portals, area_number| {
        let mut area_class_def = String::new();
        let area_class_name =
            format!("DungeonArea_{}", builder.randomizer.uid());
        area_class_def
            .push_str(&format!("{area_class_name} (RoomDescription)"));
        area_class_def.push_str(" {\n");
        area_class_def.push_str(&format!("RoomNumber = {area_number}\n"));

        area_class_def.push_str(&format!(
            "x_coords = {}\n",
            (room.area.a().x * 10 + room.area.b().x * 10) / 2
        ));
        area_class_def.push_str(&format!(
            "y_coords = {}\n",
            (room.area.a().y * 10 + room.area.b().y * 10) / 2
        ));

        let mut n_door = 1;
        let mut n_secret_door = 1;
        let mut n_passage = 1;

        for p in portals.iter() {
            if p.type_ == 0 {
                area_class_def
                    .push_str(&format!("door_{} @ DungeonDoor {{\n", n_door));
                area_class_def.push_str(&format!("Wall := {}\n}}\n", p.wall));
                n_door += 1;
            }
            if p.type_ == 1 {
                area_class_def.push_str(&format!(
                    "passage_{} @ DungeonPassage {{\n",
                    n_passage
                ));
                area_class_def.push_str(&format!(
                    "Wall := {}\n Room := {} \n }}\n",
                    p.wall, p.room_number
                ));
                n_passage += 1;
            }
            if p.type_ == 2 {
                area_class_def.push_str(&format!(
                    "secret_door_{} @ DungeonSecretDoor {{\n",
                    n_secret_door
                ));
                area_class_def.push_str(&format!("Wall := {}\n}}\n", p.wall));
                n_secret_door += 1;
            }
        }
        let feature_tier = match room.feature_tier {
            2 => "DungeonFeatureTier2",
            3 => "DungeonFeatureTier3",
            4 => "DungeonFeatureTier4",
            _ => "DungeonFeatureTier1",
        };
        area_class_def.push_str("FeatureLevelClass = :Dungeon.");
        area_class_def.push_str(feature_tier);
        area_class_def.push_str("\n");
        area_class_def.push_str("  | RoomDescription\n");

        if room.feature_tier == 4 {
            area_class_def.push_str("RoomType @ RoomTypeThrone \n");
        }

        area_class_def.push_str("}\n");

        blueprint.parse_buffer(&area_class_def);
        tx.store(&area_class_name, &Value::String(area_class_def))
            .expect("Error storing entity");

        class_def
            .push_str(&format!("  $room_{area_number} @ {area_class_name} \n"));
    });

    dungeon_builder.for_each_corridor(|room, area_number| {
        let mut area_class_def = String::new();
        let area_class_name =
            format!("DungeonCorridor_{}", builder.randomizer.uid());
        area_class_def
            .push_str(&format!("{area_class_name} (CorridorDescription)"));
        area_class_def.push_str(" {\n");
        area_class_def.push_str(&format!("RoomNumber = {area_number}\n"));

        area_class_def.push_str(&format!(
            "x_coords = {}\n",
            (room.area.a().x * 10 + room.area.b().x * 10) / 2
        ));
        area_class_def.push_str(&format!(
            "y_coords = {}\n",
            (room.area.a().y * 10 + room.area.b().y * 10) / 2
        ));
        if let Some(entrance) = room.entrance {
            area_class_def.push_str("Entrance @ DungeonEntrance {\n");
            area_class_def.push_str("  AreaUUID = &uuid\n");
            if entrance.type_ == 0 {
                area_class_def
                    .push_str("  EntranceLocationPrefix = <in the dungeon are>\n");
                area_class_def.push_str(
                    "  EntranceDescLeadingTo = <Stairs leading down into area>\n",
                );
            } else {
                area_class_def
                    .push_str("  EntranceLocationPrefix = <in the dungeon is>\n");
                area_class_def.push_str(
                    "  EntranceDescLeadingTo = <A trapdoor leading down into area>\n",
                );
            }
            area_class_def.push_str(&format!("  AreaNumber = {area_number}\n"));
            area_class_def.push_str("}\n");
        }
        if let Some(_deadend) = room.deadend {
            area_class_def.push_str("Deadend @ DungeonDeadEndFeature {\n");
            area_class_def.push_str("  AreaUUID := &uuid\n");
            area_class_def.push_str(&format!("  AreaNumber := {area_number}\n"));
            area_class_def.push_str("}\n");
        }
        area_class_def.push_str(&format!("Length = {}\n",room.area.size()));
        area_class_def.push_str("  | CorridorDescription\n}\n");

        blueprint.parse_buffer(&area_class_def);
        tx.store(&area_class_name, &Value::String(area_class_def))
            .expect("Error storing entity");
        class_def
            .push_str(&format!("  $room_{area_number} @ {area_class_name} \n"));

    });

    class_def.push_str("}\n");
    blueprint.parse_buffer(&class_def);
    tx.store(&map_class_name, &Value::String(class_def))?;

    Ok((map_class_name.to_string(), json))
}

pub fn prep_cave_map(
    builder: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    tx: &mut ReadWriteTransaction,
) -> Result<(String, Value)> {
    let prefer_smaller_dungeons = false;

    let mut rng = StdRng::seed_from_u64(builder.randomizer.u64());
    let cave_builder = CaveBuilder::new(&mut rng, prefer_smaller_dungeons);
    let json = cave_builder.as_json();

    let map_class_name = format!("DungeonMap_{}", builder.randomizer.uid());

    let mut class_def = String::new();

    class_def.push_str(&format!("{map_class_name} (DungeonMap)"));
    class_def.push_str(" {\n");
    let mut entrances_budget = 1;
    let mut area_number = 1;
    for cavern in cave_builder.caverns {
        let mut cavern_class_def = String::new();
        let cavern_class_name =
            format!("DungeonArea_{}", builder.randomizer.uid());
        cavern_class_def
            .push_str(&format!("{cavern_class_name} (CaveDescription)"));
        cavern_class_def.push_str(" {\n");
        cavern_class_def.push_str(&format!("RoomNumber = {area_number}\n"));

        let centroid = find_centroid(&cavern.polygon);

        cavern_class_def.push_str(&format!("x_coords = {}\n", centroid.x));
        cavern_class_def.push_str(&format!("y_coords = {}\n", centroid.y));

        if cavern.is_outer
            && builder.randomizer.in_range(1, 2) == 1
            && entrances_budget > 0
        {
            entrances_budget -= 1;
            cavern_class_def.push_str("Entrance @ DungeonEntrance {\n");
            cavern_class_def.push_str("  AreaUUID = &uuid\n");
            cavern_class_def
                .push_str("  EntranceLocationPrefix = <in the cave is>\n");
            cavern_class_def.push_str(
                "  EntranceDescLeadingTo = <A passage leading into area>\n",
            );
            cavern_class_def
                .push_str(&format!("  AreaNumber = {area_number}\n"));
            cavern_class_def.push_str("}\n");
        }
        let feature_tier = match cavern.n_hexes {
            n if n > 50 => "DungeonFeatureTier4",
            n if n > 30 => "DungeonFeatureTier3",
            n if n > 15 => "DungeonFeatureTier2",
            _ => "DungeonFeatureTier1",
        };

        cavern_class_def.push_str("FeatureLevelClass = :Dungeon.");
        cavern_class_def.push_str(feature_tier);
        cavern_class_def.push_str("\n");
        cavern_class_def.push_str("  | CaveDescription\n");
        cavern_class_def.push_str("}\n");

        blueprint.parse_buffer(&cavern_class_def);
        tx.store(&cavern_class_name, &Value::String(cavern_class_def))?;

        class_def.push_str(&format!(
            "  $room_{area_number} @ {cavern_class_name} \n"
        ));
        area_number += 1;
    }
    class_def.push_str("}\n");
    blueprint.parse_buffer(&class_def);
    tx.store(&map_class_name, &Value::String(class_def))?;

    Ok((map_class_name.to_string(), json))
}

pub fn prepare_dungeon_data(
    tx: &mut ReadWriteTransaction,
    uid: &str,
) -> anyhow::Result<String> {
    let dungeon_hex = tx.retrieve(uid)?;
    let dungeon_map =
        tx.retrieve(dungeon_hex.value["Dungeon"].uuid_as_str())?;
    let dungeon = tx.retrieve(dungeon_map.value["map"].uuid_as_str())?;
    let data = dungeon.value["map_data"].as_str().unwrap();

    let mut json = serde_json::Value::from_str(&data).unwrap();
    if let Some(caverns) =
        json.get_mut("caverns").and_then(|v| v.as_array_mut())
    {
        for entry in caverns.iter_mut() {
            let n = entry["n"].as_u64().unwrap();

            let uuid = dungeon.value[&format!("$room_{}", n)].uuid_as_value();

            if let Some(obj) = entry.as_object_mut() {
                obj.entry("uuid".to_string())
                    .or_insert_with(|| uuid.clone());
            }
        }
    }
    struct CollectedPortal {
        uuid: String,
        x: i32,
        y: i32,
    }
    let mut collected_portals: Vec<CollectedPortal> = Vec::new();
    if let Some(areas) = json.get_mut("areas").and_then(|v| v.as_array_mut()) {
        for area in areas.iter_mut() {
            let n = area["n"].as_u64().unwrap();

            let uuid = dungeon.value[&format!("$room_{}", n)].uuid_as_value();
            let uuid_str = dungeon.value[&format!("$room_{}", n)].uuid_as_str();

            if let Some(obj) = area.as_object_mut() {
                obj.entry("uuid".to_string())
                    .or_insert_with(|| uuid.clone());
            }

            let area_entity = tx.retrieve(&uuid_str)?;
            if let Some(portals) = area["portals"].as_array_mut() {
                let mut n = 0;
                let mut sn = 0;
                for portal in portals.iter_mut() {
                    let key = if portal["type"].as_i64().unwrap() == 2 {
                        sn += 1;
                        format!("secret_door_{sn}")
                    } else {
                        n += 1;
                        format!("door_{n}")
                    };
                    if area_entity.value.as_object().unwrap().contains_key(&key)
                    {
                        let portal_uuid =
                            area_entity.value[&key].uuid_as_value();
                        portal["uuid"] = portal_uuid.clone();
                        collected_portals.push(CollectedPortal {
                            uuid: portal_uuid.as_str().unwrap().to_string(),
                            x: portal["x"].as_i64().unwrap() as i32,
                            y: portal["y"].as_i64().unwrap() as i32,
                        });
                    }
                }
            }
        }
        for cp in collected_portals.iter() {
            let mut door_entity = tx.retrieve(&cp.uuid)?;
            if let Some(obj) = door_entity.value.as_object() {
                if !obj.contains_key("DescriptionOpposite") {
                    door_entity.value["DescriptionOpposite"] =
                        serde_json::Value::Null;
                    tx.store(&cp.uuid, &door_entity.value)?;
                }
            }
        }
        for area in areas.iter_mut() {
            let uuid_str = area["uuid"].as_str().unwrap();
            let mut area_entity = tx.retrieve(&uuid_str)?;
            area_entity.value["doors"] = serde_json::Value::Array(vec![]);
            if let Some(_is_passage) = area["t"].as_i64() {
                for cp in collected_portals.iter() {
                    let x = area["x"].as_i64().unwrap() as i32;
                    let y = area["y"].as_i64().unwrap() as i32;
                    let w = area["w"].as_i64().unwrap() as i32;
                    let h = area["h"].as_i64().unwrap() as i32;
                    if cp.x >= x
                        && cp.x <= x + w - 1
                        && cp.y >= y
                        && cp.y <= y + h - 1
                    {
                        area_entity.value["doors"]
                            .as_array_mut()
                            .unwrap()
                            .push(cp.uuid.clone().into());
                        tx.store(&uuid_str, &area_entity.value)?;
                    }
                }
            }
        }
    }

    let data = json.to_string();
    Ok(data)
}

pub fn find_centroid(v: &[Point]) -> Point {
    let n = v.len();
    debug_assert!(n >= 3);

    let mut ret = Point { x: 0, y: 0, c: 0 };
    let mut signed_area = 0_i32;

    for i in 0..n {
        let p0 = &v[i];
        let p1 = &v[(i + 1) % n];

        let a = (p0.x * p1.y) - (p1.x * p0.y);
        signed_area += a;

        ret.x += (p0.x + p1.x) * a;
        ret.y += (p0.y + p1.y) * a;
    }

    signed_area /= 2;
    let denom = 6 * signed_area;

    ret.x /= denom;
    ret.y /= denom;

    ret
}
