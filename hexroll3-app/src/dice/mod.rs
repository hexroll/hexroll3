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

use bevy::ecs::event::Event;

mod dice;

#[derive(Event)]
// This event can be triggered by the user to roll dice
pub struct RollDice {
    pub dice: String,
}

#[derive(Event)]
// This event is triggered by the module when a dice roll is resolved
pub struct DiceRollResolved {
    pub dice_roll: DiceRoll,
}

pub use dice::DiceRoll;
pub trait DiceRollHelpers {
    fn total(&self) -> i32;
    fn to_strings(&self) -> (Vec<String>, Vec<String>);
}

// This component identify any dice currently on the virtual table
pub use dice::Dice;

// This resource holds the initial forces applied to any newly spawned dice
pub use dice::DiceConfig;

// This resource holds the current dice set used to roll dice from
pub use dice::DiceResources;
pub use dice::DiceSet;
pub use dice::DiceSets;

// The DicePlugin must be added for any of this to work
pub use dice::DicePlugin;
