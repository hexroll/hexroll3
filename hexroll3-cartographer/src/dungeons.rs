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
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use hexroll3_dungeoneer::{
    cave::{CaveBuilder, Point},
    dungeon::{DungeonBuilder, DungeonConfig, DungeonSize, Randomizers, Room},
};
use hexroll3_scroll::{generators::ParentContext, instance::*, repository::*};

use crate::watabou::ValueUuidExt;

pub fn map_data_providers() -> fn(
    &hexroll3_scroll::instance::SandboxBuilder<'_>,
    &mut hexroll3_scroll::instance::SandboxBlueprint,
    &mut hexroll3_scroll::repository::ReadWriteTransaction<'_>,
    &str,
    &mut ParentContext,
) -> Result<
    std::option::Option<(std::string::String, Value)>,
    anyhow::Error,
> {
    return |builder, mut blueprint, tx, class_name, mut parent_context| {
        match class_name {
            "CaveMap" => Some(prep_cave_map(
                builder,
                &mut blueprint,
                tx,
                &mut parent_context,
            )),
            "DungeonMap" => Some(prep_dungeon_map(
                builder,
                &mut blueprint,
                tx,
                &mut parent_context,
            )),
            _ => None,
        }
        .transpose()
    };
}

pub fn prep_dungeon_map(
    builder: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    tx: &mut ReadWriteTransaction,
    parent_context: &mut ParentContext,
) -> Result<(String, Value)> {
    let multi_floor_enabled =
        blueprint.classes.contains_key("DungeonFloorExit")
            && blueprint.classes.contains_key("DungeonFloorEntrance");
    // if parent_context.ordinal == 2 {
    //     return prep_cave_map(builder, blueprint, tx, parent_context);
    // }
    // -----------------------------------------------------------------------------------------
    let upper_floor: Vec<AreaHelper> =
        serde_json::from_value(parent_context.data.clone()).unwrap_or_default();
    // -----------------------------------------------------------------------------------------

    let rng = builder.randomizer.u64();
    let dungeon_randomizers = Randomizers::new(rng);
    let mut dungeon_builder = DungeonBuilder::new(dungeon_randomizers);
    dungeon_builder.excavate_dungeon(DungeonConfig {
        size: if parent_context.count > 2 {
            DungeonSize::Scale(match parent_context.ordinal {
                0 => 2,
                1 => 7,
                _ => 10,
            })
        } else {
            DungeonSize::Random
        },
    });
    let map_class_name = format!("DungeonMap_{}", builder.randomizer.uid());

    let mut class_def = String::new();
    let area_floor = parent_context.ordinal + 1;

    class_def.push_str(&format!("{map_class_name} (DungeonMap) {{\n"));
    class_def.push_str(&format!("Level = {}\n", area_floor));

    let mut context_rooms = Vec::new();
    dungeon_builder.for_each_room(|room, portals, area_number| {
        let area_uid = builder.randomizer.uid();

        context_rooms.push(AreaHelper::from_area(room, area_uid.clone()));
        let area_exit_class_name = format!("DungeonFloorExit_{}", area_uid);
        let area_exit_class_def = format!(
            "{area_exit_class_name} (DungeonFloorExit) {{ Details! ~ \"\"}}\n"
        );

        let mut area_class_def = String::new();
        let area_class_name = format!("DungeonArea_{}", area_uid);
        area_class_def
            .push_str(&format!("{area_class_name} (RoomDescription)"));
        area_class_def.push_str(" {\n");
        area_class_def.push_str(&format!("RoomNumber = {area_number}\n"));
        area_class_def.push_str(&format!("RoomFloor = {area_floor}\n"));

        if multi_floor_enabled {
            area_class_def
                .push_str(&format!("FloorExit @ {area_exit_class_name}\n"));
        }

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

        if multi_floor_enabled {
            blueprint.parse_buffer(&area_exit_class_def);
            tx.store(
                &area_exit_class_name,
                &Value::String(area_exit_class_def),
            )
            .expect("Error storing entity");
        }
        blueprint.parse_buffer(&area_class_def);
        tx.store(&area_class_name, &Value::String(area_class_def))
            .expect("Error storing entity");

        class_def
            .push_str(&format!("  $room_{area_number} @ {area_class_name} \n"));
    });

    // -----------------------------------------------------------------------------------
    // Provide our rooms to the next floor, so it can verify its entrances and match
    // against available upper floor room areas
    parent_context.data =
        serde_json::to_value(context_rooms).unwrap_or(Value::Null);
    // -----------------------------------------------------------------------------------

    let mut valid_e_entries = Vec::new();
    let mut entrances_counter = 0;
    let entrances_goal = dungeon_builder.entrances_goal;
    dungeon_builder.for_each_corridor_mut(|room, area_number| {
        let mut area_class_def = String::new();
        let area_class_name =
            format!("DungeonCorridor_{}", builder.randomizer.uid());
        area_class_def
            .push_str(&format!("{area_class_name} (CorridorDescription)"));
        area_class_def.push_str(" {\n");
        area_class_def.push_str(&format!("RoomNumber = {area_number}\n"));
        area_class_def.push_str(&format!("RoomFloor = {area_floor}\n"));

        area_class_def.push_str(&format!(
            "x_coords = {}\n",
            (room.area.a().x * 10 + room.area.b().x * 10) / 2
        ));
        area_class_def.push_str(&format!(
            "y_coords = {}\n",
            (room.area.a().y * 10 + room.area.b().y * 10) / 2
        ));
        if let Some(entrance) = room.entrance {
            if parent_context.ordinal == 0 {
                if entrances_counter < entrances_goal {
                    entrances_counter+=1;
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
                    area_class_def.push_str(&format!("  AreaFloor = {area_floor}\n"));
                    area_class_def.push_str("}\n");
                } else {
                    room.deadend = room.entrance;
                    room.entrance = None;
                }
            } else {
                // -----------------------------------------------------------------------------
                let mut found = false;
                if multi_floor_enabled {
                    for upper_floor_room in &upper_floor {
                        if upper_floor_room.is_pos_inside(entrance.pos.x, entrance.pos.y) {
                            area_class_def.push_str("FloorEntrance @ DungeonFloorEntrance\n");
                            valid_e_entries.push(entrance);
                            found = true;
                            let area_exit_class_name =
                                format!("DungeonFloorExit_{}", upper_floor_room.uid);
                            let area_exit_class_def =
                                format!("{area_exit_class_name} (DungeonFloorExit) {{ Details! ~ <% <li> There {{{{ExitType}}}} here, leading to the next floor.%>}}\n");
                            blueprint.parse_buffer(&area_exit_class_def);
                            tx.store(
                                &area_exit_class_name,
                                &Value::String(area_exit_class_def),
                            ).expect("Error storing entity");
                            break;
                        }
                    }
                }
                // NOTE: every room that has an entrance can double as a deadend if the entrance
                // is not materialized for some reason.
                if !found {
                    room.deadend = room.entrance;
                    room.entrance = None;
                }
                // -----------------------------------------------------------------------------
            }
        }
        if let Some(_deadend) = room.deadend {
            area_class_def.push_str("Deadend @ DungeonDeadEndFeature {\n");
            area_class_def.push_str("  AreaUUID := &uuid\n");
            area_class_def.push_str(&format!("  AreaNumber := {area_number}\n"));
            area_class_def.push_str(&format!("  AreaFloor := {area_floor}\n"));
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

    // ----------------------------------------------------------------------------------
    // Making it easy for upper floors to work through their stairs
    let mut json = dungeon_builder.as_json();
    if multi_floor_enabled {
        if parent_context.ordinal > 0 {
            let stairs: Vec<Value> = valid_e_entries
            .iter()
            .map(|v| serde_json::json!({"x":v.pos.x, "y":v.pos.y, "dir":v.dir}))
            .collect();
            json["stairs"] = serde_json::to_value(stairs).unwrap_or_default();
        }
    }
    // ----------------------------------------------------------------------------------

    Ok((map_class_name.to_string(), json))
}

pub fn prep_cave_map(
    builder: &SandboxBuilder,
    blueprint: &mut SandboxBlueprint,
    tx: &mut ReadWriteTransaction,
    parent_context: &mut ParentContext,
) -> Result<(String, Value)> {
    let prefer_smaller_dungeons = false;

    let mut rng = StdRng::seed_from_u64(builder.randomizer.u64());

    let mut attempts = 10;
    let cave_builder = loop {
        let cave_builder = CaveBuilder::new(&mut rng, prefer_smaller_dungeons);
        if cave_builder.caverns.len() > 5 {
            break cave_builder;
        }
        attempts -= 1;
        if attempts == 0 {
            return Err(anyhow::anyhow!("Could not generate a valid cave map"));
        }
    };

    let json = cave_builder.as_json();

    let map_class_name = format!("DungeonMap_{}", builder.randomizer.uid());

    let mut class_def = String::new();
    let area_floor = parent_context.ordinal + 1;

    class_def.push_str(&format!("{map_class_name} (DungeonMap) {{\n"));
    class_def.push_str(&format!("Level = {}\n", area_floor));

    let mut entrances_budget = builder.randomizer.in_range(3, 4);
    let mut area_number = 1;
    for cavern in cave_builder.caverns {
        let mut cavern_class_def = String::new();
        let cavern_class_name =
            format!("DungeonArea_{}", builder.randomizer.uid());
        cavern_class_def
            .push_str(&format!("{cavern_class_name} (CaveDescription)"));
        cavern_class_def.push_str(" {\n");
        cavern_class_def.push_str(&format!("RoomNumber = {area_number}\n"));
        cavern_class_def.push_str(&format!("RoomFloor = {area_floor}\n"));

        let centroid = find_centroid(&cavern.polygon);

        cavern_class_def.push_str(&format!("x_coords = {}\n", centroid.x));
        cavern_class_def.push_str(&format!("y_coords = {}\n", centroid.y));

        if cavern.is_outer
            && builder.randomizer.in_range(1, 5 - entrances_budget) == 1
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
            cavern_class_def.push_str(&format!("  AreaFloor = {area_floor}\n"));
            cavern_class_def.push_str("}\n");
        }
        let feature_tier = match cavern.n_hexes {
            n if n > 70 => "DungeonFeatureTier4",
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
    floor: usize,
) -> anyhow::Result<String> {
    let dungeon_hex = tx.retrieve(uid)?;
    let dungeon_map =
        tx.retrieve(dungeon_hex.value["Dungeon"].uuid_as_str())?;

    let map_uids = &dungeon_map.value["map"];
    let maybe_map_uid = map_uids
        .as_array()
        .and_then(|a| a.get(floor))
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Null);

    let Some(map_uid) = maybe_map_uid.as_str() else {
        return Err(anyhow::anyhow!(
            "Unable to find dungeon map id for floor {}",
            floor
        ));
    };
    let dungeon = tx.retrieve(map_uid)?;

    // NOTE: check if this is an early alpha sandbox.
    // FIXME: We may want to remove the string conversion at some point.
    let mut json = {
        let data = &dungeon.value["map_data"];
        if data.is_string() {
            serde_json::from_str(data.as_str().unwrap()).unwrap()
        } else {
            let data = &dungeon.value["$map_data"];
            if data.is_object() {
                data.clone()
            } else {
                return Err(anyhow::anyhow!("Could not find dungeon map data"));
            }
        }
    };

    let maybe_lower_stairs = next_floor_metadata(tx, map_uids, floor)?;

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
            let area_entity = tx.retrieve(&uuid_str)?;

            if let Some(obj) = area.as_object_mut() {
                obj.entry("uuid".to_string())
                    .or_insert_with(|| uuid.clone());

                stamp_potential_exit_from_area(&maybe_lower_stairs, obj);
            }

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

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
struct AreaHelper {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    uid: String,
}

impl AreaHelper {
    fn from_area(room: &Room, uid: String) -> Self {
        Self {
            x: room.area.a().x,
            y: room.area.a().y,
            w: room.area.width(),
            h: room.area.height(),
            uid,
        }
    }
    fn from_obj(obj: &Map<String, Value>) -> Self {
        Self {
            x: obj["x"].as_i64().unwrap_or_default() as i32,
            y: obj["y"].as_i64().unwrap_or_default() as i32,
            w: obj["w"].as_i64().unwrap_or_default() as i32,
            h: obj["h"].as_i64().unwrap_or_default() as i32,
            uid: "".to_string(),
        }
    }
    fn is_pos_inside(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && x <= self.x + self.w - 1
            && y <= self.y + self.h - 1
    }
}

fn next_floor_metadata(
    tx: &mut ReadWriteTransaction,
    map_attribute: &Value,
    floor: usize,
) -> anyhow::Result<Option<Vec<Value>>> {
    // Find potential lower floor stairs list. This is used to augment this floor
    // map with exits (currently inside room areas)
    let maybe_lower_map_uid = map_attribute
        .as_array()
        .and_then(|a| a.get(floor + 1))
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Null);
    let mut maybe_lower_stairs = None;
    if let Some(map_uid) = maybe_lower_map_uid.as_str() {
        let lower_floor = tx.retrieve(map_uid)?;
        let lower_floor_map_data = &lower_floor.value["$map_data"];
        if let Some(stairs) = lower_floor_map_data
            .get("stairs")
            .and_then(|v| v.as_array())
        {
            maybe_lower_stairs = Some(stairs.clone());
        }
    }
    Ok(maybe_lower_stairs)
}

fn stamp_potential_exit_from_area(
    maybe_lower_stairs: &Option<Vec<Value>>,
    obj: &mut Map<String, Value>,
) {
    // Potentially stamp this area with an augmented 'entrance' - which is,
    // effectively an exit to the next floor.
    if let Some(lower_stairs) = maybe_lower_stairs {
        for s in lower_stairs {
            let stairs_x = s["x"].as_i64().unwrap() as i32;
            let stairs_y = s["y"].as_i64().unwrap() as i32;
            let stairs_d = s["dir"].as_i64().unwrap();

            let area_helper = AreaHelper::from_obj(&obj);

            if area_helper.is_pos_inside(stairs_x, stairs_y) {
                obj.insert("e".to_string(), serde_json::json!({"x":stairs_x, "y":stairs_y, "d":stairs_d, "t":2}));
            }
        }
    }
}
