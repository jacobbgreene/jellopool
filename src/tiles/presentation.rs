//! Board typography, surfaces and tile construction. No drag decisions.
use super::drag::{on_tile_drag, tile_drag_end, tile_drag_start};
use super::placement::{LINE_COUNT, LINE_PITCH};
use super::writing::{
    LineGuide, ScrollThumb, ScrollTrack, WritingViewport, scroll_thumb_drag, scroll_track_press,
};
use crate::config::GameOptions;
use crate::prelude::*;
use crate::word_bank::select_words;
use bevy::text::LineHeight;

// Off-white paper on a cool, pale stock. Tray and gutters share one surface;
// the words carry the character. Vermilion remains exclusive to the live preview.
const INK: Srgba = Srgba::new(0.129, 0.145, 0.137, 1.0); // #212523
const PAPER: Srgba = Srgba::new(0.965, 0.961, 0.941, 1.0); // #F6F5F0
const TILE_FACE: Srgba = Srgba::new(0.984, 0.980, 0.961, 1.0); // #FBFAF5
const BANK: Srgba = Srgba::new(0.914, 0.918, 0.894, 1.0); // #E9EAE4
const OUTLINE: Srgba = Srgba::new(0.600, 0.616, 0.584, 1.0); // #999D95
const RULE: Srgba = Srgba::new(0.773, 0.784, 0.745, 1.0); // #C5C8BE
const SIGNATURE: Srgba = Srgba::new(0.392, 0.416, 0.384, 1.0); // #646A62
pub(super) const TILE_RADIUS: f32 = 2.0;

/// Keep the full word bank within its allotted area on smaller displays.
/// Scale the entire UI together so tile layout, grid and drag coordinates use
/// the same units. Larger viewports retain the normal 20px word size.
pub fn fit_board_to_viewport(camera: Single<&Camera, With<Camera2d>>, mut scale: ResMut<UiScale>) {
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    if size.min_element() <= 0.0 {
        return;
    }
    let fit = (size.x / 1600.0).min(size.y / 900.0).min(1.0);
    if scale.0 != fit {
        scale.0 = fit;
    }
}

pub fn spawn_board_root(mut commands: Commands, assets: Res<GameAssets>) {
    let mut root = commands.spawn((
        Name::new("board_root"),
        BackgroundColor(Color::from(BANK)),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::End,
            align_items: AlignItems::Center,
            ..default()
        },
        DespawnOnExit(AppState::Playing),
    ));
    root.with_children(|parent| spawn_writing_area(parent, assets.word_font.clone()));
    root.with_child(get_board_tray())
        // Hairline over the zone/tray seam, below the drag layer.
        .with_child(get_separator());
    if let Some(font) = &assets.signature_font {
        root.with_child(get_wordmark(font.clone()));
    }
    // Spawned last so dragged tiles paint above everything else.
    root.with_child(get_drag_layer());
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
        row_gap: Val::Px(8.0),
        column_gap: Val::Px(12.0),
        // Four rows of 40px slips + three 8px gaps + 20px padding = 204px,
        // within the reference tray's 207px even for longer word selections.
        padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
        ..default()
    };

    (name, BoardTray, color, node)
}

/// The quiet page/bank seam is an overlay, so it doesn't alter drop coordinates.
pub fn get_separator() -> (Name, BackgroundColor, Pickable, Node) {
    (
        Name::new("bank_separator"),
        BackgroundColor(Color::from(RULE)),
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

/// A small horizontal signature in the gutter. Live type stays sharp at any
/// DPI; viewport-relative sizing keeps the word within the existing margin.
pub fn get_wordmark(font: Handle<Font>) -> impl Bundle {
    (
        Name::new("wordmark"),
        Text::new("jellopool"),
        TextFont {
            font: FontSource::Handle(font),
            font_size: FontSize::Vw(1.4),
            style: FontStyle::Italic,
            ..default()
        },
        TextColor(Color::from(SIGNATURE)),
        TextLayout::no_wrap(),
        // Never block the pointer from reaching the board beneath it.
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(2.5),
            bottom: Val::Percent(23.0),
            padding: UiRect::bottom(Val::Px(18.0)),
            ..default()
        },
    )
}

fn spawn_writing_area(parent: &mut ChildSpawnerCommands, font: Handle<Font>) {
    parent
        .spawn((
            Name::new("writing_section"),
            Node {
                width: Val::Percent(78.0),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_height: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::vertical(Val::Px(24.0)),
                ..default()
            },
        ))
        .with_children(|section| {
            section
                .spawn((
                    Name::new("writing_frame"),
                    Node {
                        width: Val::Px(900.0),
                        max_width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        min_height: Val::Px(0.0),
                        ..default()
                    },
                ))
                .with_children(|frame| {
                    frame
                        .spawn((
                            Name::new("writing_viewport"),
                            WritingViewport,
                            ScrollPosition::default(),
                            BackgroundColor(Color::from(PAPER)),
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                min_height: Val::Px(0.0),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                        ))
                        .with_children(|viewport| {
                            viewport
                                .spawn((
                                    Name::new("writing_page"),
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(LINE_COUNT as f32 * LINE_PITCH + 48.0),
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                ))
                                .with_children(|page| {
                                    page.spawn((
                                        Name::new("writing_zone"),
                                        WritingZone,
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: Val::Px(64.0),
                                            right: Val::Px(32.0),
                                            top: Val::Px(24.0),
                                            height: Val::Px(LINE_COUNT as f32 * LINE_PITCH),
                                            ..default()
                                        },
                                    ))
                                    .with_children(|zone| {
                                        for line in 0..LINE_COUNT {
                                            zone.spawn((
                                                Name::new(format!("line_{}_guide", line + 1)),
                                                LineGuide,
                                                LayoutConfig {
                                                    use_rounding: false,
                                                },
                                                Pickable::IGNORE,
                                                BorderColor::from(Color::srgba(
                                                    0.60, 0.62, 0.58, 0.20,
                                                )),
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    left: Val::Px(0.0),
                                                    top: Val::Px(line as f32 * LINE_PITCH),
                                                    width: Val::Percent(100.0),
                                                    height: Val::Px(40.0),
                                                    border: UiRect::bottom(Val::Px(1.0)),
                                                    ..default()
                                                },
                                            ));
                                            zone.spawn((
                                                Name::new(format!("line_{}_number", line + 1)),
                                                Pickable::IGNORE,
                                                Text::new((line + 1).to_string()),
                                                TextFont {
                                                    font: FontSource::Handle(font.clone()),
                                                    font_size: FontSize::Px(14.0),
                                                    ..default()
                                                },
                                                TextColor(Color::srgba(0.392, 0.416, 0.384, 0.70)),
                                                LineHeight::Px(40.0),
                                                TextLayout::no_wrap(),
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    left: Val::Px(-40.0),
                                                    top: Val::Px(line as f32 * LINE_PITCH),
                                                    width: Val::Px(28.0),
                                                    height: Val::Px(40.0),
                                                    ..default()
                                                },
                                            ));
                                        }
                                    });
                                });
                        });
                    frame
                        .spawn((
                            Name::new("writing_scroll_track"),
                            ScrollTrack,
                            Node {
                                position_type: PositionType::Absolute,
                                right: Val::Px(4.0),
                                top: Val::Px(12.0),
                                bottom: Val::Px(12.0),
                                width: Val::Px(16.0),
                                ..default()
                            },
                        ))
                        .observe(scroll_track_press)
                        .with_children(|track| {
                            track
                                .spawn((
                                    Name::new("writing_scroll_thumb"),
                                    ScrollThumb,
                                    BackgroundColor(Color::from(OUTLINE)),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(5.0),
                                        top: Val::Px(0.0),
                                        width: Val::Px(6.0),
                                        height: Val::Px(32.0),
                                        border_radius: BorderRadius::all(Val::Px(3.0)),
                                        ..default()
                                    },
                                ))
                                .observe(scroll_thumb_drag);
                        });
                });
        });
}

pub fn spawn_all_tiles(
    mut commands: Commands,
    tray: Single<Entity, With<BoardTray>>,
    assets: Res<GameAssets>,
    word_banks: Res<Assets<crate::word_bank::WordBank>>,
    options: Res<GameOptions>,
) {
    let Some(spawned_word_bank) = word_banks.get(&assets.words) else {
        return;
    };

    let selected_words = select_words(spawned_word_bank, options.seed);

    // The tray arranges the tiles; there are no positions to compute.
    commands.entity(*tray).with_children(|tray| {
        for word in &selected_words {
            spawn_word_tile(tray, word, assets.word_font.clone());
        }
    });
}

fn spawn_word_tile(tray: &mut ChildSpawnerCommands, word: &str, font: Handle<Font>) {
    tray.spawn((
        Name::new(word.to_string()),
        WordTile,
        // Keep subpixel hairlines: pixel rounding can otherwise erase individual
        // border edges when the whole board is scaled down. Text rounds separately.
        LayoutConfig {
            use_rounding: false,
        },
        Node {
            // Compact paper slips: a little room around the word, without the
            // large button-like inset. Text still determines each tile's width.
            padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(TILE_RADIUS)),
            ..default()
        },
        BorderColor::from(Color::from(OUTLINE)),
        BackgroundColor(Color::from(TILE_FACE)),
    ))
    // The text is decorative for picking: the outer tile is the drag target.
    .with_child((
        Text::new(word),
        LayoutConfig::default(),
        TextFont {
            font: FontSource::Handle(font),
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::from(INK)),
        LineHeight::Px(24.0),
        TextLayout::no_wrap(),
        Pickable::IGNORE,
    ))
    .observe(tile_drag_start)
    .observe(on_tile_drag)
    .observe(tile_drag_end);
}
