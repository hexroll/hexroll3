use bevy::{ecs::relationship::RelatedSpawnerCommands, prelude::*, text::LineHeight};

use crate::shared::widgets::buttons::ToggleEventWrapper;

pub struct HelpOverlayPlugin;

impl Plugin for HelpOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_help_overlay);
    }
}

#[derive(Component)]
struct HelpOverlay;

fn spawn_help_overlay(
    _: On<ToggleEventWrapper<HelpToggle>>,
    mut commands: Commands,
    existing_overlay: Query<Entity, With<HelpOverlay>>,
) {
    if existing_overlay.is_empty() {
        commands
            .spawn((
                HelpOverlay,
                Name::new("HelpOverlay"),
                Node {
                    display: Display::Grid,
                    position_type: PositionType::Absolute,
                    right: Val::Px(20.0),
                    align_items: AlignItems::Center,
                    align_self: AlignSelf::Center,
                    grid_template_columns: vec![
                        RepeatedGridTrack::default(),
                        RepeatedGridTrack::default(),
                    ],
                    column_gap: Val::Px(10.0),
                    row_gap: Val::Px(5.0),

                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn_key_help("SPACE", "reveal mode");
                c.spawn_key_help("T", "spawn token");
                c.spawn_key_help("Ctrl+T", "spawn token copy");
                c.spawn_key_help("R", "Reselect previously selected tokens");
                c.spawn_key_help("B", "Teleport select token to mouse");
                c.spawn_key_help("Ctrl+Z", "Undo dry-erase stroke");
                c.spawn_key_help("Esc", "Exit current mode / dialog");
            });
    } else {
        existing_overlay
            .iter()
            .for_each(|e| commands.entity(e).despawn());
    }
}

trait KeyboardShortcutHelpSpawner {
    fn spawn_key_help(&mut self, key: &str, desc: &str);
}

impl KeyboardShortcutHelpSpawner for RelatedSpawnerCommands<'_, ChildOf> {
    fn spawn_key_help(&mut self, key: &str, desc: &str) {
        self.spawn((Node {
            justify_content: JustifyContent::End,
            ..default()
        },))
            .with_children(|c| {
                c.spawn((
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
                    BorderColor::all(Color::BLACK),
                    BorderRadius::all(Val::Percent(20.0)),
                    Node {
                        border: UiRect::all(Val::Px(2.0)),
                        padding: UiRect::left(Val::Px(20.0)),
                        ..default()
                    },
                    TextFont::default()
                        .with_font_size(16.0)
                        .with_line_height(LineHeight::RelativeToFont(1.5)),
                    TextLayout {
                        justify: Justify::Center,
                        ..default()
                    },
                    Text::new(key),
                ));
            });
        self.spawn((
            Node::default(),
            TextFont::default().with_font_size(10.0),
            Text::new(desc),
        ));
    }
}

#[derive(Component, Default, PartialEq, Clone)]
pub enum HelpToggle {
    #[default]
    Off,
    On,
}
