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

/// Side of the closest overlapping center, normalized for rectangular sizes.
/// Horizontal wins equal-axis ties; coincident centers push right.
fn direction(held: TileRect, other: TileRect) -> Vec2 {
    let delta = (other.pos + other.size * 0.5 - held.pos - held.size * 0.5)
        / ((held.size + other.size) * 0.5);
    if delta.x.abs() >= delta.y.abs() {
        Vec2::new(if delta.x < 0.0 { -1.0 } else { 1.0 }, 0.0)
    } else {
        Vec2::new(0.0, if delta.y < 0.0 { -1.0 } else { 1.0 })
    }
}

/// Move `tile` directly away from the overlapping `source` by the smallest
/// grid multiple that separates the two along the push axis.
fn push_apart(source: TileRect, tile: TileRect) -> TileRect {
    let dir = direction(source, tile);
    let distance = if dir.x > 0.0 {
        source.pos.x + source.size.x - tile.pos.x
    } else if dir.x < 0.0 {
        tile.pos.x + tile.size.x - source.pos.x
    } else if dir.y > 0.0 {
        source.pos.y + source.size.y - tile.pos.y
    } else {
        tile.pos.y + tile.size.y - source.pos.y
    };
    let mut moved = tile;
    moved.pos += dir * (distance / GRID).ceil() * GRID;
    moved
}

/// Plan the drop of `held` among `committed`: every committed tile's position
/// (pushed where the cascade requires) or None if the drop is infeasible.
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
        for index in 0..result.len() {
            let tile = result[index];
            if tile.entity == source.entity || !overlaps(source, tile) {
                continue;
            }
            let mut moved = push_apart(source, tile);
            // The held tile never moves; a push landing on its cell must
            // also clear it, even if that means clearing toward the displacer.
            if overlaps(held, moved) {
                moved = push_apart(held, moved);
            }
            if !fits(moved, bounds) {
                return None;
            }
            result[index] = moved;
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
    fn four_sides() {
        let other = rect(1, 160.0, 160.0, 80.0);
        for (x, y, expected) in [
            (120.0, 160.0, Vec2::new(200.0, 160.0)),
            (200.0, 160.0, Vec2::new(120.0, 160.0)),
            (160.0, 120.0, Vec2::new(160.0, 200.0)),
            (160.0, 200.0, Vec2::new(160.0, 120.0)),
        ] {
            assert_eq!(
                plan(rect(0, x, y, 80.0), &[other], Vec2::splat(600.0)).unwrap()[0].pos,
                expected
            );
        }
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
        assert!(plan(held, &tiles, Vec2::new(400.0, 600.0)).is_none());
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
    fn push_direction_is_per_pair_not_global() {
        // One neighbor primarily left of the held tile, one primarily below:
        // each must move along its own away axis. With a single global
        // direction (taken from the nearest overlap, the left one) the lower
        // tile would be shoved sideways instead of down.
        let committed = [rect(1, 160.0, 200.0, 80.0), rect(2, 210.0, 240.0, 80.0)];
        let result = plan(rect(0, 200.0, 200.0, 80.0), &committed, Vec2::splat(600.0)).unwrap();
        assert_eq!(pos_of(&result, 1), Vec2::new(120.0, 200.0));
        assert_eq!(pos_of(&result, 2), Vec2::new(210.0, 280.0));
        assert!(!overlaps(result[0], result[1]));
    }
    #[test]
    fn unresolvable_crowd_returns_none_promptly() {
        // A dense ring around the held tile in a tight zone: resolving every
        // overlap eventually shoves a neighbor out of bounds (or would cycle
        // against the bounded step budget), so the plan is infeasible —
        // reported promptly instead of looping or spraying tiles around.
        let committed = [
            rect(1, 120.0, 40.0, 80.0),
            rect(2, 160.0, 80.0, 80.0),
            rect(3, 100.0, 120.0, 80.0),
            rect(4, 60.0, 80.0, 80.0),
        ];
        assert!(plan(rect(0, 80.0, 40.0, 80.0), &committed, Vec2::splat(240.0)).is_none());
    }
    #[test]
    fn cyclic_cascade_hits_step_budget() {
        // The two tiles ping-pong against the held tile's cell: pushing one
        // off the other lands it on the held cell, and clearing that overlap
        // shoves it back onto its partner. Nothing ever leaves the bounds,
        // so only the step budget ends it — promptly, with None.
        let committed = [rect(1, 40.0, 0.0, 40.0), rect(2, 0.0, 0.0, 40.0)];
        assert!(plan(rect(0, 40.0, 0.0, 160.0), &committed, Vec2::new(200.0, 120.0)).is_none());
    }
    #[test]
    fn push_beyond_bounds_is_infeasible() {
        // The overlapped tile can only escape right, but the zone is too
        // narrow: None, with no partial plan for callers to apply.
        assert!(
            plan(
                rect(0, 0.0, 0.0, 80.0),
                &[rect(1, 40.0, 0.0, 80.0)],
                Vec2::new(120.0, 200.0),
            )
            .is_none()
        );
    }
}
