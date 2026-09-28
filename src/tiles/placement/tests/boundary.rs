use super::*;

#[test]
fn left_boundary_keeps_the_left_neighbor_and_pushes_only_the_right_chain() {
    // Include fractional font widths and a scrolled line, not just integer cells.
    for (left_width, held_width) in [(80.0, 80.0), (80.25, 83.5)] {
        for row in [0, 12] {
            let held = rect(0, left_width - 20.0, row, held_width);
            let neighbors = [
                rect(1, 0.0, row, left_width),
                rect(2, left_width + 10.0, row, 80.0),
                rect(3, left_width + 90.0, row, 60.0),
                rect(4, 400.0, row, 80.0),
                rect(5, 0.0, row + 1, 80.0),
            ];
            let bounds = Vec2::new(640.0, LINE_COUNT as f32 * LINE_PITCH);
            let (cell, result) = push(held, None, &neighbors, bounds).unwrap();
            assert_eq!(cell, Vec2::new(left_width, held.pos.y));
            for (before, x) in neighbors.iter().zip([
                0.0,
                left_width + held_width,
                left_width + held_width + 80.0,
                400.0,
                0.0,
            ]) {
                let after = result
                    .iter()
                    .find(|tile| tile.entity == before.entity)
                    .unwrap();
                assert_eq!(after.pos, Vec2::new(x, before.pos.y));
            }
            assert!(valid_result(
                TileRect { pos: cell, ..held },
                &result,
                bounds
            ));
        }
    }
}

#[test]
fn left_boundary_preserves_the_entire_left_side_including_gaps() {
    let held = rect(0, 150.0, 0, 80.0);
    let neighbors = [
        rect(1, 0.0, 0, 80.0),
        rect(2, 100.0, 0, 80.0),
        rect(3, 200.0, 0, 80.0),
    ];
    let (cell, result) = push(held, None, &neighbors, Vec2::splat(640.0)).unwrap();
    assert_eq!(cell.x, 180.0);
    for (before, x) in neighbors.iter().zip([0.0, 100.0, 260.0]) {
        let after = result
            .iter()
            .find(|tile| tile.entity == before.entity)
            .unwrap();
        assert_eq!(after.pos.x, x);
    }
}

#[test]
fn left_boundary_rejects_when_the_right_side_cannot_fit_without_swapping_order() {
    let held = rect(0, 140.0, 0, 80.0);
    let neighbors = [rect(1, 0.0, 0, 200.0), rect(2, 220.0, 0, 80.0)];
    // The same overlap can resolve only when the right side has enough capacity.
    assert!(push(held, None, &neighbors, Vec2::new(440.0, 400.0)).is_some());
    assert!(push(held, None, &neighbors, Vec2::new(320.0, 400.0)).is_none());
    assert_eq!(neighbors[0].pos, Vec2::ZERO);
    assert_eq!(neighbors[1].pos.x, 220.0);
}

#[test]
fn a_blocked_right_side_does_not_change_the_intended_insertion_slot() {
    let held = rect(0, 190.0, 0, 80.0);
    let neighbor = rect(1, 220.0, 0, 80.0);
    // Moving the neighbor left of the held tile would fit, but reverse the intent.
    assert!(push(held, None, &[neighbor], Vec2::new(320.0, 400.0)).is_none());
}
