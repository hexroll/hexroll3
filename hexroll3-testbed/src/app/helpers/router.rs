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
use path_tree::PathTree;

use crate::app::{HexrollTestbedApp, RouteHandler};

impl HexrollTestbedApp {
    pub fn routes(tree: &mut PathTree<RouteHandler>) {
        let _ = tree.insert(
            "/inspect/:sandbox/:location/:id",
            Box::new(|s, args| {
                s.navigate(args["id"].as_str(), true);
            }),
        );
        let _ = tree.insert(
            "/inspect/:sandbox/location/:id/npc/:npc_id",
            Box::new(|s, args| {
                s.navigate(args["id"].as_str(), true);
            }),
        );
        let _ = tree.insert(
            "/reroll/:id",
            Box::new(|s, args| {
                s.reroll(args["id"].as_str());
            }),
        );
        let _ = tree.insert(
            "/unroll/:id",
            Box::new(|s, args| {
                s.unroll(args["id"].as_str());
            }),
        );
        let _ = tree.insert(
            "/append/:parent_id/:attr/:cls",
            Box::new(|s, args| {
                s.append(args["parent_id"].as_str(), args["attr"].as_str());
            }),
        );
    }
}
