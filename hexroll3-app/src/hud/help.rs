use bevy::{
    ecs::relationship::RelatedSpawnerCommands, input_focus::InputFocus, prelude::*,
    text::LineHeight,
};

use crate::{
    content::ContentMode,
    hexmap::{SandboxLock, elements::MapVisibilityController},
    shared::widgets::buttons::{ToggleEventWrapper, ToggleResourceWrapper},
};

use super::searchbar::SearchText;

pub struct HelpOverlayPlugin;

impl Plugin for HelpOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_spawn_help_overlay)
            .add_systems(Update, detect_keyboard_toggle)
            .add_systems(Update, update_help_overlay);
    }
}

#[derive(PartialEq, Clone, Copy)]
enum HelpSection {
    Standard,
    Search,
    SplitScreen,
    SplitScreenZoomedOut,
    UnlockedMode,
    Battlemaps,
}

#[derive(Component)]
struct HelpSectionMarker;

#[derive(Resource, Default, PartialEq)]
struct RenderedHelpState {
    sections: Vec<HelpSection>,
}

fn desired_sections(
    content_mode: &ContentMode,
    lock: &SandboxLock,
    visibility: &MapVisibilityController,
    focus: &InputFocus,
    search_text: &Query<Entity, With<SearchText>>,
) -> Vec<HelpSection> {
    let battlemaps = visibility.are_battlemaps_visible();
    let mut sections = vec![HelpSection::Standard];
    for e in search_text {
        if focus.0 == Some(e) {
            sections.clear();
            sections.push(HelpSection::Search);
            return sections;
        }
    }
    if battlemaps {
        sections.push(HelpSection::Battlemaps);
    }
    if *content_mode == ContentMode::SplitScreen {
        sections.push(HelpSection::SplitScreen);
        if !battlemaps {
            sections.push(HelpSection::SplitScreenZoomedOut);
        }
    }
    if *lock == SandboxLock::Off {
        sections.push(HelpSection::UnlockedMode);
    }

    sections
}

fn spawn_section(c: &mut RelatedSpawnerCommands<'_, ChildOf>, section: HelpSection) {
    match section {
        HelpSection::Standard => {
            c.spawn_key_help(vec!["F1"], "Toggle help overlay", section);
            c.spawn_key_help(
                vec!["0", "1", "2", "3", "4", "6", "8"],
                "Dice rollers",
                section,
            );
            c.spawn_key_help(vec!["SPACE"], "Reveal mode", section);
        }
        HelpSection::Search => {
            c.spawn_key_help(vec!["Esc"], "Exit search", section);
            c.spawn_key_help(vec!["Up", "Down"], "Navigate results", section);
            c.spawn_key_help(vec!["Enter"], "Select a result", section);
        }
        HelpSection::Battlemaps => {
            c.spawn_key_help(vec!["T"], "Spawn token", section);
            c.spawn_key_help(vec!["Ctrl", "LeftClick"], "Select token", section);
            c.spawn_key_help(vec!["Ctrl", "T"], "Spawn token copy", section);
            c.spawn_key_help(vec!["C"], "Add ruler corner", section);
            c.spawn_key_help(vec!["R"], "Reselect previously selected tokens", section);
            c.spawn_key_help(vec!["B"], "Teleport selected tokens to mouse", section);
            c.spawn_key_help(vec!["C"], "Add ruler corner", section);
            c.spawn_key_help(vec!["SHIFT", "DRAG"], "Orient tokens while moving", section);
        }
        HelpSection::SplitScreen => {
            c.spawn_key_help(vec!["Esc"], "Go back to full map view", section);
        }
        HelpSection::SplitScreenZoomedOut => {
            c.spawn_key_help(
                vec!["W", "S", "A", "D", "Q", "E"],
                "Navigate in direction",
                section,
            );
        }
        HelpSection::UnlockedMode => {
            c.spawn_key_help(vec!["Ctrl", "Z"], "Undo last sandbox mutation", section);
        }
    }
}

fn detect_keyboard_toggle(keyboard: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    if keyboard.just_pressed(KeyCode::F1) {
        commands.trigger(ToggleEventWrapper::<HelpToggle>::default());
    }
}

fn on_spawn_help_overlay(
    _: On<ToggleEventWrapper<HelpToggle>>,
    mut commands: Commands,
    existing_overlay: Query<Entity, With<HelpOverlay>>,
) {
    if existing_overlay.is_empty() {
        commands.spawn((
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
        ));
        commands.insert_resource(RenderedHelpState::default());
    } else {
        existing_overlay
            .iter()
            .for_each(|e| commands.entity(e).despawn());
        commands.remove_resource::<RenderedHelpState>();
    }
}

fn update_help_overlay(
    mut commands: Commands,
    overlay: Option<Single<Entity, With<HelpOverlay>>>,
    content_mode: Res<State<ContentMode>>,
    lock: Res<ToggleResourceWrapper<SandboxLock>>,
    visibility_controller: Res<MapVisibilityController>,
    rendered: Option<ResMut<RenderedHelpState>>,
    section_children: Query<Entity, With<HelpSectionMarker>>,
    focus: Res<InputFocus>,
    search_text: Query<Entity, With<SearchText>>,
) {
    let (Some(overlay), Some(mut rendered)) = (overlay, rendered) else {
        return;
    };

    let desired = RenderedHelpState {
        sections: desired_sections(
            content_mode.get(),
            &lock.value,
            &*visibility_controller,
            &focus,
            &search_text,
        ),
    };

    if desired == *rendered {
        return;
    }

    for child in section_children.iter() {
        commands.entity(child).despawn();
    }
    commands.entity(*overlay).with_children(|c| {
        for &section in &desired.sections {
            spawn_section(c, section);
        }
    });

    *rendered = desired;
}

trait KeyboardShortcutHelpSpawner {
    fn spawn_key_help(&mut self, keys: Vec<&str>, desc: &str, section: HelpSection);
}

impl KeyboardShortcutHelpSpawner for RelatedSpawnerCommands<'_, ChildOf> {
    fn spawn_key_help(&mut self, keys: Vec<&str>, desc: &str, _section: HelpSection) {
        self.spawn((
            HelpSectionMarker,
            Node {
                justify_content: JustifyContent::End,
                ..default()
            },
        ))
        .with_children(|c| {
            for key in keys {
                c.spawn((
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
                    BorderColor::all(Color::BLACK),
                    BorderRadius::all(Val::Percent(20.0)),
                    Node {
                        border: UiRect::all(Val::Px(2.0)),
                        padding: UiRect::left(Val::Px(20.0)),
                        margin: UiRect::left(Val::Px(5.0)),
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
            }
        });
        self.spawn((
            HelpSectionMarker,
            Node::default(),
            TextFont::default().with_font_size(10.0),
            Text::new(desc),
        ));
    }
}

#[derive(Component)]
pub struct HelpOverlay;

#[derive(Component, Default, PartialEq, Clone)]
pub enum HelpToggle {
    #[default]
    Off,
    On,
}
