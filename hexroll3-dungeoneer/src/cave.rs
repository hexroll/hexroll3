/*
// Copyright (C) 2020-2026 Pen, Dice & Paper
//
// This program is dual-licensed under the following terms:
//
// Option 1: GNU Affero General Public License (AGPL)
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

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::File,
    io::{self, Write},
};

use rand::{Rng, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DIRS: [i32; 6] = [0, 1, 2, 3, 4, 5];

#[derive(Clone, Copy, Debug, Default)]
struct Hex {
    x: i32,
    y: i32,
    softness: i32,
    r: bool,
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

fn neighbor(hex: Hex, dir: i32) -> Hex {
    match dir {
        0 => Hex {
            x: hex.x,
            y: hex.y - 2,
            ..Default::default()
        },
        1 => Hex {
            x: hex.x + (hex.y.abs() % 2),
            y: hex.y - 1,
            ..Default::default()
        },
        2 => Hex {
            x: hex.x + (hex.y.abs() % 2),
            y: hex.y + 1,
            ..Default::default()
        },
        3 => Hex {
            x: hex.x,
            y: hex.y + 2,
            ..Default::default()
        },
        4 => Hex {
            x: hex.x - ((hex.y + 1).abs() % 2),
            y: hex.y + 1,
            ..Default::default()
        },
        5 => Hex {
            x: hex.x - ((hex.y + 1).abs() % 2),
            y: hex.y - 1,
            ..Default::default()
        },
        _ => panic!("Invalid direction value"),
    }
}

fn num_of_neighbors(hex: Hex, excavated: &BTreeSet<Hex>) -> i32 {
    let mut count = 0;
    for d in DIRS {
        let check = neighbor(hex, d);
        if excavated.contains(&check) {
            count += 1;
        }
    }
    count
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    w: i32,
    h: i32,
}
impl Bounds {
    fn rand_x<R: Rng>(&self, rng: &mut R) -> i32 {
        rng.gen_range(-(self.w / 2)..=(self.w / 2))
    }
    fn rand_y<R: Rng>(&self, rng: &mut R) -> i32 {
        rng.gen_range(-(self.h / 2)..=(self.h / 2))
    }
    fn rand_spot<R: Rng>(&self, rng: &mut R) -> Hex {
        Hex {
            x: self.rand_x(rng),
            y: self.rand_y(rng),
            ..Default::default()
        }
    }
    fn inside(&self, x: i32, y: i32) -> bool {
        x > -(self.w / 2)
            && x < self.w / 2
            && y > -(self.h / 2)
            && y < self.h / 2
    }
}

fn neighbor_interfaces_with_other_area(
    n: Hex,
    excavated_spots: &BTreeSet<Hex>,
    my_spots: &BTreeSet<Hex>,
) -> bool {
    for d in DIRS {
        let on = neighbor(n, d);
        if excavated_spots.contains(&on) && !my_spots.contains(&on) {
            return true;
        }
    }
    false
}

#[derive(Clone, Debug, Default)]
struct Area {
    spots: Vec<Hex>,
    my_spots: BTreeSet<Hex>,
    valid_dirs: Vec<i32>,
    is_outer: bool,
}

impl Area {
    fn new() -> Self {
        Self {
            spots: vec![],
            my_spots: BTreeSet::new(),
            valid_dirs: DIRS.to_vec(),
            is_outer: false,
        }
    }
    fn expand<R: Rng>(
        &mut self,
        rng: &mut R,
        bedrock: &BTreeSet<Hex>,
        excavated_spots: &mut BTreeSet<Hex>,
        bounds: &Bounds,
        avoid_others: bool,
    ) {
        self.spots.shuffle(rng);
        for spot in self.spots.clone().into_iter() {
            self.valid_dirs.shuffle(rng);
            for dir in &self.valid_dirs {
                let mut n = neighbor(spot, *dir);
                if bedrock.contains(&n) || excavated_spots.contains(&n) {
                    continue;
                }
                if !bounds.inside(n.x, n.y) {
                    continue;
                }
                if avoid_others
                    && neighbor_interfaces_with_other_area(
                        n,
                        excavated_spots,
                        &self.my_spots,
                    )
                {
                    continue;
                }
                n.r = avoid_others;
                excavated_spots.insert(n);
                self.spots.push(n);
                self.my_spots.insert(n);
                return;
            }
        }
    }
}

fn is_potential_chamber_start(spot: Hex, excavated: &BTreeSet<Hex>) -> bool {
    let mut count = 0;
    for d in DIRS {
        let n = neighbor(spot, d);
        if excavated.contains(&n) {
            count += 1;
        }
    }
    count == 1
}

fn excavate_cave<R: Rng>(
    rng: &mut R,
    areas: &mut Vec<Area>,
    excavated_spots: &mut BTreeSet<Hex>,
    scale_factor: i32,
) {
    let bounds = Bounds {
        w: 40 / scale_factor,
        h: 120 / scale_factor,
    };

    let mut bedrock: BTreeSet<Hex> = BTreeSet::new();

    // Seed bedrock
    for _ in 0..(2400 / scale_factor) {
        let mut seed = bounds.rand_spot(rng);
        seed.softness = 0;

        let xb = 2 * 2;
        let yb = 4 * 3;
        if seed.x < xb && seed.x > -xb && seed.y < yb && seed.y > -yb {
            if rng.gen_range(1..=3) != 1 {
                continue;
            }
        }
        bedrock.insert(seed);
    }

    // Remove isolated rocks
    let mut to_remove = vec![];
    for &h in bedrock.iter() {
        let mut isolated = true;
        for d in DIRS {
            let n = neighbor(h, d);
            if bedrock.contains(&n) {
                isolated = false;
                break;
            }
        }
        if isolated {
            to_remove.push(h);
        }
    }
    for h in to_remove {
        bedrock.remove(&h);
    }

    bedrock.remove(&Hex {
        x: 1,
        y: 1,
        ..Default::default()
    });

    // Excavate main cavern
    let main = Hex {
        x: 1,
        y: 1,
        softness: 1,
        r: false,
    };
    let mut area = Area::new();
    excavated_spots.insert(main);
    area.spots.push(main);
    //?
    area.my_spots.insert(main);

    for _ in 0..600 {
        area.expand(rng, &bedrock, excavated_spots, &bounds, false);
    }
    areas.push(area.clone());

    // Find potential edge tunnels
    let mut chamber_starts: Vec<Hex> = vec![];
    for &spot in area.spots.iter() {
        if is_potential_chamber_start(spot, excavated_spots) {
            for d in DIRS {
                let potential = neighbor(spot, d);
                if num_of_neighbors(potential, excavated_spots) == 1 {
                    chamber_starts.push(potential);
                    break;
                }
            }
        }
    }

    // Create an area for each chamber
    for start in chamber_starts {
        let mut chamber = Area::new();
        chamber.spots.push(start);
        //?
        chamber.my_spots.insert(start);

        let steps = rng.gen_range(10..=40);
        for _ in 0..steps {
            let fake_bedrock: BTreeSet<Hex> = BTreeSet::new();
            chamber.expand(rng, &fake_bedrock, excavated_spots, &bounds, true);
        }
        areas.push(chamber);
    }
}

fn classify_caverns(
    caverns: &mut Vec<Area>,
    excavated_areas: &[Area],
    _excavated_spots2: &BTreeSet<Hex>,
    _pressure_threshold: i32,
) {
    let mut staging: Vec<Area> = vec![];

    let mut excavated_spots: BTreeSet<Hex> = BTreeSet::new();
    let mut is_outer_map: BTreeMap<Hex, bool> = BTreeMap::new();

    for a in excavated_areas {
        for &h in &a.spots {
            excavated_spots.insert(h);
            is_outer_map.insert(h, h.r);
        }
    }

    let mut neighbor_count_map: BTreeMap<Hex, i32> = BTreeMap::new();
    for a in excavated_areas {
        for &h in &a.spots {
            neighbor_count_map.insert(h, num_of_neighbors(h, &excavated_spots));
        }
    }

    let start = Hex {
        x: 1,
        y: 1,
        ..Default::default()
    };
    let mut backlog: VecDeque<Hex> = VecDeque::new();
    let mut scope: VecDeque<Hex> = VecDeque::new();
    backlog.push_back(start);

    let mut pressure: i32 = 0;
    let mut area_hexes: Vec<Hex> = vec![];
    let mut visited: BTreeSet<Hex> = BTreeSet::new();

    while let Some(current0) = backlog.pop_front() {
        let mut current = current0;
        if !visited.contains(&current) {
            visited.insert(current);

            let neigh = *neighbor_count_map.get(&current).unwrap();
            pressure += 6 - neigh;

            current.r = *is_outer_map.get(&current).unwrap();
            area_hexes.push(current);

            for d in DIRS {
                let next = neighbor(current, d);
                if excavated_spots.contains(&next) {
                    backlog.push_back(next);
                    scope.push_back(next);
                }
            }
        }

        if pressure >= 30 || backlog.is_empty() {
            let mut a = Area::new();
            a.is_outer = true;
            for &collected in &area_hexes {
                if !collected.r {
                    a.is_outer = false;
                }
                a.spots.push(collected);
                a.my_spots.insert(collected);
            }
            staging.push(a);

            area_hexes.clear();
            pressure = 0;

            if let Some(mut nex) = scope.front().copied() {
                while visited.contains(&nex) {
                    scope.pop_front();
                    if scope.is_empty() {
                        break;
                    }
                    nex = *scope.front().unwrap();
                }
                backlog = VecDeque::new();
                if !visited.contains(&nex) {
                    backlog.push_back(nex);
                }
            }
        }
    }

    for cavern in staging {
        if cavern.spots.len() > 3 {
            caverns.push(cavern);
        }
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Point {
    pub x: i32,
    pub y: i32,
    pub c: i32,
}

impl PartialEq for Point {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl Eq for Point {}

const ARM: i32 = 14;

fn hex_points_at_offset(offset_x: i32, offset_y: i32) -> Vec<Point> {
    let (w, h) = (2 * ARM, 24);

    let half_arm: i32 = 7;
    let half_h: i32 = 12;

    let p0 = Point {
        x: half_arm + offset_x,
        y: 0 + offset_y,
        c: 0,
    };
    let p1 = Point {
        x: (w - half_arm) + offset_x,
        y: 0 + offset_y,
        c: 0,
    };
    let p2 = Point {
        x: w + offset_x,
        y: half_h + offset_y,
        c: 0,
    };
    let p3 = Point {
        x: (w - half_arm) + offset_x,
        y: h + offset_y,
        c: 0,
    };
    let p4 = Point {
        x: half_arm + offset_x,
        y: h + offset_y,
        c: 0,
    };
    let p5 = Point {
        x: 0 + offset_x,
        y: half_h + offset_y,
        c: 0,
    };

    vec![p0, p1, p2, p3, p4, p5]
}

fn get_hex_points2(p: Point) -> Vec<Point> {
    let offset_x: i32 = p.x - 14;
    let offset_y: i32 = p.y - 12;
    hex_points_at_offset(offset_x, offset_y)
}

fn get_hex_points(hex: Hex) -> Vec<Point> {
    let pitch_x: i32 = 3 * ARM;
    let pitch_y_i32: i32 = 12;

    let pitch_y: i32 = pitch_y_i32 as i32;

    let odd_row: i32 = (hex.y.abs() & 1) as i32;
    let half_pitch_x: i32 = 21;

    let offset_x: i32 = hex.x * pitch_x + odd_row * half_pitch_x;
    let offset_y: i32 = hex.y * pitch_y;

    hex_points_at_offset(offset_x, offset_y)
}

#[derive(Clone, Debug)]
struct PolygonizedArea {
    polygon: Vec<Point>,
    n_hexes: usize,
    is_outer: bool,
}

fn polygonize(polygons: &mut Vec<PolygonizedArea>, caverns: &[Area]) {
    let mut cave_points_with_cavern_count: BTreeMap<Point, i32> =
        BTreeMap::new();

    for a in caverns {
        let mut cavern_points_with_hex_count: BTreeMap<Point, i32> =
            BTreeMap::new();
        for &hex in &a.spots {
            for p in get_hex_points(hex) {
                if let Some(v) = cavern_points_with_hex_count.get_mut(&p) {
                    *v += 1;
                } else {
                    cavern_points_with_hex_count.insert(p, 1);
                    *cave_points_with_cavern_count.entry(p).or_insert(0) += 1;
                }
            }
        }
    }

    for a in caverns {
        let mut all_points: Vec<Point> = vec![];
        let mut all_points_to_hex: BTreeMap<Point, BTreeSet<Hex>> =
            BTreeMap::new();
        let mut cavern_points_with_hex_count: BTreeMap<Point, i32> =
            BTreeMap::new();

        for &hex in &a.spots {
            for p in get_hex_points(hex) {
                all_points_to_hex.entry(p).or_default().insert(hex);
                *cavern_points_with_hex_count.entry(p).or_insert(0) += 1;
            }
        }

        for (p, c) in cavern_points_with_hex_count.iter() {
            if *c < 3 {
                all_points.push(*p);
            }
        }
        if all_points.is_empty() {
            continue;
        }

        let mut filter: BTreeSet<Point> = BTreeSet::new();
        let start = *all_points.last().unwrap();
        let mut p = start;
        let mut actual: Vec<Point> = vec![];

        loop {
            if filter.contains(&p) {
                break;
            }
            filter.insert(p);

            let mut p_with_c = p;
            p_with_c.c = *cave_points_with_cavern_count.get(&p).unwrap();

            let mut scaled = p_with_c;
            scaled.x = (scaled.x as f64 * 0.5) as i32;
            scaled.y = (scaled.y as f64 * 0.5) as i32;
            actual.push(scaled);

            let to_check = get_hex_points2(p);
            let mut match_pts: Vec<Point> = vec![];

            for r in to_check {
                if filter.contains(&r) {
                    continue;
                }
                for c in &all_points {
                    if filter.contains(c) {
                        continue;
                    }
                    if *c == r {
                        match_pts.push(r);
                    }
                }
            }

            if match_pts.len() == 2 {
                let p1 = match_pts[0];
                let p2 = match_pts[1];

                let c_p1 = *cavern_points_with_hex_count.get(&p1).unwrap();
                let c_p = *cavern_points_with_hex_count.get(&p).unwrap();
                let c_p2 = *cavern_points_with_hex_count.get(&p2).unwrap();

                let set_p = all_points_to_hex.get(&p).cloned().unwrap();
                let set_p1 = all_points_to_hex.get(&p1).cloned().unwrap();
                let set_p2 = all_points_to_hex.get(&p2).cloned().unwrap();

                p = if c_p1 == 1 {
                    if c_p == 1 {
                        if c_p2 == 1 {
                            if set_p == set_p2 { p2 } else { p1 }
                        } else {
                            p2
                        }
                    } else {
                        p1
                    }
                } else if c_p2 == 1 {
                    if c_p == 1 {
                        if c_p1 == 1 {
                            if set_p == set_p1 { p1 } else { p2 }
                        } else {
                            p1
                        }
                    } else {
                        p2
                    }
                } else {
                    if set_p == set_p2 { p1 } else { p2 }
                };
            } else if match_pts.len() == 1 {
                p = match_pts[0];
            } else {
                break;
            }
        }

        polygons.push(PolygonizedArea {
            polygon: actual,
            n_hexes: a.spots.len(),
            is_outer: a.is_outer,
        });
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Cavern {
    pub polygon: Vec<Point>,
    pub n_hexes: usize,
    pub is_outer: bool,
}

#[derive(Clone, Debug)]
pub struct CaveBuilder {
    pub caverns: Vec<Cavern>,
}

impl CaveBuilder {
    pub fn new<R: Rng>(rng: &mut R, prefer_small_dungeons: bool) -> Self {
        let mut caverns: Vec<Area> = vec![];
        let mut excavated_areas: Vec<Area> = vec![];
        let mut excavated_spots: BTreeSet<Hex> = BTreeSet::new();

        let scale_factor = if prefer_small_dungeons {
            3
        } else {
            rng.gen_range(1..=2)
        };

        excavate_cave(
            rng,
            &mut excavated_areas,
            &mut excavated_spots,
            scale_factor,
        );

        classify_caverns(
            &mut caverns,
            &excavated_areas,
            &excavated_spots,
            if scale_factor == 2 { 30 } else { 20 },
        );

        let mut polygons: Vec<PolygonizedArea> = vec![];
        polygonize(&mut polygons, &caverns);

        let mut out = vec![];
        for pa in polygons {
            out.push(Cavern {
                polygon: pa.polygon,
                n_hexes: pa.n_hexes,
                is_outer: pa.is_outer,
            });
        }

        let cave = Self { caverns: out };

        if true {
            if let Err(e) = write_output_js(&excavated_areas, &cave) {
                eprintln!("failed to write output.js: {e}");
            }
        }

        cave
    }

    pub fn as_json(&self) -> Value {
        let mut caverns_json = Vec::new();
        let mut n = 1;

        for c in &self.caverns {
            let polygon: Vec<Value> = c
                .polygon
                .iter()
                .map(|p| json!({ "x": p.x, "y": p.y, "c": p.c }))
                .collect();

            caverns_json.push(json!({
                "polygon": polygon,
                "n": n,
                "n_hexes": c.n_hexes,
                "is_outer": c.is_outer
            }));
            n += 1;
        }

        json!({ "caverns": caverns_json })
    }
}

fn write_output_js(
    excavated_areas: &[Area],
    cave: &CaveBuilder,
) -> io::Result<()> {
    let mut fout = File::create("output.js")?;

    writeln!(
        fout,
        "import {{draw_hex, draw_circle, draw_polygon}} from './hex.js'"
    )?;
    writeln!(fout, "export function draw() {{")?;

    let mut color: i32 = 0;
    for area in excavated_areas {
        for hex in &area.spots {
            writeln!(
                fout,
                "draw_hex({{x:{}, y:{}}}, {});",
                hex.x, hex.y, color
            )?;
        }
        color += 1;
    }

    color = 0;
    for cavern in &cave.caverns {
        write!(fout, "draw_polygon([")?;
        for p in &cavern.polygon {
            write!(fout, "{{x:{}, y:{}}}, ", p.x, p.y)?;
        }
        writeln!(fout, "], {});", color)?;
        color += 1;
    }

    writeln!(fout, "}}")?;
    Ok(())
}
