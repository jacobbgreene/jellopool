mod drag;
pub(crate) mod placement;
use crate::states::AppState;
use crate::word_bank::{WordBank, WordBankHandle, select_words};
use bevy::prelude::*;
pub use drag::{
    PlacedTile, cancel_drag_system, push_preview_system, snap_anim_system, tile_feel_system,
    tile_follow_system, tray_gap_anim_system, tray_gap_system, zone_snap_highlight_system,
};
use drag::{on_tile_drag, tile_drag_end, tile_drag_start};

/// Handle to the literary typeface used for tile text.
#[derive(Resource)]
pub struct GameFont(pub Handle<Font>);

// === Palette: warm light paper surfaces, dark ink, neutral outlines ===
// Vermilion #C44732 is the sole accent, reserved for the landing preview
// (see drag.rs); the old brass/amber scheme is retired.
const INK: Srgba       = Srgba::new(0.145, 0.137, 0.122, 1.0); // #25231F — ink text
const PAPER: Srgba     = Srgba::new(0.953, 0.937, 0.898, 1.0); // #F3EFE5 — the page
const TILE_FACE: Srgba = Srgba::new(0.933, 0.914, 0.871, 1.0); // #EEE9DE — tile face
const BANK: Srgba      = Srgba::new(0.902, 0.878, 0.831, 1.0); // #E6E0D4 — word-bank tray
const SURROUND: Srgba  = Srgba::new(0.851, 0.827, 0.776, 1.0); // #D9D3C6 — quiet surround, a step darker than the bank
const OUTLINE: Srgba   = Srgba::new(0.588, 0.569, 0.529, 1.0); // #969187 — neutral outline (tile borders, separator)

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

pub fn spawn_board_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            Name::new("board_root"),
            BackgroundColor(Color::from(SURROUND)),
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
        // Hairline over the zone/tray seam, below the drag layer.
        .with_child(get_separator())
        // Wordmark sits between the separator and the drag layer; the drag
        // layer must remain last so dragged tiles paint above everything.
        .with_child(get_wordmark(&asset_server))
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
    let color = BackgroundColor(Color::from(BANK));
    let node = Node {
        width: Val::Percent(78.0),
        height: Val::Percent(23.0),
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        justify_content: JustifyContent::Center,
        align_content: AlignContent::Center,
        row_gap: Val::Px(12.0),
        column_gap: Val::Px(12.0),
        padding: UiRect::all(Val::Px(20.0)),
        border_radius: BorderRadius::all(Val::Px(6.0)),
        ..default()
    };

    (name, BoardTray, color, node)
}

/// 1px hairline marking the seam between the writing zone and the bank tray.
/// Spawned as a root child (between tray and drag layer), never as a tray
/// child: tray indexing assumes tray children are only tiles and gaps, so an
/// in-flow or tray-parented separator would corrupt tile slot bookkeeping.
/// Absolute positioning keeps it out of the root's column flow.
pub fn get_separator() -> (Name, BackgroundColor, Pickable, Node) {
    (
        Name::new("bank_separator"),
        BackgroundColor(Color::from(OUTLINE)),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            // The zone/tray seam: the tray occupies the bottom 23% of the root.
            bottom: Val::Percent(23.0),
            width: Val::Percent(78.0),
            height: Val::Px(1.0),
            ..default()
        },
    )
}

/// Fixed gutter wordmark ("jellopool", ink on transparent) per the approved
/// proposal: a subordinate, non-pickable identity element in the left gutter,
/// outside the composition area. Absolute positioning takes it out of the
/// root's column flow, so it claims no board space. Bottom-aligned with the
/// writing zone's bottom edge (the tray reserves the bottom 23% of the root);
/// the max-width guard keeps it inside the 11% gutter.
pub fn get_wordmark(asset_server: &AssetServer) -> (Name, ImageNode, Pickable, Node) {
    (
        Name::new("wordmark"),
        ImageNode::new(asset_server.load("wordmark.png")),
        // Never block the pointer from reaching the board beneath it.
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            // Roughly gutter-centered for the derived width.
            left: Val::Percent(2.2),
            bottom: Val::Percent(23.0),
            height: Val::Percent(50.0),
            // Width derives from the 262×1240 source via the aspect ratio.
            aspect_ratio: Some(262.0 / 1240.0),
            max_width: Val::Percent(8.0),
            ..default()
        },
    )
}

pub fn get_writing_zone() -> (Name, WritingZone, BackgroundColor, Node) {
    let name = Name::new("writing_zone");
    let color = BackgroundColor(Color::from(PAPER));
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
    font: Res<GameFont>,
) {
    let Some(spawned_word_bank) = word_banks.get(&word_bank_handle.0) else {
        return;
    };

    let selected_words = select_words(spawned_word_bank);

    // The tray arranges the tiles; there are no positions to compute.
    commands.entity(*tray).with_children(|tray| {
        for word in &selected_words {
            spawn_word_tile(tray, word, font.0.clone());
        }
    });
}

fn spawn_word_tile(tray: &mut ChildSpawnerCommands, word: &str, font: Handle<Font>) {
    tray.spawn((
        Name::new(word.to_string()),
        WordTile {
            unique_word: word.to_string(),
        },
        Node {
            // Padding + text size the tile; width is not derived from word length.
            // Stage-2 fine border: 3px → 1px removes 4px per axis; +2px
            // padding per side compensates exactly, preserving outer bounds
            // and text inset (19px horizontal, 13px vertical).
            padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            // Radius may not drop below the retained border width (Bevy
            // clamps the fill's inner radius at radius − border); with the
            // 1px border, 3px corners now render crisp.
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BorderColor::from(Color::from(OUTLINE)),
        BackgroundColor(Color::from(TILE_FACE)),
    ))
    // The text is decorative for picking: the outer tile is the drag target.
    .with_child((
        Text::new(word),
        TextFont {
            font: FontSource::Handle(font),
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::from(INK)),
        Pickable::IGNORE,
    ))
    .observe(tile_drag_start)
    .observe(on_tile_drag)
    .observe(tile_drag_end);
}
