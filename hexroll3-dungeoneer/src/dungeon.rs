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

#![allow(dead_code)]
use std::cmp::{max, min};
use std::f64;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Coords {
    pub x: i32,
    pub y: i32,
}

impl Coords {
    #[inline]
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    #[inline]
    fn radius(&self) -> i32 {
        (((self.x as f64) * (self.x as f64)
            + (self.y as f64) * (self.y as f64))
            .sqrt()) as i32
    }

    #[inline]
    fn next_by_dir(&self, dir: i32) -> Self {
        let dx = if dir == 1 {
            1
        } else if dir == 3 {
            -1
        } else {
            0
        };
        let dy = if dir == 0 {
            -1
        } else if dir == 2 {
            1
        } else {
            0
        };
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    #[inline]
    fn is_inside(&self, r: &Rect, buffer: i32) -> bool {
        self.x <= r.b.x + buffer
            && self.x >= r.a.x - buffer
            && self.y >= r.a.y - buffer
            && self.y <= r.b.y + buffer
    }

    #[inline]
    fn is_on_corner(&self, r: &Rect, _buffer: i32) -> bool {
        (self.x == r.a.x - 1 && self.y == r.a.y - 1)
            || (self.x == r.a.x - 1 && self.y == r.b.y + 1)
            || (self.x == r.b.x + 1 && self.y == r.a.y - 1)
            || (self.x == r.b.x + 1 && self.y == r.b.y + 1)
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub a: Coords,
    pub b: Coords,
}

impl Rect {
    #[inline]
    fn new(a: Coords, b: Coords) -> Self {
        Self { a, b }
    }

    #[inline]
    fn width(&self) -> i32 {
        (self.b.x - self.a.x).abs() + 1
    }

    #[inline]
    fn height(&self) -> i32 {
        (self.b.y - self.a.y).abs() + 1
    }

    #[inline]
    fn is_intersecting(&self, r2: &Rect, buffer: i32) -> bool {
        self.a.x <= r2.b.x + buffer
            && self.b.x >= r2.a.x - buffer
            && self.a.y <= r2.b.y + buffer
            && self.b.y >= r2.a.y - buffer
    }

    #[inline]
    fn inflate(&mut self, amount: i32) {
        self.a.y -= amount;
        self.b.y += amount;
        self.b.x += amount;
        self.a.x -= amount;
    }

    #[inline]
    fn grow_by_dir(&mut self, dir: i32, x: i32) {
        if dir == 3 {
            self.a.x -= x;
        }
        if dir == 0 {
            self.a.y -= x;
        }
        if dir == 1 {
            self.b.x += x;
        }
        if dir == 2 {
            self.b.y += x;
        }
    }

    #[inline]
    fn out_of_bounds(&self, max_leg: i32) -> bool {
        self.b.y > max_leg
            || self.b.x > max_leg
            || self.a.x < -max_leg
            || self.a.y < -max_leg
    }

    #[inline]
    fn size(&self) -> i32 {
        ((self.b.x - self.a.x + 1).abs()) * ((self.b.y - self.a.y + 1).abs())
    }

    #[inline]
    fn length(&self) -> i32 {
        let w = self.width();
        let h = self.height();
        if w > h { w } else { h }
    }

    #[inline]
    fn center(&self) -> Coords {
        Coords::new((self.a.x + self.b.x) / 2, (self.a.y + self.b.y) / 2)
    }
}

#[derive(Clone, Debug)]
pub struct Area {
    pub rect: Rect,
    pub number: i32,
    pub done: bool,
    pub filtered: bool,
}

impl Area {
    #[inline]
    pub fn new(a: Coords, b: Coords, done: bool) -> Self {
        Self {
            rect: Rect::new(a, b),
            number: 0,
            done,
            filtered: false,
        }
    }

    #[inline]
    pub fn a(&self) -> Coords {
        self.rect.a
    }
    #[inline]
    pub fn b(&self) -> Coords {
        self.rect.b
    }

    #[inline]
    pub fn width(&self) -> i32 {
        self.rect.width()
    }
    #[inline]
    pub fn height(&self) -> i32 {
        self.rect.height()
    }
    #[inline]
    pub fn size(&self) -> i32 {
        self.rect.size()
    }
    #[inline]
    pub fn length(&self) -> i32 {
        self.rect.length()
    }
    #[inline]
    pub fn center(&self) -> Coords {
        self.rect.center()
    }

    #[inline]
    pub fn grow_by_dir(&mut self, dir: i32) {
        self.rect.grow_by_dir(dir, 1);
    }
}

#[derive(Clone, Debug)]
pub struct Room {
    pub area: Area,
    pub centerpiece: bool,
    pub feature_tier: i32,
}

impl Room {
    #[inline]
    fn new(a: Coords, b: Coords, done: bool, centerpiece: bool) -> Self {
        Self {
            area: Area::new(a, b, done),
            centerpiece,
            feature_tier: 0,
        }
    }

    #[inline]
    fn filter(&mut self) -> bool {
        self.area.filtered = if self.centerpiece { false } else { true };
        self.area.filtered
    }

    #[inline]
    fn portal_start(&self, dir: i32) -> Coords {
        let a = self.area.rect.a;
        let b = self.area.rect.b;
        match dir {
            0 => Coords::new((a.x + b.x) / 2, a.y - 1),
            2 => Coords::new((a.x + b.x) / 2, b.y + 1),
            3 => Coords::new(a.x - 1, (a.y + b.y) / 2),
            1 => Coords::new(b.x + 1, (a.y + b.y) / 2),
            _ => Coords::new(0, 0),
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct PassageEnd {
    pub pos: Coords,
    pub dir: i32,
    pub type_: i32,
}

#[derive(Clone, Debug)]
pub struct Passage {
    pub area: Area,
    pub last_close_call: bool,
    pub passage_dir: i32,
    pub empty_end: Option<PassageEnd>,
    pub entrance: Option<PassageEnd>,
    pub deadend: Option<PassageEnd>,
}

impl Passage {
    #[inline]
    fn new(a: Coords, b: Coords, dir: i32) -> Self {
        Self {
            area: Area::new(a, b, false),
            last_close_call: false,
            passage_dir: dir,
            empty_end: None,
            entrance: None,
            deadend: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Portal {
    pub type_: i32,
    pub wall: String,
    pub pos: Coords,
    pub room_number: i32,
    pub leading_to: i32,
}

#[inline]
fn is_in_range<T: PartialOrd>(v: T, r1: T, r2: T) -> bool {
    v >= r1 && v <= r2
}

#[inline]
fn get_opposing_dir(wall_dir: i32) -> i32 {
    match wall_dir {
        0 => 2,
        1 => 3,
        2 => 0,
        3 => 1,
        _ => -1,
    }
}

#[inline]
fn is_same_plane(a: &Passage, b: &Passage) -> bool {
    let av = a.passage_dir == 0 || a.passage_dir == 2;
    let bv = b.passage_dir == 0 || b.passage_dir == 2;
    (av && bv) || (!av && !bv)
}

#[inline]
fn shared_span<T: MapArea, K: MapArea>(a1: &T, a2: &K) -> i32 {
    let a1r = a1.area().rect();
    let a2r = a2.area().rect();

    if a1r.a.x == a1r.b.x {
        if a2r.a.x == a2r.a.x {
            if (a1r.a.x - a2r.a.x).abs() != 1 {
                return 0;
            } else {
                let aa = min(a1r.a.y, a1r.b.y);
                let ab = max(a1r.a.y, a1r.b.y);
                let ba = min(a2r.a.y, a2r.b.y);
                let bb = max(a2r.a.y, a2r.b.y);
                let res = min(ab, bb) - max(aa, ba);
                return if res >= 0 { res + 1 } else { 0 };
            }
        } else if (a2r.a.x - a1r.a.x).abs() == 1
            || (a2r.b.x - a1r.a.x).abs() == 1
        {
            return 1;
        } else {
            return 0;
        }
    }
    if a1r.a.y == a1r.b.y {
        if a2r.a.y == a2r.a.y {
            if (a1r.a.y - a2r.a.y).abs() != 1 {
                return 0;
            } else {
                let aa = min(a1r.a.x, a1r.b.x);
                let ab = max(a1r.a.x, a1r.b.x);
                let ba = min(a2r.a.x, a2r.b.x);
                let bb = max(a2r.a.x, a2r.b.x);
                let res = min(ab, bb) - max(aa, ba);
                return if res >= 0 { res + 1 } else { 0 };
            }
        } else if (a2r.a.y - a1r.a.y).abs() == 1
            || (a2r.b.y - a1r.a.y).abs() == 1
        {
            return 1;
        } else {
            return 0;
        }
    }
    9
}

impl Area {
    #[inline]
    pub fn rect(&self) -> &Rect {
        &self.rect
    }
}

trait MapArea {
    fn area(&self) -> &Area;
}

impl MapArea for Room {
    #[inline]
    fn area(&self) -> &Area {
        &self.area
    }
}

impl MapArea for Passage {
    #[inline]
    fn area(&self) -> &Area {
        &self.area
    }
}

#[derive(Clone)]
struct LimitedBounds {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
}

impl LimitedBounds {
    fn new() -> Self {
        Self {
            min_x: -30,
            min_y: -30,
            max_x: 30,
            max_y: 30,
        }
    }

    #[inline]
    fn update_rect(&mut self, r: &Rect) {
        if r.a.x < self.min_x {
            self.min_x = r.a.x;
        }
        if r.a.y < self.min_y {
            self.min_y = r.a.y;
        }
        if r.b.x > self.max_x {
            self.max_x = r.b.x;
        }
        if r.b.y > self.max_y {
            self.max_y = r.b.y;
        }
    }

    #[inline]
    fn update_room(&mut self, r: &Room) {
        self.update_rect(&r.area.rect);
    }

    #[inline]
    fn are_invalid(&self) -> bool {
        self.min_x + 1 > self.max_x - 1 || self.min_y + 1 > self.max_y - 1
    }

    #[inline]
    fn inflate(&mut self, v: i32) {
        self.min_x -= v;
        self.min_y -= v;
        self.max_x += v;
        self.max_y += v;
    }

    #[inline]
    fn point_within_range(&self, rng: &mut Randomizers, range: i32) -> Coords {
        let nx = rng.roll_in_range(self.min_x + range, self.max_x - range);
        let ny = rng.roll_in_range(self.min_y + range, self.max_y - range);
        Coords::new(nx, ny)
    }
}

#[derive(Copy, Clone, Debug)]
struct MapConfig {
    max_leg: i32,
    centerpiece: bool,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            max_leg: 30,
            centerpiece: true,
        }
    }
}

const CHECK_MAX_NUM_OF_PORTALS: i32 = 5;
const CHECK_MAX_LENGTH_OF_PASSAGE: i32 = 10;
const ITERATIONS: i32 = 100;

const LEFT_EDGE: i32 = 1;
const RIGHT_EDGE: i32 = 2;
const TOP_EDGE: i32 = 4;
const BOTTOM_EDGE: i32 = 8;

pub struct DungeonBuilder {
    n_entrances: i32,
    bias_square_rooms: bool,
    randomizers: Randomizers,
    passages: Vec<Passage>,
    rooms: Vec<Room>,
    bounds: LimitedBounds,
    config: MapConfig,
}

impl DungeonBuilder {
    pub fn new(mut randomizers: Randomizers) -> Self {
        let _ = &mut randomizers;
        Self {
            n_entrances: 0,
            bias_square_rooms: true,
            randomizers,
            passages: Vec::new(),
            rooms: Vec::new(),
            bounds: LimitedBounds::new(),
            config: MapConfig::default(),
        }
    }

    #[inline]
    fn rand(&mut self, x: i32, y: i32) -> i32 {
        self.randomizers.roll_in_range(x, y)
    }

    fn can_grow_in_dir(&mut self, r: &Area, dir: i32) -> bool {
        let mut test = r.rect;
        test.grow_by_dir(dir, 1);
        if test.out_of_bounds(self.config.max_leg) {
            return false;
        }

        let w = ((test.b.x - test.a.x).abs() + 1) as f64;
        let h = ((test.b.y - test.a.y).abs() + 1) as f64;
        let ratio = w / h;

        let roll = self.rand(1, 6);
        if roll > 5 {
            if !is_in_range(ratio, 0.5_f64, 3.0_f64) {
                return false;
            }
        } else if roll > 0 {
            if !is_in_range(ratio, 0.75_f64, 1.25_f64) {
                return false;
            }
        }

        for t in &self.rooms {
            if t.area.rect == r.rect {
                continue;
            }
            if test.is_intersecting(&t.area.rect, 2) {
                return false;
            }
        }
        for t in &self.passages {
            if test.is_intersecting(&t.area.rect, 1) {
                return false;
            }
        }
        true
    }

    fn num_of_portals(&self, r: &Area) -> i32 {
        let mut c = 0;
        for p in &self.passages {
            let mut wall_outside = r.rect;
            wall_outside.inflate(1);
            if p.area.rect.is_intersecting(&wall_outside, 0) {
                c += 1;
            }
        }
        c
    }

    fn has_passage_on_wall(&self, r: &Area, dir: i32) -> bool {
        for p in &self.passages {
            let mut wall_outside = r.rect;
            if dir == 0 {
                wall_outside.a.y -= 1;
                wall_outside.b.y = wall_outside.a.y;
            }
            if dir == 2 {
                wall_outside.b.y += 1;
                wall_outside.a.y = wall_outside.b.y;
            }
            if dir == 1 {
                wall_outside.b.x += 1;
                wall_outside.a.x = wall_outside.b.x;
            }
            if dir == 3 {
                wall_outside.a.x -= 1;
                wall_outside.b.x = wall_outside.a.x;
            }
            if p.area.rect.is_intersecting(&wall_outside, 0) {
                return true;
            }
        }
        false
    }

    fn is_valid_portal_position(
        &self,
        new_seed: Coords,
        dir: i32,
        t: &Room,
    ) -> bool {
        if new_seed.is_on_corner(&t.area.rect, 0) {
            return false;
        }
        if new_seed.next_by_dir(dir).is_on_corner(&t.area.rect, 0) {
            return false;
        }

        if new_seed.is_inside(&t.area.rect, 1) {
            let opposing_dir = get_opposing_dir(dir);
            if self.has_passage_on_wall(&t.area, opposing_dir) {
                return false;
            }
            if self.num_of_portals(&t.area) > CHECK_MAX_NUM_OF_PORTALS {
                return false;
            }
        }
        true
    }

    fn is_valid_passage_position(
        &self,
        new_seed: Coords,
        _dir: i32,
        t: &Passage,
    ) -> bool {
        if new_seed.is_on_corner(&t.area.rect, 0) {
            return false;
        }
        if new_seed.is_inside(&t.area.rect, 0) {
            return false;
        }
        let seed_area = Rect::new(new_seed, new_seed);
        let seed_like = PhantomArea {
            area: Area::new(seed_area.a, seed_area.b, true),
        };
        if shared_span(&seed_like, t) > 1 {
            return false;
        }
        true
    }

    fn seed_portals(&mut self) {
        let rooms_len = self.rooms.len();
        for i in 0..rooms_len {
            if self.rooms[i].area.filtered {
                continue;
            }
            if self.num_of_portals(&self.rooms[i].area)
                > CHECK_MAX_NUM_OF_PORTALS
            {
                continue;
            }

            let mut dirs = [0_i32, 1_i32, 2_i32, 3_i32];
            self.randomizers.shuffle_i32_4(&mut dirs);

            for k in 0..4 {
                let wall_dir = dirs[k];
                let new_seed = self.rooms[i].portal_start(wall_dir);

                if new_seed.x > 5 && wall_dir == 1 {
                    continue;
                }
                if new_seed.x < -5 && wall_dir == 3 {
                    continue;
                }
                if new_seed.y < -5 && wall_dir == 0 {
                    continue;
                }
                if new_seed.y > 5 && wall_dir == 2 {
                    continue;
                }

                if self.has_passage_on_wall(&self.rooms[i].area, wall_dir) {
                    continue;
                }

                let mut valid = true;
                for t in &self.rooms {
                    if t.area.filtered {
                        continue;
                    }
                    if t.area.rect == self.rooms[i].area.rect {
                        continue;
                    }
                    if !self.is_valid_portal_position(new_seed, wall_dir, t) {
                        valid = false;
                        break;
                    }
                }
                if !valid {
                    continue;
                }
                for t in &self.passages {
                    if t.area.filtered {
                        continue;
                    }
                    if !self.is_valid_passage_position(new_seed, wall_dir, t) {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    self.passages
                        .push(Passage::new(new_seed, new_seed, wall_dir));
                }
            }
        }
    }

    fn closes_a_square(&self, seed: Coords, backlog: &[Passage]) -> bool {
        let nn = Coords::new(seed.x + 0, seed.y - 1);
        let ne = Coords::new(seed.x + 1, seed.y - 1);
        let ee = Coords::new(seed.x + 1, seed.y + 0);
        let se = Coords::new(seed.x + 1, seed.y + 1);
        let ss = Coords::new(seed.x + 0, seed.y + 1);
        let sw = Coords::new(seed.x - 1, seed.y + 1);
        let ww = Coords::new(seed.x - 1, seed.y + 0);
        let nw = Coords::new(seed.x - 1, seed.y - 1);

        let mut nn_bit = false;
        let mut ne_bit = false;
        let mut ee_bit = false;
        let mut se_bit = false;
        let mut ss_bit = false;
        let mut sw_bit = false;
        let mut ww_bit = false;
        let mut nw_bit = false;

        let mut check = |p: &Passage| {
            let r = &p.area.rect;
            if nn.is_inside(r, 0) {
                nn_bit = true;
            }
            if ne.is_inside(r, 0) {
                ne_bit = true;
            }
            if ee.is_inside(r, 0) {
                ee_bit = true;
            }
            if se.is_inside(r, 0) {
                se_bit = true;
            }
            if ss.is_inside(r, 0) {
                ss_bit = true;
            }
            if sw.is_inside(r, 0) {
                sw_bit = true;
            }
            if ww.is_inside(r, 0) {
                ww_bit = true;
            }
            if nw.is_inside(r, 0) {
                nw_bit = true;
            }
        };

        for p in &self.passages {
            check(p);
        }
        for p in backlog {
            check(p);
        }

        (nn_bit && ne_bit && ee_bit)
            || (ee_bit && se_bit && ss_bit)
            || (ss_bit && sw_bit && ww_bit)
            || (ww_bit && nw_bit && nn_bit)
    }

    fn passage_can_grow_in_dir(
        &mut self,
        r_index: usize,
        backlog: &[Passage],
        dir: i32,
    ) -> bool {
        if self.passages[r_index].area.length() > CHECK_MAX_LENGTH_OF_PASSAGE {
            return false;
        }

        let mut test = self.passages[r_index].area.rect;
        test.grow_by_dir(dir, 1);

        let mut newp = Coords::new(0, 0);
        let orig = self.passages[r_index].area.rect;
        if orig.a.x != test.a.x || orig.a.y != test.a.y {
            newp = Coords::new(test.a.x, test.a.y);
        }
        if orig.b.x != test.b.x || orig.b.y != test.b.y {
            newp = Coords::new(test.b.x, test.b.y);
        }

        if test.out_of_bounds(self.config.max_leg) {
            return false;
        }

        if self.closes_a_square(newp, backlog) {
            return false;
        }

        let w = (test.b.x - test.a.x).abs() + 1;
        let h = (test.b.y - test.a.y).abs() + 1;
        if w > 1 && h > 1 {
            self.passages[r_index].area.done = true;
            return false;
        }

        let testing_rect = Rect::new(newp, newp);

        for j in 0..self.passages.len() {
            if j == r_index {
                continue;
            }
            let t = self.passages[j].clone();
            if test.is_intersecting(&t.area.rect, 0) {
                self.passages[r_index].area.done = true;
                return false;
            }
            if t.area.rect.is_intersecting(&testing_rect, 1) {
                if self.passages[r_index].last_close_call {
                    self.passages[r_index].area.done = true;
                    return false;
                }
                self.passages[r_index].last_close_call = true;
            } else {
                self.passages[r_index].last_close_call = false;
            }
            let span = shared_span(&t, &self.passages[r_index]);
            if span > 0 && is_same_plane(&t, &self.passages[r_index]) {
                self.passages[r_index].area.done = true;
                return false;
            }
        }

        for t in backlog {
            if test.is_intersecting(&t.area.rect, 0) {
                self.passages[r_index].area.done = true;
                return false;
            }
            if t.area.rect.is_intersecting(&testing_rect, 1) {
                if self.passages[r_index].last_close_call {
                    self.passages[r_index].area.done = true;
                    return false;
                }
                self.passages[r_index].last_close_call = true;
            } else {
                self.passages[r_index].last_close_call = false;
            }
            let span = shared_span(t, &self.passages[r_index]);
            if span > 0 && is_same_plane(t, &self.passages[r_index]) {
                self.passages[r_index].area.done = true;
                return false;
            }
        }

        for t in &self.rooms {
            if t.area.filtered {
                continue;
            }
            if newp.is_on_corner(&t.area.rect, 0) {
                return false;
            }
            if test.is_intersecting(&t.area.rect, 0) {
                self.passages[r_index].area.done = true;
                return false;
            }
            if newp.is_inside(&t.area.rect, 1) {
                if self.has_passage_on_wall(&t.area, get_opposing_dir(dir)) {
                    return false;
                }
                if self.num_of_portals(&t.area) > CHECK_MAX_NUM_OF_PORTALS {
                    return false;
                }
            }
        }

        true
    }

    fn extend_passages(&mut self) {
        let mut backlog: Vec<Passage> = Vec::new();

        let mut i = 0usize;
        while i < self.passages.len() {
            if self.passages[i].area.done {
                i += 1;
                continue;
            }

            let dir = self.passages[i].passage_dir;
            if self.passage_can_grow_in_dir(i, &backlog, dir) {
                self.passages[i].area.grow_by_dir(dir);
                i += 1;
                continue;
            }

            if self.passages[i].area.rect.a == self.passages[i].area.rect.b {
                let mut n_rooms_met = 0;
                for t in &self.rooms {
                    if self.passages[i]
                        .area
                        .rect
                        .is_intersecting(&t.area.rect, 1)
                    {
                        n_rooms_met += 1;
                        if n_rooms_met > 1 {
                            break;
                        }
                    }
                }
                if n_rooms_met != 2 {
                    self.passages.remove(i);
                    break;
                }
            }

            if self.passages[i].area.done {
                i += 1;
                continue;
            }

            let mut new_seed = Coords::new(0, 0);
            let mut ndir = 0;

            let r = self.passages[i].clone();
            if r.passage_dir == 3 {
                new_seed.x = r.area.rect.a.x;
                if self.rand(0, 1) == 1 {
                    new_seed.y = r.area.rect.a.y - 1;
                    ndir = 0;
                } else {
                    new_seed.y = r.area.rect.a.y + 1;
                    ndir = 2;
                }
            }
            if r.passage_dir == 1 {
                new_seed.x = r.area.rect.b.x;
                if self.rand(0, 1) == 1 {
                    new_seed.y = r.area.rect.b.y - 1;
                    ndir = 0;
                } else {
                    new_seed.y = r.area.rect.b.y + 1;
                    ndir = 2;
                }
            }
            if r.passage_dir == 0 {
                new_seed.y = r.area.rect.a.y;
                if self.rand(0, 1) == 1 {
                    new_seed.x = r.area.rect.a.x - 1;
                    ndir = 3;
                } else {
                    new_seed.x = r.area.rect.a.x + 1;
                    ndir = 1;
                }
            }
            if r.passage_dir == 2 {
                new_seed.y = r.area.rect.b.y;
                if self.rand(0, 1) == 1 {
                    new_seed.x = r.area.rect.b.x - 1;
                    ndir = 3;
                } else {
                    new_seed.x = r.area.rect.b.x + 1;
                    ndir = 1;
                }
            }

            let mut valid = true;
            for t in &self.rooms {
                if t.area.filtered {
                    continue;
                }
                if new_seed.is_on_corner(&t.area.rect, 0) {
                    valid = false;
                    break;
                }
                if new_seed.is_inside(&t.area.rect, 1) {
                    valid = false;
                }
            }

            if valid {
                for t in &self.passages {
                    if !self.is_valid_passage_position(new_seed, ndir, t) {
                        valid = false;
                        break;
                    }
                }
            }
            if valid {
                for t in &backlog {
                    if !self.is_valid_passage_position(new_seed, ndir, t) {
                        valid = false;
                        break;
                    }
                }
            }

            if valid && self.closes_a_square(new_seed, &backlog) {
                valid = false;
            }

            if valid {
                backlog.push(Passage::new(new_seed, new_seed, ndir));
            }

            i += 1;
        }

        self.passages.extend(backlog);
    }

    fn grow_existing_rooms_to_limit(&mut self) {
        for i in 0..self.rooms.len() {
            if self.rand(1, 2) == 1 {
                continue;
            }
            if self.rooms[i].area.done {
                continue;
            }

            self.bounds.update_room(&self.rooms[i]);

            let r_center_radius = self.rooms[i].area.center().radius();
            let w = self.rooms[i].area.width();
            let h = self.rooms[i].area.height();
            let size = self.rooms[i].area.size();

            if r_center_radius < 10 {
                if self.bias_square_rooms {
                    if w == h && w >= 7 && (w % 2 == 1) && self.rand(1, 2) == 1
                    {
                        self.rooms[i].area.done = true;
                        continue;
                    }
                }
                if size > self.rand(100, 162) {
                    self.rooms[i].area.done = true;
                    continue;
                }
            }

            if r_center_radius >= 10 && r_center_radius < 30 {
                if self.bias_square_rooms {
                    if w == h && w > 4 && (w % 2 == 1) && self.rand(1, 2) == 1 {
                        self.rooms[i].area.done = true;
                        continue;
                    }
                }
                if size > self.rand(50, 100) {
                    self.rooms[i].area.done = true;
                    continue;
                }
            }

            if r_center_radius >= 30 {
                if size > self.rand(10, 50) {
                    self.rooms[i].area.done = true;
                    continue;
                }
            }

            let mut dirs = [0_i32, 1_i32, 2_i32, 3_i32];
            self.randomizers.shuffle_i32_4(&mut dirs);
            for k in 0..4 {
                let dir = dirs[k];
                let area_clone = self.rooms[i].area.clone();
                if self.can_grow_in_dir(&area_clone, dir) {
                    self.rooms[i].area.grow_by_dir(dir);
                    break;
                }
            }
        }
    }

    fn seed_new_rooms(&mut self) {
        let mut found_new_cell = false;
        let mut attempts = 0;
        while !found_new_cell && attempts < 10 {
            attempts += 1;
            found_new_cell = true;

            if self.bounds.are_invalid() {
                std::process::exit(1);
            }
            let new_seed =
                self.bounds.point_within_range(&mut self.randomizers, 1);
            if new_seed.radius() < 10 {
                found_new_cell = false;
                continue;
            }

            for r in &self.rooms.clone() {
                let buffer = if self.rand(1, 5) == 1 { 1 } else { 2 };
                if new_seed.is_inside(&r.area.rect, buffer) {
                    found_new_cell = false;
                    break;
                }
            }
            if found_new_cell {
                for r in &self.passages {
                    if new_seed.is_inside(&r.area.rect, 1) {
                        found_new_cell = false;
                        break;
                    }
                }
            }
            if found_new_cell {
                self.rooms.push(Room::new(new_seed, new_seed, false, false));
            }
        }
    }

    fn filter_undesirable_rooms(&mut self) {
        let filter_room_prob = 224;
        let filter_corridors_and_pixels = true;

        for i in 0..self.rooms.len() {
            let roll = self.rand(1, filter_room_prob);
            if roll == 1 {
                self.rooms[i].filter();
            }
        }

        if filter_corridors_and_pixels {
            for r in &mut self.rooms {
                if r.area.width() == 1 || r.area.height() == 1 {
                    r.filter();
                }
            }
        }
    }

    fn make_centerpiece(
        &mut self,
        half_width: i32,
        half_height: i32,
        edges: i32,
    ) {
        self.rooms.push(Room::new(
            Coords::new(-half_width, -half_height),
            Coords::new(half_width, half_height),
            true,
            true,
        ));

        if (edges & LEFT_EDGE) != 0 {
            self.passages.push(Passage::new(
                Coords::new(-half_width - 5, 0),
                Coords::new(-half_width - 1, 0),
                1,
            ));
        }
        if (edges & RIGHT_EDGE) != 0 {
            self.passages.push(Passage::new(
                Coords::new(half_width + 5, 0),
                Coords::new(half_width + 1, 0),
                3,
            ));
        }
        if (edges & TOP_EDGE) != 0 {
            self.passages.push(Passage::new(
                Coords::new(0, -half_height - 5),
                Coords::new(0, -half_height - 1),
                2,
            ));
        }
        if (edges & BOTTOM_EDGE) != 0 {
            self.passages.push(Passage::new(
                Coords::new(0, half_height + 5),
                Coords::new(0, half_height + 1),
                0,
            ));
        }
    }

    fn excavate_rooms(&mut self) {
        if self.config.centerpiece {
            let var = self.rand(1, 5);
            if var == 1 {
                self.make_centerpiece(
                    5,
                    5,
                    LEFT_EDGE | RIGHT_EDGE | TOP_EDGE | BOTTOM_EDGE,
                );
            }
            if var == 2 {
                self.make_centerpiece(6, 3, LEFT_EDGE | TOP_EDGE | BOTTOM_EDGE);
            }
            if var == 3 {
                self.make_centerpiece(3, 6, LEFT_EDGE | RIGHT_EDGE | TOP_EDGE);
            }
            if var == 4 {
                self.make_centerpiece(7, 2, LEFT_EDGE | TOP_EDGE | BOTTOM_EDGE);
            }
            if var == 5 {
                self.make_centerpiece(8, 3, LEFT_EDGE | TOP_EDGE | BOTTOM_EDGE);
            }
        }

        for _ in 0..ITERATIONS {
            self.grow_existing_rooms_to_limit();
            for _ in 0..5 {
                self.seed_new_rooms();
            }
        }
        self.filter_undesirable_rooms();
    }

    fn excavate_passages(&mut self) {
        for _ in 0..100 {
            self.extend_passages();
        }
    }

    fn filter_smallest_islands(&mut self) {
        #[derive(Copy, Clone)]
        enum NodeRef {
            Room(usize),
            Passage(usize),
        }

        let mut backlog: Vec<NodeRef> = Vec::new();
        for (i, r) in self.rooms.iter().enumerate() {
            if !r.area.filtered {
                backlog.push(NodeRef::Room(i));
            }
        }
        for (i, p) in self.passages.iter().enumerate() {
            if !p.area.filtered {
                backlog.push(NodeRef::Passage(i));
            }
        }
        if backlog.is_empty() {
            return;
        }

        let mut islands: Vec<Vec<NodeRef>> = Vec::new();
        let mut tasks: Vec<NodeRef> = Vec::new();
        let mut island: Vec<NodeRef> = Vec::new();

        tasks.push(backlog[0]);
        island.push(backlog[0]);
        backlog.remove(0);

        while !backlog.is_empty() || !tasks.is_empty() {
            while let Some(rn) = tasks.pop() {
                let rrect = match rn {
                    NodeRef::Room(i) => self.rooms[i].area.rect,
                    NodeRef::Passage(i) => self.passages[i].area.rect,
                };

                let mut to_delete_idx: Vec<usize> = Vec::new();
                for (idx, p) in backlog.iter().enumerate() {
                    let prect = match *p {
                        NodeRef::Room(i) => self.rooms[i].area.rect,
                        NodeRef::Passage(i) => self.passages[i].area.rect,
                    };
                    if rrect.is_intersecting(&prect, 1) {
                        island.push(*p);
                        to_delete_idx.push(idx);
                        tasks.push(*p);
                    }
                }
                for idx in to_delete_idx.into_iter().rev() {
                    backlog.remove(idx);
                }
            }

            islands.push(island.clone());
            island.clear();

            if !backlog.is_empty() {
                tasks.push(backlog[0]);
                island.push(backlog[0]);
                backlog.remove(0);
            }
        }

        islands.sort_by(|a, b| b.len().cmp(&a.len()));

        for isl in islands.iter().skip(1) {
            for n in isl {
                match *n {
                    NodeRef::Room(i) => self.rooms[i].area.filtered = true,
                    NodeRef::Passage(i) => {
                        self.passages[i].area.filtered = true
                    }
                }
            }
        }
    }

    fn rank_and_tag(&mut self) {
        #[derive(Clone, Copy)]
        enum NodeRef {
            Room(usize),
            Passage(usize),
        }

        #[derive(Clone, Debug)]
        struct FeaturesBudget {
            tier4: i32,
            tier3: i32,
        }

        fn rank_room(r: &mut Room, budget: &mut FeaturesBudget) {
            let rad = r.area.center().radius();
            let size = r.area.size();
            if rad < 10 && size > 40 && {
                let b = budget.tier4;
                budget.tier4 -= 1;
                b > 0
            } {
                r.feature_tier = 4;
            } else if rad < 20 && size > 30 && {
                let b = budget.tier3;
                budget.tier3 -= 1;
                b > 0
            } {
                r.feature_tier = 3;
            } else if rad < 30 {
                r.feature_tier = 2;
            } else {
                r.feature_tier = 1;
            }
        }

        let mut backlog: Vec<NodeRef> = Vec::new();

        let mut budget = FeaturesBudget { tier4: 1, tier3: 2 };

        for i in 0..self.rooms.len() {
            if !self.rooms[i].area.filtered {
                backlog.push(NodeRef::Room(i));
            }
            rank_room(&mut self.rooms[i], &mut budget);
        }

        let mut tasks: Vec<NodeRef> = Vec::new();

        for i in 0..self.passages.len() {
            if self.passages[i].area.filtered {
                continue;
            }
            if self.passages[i].entrance.is_some() {
                if let Some(first) = backlog.first().copied() {
                    tasks.push(first);
                }
            } else {
                backlog.push(NodeRef::Passage(i));
            }
        }

        if tasks.is_empty() && !backlog.is_empty() {
            tasks.push(backlog[0]);
            backlog.remove(0);
        }

        let mut rank = 1;
        while !backlog.is_empty() || !tasks.is_empty() {
            while let Some(rn) = tasks.pop() {
                let rrect = match rn {
                    NodeRef::Room(i) => self.rooms[i].area.rect,
                    NodeRef::Passage(i) => self.passages[i].area.rect,
                };

                let mut to_delete_idx: Vec<usize> = Vec::new();
                for (idx, p) in backlog.iter().enumerate() {
                    let prect = match *p {
                        NodeRef::Room(i) => self.rooms[i].area.rect,
                        NodeRef::Passage(i) => self.passages[i].area.rect,
                    };
                    if rrect.is_intersecting(&prect, 1) {
                        match *p {
                            NodeRef::Room(i) => {
                                self.rooms[i].area.number = rank
                            }
                            NodeRef::Passage(i) => {
                                self.passages[i].area.number = rank
                            }
                        }
                        to_delete_idx.push(idx);
                        tasks.push(*p);
                        rank += 1;
                    }
                }
                for idx in to_delete_idx.into_iter().rev() {
                    backlog.remove(idx);
                }
            }

            if !backlog.is_empty() {
                tasks.push(backlog[0]);
                backlog.remove(0);
            }
        }
    }

    pub fn print_dungeon(&self) {
        for row in self.bounds.min_y..self.bounds.max_y {
            let mut line = String::new();
            for col in self.bounds.min_x..self.bounds.max_x {
                let mut c = "░░░".to_string();
                let mut count = 0;

                for r in &self.rooms {
                    if r.area.filtered {
                        continue;
                    }
                    if Coords::new(col, row).is_inside(&r.area.rect, 0) {
                        c = "███".to_string();
                        count += 1;
                        if count > 1 {
                            c = "XXX".to_string();
                        }
                    }
                }

                for r in &self.passages {
                    if r.area.filtered {
                        continue;
                    }
                    if Coords::new(col, row).is_inside(&r.area.rect, 0) {
                        c = "▓▓▓".to_string();
                        if let Some(e) = r.entrance {
                            if Coords::new(col, row) == e.pos {
                                c = "EEE".to_string();
                            }
                        }
                        count += 1;
                        if count > 1 {
                            c = "XXX".to_string();
                        }
                    }
                }

                line.push_str(&c);
            }
        }
    }

    fn filter_connected(&mut self, passage_index: usize) {
        self.passages[passage_index].area.filtered = true;
        let p_rect = self.passages[passage_index].area.rect;

        for i in 0..self.passages.len() {
            if self.passages[i].area.filtered {
                continue;
            }
            if p_rect.is_intersecting(&self.passages[i].area.rect, 1) {
                self.filter_connected(i);
            }
        }
    }

    fn filter_room_and_connected(&mut self, room_index: usize) {
        self.rooms[room_index].area.filtered = true;
        let r_rect = self.rooms[room_index].area.rect;

        for i in 0..self.passages.len() {
            if self.passages[i].area.filtered {
                continue;
            }
            if r_rect.is_intersecting(&self.passages[i].area.rect, 1) {
                self.filter_connected(i);
            }
        }
    }

    fn filter_random_parts(&mut self) {
        let mut filtered = 0;
        let mut safety = 100000;
        while filtered < 15 && safety > 0 {
            safety -= 1;
            for i in 0..self.rooms.len() {
                if self.rooms[i].area.filtered {
                    continue;
                }
                let rad = self.rooms[i].area.center().radius();
                if rad > 30 {
                    if self.rand(1, 2) == 1 {
                        self.filter_room_and_connected(i);
                        filtered += 1;
                        break;
                    }
                }
                if rad < 10 {
                    if self.rooms[i].area.size() > 10 {
                        continue;
                    }
                    if self.rand(1, 2) == 1 {
                        self.filter_room_and_connected(i);
                        filtered += 1;
                        break;
                    }
                }
            }
        }
    }

    fn get_passage_overlap_dir(pivot: Coords, a: &Rect) -> i32 {
        let mut dir = 0;
        if a.a.x < pivot.x && a.b.x < pivot.x {
            dir = 3;
        }
        if a.a.x > pivot.x && a.b.x > pivot.x {
            dir = 1;
        }
        if a.a.y > pivot.y && a.b.y > pivot.y {
            dir = 2;
        }
        if a.a.y < pivot.y && a.b.y < pivot.y {
            dir = 0;
        }
        dir
    }

    fn find_empty_ends(&mut self) -> i32 {
        let mut n_entrances = 0;
        let plen = self.passages.len();
        for i in 0..plen {
            if self.passages[i].area.filtered {
                continue;
            }

            let t_rect = self.passages[i].area.rect;
            let t_a = t_rect.a;
            let t_b = t_rect.b;

            let mut a_is_clear = true;
            let mut b_is_clear = true;
            let mut n_passages_overlap = 0;
            let mut overlap_dir = 0;
            let mut n_rooms_overlap = 0;

            for j in 0..plen {
                if i == j {
                    continue;
                }
                if self.passages[j].area.filtered {
                    continue;
                }
                let c_rect = self.passages[j].area.rect;

                let mut passage_overlap = false;
                if t_a.is_inside(&c_rect, 1) {
                    a_is_clear = false;
                    passage_overlap = true;
                }
                if t_b.is_inside(&c_rect, 1) {
                    b_is_clear = false;
                    passage_overlap = true;
                }
                if passage_overlap {
                    n_passages_overlap += 1;
                    overlap_dir = Self::get_passage_overlap_dir(t_a, &c_rect);
                }
            }

            for r in &self.rooms {
                if r.area.filtered {
                    continue;
                }
                let c_rect = r.area.rect;
                let mut room_overlap = false;
                if t_a.is_inside(&c_rect, 1) {
                    a_is_clear = false;
                    room_overlap = true;
                }
                if t_b.is_inside(&c_rect, 1) {
                    b_is_clear = false;
                    room_overlap = true;
                }
                if room_overlap {
                    n_rooms_overlap += 1;
                }
            }

            if a_is_clear || b_is_clear {
                if a_is_clear {
                    let dir = if self.passages[i].area.width() == 1 {
                        2
                    } else {
                        1
                    };
                    self.passages[i].empty_end = Some(PassageEnd {
                        pos: t_a,
                        dir,
                        type_: 0,
                    });
                }
                if b_is_clear {
                    let dir = if self.passages[i].area.width() == 1 {
                        0
                    } else {
                        3
                    };
                    self.passages[i].empty_end = Some(PassageEnd {
                        pos: t_b,
                        dir,
                        type_: 0,
                    });
                }
                n_entrances += 1;
            } else if n_rooms_overlap == 0 && n_passages_overlap == 1 {
                self.passages[i].empty_end = Some(PassageEnd {
                    pos: t_a,
                    dir: overlap_dir,
                    type_: 0,
                });
                n_entrances += 1;
            }
        }
        n_entrances
    }

    pub fn excavate_dungeon(&mut self, prefer_small_dungeons: bool) {
        let config_roll = if prefer_small_dungeons {
            self.rand(1, 2)
        } else {
            self.rand(1, 10)
        };

        match config_roll {
            10 | 9 => {
                self.config = MapConfig {
                    max_leg: 30,
                    centerpiece: true,
                }
            }
            8 | 7 | 6 => {
                self.config = MapConfig {
                    max_leg: 20,
                    centerpiece: false,
                }
            }
            _ => {
                self.config = MapConfig {
                    max_leg: 15,
                    centerpiece: false,
                }
            }
        }

        self.excavate_rooms();
        self.seed_portals();
        self.excavate_passages();

        self.filter_random_parts();
        self.filter_smallest_islands();
        self.bounds.inflate(5);

        self.n_entrances = self.find_empty_ends();
        self.rank_and_tag();

        let mut to_be_numbered_rooms: Vec<(Coords, AreaId)> = Vec::new();

        for (i, a) in self.rooms.iter().enumerate() {
            if a.area.filtered {
                continue;
            }
            to_be_numbered_rooms.push((a.area.center(), AreaId::Room(i)));
        }

        let mut desired_entrances = 1;
        let num_of_rooms = to_be_numbered_rooms.len() as i32;
        if num_of_rooms > self.rand(6, 10) {
            desired_entrances = 2;
        }
        if num_of_rooms > 30 {
            desired_entrances = 3;
        }

        let missing_entrances = if self.n_entrances < desired_entrances {
            desired_entrances - self.n_entrances
        } else {
            0
        };
        let mut missing_entrances_mut = missing_entrances;
        let mut assigned_entrances = 0;

        for i in 0..self.passages.len() {
            if self.passages[i].area.filtered {
                continue;
            }
            if assigned_entrances < desired_entrances {
                if let Some(e) = self.passages[i].empty_end {
                    to_be_numbered_rooms.push((
                        self.passages[i].area.center(),
                        AreaId::Passage(i),
                    ));
                    self.passages[i].entrance = Some(e);
                    assigned_entrances += 1;
                } else if missing_entrances_mut > 0
                    && self.passages[i].area.size() > 5
                {
                    to_be_numbered_rooms.push((
                        self.passages[i].area.center(),
                        AreaId::Passage(i),
                    ));
                    self.passages[i].entrance = Some(PassageEnd {
                        pos: self.passages[i].area.center(),
                        dir: 0,
                        type_: 1,
                    });
                    missing_entrances_mut -= 1;
                    assigned_entrances += 1;
                }
            } else {
                if let Some(e) = self.passages[i].empty_end {
                    to_be_numbered_rooms.push((
                        self.passages[i].area.center(),
                        AreaId::Passage(i),
                    ));
                    self.passages[i].deadend = Some(e);
                }
            }
        }

        to_be_numbered_rooms.sort_by(|a, b| b.0.radius().cmp(&a.0.radius()));

        let mut room_number = 1;
        for (_, id) in &to_be_numbered_rooms {
            match *id {
                AreaId::Room(i) => self.rooms[i].area.number = room_number,
                AreaId::Passage(i) => {
                    self.passages[i].area.number = room_number
                }
            }
            room_number += 1;
        }
        for i in 0..self.passages.len() {
            if self.passages[i].area.filtered {
                continue;
            }
            if self.passages[i].empty_end.is_none() {
                self.passages[i].area.number = room_number;
                room_number += 1;
            }
        }
    }

    fn get_room_portals(
        &self,
        r: &Area,
        portals: &mut Vec<Portal>,
        mut local_portals: Option<&mut Vec<Portal>>,
    ) {
        for p in &self.passages {
            if p.area.filtered {
                continue;
            }
            let mut room_number = r.number;
            let mut wall_outside = r.rect;
            wall_outside.inflate(1);
            if p.area.rect.is_intersecting(&wall_outside, 0) {
                let mut type_ = 0;

                if (p.area.width() == 2 || p.area.height() == 2)
                    || r.size() < 13
                {
                    let mut portal_is_redundant = false;
                    for j in portals.iter() {
                        if (j.pos.x == p.area.rect.a.x
                            && j.pos.y == p.area.rect.a.y)
                            || (j.pos.x == p.area.rect.b.x
                                && j.pos.y == p.area.rect.b.y)
                        {
                            portal_is_redundant = true;
                            room_number = j.room_number;
                            break;
                        }
                    }
                    if portal_is_redundant {
                        type_ = 1;
                    }
                }

                if (room_number % 3) == 1 && type_ == 1 {
                    type_ = 2;
                }

                let mut portal = Portal {
                    type_,
                    wall: String::new(),
                    pos: Coords::new(0, 0),
                    room_number,
                    leading_to: 0,
                };

                let pr = p.area.rect;
                let rr = r.rect;

                if pr.b.y == rr.a.y - 1 && pr.a.x >= rr.a.x && pr.b.x <= rr.b.x
                {
                    portal.wall = "N".to_string();
                    portal.pos = Coords::new(pr.a.x, pr.b.y);
                }
                if pr.a.y == rr.b.y + 1 && pr.a.x >= rr.a.x && pr.b.x <= rr.b.x
                {
                    portal.wall = "S".to_string();
                    portal.pos = Coords::new(pr.a.x, pr.a.y);
                }
                if pr.b.x == rr.a.x - 1 && pr.a.y >= rr.a.y && pr.b.y <= rr.b.y
                {
                    portal.wall = "W".to_string();
                    portal.pos = Coords::new(pr.b.x, pr.b.y);
                }
                if pr.a.x == rr.b.x + 1 && pr.a.y >= rr.a.y && pr.b.y <= rr.b.y
                {
                    portal.wall = "E".to_string();
                    portal.pos = Coords::new(pr.a.x, pr.a.y);
                }

                portal.room_number = room_number;
                portals.push(portal.clone());
                if let Some(lp) = local_portals.as_mut() {
                    lp.push(portal.clone());
                }
            }
        }
    }

    pub fn as_json(&self) -> serde_json::Value {
        let mut ret = serde_json::json!({});

        for a in &self.passages {
            if a.area.filtered {
                continue;
            }
            let mut area = serde_json::json!({});
            area["x"] = serde_json::json!(a.area.rect.a.x);
            area["y"] = serde_json::json!(a.area.rect.a.y);
            area["w"] = serde_json::json!(a.area.width());
            area["h"] = serde_json::json!(a.area.height());
            area["t"] = serde_json::json!(0);
            area["n"] = serde_json::json!(a.area.number);

            if let Some(entrance) = a.entrance {
                area["e"] = serde_json::json!({
                    "x": entrance.pos.x,
                    "y": entrance.pos.y,
                    "d": entrance.dir,
                    "t": entrance.type_
                });
                area["t"] = serde_json::json!(2);
            }
            if let Some(deadend) = a.deadend {
                area["d"] = serde_json::json!({
                    "x": deadend.pos.x,
                    "y": deadend.pos.y
                });
                area["t"] = serde_json::json!(3);
            }
            if !ret.get("areas").is_some_and(|v| v.is_array()) {
                ret["areas"] = serde_json::json!([]);
            }
            ret["areas"].as_array_mut().unwrap().push(area);
        }

        let mut portals: Vec<Portal> = Vec::new();

        for a in &self.rooms {
            if a.area.filtered {
                continue;
            }
            let mut local_portals: Vec<Portal> = Vec::new();
            self.get_room_portals(
                &a.area,
                &mut portals,
                Some(&mut local_portals),
            );

            let mut area = serde_json::json!({});
            area["x"] = serde_json::json!(a.area.rect.a.x);
            area["y"] = serde_json::json!(a.area.rect.a.y);
            area["w"] = serde_json::json!(a.area.width());
            area["h"] = serde_json::json!(a.area.height());
            area["t"] = serde_json::json!(1);
            area["n"] = serde_json::json!(a.area.number);

            for p in &local_portals {
                let portal = serde_json::json!({
                    "x": p.pos.x,
                    "y": p.pos.y,
                    "wall": p.wall,
                    "type": p.type_
                });
                if p.type_ == 0 || p.type_ == 2 {
                    if !area.get("portals").is_some_and(|v| v.is_array()) {
                        area["portals"] = serde_json::json!([]);
                    }
                    area["portals"]
                        .as_array_mut()
                        .unwrap()
                        .push(portal.clone());
                }
                if p.type_ == 1 {
                    if !ret.get("passages").is_some_and(|v| v.is_array()) {
                        ret["passages"] = serde_json::json!([]);
                    }
                    ret["passages"]
                        .as_array_mut()
                        .unwrap()
                        .push(portal.clone());
                }
            }
            if !ret.get("areas").is_some_and(|v| v.is_array()) {
                ret["areas"] = serde_json::json!([]);
            }
            ret["areas"].as_array_mut().unwrap().push(area.clone());
        }

        for p in &portals {
            if p.type_ == 0 || p.type_ == 2 {
                let portal = serde_json::json!({
                    "x": p.pos.x,
                    "y": p.pos.y,
                    "wall": p.wall,
                    "type": p.type_
                });
                if !ret.get("portals").is_some_and(|v| v.is_array()) {
                    ret["portals"] = serde_json::json!([]);
                }
                ret["portals"].as_array_mut().unwrap().push(portal.clone());
            }
        }

        ret
    }

    pub fn for_each_room<F>(&self, mut callback: F)
    where
        F: FnMut(&Room, &Vec<Portal>, i32),
    {
        let mut portals: Vec<Portal> = Vec::new();
        for r in &self.rooms {
            if r.area.filtered {
                continue;
            }
            let mut local: Vec<Portal> = Vec::new();
            self.get_room_portals(&r.area, &mut portals, Some(&mut local));
            callback(r, &local, r.area.number);
        }
    }

    pub fn for_each_corridor<F>(&self, mut callback: F)
    where
        F: FnMut(&Passage, i32),
    {
        for p in &self.passages {
            if p.area.filtered {
                continue;
            }
            callback(p, p.area.number);
        }
    }
}

#[derive(Copy, Clone)]
enum AreaId {
    Room(usize),
    Passage(usize),
}

#[derive(Clone)]
struct PhantomArea {
    area: Area,
}

impl MapArea for PhantomArea {
    fn area(&self) -> &Area {
        &self.area
    }
}

#[derive(Clone)]
pub struct Randomizers {
    state: u64,
}

impl Randomizers {
    pub fn new(seed: u64) -> Self {
        let s = if seed == 0 { 0x9E3779B97F4A7C15 } else { seed };
        Self { state: s }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    #[inline]
    fn roll_in_range(&mut self, lo: i32, hi: i32) -> i32 {
        if lo >= hi {
            return lo;
        }
        let span = (hi as i64 - lo as i64 + 1) as u64;
        let v = self.next_u64() % span;
        lo + v as i32
    }

    #[inline]
    fn shuffle_i32_4(&mut self, a: &mut [i32; 4]) {
        for i in (1..a.len()).rev() {
            let j = (self.next_u64() % ((i as u64) + 1)) as usize;
            a.swap(i, j);
        }
    }
}

impl Default for Randomizers {
    fn default() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self::new(seed)
    }
}
