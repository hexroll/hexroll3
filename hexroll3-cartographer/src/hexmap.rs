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
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use hexroll3_scroll::{instance::*, renderer::render_entity, repository::*};

#[derive(Debug)]
pub struct HexMapData {
    pub uid: String,
    pub class: String,
    pub has_river: bool,
    pub has_settlement: bool,
}

#[derive(Debug)]
pub struct HexMap {
    map: BTreeMap<Hex, HexMapData>,
}

impl HexMap {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
    // RECONSTRUCT (from sandbox hex data)
    // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

    pub fn reconstruct(&mut self, instance: &mut SandboxInstance) {
        if let Some(sid) = instance.sid() {
            let _ = instance.repo.inspect(|tx| {
                self.reconstruct_in_transaction(&sid, tx)?;
                Ok(())
            });
        }
    }

    pub fn reconstruct_in_transaction<T>(
        &mut self,
        sid: &str,
        tx: &mut T,
    ) -> anyhow::Result<()>
    where
        T: ReadOnlyLoader,
    {
        let root = tx.retrieve(sid)?;
        match root.value["ocean"].as_array() {
            Some(ocean_hexes) => ocean_hexes.iter().for_each(|hex_uid| {
                let hex_uid_str = hex_uid.as_str().unwrap();
                tx.retrieve(hex_uid_str).ok().map(|hex| {
                    let hex_obj = hex.value.as_object().unwrap();
                    let coords = hex
                        .value
                        .get("$coords")
                        .unwrap_or(&serde_json::Value::Null);
                    let x = coords["x"].as_i64().unwrap() as i32;
                    let y = coords["y"].as_i64().unwrap() as i32;
                    self.map.insert(
                        Hex { x, y },
                        HexMapData {
                            uid: hex_uid_str.to_string(),
                            class: hex_obj["class"]
                                .as_str()
                                .unwrap()
                                .to_string(),
                            has_river: false,
                            has_settlement: false,
                        },
                    );
                });
            }),
            None => {}
        };

        if let Some(realms) =
            root.value.get("realms").and_then(|v| v.as_array())
        {
            for realm_uid in realms.iter().filter_map(|v| v.as_str()) {
                let Ok(realm) = tx.retrieve(realm_uid) else {
                    continue;
                };
                let Some(regions) =
                    realm.value.get("regions").and_then(|v| v.as_array())
                else {
                    continue;
                };
                for region_uid in regions.iter().filter_map(|v| v.as_str()) {
                    let Ok(region) = tx.retrieve(region_uid) else {
                        continue;
                    };
                    let Some(hexes) =
                        region.value.get("Hexmap").and_then(|v| v.as_array())
                    else {
                        continue;
                    };

                    for hex_uid in hexes.iter() {
                        let Some(hex_uid_str) = hex_uid.as_str() else {
                            continue;
                        };
                        let Ok(hex) = tx.retrieve(hex_uid_str) else {
                            continue;
                        };

                        let coords = hex
                            .value
                            .get("$coords")
                            .unwrap_or(&serde_json::Value::Null);
                        let x = coords["x"].as_i64().unwrap() as i32;
                        let y = coords["y"].as_i64().unwrap() as i32;

                        let hex_obj = hex.value.as_object().unwrap();
                        self.map.insert(
                            Hex { x, y },
                            HexMapData {
                                uid: hex_uid.as_str().unwrap().to_string(),
                                class: hex_obj["class"]
                                    .as_str()
                                    .unwrap()
                                    .to_string(),
                                has_river: hex_obj.contains_key("$rivers"),
                                has_settlement: hex_obj["Settlement"]
                                    .as_array()
                                    .unwrap()
                                    .len()
                                    == 1,
                            },
                        );
                    }
                }
            }
        }
        Ok(())
    }

    // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
    // LAYOUT (apply changes to newly generated or updated hexes)
    // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

    pub fn apply_layout(
        &mut self,
        tx: &mut ReadWriteTransaction,
        randomizer: &Randomizer,
    ) -> anyhow::Result<()> {
        let mut settlements_backlog = Vec::new();
        for (_, hex_data) in self.map.iter() {
            let mut hex = tx.retrieve(&hex_data.uid)?;

            let Some(coords) = Hex::from_entity(&hex.value) else {
                return Err(anyhow::anyhow!("Error extracting hex coords"));
            };

            let coasts = self.get_unmapped_directions(&coords);

            let Some(hex_obj) = hex.value.as_object_mut() else {
                return Err(anyhow::anyhow!("Error treating hex as object"));
            };

            let coast = !coasts.is_empty();
            let river = hex_obj.contains_key("$rivers");

            let estuary_dir = if river && coast {
                Some(hex_obj["$rivers"][1].as_i64().unwrap() as i32)
            } else {
                None
            };

            if !coasts.is_empty() {
                hex_obj.insert(
                    "$coast_dir".to_string(),
                    if river {
                        estuary_dir.into()
                    } else {
                        coasts[0].into()
                    },
                );
                // TODO: Move to cargoraphy logic
                hex_obj.insert("$coasts".to_string(), coasts.into());
            } else {
                if hex_obj.contains_key("$coasts") {
                    hex_obj.remove("$coasts");
                }
                if hex_obj.contains_key("$coast_dir") {
                    hex_obj.remove("$coast_dir");
                }
            }

            if hex_data.has_settlement {
                settlements_backlog.push(hex_data.uid.clone());
            }

            tx.store(&hex_data.uid, &hex.value)?;
        }

        for uid in settlements_backlog {
            crate::watabou::map_settlement(tx, randomizer, self, &uid)?;
        }
        Ok(())
    }

    // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
    // TRAILS
    // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

    fn draw_trail(&self, a: Hex, b: Hex, proposal: &mut TrailProposal) -> bool {
        let mut dx: i32 = b.x - a.x;
        let mut dy: i32 = b.y - a.y;
        if a.y % 2 == 0 || dy == 0 {
            dx *= 2;
        } else {
            dx = dx * 2 - 1;
        }

        const INVERT: [i32; 6] = [3, 4, 5, 0, 1, 2];

        #[derive(Clone)]
        struct SolutionStep {
            hex_uid: String,
            hex: Hex,
            dir_enter: i32,
            dir_exit: i32,
        }

        let mut visited: BTreeSet<Hex> = BTreeSet::new();
        let mut solution: Vec<SolutionStep> = Vec::new();

        let Some(a_data) = self.map.get(&a) else {
            return false;
        };

        let mut step = SolutionStep {
            hex: a,
            hex_uid: a_data.uid.clone(),
            dir_enter: -1,
            dir_exit: -1,
        };

        while dx != 0 || dy != 0 {
            let mut dirs_to_try: Vec<i32> = vec![];

            if dx.abs() >= dy.abs() {
                if dx > 0 && dy <= 0 {
                    dirs_to_try = vec![1, 0, 5, 2, 4, 3];
                }
                if dx > 0 && dy > 0 {
                    dirs_to_try = vec![2, 3, 1, 4, 0, 5];
                }
                if dx < 0 && dy >= 0 {
                    dirs_to_try = vec![4, 3, 5, 0, 2, 1];
                }
                if dx < 0 && dy < 0 {
                    dirs_to_try = vec![5, 0, 4, 3, 1, 2];
                }
            } else {
                if dy < 0 {
                    dirs_to_try = vec![0, 1, 5, 4, 2, 3];
                }
                if dy > 0 {
                    dirs_to_try = vec![3, 4, 2, 1, 5, 0];
                }
            }

            let mut dir: i32 = -1;

            if solution.len() < 20 {
                for dir_to_try in dirs_to_try {
                    let test = step.hex.neighbor(dir_to_try);
                    if self.map.contains_key(&test) && !visited.contains(&test)
                    {
                        dir = dir_to_try;
                        visited.insert(test);
                        break;
                    }
                    visited.insert(test);
                }
            }

            if dir == -1 {
                if let Some(prev) = solution.pop() {
                    step = prev;
                } else {
                    return false;
                }
            } else {
                step.dir_exit = dir;
                solution.push(step.clone());

                let next_hex = step.hex.neighbor(dir);
                let Some(next_data) = self.map.get(&next_hex) else {
                    return false;
                };

                step.dir_enter = INVERT[dir as usize];
                step.dir_exit = -1;
                step.hex = next_hex;
                step.hex_uid = next_data.uid.clone();

                dx = b.x - step.hex.x;
                dy = b.y - step.hex.y;
                if step.hex.y % 2 == 0 || dy == 0 {
                    dx *= 2;
                } else {
                    dx = dx * 2 - 1;
                }
            }
        }

        solution.push(step);

        proposal.a = a;
        proposal.b = b;

        while let Some(s) = solution.pop() {
            let mut segment: Vec<i32> = Vec::new();
            if s.dir_exit != -1 {
                segment.push(s.dir_exit);
            }
            if s.dir_enter != -1 {
                segment.push(s.dir_enter);
            }
            proposal.trail.push(TrailSegment {
                segment,
                hex_uid: s.hex_uid,
            });
            proposal.distance += 1;
        }

        true
    }

    pub fn stage_trails(
        &self,
        tx: &mut ReadWriteTransaction,
    ) -> anyhow::Result<()> {
        // Find all settlement hexes
        let settlement_hexes: Vec<Hex> = self
            .map
            .iter()
            .filter_map(|(h, d)| if d.has_settlement { Some(*h) } else { None })
            .collect();

        if settlement_hexes.is_empty() {
            return Ok(());
        }

        // Build all pairs
        let mut trail_requests: Vec<TrailRequest> = Vec::new();
        for j in 0..settlement_hexes.len().saturating_sub(1) {
            for k in (j + 1)..settlement_hexes.len() {
                trail_requests.push(TrailRequest {
                    a: settlement_hexes[j],
                    b: settlement_hexes[k],
                });
            }
        }

        // Propose trails
        let mut proposals: Vec<TrailProposal> = Vec::new();
        let mut failed_trails: i32 = 0;

        for tr in trail_requests {
            let mut proposal = TrailProposal::default();
            if self.draw_trail(tr.a, tr.b, &mut proposal) {
                proposals.push(proposal);
            } else {
                failed_trails += 1;
                if failed_trails > 10_000 {
                    eprintln!("10,000 trail proposal attempts failed");
                    failed_trails = 0;
                }
            }
        }

        // Sort by distance
        proposals.sort_by_key(|p| p.distance);

        // Apply proposals
        let mut hits: BTreeSet<Hex> = BTreeSet::new();

        for p in proposals.iter() {
            if hits.contains(&p.a) {
                continue;
            }
            hits.insert(p.a);

            for part in p.trail.iter() {
                let ent = tx.load(&part.hex_uid)?;
                let obj = ent.as_object_mut().ok_or_else(|| {
                    anyhow::anyhow!("hex entity not an object")
                })?;

                let trails = obj
                    .entry("$trails")
                    .or_insert_with(|| Value::Array(vec![]));
                if !trails.is_array() {
                    *trails = Value::Array(vec![]);
                }

                let arr = trails.as_array_mut().unwrap();
                for dir in part.segment.iter().copied() {
                    let exists =
                        arr.iter().any(|v| v.as_i64() == Some(dir as i64));
                    if !exists {
                        arr.push(Value::from(dir));
                    }
                }

                tx.save(&part.hex_uid)?;
            }
        }

        Ok(())
    }

    // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
    // RIVERS
    // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
    pub fn draw_river(
        &mut self,
        tx: &mut ReadWriteTransaction,
        randomizer: &Randomizer,
        start: Hex,
        hex_uid: String,
    ) -> anyhow::Result<()> {
        let mut visited = std::collections::BTreeSet::new();
        let mut path = Vec::new();
        path.push((start.clone(), hex_uid.clone()));
        self.draw_river_from(tx, start, &mut visited, &mut path, randomizer)?;
        for e in path {
            let mut t = false;
            let uid = {
                let data = self.map.get_mut(&e.0).unwrap();
                data.has_river = true;
                if data.has_settlement {
                    t = true;
                }
                data.uid.clone()
            };
            if t {
                crate::watabou::map_settlement(tx, randomizer, self, &uid)?;
            }
        }
        Ok(())
    }

    pub fn clear_river(
        &mut self,
        tx: &mut ReadWriteTransaction,
        randomizer: &Randomizer,
        hex_uid: &str,
    ) -> anyhow::Result<Vec<Hex>> {
        let impacted_hexes =
            self.clear_path(tx, hex_uid, "$rivers", false, Some("$river_dir"))?;

        for h in impacted_hexes.iter().copied() {
            if let Some(hd) = self.map.get(&h) {
                if hd.has_settlement {
                    let uid = hd.uid.clone();
                    crate::watabou::map_settlement(tx, randomizer, self, &uid)?;
                }
            }
        }

        Ok(impacted_hexes)
    }

    pub fn can_start_river_from(&self, hex: &Hex) -> bool {
        DIRS.iter().any(|&dir| {
            let neighbor = hex.neighbor(dir);
            self.map
                .get(&neighbor)
                .is_some_and(|d| d.class != "MountainsHex")
        })
    }

    pub fn extend_existing_rivers(
        &mut self,
        tx: &mut ReadWriteTransaction,
        randomizer: &Randomizer,
    ) -> anyhow::Result<()> {
        let mut rivers_to_extend = Vec::new();
        for (hex_coords, hex_data) in self.map.iter() {
            if hex_data.has_river && hex_data.class == "MountainsHex" {
                rivers_to_extend
                    .push((hex_coords.clone(), hex_data.uid.clone()));
            }
        }

        for river_to_extend in rivers_to_extend {
            let mut visited = std::collections::BTreeSet::new();
            let mut path = Vec::new();
            if let Some(extend_coords) = self.follow_existing_river(
                tx,
                river_to_extend.0,
                &mut visited,
                &mut path,
            )? {
                self.draw_river_from(
                    tx,
                    extend_coords,
                    &mut visited,
                    &mut path,
                    randomizer,
                )?;
            }
        }
        Ok(())
    }

    pub fn draw_river_from(
        &mut self,
        tx: &mut ReadWriteTransaction,
        start: Hex,
        visited: &mut std::collections::BTreeSet<Hex>,
        path: &mut Vec<(Hex, String)>,
        randomizer: &Randomizer,
    ) -> anyhow::Result<()> {
        let mut current = start;

        let mut attempts_budget: i32 = 12;

        loop {
            let mut dirs = DIRS;

            randomizer.shuffle(&mut dirs);

            let mut next = current;
            let mut found = false;

            // coastal hex? (less than 6 occupied neighbors)
            if self.num_of_occupied_neighbors(current) < 6 {
                for &dir in dirs.iter() {
                    let neighbor = current.neighbor(dir);
                    if !self.map.contains_key(&neighbor) {
                        // found ocean - set river to flow here
                        let uid = self
                            .map
                            .get(&current)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "missing hex in map: {:?}",
                                    current
                                )
                            })?
                            .uid
                            .clone();

                        let curr_hex_data = tx.load(&uid)?;
                        Self::push_river_dir(curr_hex_data, dir);
                        self.map.get_mut(&current).unwrap().has_river = true;

                        tx.save(&uid)?;

                        path.push((current, uid));
                        break;
                    }
                }
                break;
            }

            let mut selected_dir: i32 = -1;
            for &dir in dirs.iter() {
                next = current.neighbor(dir);
                if visited.contains(&next) {
                    continue;
                }
                let Some(next_data) = self.map.get(&next) else {
                    continue;
                };
                if next_data.class == "MountainsHex" {
                    continue;
                }
                if next_data.has_river {
                    continue;
                }
                selected_dir = dir;
                found = true;
                break;
            }

            visited.insert(current);
            visited.insert(next);

            if !found {
                if path.is_empty() {
                    break;
                }
                self.revert_river_step(tx, path)?;
                if path.is_empty() {
                    break;
                }
                current = path.last().unwrap().0;
                attempts_budget -= 1;
                if attempts_budget == 0 {
                    break;
                }
            } else {
                let curr_uid = self
                    .map
                    .get(&current)
                    .ok_or_else(|| {
                        anyhow::anyhow!("missing hex in map: {:?}", current)
                    })?
                    .uid
                    .clone();
                let curr_hex_data = tx.load(&curr_uid)?;
                Self::push_river_dir(curr_hex_data, selected_dir);
                self.map.get_mut(&current).unwrap().has_river = true;
                tx.save(&curr_uid)?;

                const INVERT: [i32; 6] = [3, 4, 5, 0, 1, 2];
                let next_uid = self
                    .map
                    .get(&next)
                    .ok_or_else(|| {
                        anyhow::anyhow!("missing hex in map: {:?}", next)
                    })?
                    .uid
                    .clone();
                let next_hex_data = tx.load(&next_uid)?;
                Self::push_river_dir(
                    next_hex_data,
                    INVERT[selected_dir as usize],
                );
                self.map.get_mut(&next).unwrap().has_river = true;
                tx.save(&next_uid)?;

                current = next;
                path.push((current, next_uid));
            }
        }

        if path.len() < 4 || attempts_budget == 0 {
            while !path.is_empty() {
                self.revert_river_step(tx, path)?;
            }
        }

        Ok(())
    }

    fn revert_river_step(
        &mut self,
        tx: &mut ReadWriteTransaction,
        path: &mut Vec<(Hex, String)>,
    ) -> anyhow::Result<()> {
        let (curr_hex, curr_uid) = path.last().cloned().ok_or_else(|| {
            anyhow::anyhow!("revert_river_step called with empty path")
        })?;

        {
            let curr = tx.load(&curr_uid)?;
            if curr.get("$rivers").is_some() {
                let curr_obj = curr.as_object_mut().unwrap();
                curr_obj.remove("$rivers");
                curr_obj.remove("$river_dir");
                self.map.get_mut(&curr_hex).unwrap().has_river = false;
                tx.save(&curr_uid)?;
            }
        }

        path.pop();
        if path.is_empty() {
            return Ok(());
        }

        let (prev_hex, prev_uid) = path.last().cloned().unwrap();
        let prev = tx.load(&prev_uid)?;
        if prev.get("$rivers").is_some() {
            let rivers_len = prev
                .get("$rivers")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);

            if rivers_len == 1 {
                let prev_obj = prev.as_object_mut().unwrap();
                prev_obj.remove("$rivers");
                prev_obj.remove("$river_dir");
                self.map.get_mut(&prev_hex).unwrap().has_river = false;
            } else {
                if let Some(arr) =
                    prev.get_mut("$rivers").and_then(|v| v.as_array_mut())
                {
                    if arr.len() > 1 {
                        arr.remove(1);
                    }
                }
            }
            tx.save(&prev_uid)?;
        }

        Ok(())
    }

    fn push_river_dir(entity: &mut serde_json::Value, dir: i32) {
        let obj = entity.as_object_mut().unwrap();
        let rivers = obj
            .entry("$rivers")
            .or_insert_with(|| serde_json::Value::Array(vec![]));
        if !rivers.is_array() {
            *rivers = serde_json::Value::Array(vec![]);
        }
        let rivers = rivers.as_array_mut().unwrap();
        rivers.push(serde_json::Value::from(dir));
    }

    fn follow_existing_river(
        &mut self,
        tx: &mut ReadWriteTransaction,
        start: Hex,
        visited: &mut BTreeSet<Hex>,
        path: &mut Vec<(Hex, String)>,
    ) -> anyhow::Result<Option<Hex>> {
        const INVERT: [i32; 6] = [3, 4, 5, 0, 1, 2];

        let mut follow_hex = start;

        let mut follow_uid = match self.map.get(&follow_hex) {
            Some(d) => d.uid.clone(),
            None => return Ok(None),
        };

        let mut follow_dir: i32 = -1;

        loop {
            visited.insert(follow_hex);

            let ent = tx.load(&follow_uid)?;
            let obj = ent
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("hex entity not an object"))?;

            let has_rivers = obj.contains_key("$rivers");
            if !has_rivers {
                let class =
                    obj.get("class").and_then(|v| v.as_str()).unwrap_or("");

                if class == "MountainsHex" {
                    self.revert_river_step(tx, path)?;
                    if path.is_empty() {
                        return Ok(None);
                    }
                    follow_hex = path.last().unwrap().0;
                } else {
                    let inv = INVERT[follow_dir as usize];

                    let ent2 = tx.load(&follow_uid)?;
                    Self::push_river_dir(ent2, inv);
                    self.map.get_mut(&follow_hex).unwrap().has_river = true;

                    tx.save(&follow_uid)?;

                    path.push((follow_hex, follow_uid.clone()));
                }
                break;
            }
            path.push((follow_hex, follow_uid.clone()));
            let ent2 = tx.load(&follow_uid)?;
            let rivers =
                ent2.get("$rivers").and_then(|v| v.as_array()).ok_or_else(
                    || anyhow::anyhow!("$rivers present but not an array"),
                )?;

            let last_dir =
                rivers.last().and_then(|v| v.as_i64()).ok_or_else(|| {
                    anyhow::anyhow!("$rivers last element not an int")
                })? as i32;

            follow_dir = last_dir;

            follow_hex = follow_hex.neighbor(follow_dir);

            let Some(next_data) = self.map.get(&follow_hex) else {
                return Ok(None);
            };
            follow_uid = next_data.uid.clone();
        }

        Ok(Some(follow_hex))
    }

    // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
    // UTILS
    // - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

    fn clear_path(
        &self,
        tx: &mut ReadWriteTransaction,
        hex_uid: &str,
        key: &str,
        with_flow: bool,
        extra_key: Option<&str>,
    ) -> anyhow::Result<Vec<Hex>> {
        let mut to_clean: BTreeSet<String> = BTreeSet::new();
        let mut tasks: Vec<String> = Vec::new();
        let mut impacted: Vec<Hex> = Vec::new();

        tasks.push(hex_uid.to_string());

        while let Some(uid) = tasks.pop() {
            let ent = tx.load(&uid)?;
            let obj = match ent.as_object() {
                Some(o) => o,
                None => break,
            };

            if !obj.contains_key(key) {
                break;
            }

            if !to_clean.insert(uid.clone()) {
                continue;
            }

            let Some(hex) = Hex::from_entity(&ent) else {
                break;
            };
            impacted.push(hex);

            let mut path_dir: Vec<i32> = ent
                .get(key)
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_i64().map(|n| n as i32))
                        .collect()
                })
                .unwrap_or_default();

            if with_flow && path_dir.len() >= 2 {
                path_dir.remove(0);
            }

            for dir in path_dir {
                let next_hex = hex.neighbor(dir);
                if let Some(next_data) = self.map.get(&next_hex) {
                    let next_uid = next_data.uid.clone();
                    if !to_clean.contains(&next_uid) {
                        tasks.push(next_uid);
                    }
                }
            }
        }

        for uid in to_clean.iter() {
            let ent = tx.load(uid)?;
            if let Some(o) = ent.as_object_mut() {
                o.remove(key);
                if let Some(extra) = extra_key {
                    o.remove(extra);
                }
            }
            tx.save(uid)?;
        }

        Ok(impacted)
    }

    fn num_of_occupied_neighbors(&self, hex: Hex) -> usize {
        DIRS.iter()
            .filter(|&&d| self.map.contains_key(&hex.neighbor(d)))
            .count()
    }

    pub fn get_hex_mut(&mut self, hex: &Hex) -> &mut HexMapData {
        self.map.get_mut(hex).unwrap()
    }

    pub fn get_unmapped_directions(&self, hex: &Hex) -> Vec<i32> {
        DIRS.into_iter()
            .filter(|&dir| !self.map.contains_key(&hex.neighbor(dir)))
            .collect()
    }
}

const DIRS: [i32; 6] = [0, 1, 2, 3, 4, 5];

#[derive(Clone, Copy, Debug, Default)]
pub struct Hex {
    x: i32,
    y: i32,
}

impl PartialOrd for Hex {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Hex {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.x, self.y).cmp(&(other.x, other.y))
    }
}

impl PartialEq for Hex {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl Eq for Hex {}

impl Hex {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn from_entity(entity: &Value) -> Option<Hex> {
        let Some(coords) = entity.get("$coords") else {
            return None;
        };
        let x = coords["x"].as_i64().unwrap() as i32;
        let y = coords["y"].as_i64().unwrap() as i32;
        Some(Hex { x, y })
    }
    pub fn neighbor(&self, dir: i32) -> Hex {
        match dir {
            0 => Hex {
                x: self.x,
                y: self.y - 2,
                ..Default::default()
            },
            1 => Hex {
                x: self.x + (self.y.abs() % 2),
                y: self.y - 1,
                ..Default::default()
            },
            2 => Hex {
                x: self.x + (self.y.abs() % 2),
                y: self.y + 1,
                ..Default::default()
            },
            3 => Hex {
                x: self.x,
                y: self.y + 2,
                ..Default::default()
            },
            4 => Hex {
                x: self.x - ((self.y + 1).abs() % 2),
                y: self.y + 1,
                ..Default::default()
            },
            5 => Hex {
                x: self.x - ((self.y + 1).abs() % 2),
                y: self.y - 1,
                ..Default::default()
            },
            _ => panic!("Invalid direction value"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TrailRequest {
    a: Hex,
    b: Hex,
}

#[derive(Debug, Clone)]
struct TrailSegment {
    segment: Vec<i32>,
    hex_uid: String,
}

#[derive(Debug, Clone)]
struct TrailProposal {
    trail: Vec<TrailSegment>,
    a: Hex,
    b: Hex,
    distance: i32,
}

impl Default for TrailProposal {
    fn default() -> Self {
        Self {
            trail: vec![],
            a: Hex::new(0, 0),
            b: Hex::new(0, 0),
            distance: 0,
        }
    }
}

pub fn generate_hex_map_json(
    instance: &SandboxInstance,
) -> anyhow::Result<serde_json::Value> {
    let sid = instance
        .sid
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No sandbox ID"))?;

    instance.repo.inspect(|tx| {
        let root = tx.load(&sid)?;
        let ocean_hexes: Vec<serde_json::Value> = match root.value["ocean"].as_array() {
            Some(ocean_hexes) => ocean_hexes
                .iter()
                .filter_map(|uid| {
                    let uid_str = uid.as_str().unwrap();
                    tx.load(uid_str).ok().map(|v| {
                        json!({
                            "x": v.value["$coords"]["x"],
                            "y": v.value["$coords"]["y"],
                            "type": v.value["class"],
                            "uuid": uid,
                        })
                    })
                })
                .collect(),
            None => Vec::new(),
        };

        let mut ret = json!({"map": ocean_hexes, "realms": {}, "regions": {}, "borders": {}});

        if let Some(realms) = root.value.get("realms").and_then(|v| v.as_array()) {
            for realm_uid in realms.iter().filter_map(|v| v.as_str()) {
                let Ok(realm) = tx.load(realm_uid) else {
                    continue;
                };

                ret["realms"][realm_uid]["name"] = realm.value["Title"].clone();

                let Some(regions) = realm.value.get("regions").and_then(|v| v.as_array())
                else {
                    continue;
                };

                for region_uid in regions.iter().filter_map(|v| v.as_str()) {
                    let Ok(region) = tx.load(region_uid) else {
                        continue;
                    };
                    let rendered_region = {
                        let Ok(mut blueprint) = instance.blueprint.lock() else {
                            return anyhow::Result::Err(anyhow::anyhow!(
                                "Error trying to lock the sandbox blueprint"
                            ));
                        };
                        render_entity(&instance, &mut blueprint, tx, &region.value, false)
                            .unwrap()
                    };
                    ret["regions"][region_uid] = rendered_region["Name"].clone();
                    let Some(hexes) = region.value.get("Hexmap").and_then(|v| v.as_array())
                    else {
                        continue;
                    };

                    for hex_uid in hexes.iter() {
                        let Some(hex_uid_str) = hex_uid.as_str() else {
                            continue;
                        };
                        let Ok(hex) = tx.load(hex_uid_str) else {
                            continue;
                        };
                        let Some(hex_obj) = hex.value.as_object() else {
                            continue;
                        };

                        let coords =
                            hex.value.get("$coords").unwrap_or(&serde_json::Value::Null);
                        let x = coords.get("x").unwrap_or(&serde_json::Value::Null).clone();
                        let y = coords.get("y").unwrap_or(&serde_json::Value::Null).clone();

                        let mut v = json!({
                            "x": x,
                            "y": y,
                            "type": hex.value.get("class").unwrap_or(&serde_json::Value::Null),
                            "uuid": hex_uid,
                            "realm": realm_uid,
                            "region": region_uid,
                        });

                        if hex_obj.contains_key("$coasts") {
                            v.as_object_mut().unwrap().insert(
                                "harbor".to_string(),
                                if hex_obj.contains_key("$coast_dir") {
                                    hex_obj["$coast_dir"].clone()
                                } else {
                                    hex_obj["$coasts"][0].clone()
                                },
                            );
                        }

                        if let Some(dungeon) = hex
                            .value
                            .get("Dungeon")
                            .and_then(|v| v.as_array())
                            .and_then(|v| v.first())
                        {
                            v["feature"] = "Dungeon".into();
                            v["feature_uuid"] = dungeon.clone();
                        }
                        if let Some(city) = hex
                            .value
                            .get("Settlement")
                            .and_then(|v| v.as_array())
                            .and_then(|v| v.first())
                        {
                            if let Ok(set) = tx.retrieve(city.as_str().unwrap()) {
                                v["feature"] = set.value["class"].clone();
                                v["label"] = set.value["NamePart"].clone();
                                v["feature_uuid"] = city.clone();
                            }
                        }
                        if let Some(_dwelling) = hex
                            .value
                            .get("Residency")
                            .and_then(|v| v.as_array())
                            .and_then(|v| v.first())
                        {
                            v["feature"] = "Residency".into();
                        }
                        if let Some(inn) = hex
                            .value
                            .get("Inn")
                            .and_then(|v| v.as_array())
                            .and_then(|v| v.first())
                        {
                            v["feature"] = "Inn".into();
                            v["feature_uuid"] = inn.clone();
                        }

                        if hex_obj.contains_key("$rivers") {
                            v.as_object_mut()
                                .unwrap()
                                .insert("rivers".to_string(), hex_obj["$rivers"].clone());
                        }
                        if hex_obj.contains_key("$trails") {
                            v.as_object_mut()
                                .unwrap()
                                .insert("trails".to_string(), hex_obj["$trails"].clone());
                        }

                        ret["map"].as_array_mut().unwrap().push(v);
                    }
                }
            }
        }

        Ok(ret)
    })
}

pub struct MapContextHexData {
    pub coords: Option<Value>,
    pub rivers: Option<Value>,
    pub trails: Option<Value>,
    pub borders: Option<Value>,
    pub coasts: Option<Value>,
    pub coast_dir: Option<Value>,
    pub river_dir: Option<Value>,
}

pub struct MapContextBuildingData {
    pub x_coords: Value,
    pub y_coords: Value,
    pub building_index: Value,
}

pub struct MapContextLocationData {
    pub x_coords: Value,
    pub y_coords: Value,
}

pub enum MapContext {
    HexData(MapContextHexData),
    SettlementData,
    LocationData(MapContextLocationData),
    BuildingData(MapContextBuildingData),
}

pub fn extract_map_context(
    tx: &mut ReadWriteTransaction,
    uid: &str,
) -> anyhow::Result<Option<MapContext>> {
    let old_entity = tx.load(uid)?;
    let old_entity_obj = old_entity.as_object().unwrap();
    if old_entity_obj.contains_key("$coords") {
        let hex_data = MapContextHexData {
            coords: old_entity_obj.get("$coords").cloned(),
            rivers: old_entity_obj.get("$rivers").cloned(),
            trails: old_entity_obj.get("$trails").cloned(),
            borders: old_entity_obj.get("$borders").cloned(),
            coasts: old_entity_obj.get("$coasts").cloned(),
            coast_dir: old_entity_obj.get("$coast_dir").cloned(),
            river_dir: old_entity_obj.get("$river_dir").cloned(),
        };
        return Ok(Some(MapContext::HexData(hex_data)));
    }

    if old_entity_obj.contains_key("$map_data") {
        return Ok(Some(MapContext::SettlementData));
    }

    let (x_coords, y_coords) = if old_entity_obj.contains_key("x_coords")
        && old_entity_obj.contains_key("y_coords")
    {
        (
            old_entity_obj.get("x_coords").cloned(),
            old_entity_obj.get("y_coords").cloned(),
        )
    } else {
        (None, None)
    };

    if let Some(x_coords) = x_coords
        && let Some(y_coords) = y_coords
    {
        if old_entity_obj.contains_key("building_index") {
            let building_data = MapContextBuildingData {
                x_coords: x_coords,
                y_coords: y_coords,
                building_index: old_entity_obj
                    .get("building_index")
                    .unwrap()
                    .clone(),
            };
            return Ok(Some(MapContext::BuildingData(building_data)));
        } else {
            let location_data = MapContextLocationData {
                x_coords: x_coords,
                y_coords: y_coords,
            };
            return Ok(Some(MapContext::LocationData(location_data)));
        }
    }

    Ok(None)
}

pub fn apply_map_context(
    instance: &SandboxInstance,
    hex_map: &mut HexMap,
    tx: &mut ReadWriteTransaction,
    map_context: MapContext,
    uid: &str,
) -> anyhow::Result<()> {
    match map_context {
        MapContext::HexData(data) => {
            let entity = tx.load(uid)?;
            let entity_obj = entity.as_object_mut().unwrap();
            if let Some(coords) = data.coords {
                entity_obj["coord_x"] = coords["x"].clone();
                entity_obj["coord_y"] = coords["y"].clone();
                entity_obj.insert("$coords".to_string(), coords);
            }
            if let Some(borders) = data.borders {
                entity_obj.insert("$borders".to_string(), borders);
            }
            if let Some(rivers) = data.rivers {
                entity_obj.insert("$rivers".to_string(), rivers);
            }
            if let Some(trails) = data.trails {
                entity_obj.insert("$trails".to_string(), trails);
            }
            if let Some(coasts) = data.coasts {
                entity_obj.insert("$coasts".to_string(), coasts);
            }
            if let Some(coast_dir) = data.coast_dir {
                entity_obj.insert("$coast_dir".to_string(), coast_dir);
            }
            if let Some(river_dir) = data.river_dir {
                entity_obj.insert("$river_dir".to_string(), river_dir);
            }
            tx.save(uid)?;
        }
        MapContext::SettlementData => {
            let entity = tx.load(uid)?;
            let entity_obj = entity.as_object().unwrap();
            let builder = SandboxBuilder::from_instance(instance);
            let hex_uid =
                entity_obj["parent_uid"].as_str().unwrap().to_string();
            crate::watabou::map_settlement(
                tx,
                &builder.randomizer,
                hex_map,
                &hex_uid,
            )?;

            hex_map.stage_trails(tx).unwrap();
        }
        MapContext::BuildingData(data) => {
            let entity = tx.load(uid)?;
            let entity_obj = entity.as_object_mut().unwrap();
            entity_obj["x_coords"] = data.x_coords;
            entity_obj["y_coords"] = data.y_coords;
            entity_obj
                .insert("building_index".to_string(), data.building_index);
            tx.save(uid)?;
        }
        MapContext::LocationData(data) => {
            let entity = tx.load(uid)?;
            let entity_obj = entity.as_object_mut().unwrap();
            entity_obj["x_coords"] = data.x_coords;
            entity_obj["y_coords"] = data.y_coords;
            tx.save(uid)?;
        }
    }
    Ok(())
}
