use bevy::prelude::*;

use crate::hexmap::{HexMapTime, HexmapTheme, TileSetThemesMetadata};

use super::toggles::{toggle_day_night, toggle_map_theme};

pub(crate) fn keyboard_shortcuts(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    hexmap_time: Single<&HexMapTime>,
    handle: Res<TileSetThemesMetadata>,
    current_theme: Res<HexmapTheme>,
) {
    if keyboard.just_pressed(KeyCode::F10) {
        toggle_day_night(&mut commands, &hexmap_time);
    }
    if keyboard.just_pressed(KeyCode::F11) {
        toggle_map_theme(&mut commands, &handle, &current_theme);
    }
}
