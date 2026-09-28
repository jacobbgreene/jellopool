use super::*;

mod performance;

fn rect(id: u32, x: f32, row: usize, width: f32) -> TileRect {
    TileRect {
        entity: Entity::from_raw_u32(id).unwrap(),
        pos: Vec2::new(x, row as f32 * LINE_PITCH),
        size: Vec2::new(width, 40.0),
    }
}

#[test]
fn fast_live_push_keeps_words_ahead_and_stops_at_the_page_edge() {
    let neighbors = [
        rect(1, 160.0, 0, 80.0),
        rect(2, 240.0, 0, 80.0),
        rect(3, 160.0, 1, 80.0),
    ];
    for (target, resolved) in [(330.0, 330.0), (560.0, 400.0)] {
        let (cell, result) = push(
            rect(0, target, 0, 80.0),
            Some(Vec2::ZERO),
            &neighbors,
            Vec2::splat(640.0),
        )
        .unwrap();
        assert_eq!(cell.x, resolved);
        assert_eq!(result[0].pos.x, resolved + 80.0);
        assert_eq!(result[1].pos.x, resolved + 160.0);
        assert_eq!(result[2].pos, neighbors[2].pos);
    }
}

#[test]
fn live_push_left_and_reversal_do_not_pull_words_back() {
    let neighbors = [rect(1, 160.0, 0, 80.0), rect(2, 240.0, 0, 80.0)];
    let (cell, result) = push(
        rect(0, 0.0, 0, 80.0),
        Some(Vec2::new(400.0, 0.0)),
        &neighbors,
        Vec2::splat(640.0),
    )
    .unwrap();
    assert_eq!(cell.x, 160.0);
    assert_eq!(result[0].pos.x, 0.0);
    assert_eq!(result[1].pos.x, 80.0);
    let (cell, reversed) = push(
        rect(0, 400.0, 0, 80.0),
        Some(cell),
        &result,
        Vec2::splat(640.0),
    )
    .unwrap();
    assert_eq!(cell.x, 400.0);
    for (after, before) in reversed.iter().zip(result) {
        assert_eq!(after.pos, before.pos);
    }
}

#[test]
fn randomized_live_pushes_preserve_bounds_order_and_lines() {
    use rand::{RngExt, SeedableRng, rngs::SmallRng};
    let mut rng = SmallRng::seed_from_u64(0x50555348);
    let bounds = Vec2::new(804.0, 24.0 * LINE_PITCH);
    let mut held = rect(0, 0.0, 0, 80.0);
    let mut neighbors = vec![
        rect(1, 120.0, 0, 71.25),
        rect(2, 220.0, 0, 143.5),
        rect(3, 520.0, 0, 46.25),
        rect(4, 40.0, 1, 140.0),
    ];
    for _ in 0..10_000 {
        let previous = held.pos;
        held.pos.x = rng.random_range(0..725) as f32;
        let before = neighbors.clone();
        let (cell, result) = push(held, Some(previous), &neighbors, bounds).unwrap();
        held.pos = cell;
        assert!(valid_result(held, &result, bounds));
        let position = |id| {
            result
                .iter()
                .find(|tile| tile.entity == rect(id, 0.0, 0, 1.0).entity)
                .unwrap()
                .pos
        };
        assert!(position(1).x < position(2).x && position(2).x < position(3).x);
        assert_eq!(position(4), Vec2::new(40.0, LINE_PITCH));
        for after in &result {
            let before = before
                .iter()
                .find(|tile| tile.entity == after.entity)
                .unwrap();
            assert_eq!(after.pos.y, before.pos.y);
        }
        neighbors = result;
    }
}

#[test]
fn empty_space_and_touching_edges_never_shift_neighbors() {
    let neighbors = [
        rect(1, 0.0, 0, 80.0),
        rect(2, 240.0, 0, 80.0),
        rect(3, 80.0, 1, 160.0),
    ];
    let held = rect(0, 80.0, 0, 160.0);
    let result = plan(held, &neighbors, Vec2::splat(640.0)).unwrap();
    for tile in result {
        assert_eq!(
            tile.pos,
            neighbors
                .iter()
                .find(|other| other.entity == tile.entity)
                .unwrap()
                .pos
        );
    }
}

#[test]
fn a_small_overlap_only_opens_the_missing_space() {
    let held = rect(0, 80.0, 1, 82.25);
    let neighbors = [rect(1, 160.0, 1, 80.0), rect(2, 300.0, 1, 80.0)];
    let result = plan(held, &neighbors, Vec2::splat(640.0)).unwrap();
    assert_eq!(
        result
            .iter()
            .find(|tile| tile.entity == neighbors[0].entity)
            .unwrap()
            .pos
            .x,
        162.25
    );
    assert_eq!(
        result
            .iter()
            .find(|tile| tile.entity == neighbors[1].entity)
            .unwrap()
            .pos
            .x,
        300.0
    );
    assert!(valid_result(held, &result, Vec2::splat(640.0)));
}

#[test]
fn overlap_can_shift_left_without_repacking_gaps() {
    let held = rect(0, 150.0, 0, 80.0);
    let neighbors = [rect(1, 0.0, 0, 40.0), rect(2, 80.0, 0, 80.0)];
    let result = plan(held, &neighbors, Vec2::splat(640.0)).unwrap();
    assert_eq!(
        result
            .iter()
            .find(|tile| tile.entity == neighbors[1].entity)
            .unwrap()
            .pos
            .x,
        70.0
    );
    assert_eq!(
        result
            .iter()
            .find(|tile| tile.entity == neighbors[0].entity)
            .unwrap()
            .pos
            .x,
        0.0
    );
}

#[test]
fn only_contacting_neighbors_on_the_same_line_can_follow_a_push() {
    let held = rect(0, 80.0, 1, 90.0);
    let neighbors = [
        rect(1, 160.0, 1, 80.0),
        rect(2, 240.0, 1, 80.0),
        rect(3, 160.0, 2, 80.0),
        rect(4, 400.0, 1, 80.0),
    ];
    let result = plan(held, &neighbors, Vec2::splat(640.0)).unwrap();
    for (before, x) in neighbors.iter().zip([170.0, 250.0, 160.0, 400.0]) {
        let after = result
            .iter()
            .find(|tile| tile.entity == before.entity)
            .unwrap();
        assert_eq!(after.pos, Vec2::new(x, before.pos.y));
    }
}

#[test]
fn full_line_rejects_without_wrapping_or_mutating_input() {
    let held = rect(0, 0.0, 0, 80.0);
    let neighbors = [rect(1, 0.0, 0, 160.0), rect(2, 160.0, 0, 160.0)];
    assert!(plan(held, &neighbors, Vec2::new(320.0, 1000.0)).is_none());
    assert_eq!(neighbors[0].pos, Vec2::ZERO);
}

#[test]
fn blocked_side_can_open_space_on_the_other_side_of_the_word() {
    let held = rect(0, 100.0, 0, 80.0);
    let neighbor = rect(1, 0.0, 0, 150.0);
    let result = plan(held, &[neighbor], Vec2::splat(640.0)).unwrap();
    assert_eq!(result[0].pos.x, 180.0);
}

#[test]
fn invalid_arrangements_are_never_reported_as_success() {
    let held = rect(0, 0.0, 3, 80.0);
    let bounds = Vec2::splat(400.0);
    assert!(plan(held, &[rect(1, 360.0, 0, 80.0)], bounds).is_none());
    assert!(
        plan(
            held,
            &[rect(1, 80.0, 0, 80.0), rect(2, 120.0, 0, 80.0)],
            bounds
        )
        .is_none()
    );
    assert!(plan(rect(0, 0.0, 0, 500.0), &[], bounds).is_none());
}

#[test]
fn randomized_plans_preserve_geometry_lines_and_empty_space() {
    use rand::{RngExt, SeedableRng, rngs::SmallRng};
    let mut rng = SmallRng::seed_from_u64(0xCA5CADE);
    let bounds = Vec2::new(804.0, LINE_COUNT as f32 * LINE_PITCH);
    for _ in 0..10_000 {
        let mut make = |id| {
            rect(
                id,
                rng.random_range(0..19) as f32 * GRID,
                rng.random_range(0..8),
                rng.random_range(120..640) as f32 / 4.0,
            )
        };
        let held = make(0);
        let mut neighbors = Vec::new();
        for id in 1..40 {
            let tile = make(id);
            if fits(tile, bounds) && neighbors.iter().all(|other| !overlaps(tile, *other)) {
                neighbors.push(tile);
            }
        }
        let touching = neighbors.iter().any(|tile| overlaps(held, *tile));
        if let Some(result) = plan(held, &neighbors, bounds) {
            assert!(valid_result(held, &result, bounds));
            assert_eq!(result.len(), neighbors.len());
            for after in &result {
                let before = neighbors
                    .iter()
                    .find(|tile| tile.entity == after.entity)
                    .unwrap();
                assert_eq!(after.size, before.size);
                assert_eq!(after.pos.y, before.pos.y);
                if !touching || before.pos.y != held.pos.y {
                    assert_eq!(after.pos, before.pos);
                }
            }
        }
    }
}
