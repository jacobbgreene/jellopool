mod drag;
use crate::states::AppState;
use crate::word_bank::{WordBank, WordBankHandle, select_words};
use bevy::color::palettes::basic::{BLACK, GRAY, WHITE};
use bevy::prelude::*;
use drag::{on_tile_drag, tile_drag_end, tile_drag_start};
pub use drag::{tile_feel_system, tile_follow_system, tray_gap_anim_system, tray_gap_system};

const BOARD_COLOR: Srgba = GRAY;
const LETTER_COLOR: Srgba = BLACK;
const TILE_COLOR: Srgba = WHITE;

/// Marks the tray region so tile spawning can find it after the root spawns.
#[derive(Component)]
pub struct BoardTray;

/// Marks the composition region tiles can be dropped into.
#[derive(Component)]
pub struct WritingZone;

/// Marks the full-screen layer dragged tiles are reparented into, so they
/// paint above the tray and writing zone.
#[derive(Component)]
pub struct DragLayer;

#[derive(Component)]
pub struct WordTile {
    // Read again when UI drag lands (migration step 5).
    #[allow(dead_code)]
    unique_word: String,
}

pub fn spawn_board_root(mut commands: Commands) {
    commands
        .spawn((
            Name::new("board_root"),
            BackgroundColor(Color::from(BOARD_COLOR)),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::End,
                align_items: AlignItems::Center,
                ..default()
            },
            DespawnOnExit(AppState::Playing),
        ))
        .with_child(get_writing_zone())
        .with_child(get_board_tray())
        // Spawned last so dragged tiles paint above everything else.
        .with_child(get_drag_layer());
}

pub fn get_drag_layer() -> (Name, DragLayer, Pickable, Node) {
    (
        Name::new("drag_layer"),
        DragLayer,
        // The layer renders and holds dragged tiles, but never blocks the
        // pointer from reaching tiles and regions beneath it.
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
    )
}

pub fn get_board_tray() -> (Name, BoardTray, BackgroundColor, Node) {
    let name = Name::new("board_tray");
    let color = BackgroundColor(Color::from(LETTER_COLOR));
    let node = Node {
        width: Val::Percent(78.0),
        height: Val::Percent(23.0),
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        justify_content: JustifyContent::Center,
        align_content: AlignContent::Center,
        row_gap: Val::Px(8.0),
        column_gap: Val::Px(8.0),
        padding: UiRect::all(Val::Px(12.0)),
        ..default()
    };

    (name, BoardTray, color, node)
}

pub fn get_writing_zone() -> (Name, WritingZone, BackgroundColor, Node) {
    let name = Name::new("writing_zone");
    let color = BackgroundColor(Color::from(TILE_COLOR));
    let node = Node {
        width: Val::Percent(78.0),
        // Takes all vertical space the tray doesn't reserve.
        flex_grow: 1.0,
        ..default()
    };
    (name, WritingZone, color, node)
}

pub fn spawn_all_tiles(
    mut commands: Commands,
    tray: Single<Entity, With<BoardTray>>,
    word_bank_handle: Res<WordBankHandle>,
    word_banks: Res<Assets<WordBank>>,
) {
    let Some(spawned_word_bank) = word_banks.get(&word_bank_handle.0) else {
        return;
    };

    let selected_words = select_words(spawned_word_bank);

    // The tray arranges the tiles; there are no positions to compute.
    commands.entity(*tray).with_children(|tray| {
        for word in &selected_words {
            spawn_word_tile(tray, word);
        }
    });
}

fn spawn_word_tile(tray: &mut ChildSpawnerCommands, word: &str) {
    tray.spawn((
        Name::new(word.to_string()),
        WordTile {
            unique_word: word.to_string(),
        },
        Node {
            // Padding + text size the tile; width is not derived from word length.
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BorderColor::from(Color::from(BOARD_COLOR)),
        BackgroundColor(Color::from(TILE_COLOR)),
    ))
    // The text is decorative for picking: the outer tile is the drag target.
    .with_child((
        Text::new(word),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::from(LETTER_COLOR)),
        Pickable::IGNORE,
    ))
    .observe(tile_drag_start)
    .observe(on_tile_drag)
    .observe(tile_drag_end);
}
