# HEXROLL 3

This is the new work-in-progress version of HEXROLL (https://hexroll.app) - the OSR sandbox generator.

![hexroll3-screenshot](https://raw.githubusercontent.com/hexroll/hexroll3/master/screenshot.png)

## Currently Included

- hexroll3-scroll: the core content generator
- hexroll3-scroll-data: the new data model, based on the hexroll2e model
- hexroll3-app: The hexroll3 app
- hexroll3-testbed: an egui application for testing and messing around
- hexroll3-signaling: the VTT signaling server

## Running the testbed

Make sure you have `git` and an up-to-date Rust development environment, and then:

```
git clone https://github.com/hexroll/hexroll3 && cd hexroll3
cargo run --release
```

This should open up the testbed where you can generate your first sandbox:

![testbed-screenshot](https://raw.githubusercontent.com/hexroll/hexroll3/master/hexroll3-testbed/assets/screenshot.png)

## Code Credits

Hexroll is built using:

* **Bevy**
* **Bevy crates:** Modified versions of `avian3d`, `bevy_editor_cam`, `bevy_simple_scroll_view`, `bevy_mod_billboard`, and `bevy_tweening` and the official versions of `bevy_mod_outline`, `bevy_matchbox`, `bevy_rich_text3d`, `hexx`, `bevy_ui_text_input`, `bevy_vector_shapes`, `bevy_seedling`, `bevy_hanabi`, `bevy-inspector-egui`
* **Other core crates:** `serde`, `serde_json`, `rand`, `cosmic-text`, `bincode`, `ureq`, `html5ever`, `lamport`, `lyon`, `regex`, `smallvec`, `chrono`, `ron`, `dirs`, and `anyhow`

A huge thanks to all the open-source contributors who make building projects like this possible!

## License

```text
Copyright (C) 2020-2026 Pen, Dice & Paper

This program is dual-licensed under the following terms:

Option 1: (Non-Commercial) GNU Affero General Public License (AGPL)
This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as
published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program. If not, see <http://www.gnu.org/licenses/>.

Option 2: Commercial License
In addition to the AGPLv3, this software is available under a separate
commercial license for use that does not comply with the AGPLv3 requirements.
Please contact ithai at pendicepaper.com for more information about
commercial licensing terms.

HEXROLL3 contains Open Game Content, subject to the Open Game License,
released under the Open Game License, Version 1.0a (enclosed in the LICENSE
file), as described in Section 1(d) of the License.
```
