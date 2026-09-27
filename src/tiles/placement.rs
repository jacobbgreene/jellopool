//! Pure, atomic placement planning. Inputs are committed, unanimated rectangles.
use bevy::prelude::*;

pub(crate) const GRID: f32 = 40.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TileRect {
    pub entity: Entity,
    pub pos: Vec2,
    pub size: Vec2,
}

fn overlaps(a: TileRect, b: TileRect) -> bool {
    a.pos.x < b.pos.x + b.size.x
        && a.pos.x + a.size.x > b.pos.x
        && a.pos.y < b.pos.y + b.size.y
        && a.pos.y + a.size.y > b.pos.y
}

fn fits(tile: TileRect, bounds: Vec2) -> bool {
    tile.pos.cmpge(Vec2::ZERO).all() && (tile.pos + tile.size).cmple(bounds).all()
}

/// One row's vertical pitch: the smallest grid multiple that separates two
/// tile-height rectangles. Tile heights are uniform (fixed font + padding),
/// so this is effectively a constant per tile.
fn row_pitch(tile: TileRect) -> f32 {
    (tile.size.y / GRID).ceil() * GRID
}

/// Flow rule, side-aware: push `tile` away from the overlapping `source`
/// along x only — left if the tile's center is left of the source's, right
/// otherwise (coincident centers push right). The push is the smallest grid
/// multiple that separates the pair. If the left escape would cross x = 0,
/// the tile extends right instead; if the right side would cross the right
/// edge of `bounds`, the tile wraps to the start of the next row (x = 0,
/// one row pitch down), like a word moving to the next line.
///
/// Side-awareness matters: a purely rightward rule flings left neighbors
/// across the source (push distance = source.right - tile.left), so dragging
/// into a row shoves every tile ahead of the cursor to the zone's edge.
fn push_apart(source: TileRect, tile: TileRect, bounds: Vec2) -> TileRect {
    let push_left = tile.pos.x + tile.size.x * 0.5 < source.pos.x + source.size.x * 0.5;
    let mut moved = tile;
    if push_left {
        // Overlap implies tile.right > source.left: positive overlap depth.
        let distance = tile.pos.x + tile.size.x - source.pos.x;
        moved.pos.x -= (distance / GRID).ceil() * GRID;
        if moved.pos.x < 0.0 {
            // No room on the left: extend right instead (the wrap check
            // below still applies).
            moved = tile;
            let distance = source.pos.x + source.size.x - tile.pos.x;
            moved.pos.x += (distance / GRID).ceil() * GRID;
        }
    } else {
        // Overlap implies tile.left < source.right: positive overlap depth.
        let distance = source.pos.x + source.size.x - tile.pos.x;
        moved.pos.x += (distance / GRID).ceil() * GRID;
    }
    if moved.pos.x + moved.size.x > bounds.x {
        moved.pos.x = 0.0;
        moved.pos.y += row_pitch(tile);
    }
    moved
}

/// Plan the drop of `held` among `committed`: every committed tile's position
/// (pushed sideways / wrapped down where the cascade requires) or None if the
/// drop is infeasible (something fell off the bottom edge, or the cascade
/// ping-ponged without resolving).
///
/// Pushes are side-aware (left escapes exist), so reading-order rank is not
/// monotone and cycles are possible; the step budget turns a cyclic cascade
/// into a prompt None instead of an infinite loop.
pub(crate) fn plan(held: TileRect, committed: &[TileRect], bounds: Vec2) -> Option<Vec<TileRect>> {
    if !fits(held, bounds) {
        return None;
    }
    let mut result = committed.to_vec();
    result.sort_by_key(|tile| tile.entity.to_bits());
    if !result.iter().any(|tile| overlaps(held, *tile)) {
        return Some(result);
    }
    // Each step pushes one tile at least one grid cell, and a resolving
    // cascade pushes each tile only a handful of times. A budget well above
    // that turns a cyclic cascade into a prompt infeasible answer instead
    // of an infinite loop.
    let mut budget = 16 * (result.len() + 1) * (result.len() + 1);
    // The queue holds entities, not rectangles: a tile enqueued by an
    // earlier push may have moved again since, and only its current
    // position may push neighbors.
    let mut queue = std::collections::VecDeque::from([held.entity]);
    while let Some(entity) = queue.pop_front() {
        budget = budget.checked_sub(1)?;
        let source = if entity == held.entity {
            held
        } else {
            *result.iter().find(|tile| tile.entity == entity)?
        };
        for tile in result.iter_mut() {
            if tile.entity == source.entity || !overlaps(source, *tile) {
                continue;
            }
            let mut moved = push_apart(source, *tile, bounds);
            // The held tile never moves; a push landing on its cell must
            // also clear it. Rightward re-push keeps the rank monotone.
            if overlaps(held, moved) {
                moved = push_apart(held, moved, bounds);
            }
            if !fits(moved, bounds) {
                return None;
            }
            *tile = moved;
            queue.push_back(moved.entity);
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rect(id: u32, x: f32, y: f32, w: f32) -> TileRect {
        TileRect {
            entity: Entity::from_raw_u32(id).unwrap(),
            pos: Vec2::new(x, y),
            size: Vec2::new(w, 60.0),
        }
    }
    fn pos_of(result: &[TileRect], id: u32) -> Vec2 {
        result
            .iter()
            .find(|tile| tile.entity == Entity::from_raw_u32(id).unwrap())
            .unwrap()
            .pos
    }
    #[test]
    fn pushes_are_horizontal_and_side_aware() {
        // Vertical overlaps with coincident centers push right; horizontal
        // overlaps push away along x — never vertical.
        let other = rect(1, 160.0, 160.0, 80.0);
        for (x, y, expected) in [
            (120.0, 160.0, Vec2::new(200.0, 160.0)),
            (200.0, 160.0, Vec2::new(120.0, 160.0)),
            (160.0, 120.0, Vec2::new(240.0, 160.0)),
            (160.0, 200.0, Vec2::new(240.0, 160.0)),
        ] {
            assert_eq!(
                plan(rect(0, x, y, 80.0), &[other], Vec2::splat(600.0)).unwrap()[0].pos,
                expected
            );
        }
    }
    #[test]
    fn left_neighbor_nudges_left_not_across() {
        // Dragging rightward into a tile whose center is left of the held
        // tile's: a small leftward nudge, NOT a leapfrog across the held
        // tile to its right side (the "shoved to the zone edge" bug).
        let committed = [rect(1, 80.0, 0.0, 100.0)];
        let result = plan(
            rect(0, 140.0, 0.0, 100.0),
            &committed,
            Vec2::splat(600.0),
        )
        .unwrap();
        assert_eq!(pos_of(&result, 1), Vec2::new(40.0, 0.0));
    }
    #[test]
    fn left_escape_at_edge_extends_right() {
        // No room left of the source: the tile comes back and extends right.
        let committed = [rect(1, 0.0, 0.0, 100.0)];
        let result = plan(rect(0, 60.0, 0.0, 100.0), &committed, Vec2::splat(600.0)).unwrap();
        assert_eq!(pos_of(&result, 1), Vec2::new(160.0, 0.0));
    }
    #[test]
    fn variable_width_cascade_and_repeat() {
        let tiles = [rect(1, 80.0, 80.0, 130.0), rect(2, 240.0, 80.0, 90.0)];
        let held = rect(0, 0.0, 80.0, 190.0);
        for _ in 0..3 {
            let result = plan(held, &tiles, Vec2::splat(600.0)).unwrap();
            assert_eq!(
                result
                    .iter()
                    .find(|tile| tile.entity == tiles[0].entity)
                    .unwrap()
                    .pos
                    .x,
                200.0
            );
            assert_eq!(
                result
                    .iter()
                    .find(|tile| tile.entity == tiles[1].entity)
                    .unwrap()
                    .pos
                    .x,
                360.0
            );
            assert!(!overlaps(result[0], result[1]));
        }
        let away = plan(rect(0, 0.0, 240.0, 190.0), &tiles, Vec2::splat(600.0)).unwrap();
        assert_eq!(
            away.iter()
                .find(|tile| tile.entity == tiles[0].entity)
                .unwrap()
                .pos,
            tiles[0].pos
        );
        // Narrow bounds force tile 2 to wrap; short bounds then leave no
        // row below, so the drop is infeasible.
        assert!(plan(held, &tiles, Vec2::new(400.0, 200.0)).is_none());
        assert_eq!(tiles[0].pos.x, 80.0);
    }
    #[test]
    fn oversized_and_tie() {
        assert!(plan(rect(0, 0.0, 0.0, 700.0), &[], Vec2::splat(600.0)).is_none());
        let result = plan(
            rect(0, 80.0, 80.0, 80.0),
            &[rect(1, 80.0, 80.0, 80.0)],
            Vec2::splat(600.0),
        )
        .unwrap();
        assert_eq!(result[0].pos, Vec2::new(160.0, 80.0));
    }
    #[test]
    fn sparse_field_leaves_distant_tiles_alone() {
        let committed = [
            rect(1, 80.0, 80.0, 80.0),
            rect(2, 600.0, 80.0, 80.0),
            rect(3, 80.0, 400.0, 120.0),
            rect(4, 1000.0, 600.0, 80.0),
        ];
        let result = plan(
            rect(0, 0.0, 80.0, 120.0),
            &committed,
            Vec2::new(1200.0, 800.0),
        )
        .unwrap();
        // Only the tile overlapping the held tile joins the cascade.
        assert_eq!(pos_of(&result, 1), Vec2::new(120.0, 80.0));
        for id in [2, 3, 4] {
            let original = committed
                .iter()
                .find(|tile| tile.entity == Entity::from_raw_u32(id).unwrap())
                .unwrap()
                .pos;
            assert_eq!(pos_of(&result, id), original);
        }
    }
    #[test]
    fn push_past_right_edge_wraps_to_next_row() {
        // The tile fills the row up to the right edge; pushing it right by
        // any amount overflows, so it wraps to x = 0 one row pitch (80px:
        // the smallest grid multiple >= the 60px test height) down.
        let committed = [rect(1, 520.0, 0.0, 80.0)];
        let result = plan(
            rect(0, 480.0, 0.0, 80.0),
            &committed,
            Vec2::splat(600.0),
        )
        .unwrap();
        assert_eq!(pos_of(&result, 1), Vec2::new(0.0, 80.0));
    }
    #[test]
    fn unresolvable_crowd_returns_none_promptly() {
        // A dense ring around the held tile in a tight zone: resolving every
        // overlap wraps tiles downward until one falls off the bottom, so
        // the plan is infeasible — reported promptly instead of spraying
        // tiles around.
        let committed = [
            rect(1, 120.0, 40.0, 80.0),
            rect(2, 160.0, 80.0, 80.0),
            rect(3, 100.0, 120.0, 80.0),
            rect(4, 60.0, 80.0, 80.0),
        ];
        assert!(plan(rect(0, 80.0, 40.0, 80.0), &committed, Vec2::splat(240.0)).is_none());
    }
    #[test]
    fn bottom_overflow_returns_none() {
        // A column of tiles in a one-tile-wide zone: each push overflows the
        // right edge and wraps a row down, until the last tile wraps past
        // the bottom. Termination comes from monotone reading-order rank,
        // not a budget: the cascade simply runs out of rows.
        let committed = [
            rect(1, 40.0, 0.0, 80.0),
            rect(2, 40.0, 80.0, 80.0),
            rect(3, 0.0, 160.0, 80.0),
        ];
        assert!(plan(rect(0, 0.0, 0.0, 80.0), &committed, Vec2::new(120.0, 240.0)).is_none());
    }
    #[test]
    fn push_beyond_bounds_is_infeasible() {
        // The overlapped tile can only escape right, but the zone is too
        // narrow to hold it there and too short to hold the wrapped row:
        // None, with no partial plan for callers to apply.
        assert!(
            plan(
                rect(0, 0.0, 0.0, 80.0),
                &[rect(1, 40.0, 0.0, 80.0)],
                Vec2::new(120.0, 100.0),
            )
            .is_none()
        );
    }
}
