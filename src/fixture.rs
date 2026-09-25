//! Stage-0 test-only fixture for the approved deterministic-board proposal.
//!
//! This module arranges a reproducible board for tests and screenshots. It
//! is activated ONLY by environment variables and NEVER affects normal
//! launches: when `JELLOPOOL_SCENE` is unset, none of these systems are
//! registered (see main.rs) and the game behaves exactly as before.
//!
//! Activation:
//!   - `JELLOPOOL_SCENE=dense|scattered|edge` — which scene to commit. Setting
//!     it also implies `JELLOPOOL_SEED = FIXTURE_SEED` (see word_bank.rs) so
//!     the scene's words always exist in the tray.
//!   - `JELLOPOOL_WINDOWED` / `JELLOPOOL_SCALE_FACTOR` — the matching window
//!     override lives in main.rs.
//!
//! Scenes are authored against the reference viewport of 1600x900 logical
//! pixels: the writing zone is ~= 1248x693 logical px (78% width, flex-grow
//! above the 23% tray), so with the 40px snap grid a tile of typical size
//! (~50px tall, wider the longer the word) fits columns 0..~29, rows 0..~16.
//! Every entry is planned through `crate::tiles::placement::plan`, so any
//! authoring overlap is resolved by the same push cascade a real drop uses.

use crate::tiles::placement::{GRID, TileRect, plan};
use crate::tiles::{PlacedTile, WordTile, WritingZone};
use bevy::prelude::*;

/// Seed used for word selection whenever `JELLOPOOL_SCENE` is set (and
/// `JELLOPOOL_SEED` is not). Every scene word below comes from the selection
/// this seed produces with the shipped `assets/word_bank.ron`.
pub const FIXTURE_SEED: u64 = 0x5EED_5EED_5EED_5EED;

/// Bail out if UI layout has not produced real sizes after this many frames.
const MAX_WAIT_FRAMES: u32 = 600;

/// One fixture placement: the tile named `word` commits at grid (col, row).
type SceneEntry = (&'static str, u32, u32);

/// Tight multi-row block with several short/long adjacencies. Rows are
/// pitched 2 cells apart (tiles are taller than one 40px row); "the" is
/// authored one cell into "collapse"'s tail on purpose, so every run
/// exercises `plan`'s push cascade and ends with them flush-adjacent.
const DENSE: [SceneEntry; 12] = [
    ("collapse", 0, 0),
    ("the", 2, 0),
    ("shudder", 5, 0),
    ("a", 8, 0),
    ("if", 0, 2),
    ("beneath", 2, 2),
    ("moss", 5, 2),
    ("perhaps", 7, 2),
    ("all", 10, 2),
    ("his", 0, 4),
    ("weapon", 2, 4),
    ("world", 5, 4),
];

/// Spread across the zone. "but" is authored as "some"'s grid neighbor;
/// since tiles are taller than one row, `plan` settles them flush-adjacent.
const SCATTERED: [SceneEntry; 7] = [
    ("origin", 2, 1),
    ("glow", 14, 0),
    ("pit", 24, 3),
    ("some", 10, 5),
    ("but", 10, 6),
    ("under", 6, 9),
    ("closed", 20, 12),
];

/// Zone extremes: column 0, the rightmost feasible column for a short tile
/// (col 29: 1160 + ~56px <= 1248), row 0, and the bottom row adjacent to
/// the zone/tray seam (row 16: 640 + ~50px <= 693).
const EDGE: [SceneEntry; 6] = [
    ("within", 0, 5),
    ("an", 29, 8),
    ("upon", 12, 0),
    ("cliff", 15, 16),
    ("dead", 0, 16),
    ("lush", 24, 0),
];

/// The scene chosen at startup, made a resource so the OnEnter system can
/// arm the pending placement.
#[derive(Resource, Clone, Copy)]
pub struct FixtureScene(&'static [SceneEntry]);

/// Present while a fixture scene still needs to be committed. Removed once
/// the scene has been applied (or given up on).
#[derive(Resource)]
pub(crate) struct PendingFixture {
    scene: &'static [SceneEntry],
}

/// Read `JELLOPOOL_SCENE`; `None` (the default) leaves the game untouched.
pub fn scene_from_env() -> Option<FixtureScene> {
    let value = std::env::var("JELLOPOOL_SCENE").ok()?;
    let scene: &'static [SceneEntry] = match value.as_str() {
        "dense" => Some(&DENSE[..]),
        "scattered" => Some(&SCATTERED[..]),
        "edge" => Some(&EDGE[..]),
        other => {
            warn!("JELLOPOOL_SCENE={other:?} is not dense|scattered|edge; ignoring fixture");
            None
        }
    }?;
    Some(FixtureScene(scene))
}

/// Chained after `spawn_all_tiles` in `OnEnter(AppState::Playing)`: arms the
/// fixture. The zone and tiles were spawned in this same state transition
/// and UI layout only runs in PostUpdate, so real sizes do not exist yet —
/// placement happens in `apply_fixture_scene` on the first laid-out frame.
pub fn arm_fixture_scene(mut commands: Commands, scene: Res<FixtureScene>) {
    commands.insert_resource(PendingFixture { scene: scene.0 });
}

/// Commits the armed scene, once, on the first Update where the zone and
/// every tile have real layout sizes (font load gates text measurement).
/// Each entry is committed exactly like a valid `tile_drag_end` drop —
/// `ChildOf(zone)`, `PlacedTile(cell)`, absolute `Node` at left/top = cell —
/// minus SnapAnim/TileFeel, since an at-rest fixture needs no animation.
pub fn apply_fixture_scene(
    mut commands: Commands,
    pending: Res<PendingFixture>,
    zone: Single<(Entity, &ComputedNode), With<WritingZone>>,
    mut tiles: Query<(Entity, &Name, &ComputedNode, &mut Node), With<WordTile>>,
    mut frames_waited: Local<u32>,
) {
    let (zone_entity, zone_node) = zone.into_inner();
    // Zone bounds in logical units, exactly as tile_drag_end computes them.
    let bounds = zone_node.size * zone_node.inverse_scale_factor;

    let laid_out = bounds.cmpgt(Vec2::ZERO).all()
        && tiles
            .iter()
            .all(|(_, _, computed, _)| computed.size.cmpgt(Vec2::ZERO).all());
    if !laid_out {
        *frames_waited += 1;
        if *frames_waited > MAX_WAIT_FRAMES {
            warn!("fixture: UI layout never produced tile sizes; abandoning scene");
            commands.remove_resource::<PendingFixture>();
        }
        return;
    }

    // Rectangles the fixture itself has committed this run, fed to `plan`
    // exactly like the drag code's already-placed list.
    let mut committed: Vec<TileRect> = Vec::new();
    for &(word, col, row) in pending.scene {
        let cell = Vec2::new(col as f32 * GRID, row as f32 * GRID);
        let found = tiles
            .iter()
            .find_map(|(entity, name, computed, _)| {
                (name.as_str() == word
                    && !committed.iter().any(|tile| tile.entity == entity))
                    .then_some((entity, computed.size * computed.inverse_scale_factor))
            });
        let Some((entity, size)) = found else {
            warn!("fixture: no uncommitted tile named {word:?}; skipping");
            continue;
        };
        let held = TileRect {
            entity,
            pos: cell,
            size,
        };
        let Some(moved) = plan(held, &committed, bounds) else {
            warn!("fixture: no feasible plan for {word:?} at cell {cell:?}; skipping");
            continue;
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
    commands.remove_resource::<PendingFixture>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word_bank::{WordBank, select_words_with_rng};
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn fixture_selection() -> Vec<String> {
        let text = std::fs::read_to_string("assets/word_bank.ron").unwrap();
        let bank: WordBank = ron::from_str(&text).unwrap();
        select_words_with_rng(&bank, &mut SmallRng::seed_from_u64(FIXTURE_SEED))
    }

    /// Scene entries must reference words the fixture seed actually selects,
    /// or the fixture would silently skip tiles.
    #[test]
    fn scene_words_exist_in_fixture_seed_selection() {
        let selection = fixture_selection();
        for (scene, entries) in [
            ("dense", &DENSE[..]),
            ("scattered", &SCATTERED[..]),
            ("edge", &EDGE[..]),
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
        for entries in [&DENSE[..], &SCATTERED[..], &EDGE[..]] {
            for (index, (word, _, _)) in entries.iter().enumerate() {
                assert!(!entries[..index].iter().any(|(other, _, _)| other == word));
            }
        }
    }
}
