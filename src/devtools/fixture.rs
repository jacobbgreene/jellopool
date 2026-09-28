//! Deterministic, opt-in arrangements for visual regression checks.
//!
//! This module arranges a reproducible board for tests and screenshots. It
//! is activated ONLY by environment variables and NEVER affects normal
//! launches: when `JELLOPOOL_SCENE` is unset, none of these systems are
//! registered (see devtools/mod.rs).
//!
//! Activation:
//!   - `JELLOPOOL_SCENE=empty|poem|dense|scattered|edge|scrolled|push|phase` — scene to commit. Setting
//!     it also implies `JELLOPOOL_SEED = FIXTURE_SEED` unless overridden, so
//!     the scene's words always exist in the tray.
//!   - `JELLOPOOL_WINDOWED` / `JELLOPOOL_SCALE_FACTOR` — the matching window
//!     override lives in devtools/mod.rs.
//!
//! Scenes target the centered 900px paper, with an 804px writing width,
//! 40px horizontal steps and 24 numbered lines at a 56px pitch.
//! Every entry is planned through `crate::tiles::placement::plan`, so any
//! authoring overlap is resolved by the same push cascade a real drop uses.

use crate::prelude::*;
use crate::tiles::placement::{GRID, LINE_PITCH, TileRect, plan};
use crate::tiles::writing::WritingViewport;
use bevy::app::AppExit;
use bevy::text::TextLayoutInfo;

/// Seed used for word selection whenever `JELLOPOOL_SCENE` is set (and
/// `JELLOPOOL_SEED` is not). Every scene word below comes from the selection
/// this seed produces with the shipped `assets/word_bank.ron`.
pub const FIXTURE_SEED: u64 = 0x5EED_5EED_5EED_5EED;

/// Bail out if UI layout has not produced real sizes after this many frames.
const MAX_WAIT_FRAMES: u32 = 600;

/// One fixture placement: the tile named `word` commits at grid (col, row).
type SceneEntry = (&'static str, u32, u32);

/// Tight multi-row block with several short/long adjacencies. Rows are
/// pitched 2 cells apart to leave a row of breathing room; "the" is
/// authored one cell into "collapse"'s tail on purpose, so every run
/// exercises `plan`'s push cascade and ends with them flush-adjacent.
const DENSE: [SceneEntry; 12] = [
    ("collapse", 0, 0),
    ("the", 2, 0),
    ("shudder", 7, 0),
    ("a", 10, 0),
    ("if", 0, 2),
    ("beneath", 2, 2),
    ("moss", 5, 2),
    ("perhaps", 7, 2),
    ("all", 10, 2),
    ("his", 0, 4),
    ("weapon", 2, 4),
    ("world", 5, 4),
];

/// Spread across numbered lines. The last word sits below the initial viewport
/// so a scroll reveals another part of the composition.
const SCATTERED: [SceneEntry; 7] = [
    ("origin", 2, 1),
    ("glow", 12, 0),
    ("pit", 17, 3),
    ("some", 8, 5),
    ("but", 8, 6),
    ("under", 4, 9),
    ("closed", 14, 12),
];

/// A deliberately odd little arrangement for judging type and visual hierarchy.
const POEM: [SceneEntry; 6] = [
    ("the", 0, 0),
    ("moss", 2, 0),
    ("had", 0, 1),
    ("a", 2, 1),
    ("weapon", 4, 1),
    ("perhaps", 0, 3),
];

/// Visible page extremes at the reference viewport.
const EDGE: [SceneEntry; 6] = [
    ("within", 0, 5),
    ("an", 18, 8),
    ("upon", 12, 0),
    ("cliff", 15, 10),
    ("dead", 0, 10),
    ("lush", 17, 0),
];

const SCROLLED: [SceneEntry; 6] = [
    ("the", 0, 14),
    ("moss", 2, 14),
    ("had", 0, 16),
    ("a", 2, 16),
    ("weapon", 4, 16),
    ("perhaps", 0, 23),
];

/// The scene chosen at startup, made a resource so the OnEnter system can
/// arm the pending placement.
#[derive(Resource, Clone, Copy)]
pub struct FixtureScene(&'static [SceneEntry], f32, pub(super) Option<bool>);

/// Present while a fixture scene still needs to be committed. Removed once
/// the scene has been applied (or given up on).
#[derive(Resource)]
pub(crate) struct PendingFixture {
    scene: &'static [SceneEntry],
    scroll: f32,
}

/// Validate a requested scene; unknown names must not silently run a random board.
pub fn parse_scene(value: &str) -> Result<FixtureScene, String> {
    let scene: &'static [SceneEntry] = match value {
        "empty" => &[],
        "poem" | "push" | "phase" => &POEM,
        "dense" => &DENSE,
        "scattered" => &SCATTERED,
        "edge" => &EDGE,
        "scrolled" => &SCROLLED,
        other => return Err(format!("Unknown JELLOPOOL_SCENE={other:?}")),
    };
    Ok(FixtureScene(
        scene,
        if value == "scrolled" {
            24.0 * LINE_PITCH
        } else {
            0.0
        },
        matches!(value, "push" | "phase").then_some(value == "phase"),
    ))
}

/// Chained after `spawn_all_tiles` in `OnEnter(AppState::Playing)`: arms the
/// fixture. The zone and tiles were spawned in this same state transition
/// and UI layout only runs in PostUpdate, so real sizes do not exist yet —
/// placement happens in `apply_fixture_scene` on the first laid-out frame.
pub fn arm_fixture_scene(mut commands: Commands, scene: Res<FixtureScene>) {
    commands.insert_resource(PendingFixture {
        scene: scene.0,
        scroll: scene.1,
    });
}

/// Commits the armed scene, once, on the first Update where the zone and
/// every tile have real layout sizes (font load gates text measurement).
/// Each entry is committed exactly like a valid `tile_drag_end` drop —
/// `ChildOf(zone)`, `PlacedTile(cell)`, absolute `Node` at left/top = cell —
/// minus SnapAnim/TileFeel, since an at-rest fixture needs no animation.
#[allow(clippy::too_many_arguments)]
pub fn apply_fixture_scene(
    mut commands: Commands,
    pending: Res<PendingFixture>,
    zone: Single<(Entity, &ComputedNode), With<WritingZone>>,
    mut tiles: Query<(Entity, &Name, &ComputedNode, &mut Node), With<WordTile>>,
    text: Query<(&Text, &TextLayoutInfo)>,
    mut viewport: Query<(&ComputedNode, &mut ScrollPosition), With<WritingViewport>>,
    mut title: Query<&mut bevy::text::EditableText, With<crate::tiles::title::PoemTitle>>,
    mut exit: MessageWriter<AppExit>,
    mut frames_waited: Local<u32>,
) {
    let (zone_entity, zone_node) = zone.into_inner();
    // Zone bounds in logical units, exactly as tile_drag_end computes them.
    let bounds = zone_node.size * zone_node.inverse_scale_factor;

    let laid_out = !text.is_empty()
        && text.iter().all(|(value, layout)| {
            value.is_empty() || (!layout.glyphs.is_empty() && layout.size.cmpgt(Vec2::ZERO).all())
        })
        && bounds.cmpgt(Vec2::ZERO).all()
        && tiles
            .iter()
            .all(|(_, _, computed, _)| computed.size.cmpgt(Vec2::ZERO).all());
    if !laid_out {
        *frames_waited += 1;
        if *frames_waited > MAX_WAIT_FRAMES {
            error!("fixture: text/layout did not become ready; failing scene");
            exit.write(AppExit::error());
            commands.remove_resource::<PendingFixture>();
        }
        return;
    }

    // Rectangles the fixture itself has committed this run, fed to `plan`
    // exactly like the drag code's already-placed list.
    let mut committed: Vec<TileRect> = Vec::new();
    for &(word, col, row) in pending.scene {
        let cell = Vec2::new(col as f32 * GRID, row as f32 * LINE_PITCH);
        let found = tiles.iter().find_map(|(entity, name, computed, _)| {
            (name.as_str() == word && !committed.iter().any(|tile| tile.entity == entity))
                .then_some((entity, computed.size * computed.inverse_scale_factor))
        });
        let Some((entity, size)) = found else {
            error!("fixture: no uncommitted tile named {word:?}");
            exit.write(AppExit::error());
            commands.remove_resource::<PendingFixture>();
            return;
        };
        let held = TileRect {
            entity,
            pos: cell,
            size,
        };
        let Some((cell, moved)) = plan(held, &committed, bounds) else {
            error!("fixture: no feasible plan for {word:?} at cell {cell:?}");
            exit.write(AppExit::error());
            commands.remove_resource::<PendingFixture>();
            return;
        };
        // Mirror tile_drag_end: tiles the plan pushed adopt their new
        // logical positions. With no preview animation, set them directly.
        for tile in &moved {
            commands.entity(tile.entity).insert(PlacedTile(tile.pos));
            if let Ok((_, _, _, mut node)) = tiles.get_mut(tile.entity) {
                node.left = Val::Px(tile.pos.x);
                node.top = Val::Px(tile.pos.y);
            }
            if let Some(entry) = committed
                .iter_mut()
                .find(|entry| entry.entity == tile.entity)
            {
                entry.pos = tile.pos;
            }
        }
        // The committed tile itself: the exact drop invariants, at rest.
        if let Ok((_, _, _, mut node)) = tiles.get_mut(entity) {
            node.position_type = PositionType::Absolute;
            node.left = Val::Px(cell.x);
            node.top = Val::Px(cell.y);
        }
        commands
            .entity(entity)
            .insert(ChildOf(zone_entity))
            .insert(PlacedTile(cell));
        committed.push(TileRect {
            entity,
            pos: cell,
            size,
        });
    }
    if let Ok((node, mut scroll)) = viewport.single_mut() {
        scroll.0.y = pending
            .scroll
            .min((node.content_size.y - node.size.y).max(0.0) * node.inverse_scale_factor);
    }
    if pending.scene == POEM
        && let Ok(mut title) = title.single_mut()
    {
        title.queue_edit(bevy::text::TextEdit::Insert("A small possibility".into()));
    }
    commands.remove_resource::<PendingFixture>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word_bank::{WordBank, select_words_with_rng};
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn drag_scenes_select_real_words_and_the_requested_mode() {
        assert!(fixture_selection().iter().any(|word| word == "just"));
        assert_eq!(parse_scene("push").unwrap().2, Some(false));
        assert_eq!(parse_scene("phase").unwrap().2, Some(true));
        assert_eq!(parse_scene("poem").unwrap().2, None);
    }

    fn fixture_selection() -> Vec<String> {
        let text = std::fs::read_to_string("assets/word_bank.ron").unwrap();
        let bank: WordBank = ron::from_str(&text).unwrap();
        select_words_with_rng(&bank, &mut SmallRng::seed_from_u64(FIXTURE_SEED)).unwrap()
    }

    /// Scene entries must reference words the fixture seed actually selects,
    /// or the fixture would silently skip tiles.
    #[test]
    fn scene_words_exist_in_fixture_seed_selection() {
        let selection = fixture_selection();
        for (scene, entries) in [
            ("poem", &POEM[..]),
            ("dense", &DENSE[..]),
            ("scattered", &SCATTERED[..]),
            ("edge", &EDGE[..]),
            ("scrolled", &SCROLLED[..]),
        ] {
            for &(word, _, _) in entries {
                assert!(
                    selection.iter().any(|selected| selected == word),
                    "{scene}: word {word:?} is not in the FIXTURE_SEED selection"
                );
            }
        }
    }

    /// A duplicated word in a scene would place two tiles at two cells under
    /// one name; scenes must use distinct words.
    #[test]
    fn scene_words_are_distinct_within_each_scene() {
        for entries in [
            &POEM[..],
            &DENSE[..],
            &SCATTERED[..],
            &EDGE[..],
            &SCROLLED[..],
        ] {
            for (index, (word, _, _)) in entries.iter().enumerate() {
                assert!(!entries[..index].iter().any(|(other, _, _)| other == word));
            }
        }
    }
}
