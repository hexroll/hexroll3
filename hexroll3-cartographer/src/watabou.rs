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

// Provide and process Watabou settelement maps data for the generator
// Map data is the watabou exported json format, compressed.
// This format is mostly kept as is, but augmented with some data from
// the generated sandbox.

use rand::seq::IteratorRandom;
use serde_json::Value;
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use hexroll3_scroll::{instance::*, repository::*};

use crate::hexmap::{Hex, HexMap};

#[derive(Default)]
struct District {
    geometry: Vec<geometry::Point2f>,
    buildings: Vec<usize>,
    used: bool,
}

fn locate_districts<'a>(data: &'a json::MapData) -> &'a Vec<json::Geometry> {
    for feature in data.features.iter() {
        match feature {
            json::Feature::GeometryCollection { id, geometries } => {
                if id == "districts" {
                    return geometries;
                }
            }
            _ => {}
        }
    }
    unreachable!();
}

fn locate_buildings<'a>(data: &'a json::MapData) -> &'a Vec<json::Polygon> {
    for feature in data.features.iter() {
        match feature {
            json::Feature::MultiPolygon { id, coordinates } => {
                if id == "buildings" {
                    return coordinates;
                }
            }
            _ => {}
        }
    }
    unreachable!();
}

fn locate_buildings_mut<'a>(
    data: &'a mut json::MapData,
) -> &'a mut Vec<json::Polygon> {
    for feature in data.features.iter_mut() {
        match feature {
            json::Feature::MultiPolygon { id, coordinates } => {
                if id == "buildings" {
                    return coordinates;
                }
            }
            _ => {}
        }
    }
    unreachable!();
}

fn partition_districts(data: &json::MapData) -> Vec<District> {
    let mut our_districts = Vec::new();
    let orig_districts = locate_districts(data);
    for orig_district in orig_districts {
        let mut our_district = District::default();
        if let json::Geometry::Polygon {
            width: _,
            coordinates: polygon,
        } = orig_district
        {
            our_district.geometry = polygon[0]
                .iter()
                .map(|v| geometry::Point2f { x: v[0], y: v[1] })
                .collect();
        }
        our_districts.push(our_district);
    }

    let orig_buildings = locate_buildings(data);
    for (orig_building_index, orig_building_geom) in
        orig_buildings.iter().enumerate()
    {
        let used = orig_building_geom.uid.is_some();

        let x_coord = orig_building_geom.points[0][0];
        let y_coord = orig_building_geom.points[0][1];

        for our_district in our_districts.iter_mut() {
            if geometry::point_in_polygon(
                &our_district.geometry,
                &geometry::Point2f::new(x_coord, y_coord),
            ) {
                our_district.buildings.push(orig_building_index);
                our_district.used = our_district.used || used;
            }
        }
    }

    our_districts
        .sort_unstable_by(|a, b| b.buildings.len().cmp(&a.buildings.len()));
    return our_districts;
}

pub fn refresh_city_map(
    tx: &mut ReadWriteTransaction,
    randomizer: &Randomizer,
    city_uid: &str,
) -> Result<(), anyhow::Error> {
    let mut city_data = tx.retrieve(city_uid)?;

    let mut map_data: json::MapData =
        serde_json::from_str(&city_data.value["$map_data"].to_string())
            .expect("Failed to correctly parse settlement data");

    let buildings = locate_buildings_mut(&mut map_data);
    for b in buildings.iter_mut() {
        if let Some(uid) = &b.uid {
            if tx.load(uid).is_err() {
                b.uid = None;
            }
        }
    }
    populate_city_map(tx, randomizer, &city_data.value, &mut map_data, true)?;
    let tmp = serde_json::to_value(map_data)
        .expect("failed to convert to serde_json::Value");
    city_data
        .value
        .as_object_mut()
        .unwrap()
        .insert("$map_data".to_string(), tmp);
    tx.store(city_uid, &city_data.value)?;
    Ok(())
}

pub fn populate_city_map(
    tx: &mut ReadWriteTransaction,
    randomizer: &Randomizer,
    city_data: &Value,
    map_data: &mut json::MapData,
    partial: bool,
) -> Result<(), anyhow::Error> {
    if let Some(districts) = city_data.get("districts") {
        let map_districts = partition_districts(map_data);
        let mut relevant_map_districts = map_districts
            .iter()
            .filter(|d| d.buildings.len() > 1 && (!d.used || !partial))
            .collect::<Vec<_>>();

        if relevant_map_districts.is_empty() {
            relevant_map_districts.push(map_districts.first().unwrap());
        }

        let mut relevant_district_entities = Vec::new();
        for district_uid in districts.as_array().unwrap() {
            if let Ok(district_entity) =
                tx.retrieve(district_uid.as_str().unwrap())
            {
                relevant_district_entities.push(district_entity);
            }
        }
        relevant_district_entities.sort_unstable_by(|a, b| {
            b.value["shops"]
                .as_array()
                .unwrap()
                .len()
                .cmp(&a.value["shops"].as_array().unwrap().len())
        });

        if relevant_map_districts.len() < relevant_district_entities.len()
            && !partial
        {
            return anyhow::Result::Err(anyhow::anyhow!(
                "Number of map districts is lower than number of district entities"
            ));
        }

        for (entity_district, map_district) in relevant_district_entities
            .iter_mut()
            .zip(relevant_map_districts.iter_mut())
        {
            let shops = entity_district.value["shops"].as_array().unwrap();
            let mut shops_placed = false;
            for shop_uid in shops {
                shops_placed = populate_city_entity(
                    tx,
                    randomizer,
                    shop_uid.as_str().unwrap(),
                    map_data,
                    Some(map_district),
                    partial,
                )?;
            }
            populate_city_entity(
                tx,
                randomizer,
                entity_district.value["Tavern"][0].as_str().unwrap(),
                map_data,
                Some(map_district),
                partial,
            )?;
            if shops_placed {
                let centroid = find_centroid_f(&map_district.geometry);
                entity_district
                    .value
                    .as_object_mut()
                    .unwrap()
                    .insert("x_coords".to_string(), centroid.x.into());
                entity_district
                    .value
                    .as_object_mut()
                    .unwrap()
                    .insert("y_coords".to_string(), centroid.y.into());
                let district_uid =
                    entity_district.value["uuid"].as_str().unwrap();

                tx.store(district_uid, &entity_district.value)?;
            }
        }
    } else if let Some(district) = city_data.get("District") {
        let district_uid = district.uuid_as_str();

        let district_entity = tx.retrieve(district_uid)?;

        let shops = district_entity.value["shops"].as_array().unwrap();
        for shop_uid in shops {
            populate_city_entity(
                tx,
                randomizer,
                shop_uid.as_str().unwrap(),
                map_data,
                None,
                partial,
            )?;
        }
        populate_city_entity(
            tx,
            randomizer,
            district_entity.value["Tavern"].uuid_as_str(),
            map_data,
            None,
            partial,
        )?;
    }
    Ok(())
}

fn populate_city_entity(
    tx: &mut ReadWriteTransaction,
    randomizer: &Randomizer,
    shop_uid: &str,
    map_data: &mut json::MapData,
    map_district: Option<&District>,
    partial: bool,
) -> Result<bool, anyhow::Error> {
    let mut shop = tx.retrieve(shop_uid)?;

    if shop.value.as_object().unwrap().contains_key("x_coords")
        && shop.value["x_coords"].is_f64()
        && shop.value["x_coords"].as_f64().unwrap() != 0.0
        && partial
    {
        return Ok(false);
    }

    let buildings = locate_buildings_mut(map_data);
    if buildings.len() < 2 {
        return anyhow::Result::Err(anyhow::anyhow!(
            "No buildings to choose from"
        ));
    }

    let mut maybe_index = None;

    if let Some(map_district) = map_district {
        for _attempt in 0..10 {
            let preindex = randomizer
                .in_range(1, (map_district.buildings.len() - 1) as i32);
            let tmp_index = map_district.buildings[preindex as usize];
            if buildings[tmp_index].uid.is_none() {
                maybe_index = Some(tmp_index);
                break;
            }
        }
    } else {
        for _attempt in 0..10 {
            let tmp_index =
                randomizer.in_range(1, (buildings.len() - 1) as i32) as usize;
            if buildings[tmp_index].uid.is_none() {
                maybe_index = Some(tmp_index);
                break;
            }
        }
    }
    let Some(index) = maybe_index else {
        return Err(anyhow::anyhow!("Could not find building for entity"));
    };
    buildings[index].uid = Some(shop_uid.to_string());
    let shop_point = if buildings[index].points.len() > 3 {
        let mut building_polygon = Vec::new();
        for pt in buildings[index].points.iter() {
            building_polygon.push(geometry::Point2f::new(pt[0], pt[1]));
        }
        find_centroid_f(&building_polygon)
    } else {
        geometry::Point2f::new(
            buildings[index].points[0][0],
            buildings[index].points[0][1],
        )
    };
    shop.value
        .as_object_mut()
        .unwrap()
        .insert("x_coords".to_string(), shop_point.x.into());
    shop.value
        .as_object_mut()
        .unwrap()
        .insert("y_coords".to_string(), shop_point.y.into());
    shop.value
        .as_object_mut()
        .unwrap()
        .insert("building_index".to_string(), index.into());
    tx.store(shop_uid, &shop.value)?;

    Ok(true)
}

pub fn map_settlement(
    tx: &mut ReadWriteTransaction,
    randomizer: &Randomizer,
    map: &mut HexMap,
    hex_uid: &str,
) -> Result<(), anyhow::Error> {
    let mut hex = tx.retrieve(&hex_uid)?;

    let Some(coords) = Hex::from_entity(&hex.value) else {
        return Err(anyhow::anyhow!("Error extracting hex coords"));
    };

    let coasts = map.get_unmapped_directions(&coords);

    let settlement_uid = match hex
        .value
        .get("Settlement")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(Value::as_str)
    {
        Some(uid) => uid.to_string(),
        None => panic!(
            "missing or invalid `settlement` (expected non-empty array with string first element)"
        ),
    };
    let Some(hex_obj) = hex.value.as_object_mut() else {
        return Err(anyhow::anyhow!("Error treating hex as object"));
    };
    let settlement = tx.retrieve(&settlement_uid)?;
    let settlement = settlement.value;

    let mut mapping_is_needed = true;

    let mut population = 2000;
    if settlement.as_object().unwrap().contains_key("districts") {
        population = settlement["districts"].as_array().unwrap().len() * 2000;
    }

    let coast = !coasts.is_empty();
    let river = hex_obj.contains_key("$rivers")
        && !hex_obj["$rivers"].as_array().unwrap().is_empty();
    let population_as_str = get_nearest_number_as_string(population as i32);

    let estuary_dir = if river && coast {
        Some(hex_obj["$rivers"][1].as_i64().unwrap() as i32)
    } else {
        None
    };

    let Some(settlement_obj) = settlement.as_object() else {
        return Err(anyhow::anyhow!("Error treating settlement as object"));
    };
    if settlement_obj.contains_key("$map_data") {
        let features = &settlement_obj["$map_data"]["features"][0]
            .as_object()
            .unwrap();
        let current_map_has_river = features.contains_key("has_river")
            && features["has_river"].is_boolean()
            && features["has_river"].as_bool().unwrap();

        let current_map_has_coast = features.contains_key("coast_dir")
            && features["coast_dir"].is_i64();
        let coast_is_now_occupied = if current_map_has_coast {
            let mut coast_is_now_occupied = true;
            let coast_dir = features["coast_dir"].as_i64().unwrap() as i32;
            for d in coasts.iter() {
                if coast_dir == *d {
                    coast_is_now_occupied = false;
                    break;
                }
            }
            coast_is_now_occupied
        } else {
            false
        };

        mapping_is_needed = coast != current_map_has_coast
            || river != current_map_has_river
            || coast_is_now_occupied;
    }

    if mapping_is_needed {
        let Some(settlement_obj) = settlement.as_object() else {
            return Err(anyhow::anyhow!("Error treating settlement as object"));
        };
        let settlement_class = settlement_obj["class"].as_str().unwrap();

        let water_path_part = {
            if coast
                && river
                && (settlement_class == "City" || settlement_class == "Town")
            {
                "estuary"
            } else if coast {
                "coast"
            } else if river {
                "river"
            } else {
                "none"
            }
        };

        let dir = if settlement_class == "City" || settlement_class == "Town" {
            let wall_path_part = if settlement_class == "City" {
                "walled"
            } else {
                "unwalled"
            };

            assets_path()
                .join("watabou")
                .join("factory")
                .join("cities")
                .join(water_path_part)
                .join(wall_path_part)
                .join(population_as_str)
        } else {
            assets_path()
                .join("watabou")
                .join("factory")
                .join("villages")
                .join(&water_path_part)
                .join("250")
        };

        let map_json = select_random_map_file(&dir)
            .unwrap()
            .map(decompress_json)
            .unwrap();
        let mut map_obj: json::MapData = serde_json::from_str(&map_json)
            .expect("Failed to correctly parse settlement data");

        populate_city_map(tx, randomizer, &settlement, &mut map_obj, false)?;

        let tmp = serde_json::to_value(map_obj)
            .expect("failed to convert to serde_json::Value");

        {
            let mut settlement = tx.retrieve(&settlement_uid)?;
            let Some(settlement_obj) = settlement.value.as_object_mut() else {
                return Err(anyhow::anyhow!(
                    "Error treating settlement as object"
                ));
            };

            settlement_obj.insert("$map_data".to_string(), tmp);

            if coast {
                settlement_obj["$map_data"]["features"][0]["coast_dir"] =
                    if river {
                        estuary_dir.into()
                    } else {
                        coasts[0].into()
                    };
            }
            if river
                && hex_obj.contains_key("$rivers")
                && !hex_obj["$rivers"].as_array().unwrap().is_empty()
            {
                if !coast && settlement_class != "Village" {
                    // TODO: Optionally rotate the city to better align the river
                }
                settlement_obj["$map_data"]["features"][0]["has_river"] =
                    true.into();
                settlement_obj["$map_data"]["features"][0]["rotate_river"] =
                    0.0.into();
            }

            tx.store(&settlement_uid, &settlement.value)?;
        }
    }
    map.get_hex_mut(&coords).has_settlement = true;
    Ok(())
}

pub fn decompress_json<P: AsRef<Path>>(path: P) -> String {
    let mut source = File::open(path).unwrap();

    let source_size = source.seek(SeekFrom::End(0)).unwrap() as usize;
    source.seek(SeekFrom::Start(0)).unwrap();

    let mut source_buf = vec![0u8; source_size];
    source.read_exact(&mut source_buf).unwrap();

    let mut decomp_buf = vec![0u8; source_size.saturating_mul(10)];

    let r =
        lz4_flex::block::decompress_into(&source_buf, &mut decomp_buf).unwrap();

    String::from_utf8_lossy(&decomp_buf[..r]).into_owned()
}

pub fn select_random_map_file<P: AsRef<Path>>(
    dir: P,
) -> std::io::Result<Option<PathBuf>> {
    let mut rng = rand::thread_rng();
    let iter = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("lz4"))
                .unwrap_or(false)
        })
        .map(|e| e.path());

    Ok(iter.choose(&mut rng))
}

fn _get_nearest_thousands_as_string(number: i32) -> String {
    const MIN: i32 = 1_000;
    const MAX: i32 = 25_000;

    let clamped = number.clamp(MIN, MAX);

    let nearest = ((clamped + 500) / 1_000) * 1_000;

    nearest.to_string()
}

fn get_nearest_number_as_string(number: i32) -> String {
    const VALUES: [i32; 9] = [
        1_000, 4_000, 7_000, 10_000, 13_000, 16_000, 19_000, 22_000, 25_000,
    ];

    let clamped = number.clamp(VALUES[0], VALUES[VALUES.len() - 1]);

    let mut nearest = VALUES[0];
    let mut best_dist = (clamped - nearest).abs();

    for &v in &VALUES[1..] {
        let dist = (clamped - v).abs();
        if dist < best_dist {
            best_dist = dist;
            nearest = v;
        } else if dist > best_dist {
            break;
        }
    }

    nearest.to_string()
}

pub fn find_centroid_f(v: &[geometry::Point2f]) -> geometry::Point2f {
    let mut sum = geometry::Point2f::new(0.0, 0.0);
    for &p in v {
        sum = sum + p;
    }

    geometry::Point2f::new(sum.x / (v.len() as f64), sum.y / (v.len() as f64))
}

pub trait ValueUuidExt {
    fn uuid_as_str(&self) -> &str;
    fn uuid_as_value(&self) -> &Self;
}

impl ValueUuidExt for serde_json::Value {
    #[inline]
    fn uuid_as_str(&self) -> &str {
        self.as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .unwrap()
    }
    #[inline]
    fn uuid_as_value(&self) -> &Self {
        self.as_array().and_then(|a| a.first()).unwrap()
    }
}

mod geometry {
    use std::ops::{Add, Sub};

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Point2f {
        pub x: f64,
        pub y: f64,
    }

    impl Point2f {
        #[inline]
        pub const fn new(x: f64, y: f64) -> Self {
            Self { x, y }
        }
    }

    impl Add for Point2f {
        type Output = Self;
        #[inline]
        fn add(self, rhs: Self) -> Self::Output {
            Self {
                x: self.x + rhs.x,
                y: self.y + rhs.y,
            }
        }
    }

    impl Sub for Point2f {
        type Output = Self;
        #[inline]
        fn sub(self, rhs: Self) -> Self::Output {
            Self {
                x: self.x - rhs.x,
                y: self.y - rhs.y,
            }
        }
    }

    #[inline]
    fn cross(a: Point2f, b: Point2f) -> f64 {
        a.x * b.y - a.y * b.x
    }

    #[inline]
    fn dot(a: Point2f, b: Point2f) -> f64 {
        a.x * b.x + a.y * b.y
    }

    #[inline]
    fn point_on_segment(a: Point2f, b: Point2f, p: Point2f) -> bool {
        let ab = b - a;
        let ap = p - a;

        if cross(ab, ap) != 0.0 {
            return false;
        }

        let proj = dot(ap, ab);
        if proj < 0.0 {
            return false;
        }

        let ab_len2 = dot(ab, ab);
        proj <= ab_len2
    }

    pub fn point_in_polygon(polygon: &[Point2f], point: &Point2f) -> bool {
        let n = polygon.len();
        if n < 3 {
            return false;
        }

        let p = *point;
        let mut inside = false;

        let mut j = n - 1;
        for i in 0..n {
            let a = polygon[j];
            let b = polygon[i];

            if point_on_segment(a, b, p) {
                return true;
            }

            let ay = a.y;
            let by = b.y;

            let intersects = (ay > p.y) != (by > p.y);
            if intersects {
                let x_intersect = a.x + (p.y - ay) * (b.x - a.x) / (by - ay);
                if x_intersect > p.x {
                    inside = !inside;
                }
            }

            j = i;
        }

        inside
    }
}

pub mod json {
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Deserialize, Serialize, Debug, Clone)]
    #[allow(dead_code)]
    pub struct MapData {
        #[serde(rename = "type")]
        pub map_type: String,
        pub features: Vec<Feature>,
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    #[serde(tag = "type")]
    #[allow(non_snake_case)]
    #[allow(clippy::large_enum_variant)]
    #[allow(clippy::enum_variant_names)]
    #[allow(dead_code)]
    pub enum Feature {
        Feature {
            id: String,
            roadWidth: Option<f64>,
            towerRadius: Option<f64>,
            wallThickness: Option<f64>,
            generator: Option<String>,
            coast_dir: Option<i32>,
            has_river: Option<bool>,
            rotate_river: Option<f64>,
            version: Option<String>,
        },
        Polygon {
            id: String,
            coordinates: Vec<Vec<[f64; 2]>>,
        },
        GeometryCollection {
            id: String,
            geometries: Vec<Geometry>,
        },
        MultiPolygon {
            id: String,
            coordinates: Vec<Polygon>,
        },
        MultiPoint {
            id: String,
            coordinates: Vec<[f64; 2]>,
        },
    }

    #[derive(Deserialize, Serialize, Debug, Clone, Default)]
    #[serde(default)]
    pub struct Polygon {
        pub points: Vec<Vec<f64>>,
        pub uid: Option<String>,
        pub uid1: Option<String>,
        pub uid2: Option<String>,
        pub uid3: Option<String>,
        pub uid4: Option<String>,
        pub uid5: Option<String>,
        pub uid6: Option<String>,
        pub uid7: Option<String>,
        pub uid8: Option<String>,
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    #[serde(tag = "type")]
    pub enum Geometry {
        LineString {
            width: Option<f64>,
            coordinates: Vec<[f64; 2]>,
        },
        Polygon {
            width: Option<f64>,
            coordinates: Vec<Vec<[f64; 2]>>,
        },
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    #[allow(dead_code)]
    pub struct PointOfInterest {
        pub coords: Coords,
        pub title: String,
        pub uuid: String,
        pub building: Option<i32>,
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    #[allow(dead_code)]
    pub struct Coords {
        pub x: f64,
        pub y: f64,
    }
}

fn assets_path() -> PathBuf {
    #[cfg(not(target_os = "macos"))]
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });
    #[cfg(target_os = "macos")]
    let mut path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });
    #[cfg(target_os = "macos")]
    {
        path.pop();
        path.pop();
        path.push("Resources");
    }
    path
}
