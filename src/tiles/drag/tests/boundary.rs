use super::super::*;
use super::harness::DragHarness;

fn near_left_edge(dpi: f32, ui_scale: f32) -> DragHarness {
    let mut f = DragHarness::scaled(dpi, ui_scale, UVec2::new(120, 60));
    for (entity, x) in f.neighbors.into_iter().zip([0.0, 90.0]) {
        f.app
            .world_mut()
            .entity_mut(entity)
            .insert(PlacedTile(Vec2::new(x, 2.0 * LINE_PITCH)));
    }
    f
}

#[test]
fn shift_release_between_words_keeps_the_left_word_fixed_at_all_scales() {
    for (dpi, ui_scale) in [(1.0, 1.0), (1.5, 0.8), (2.0, 1.25)] {
        for key in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
            let mut f = near_left_edge(dpi, ui_scale);
            let y = 2.0 * LINE_PITCH;
            f.grab(Vec2::new(10.0, 410.0));
            f.set_key(key, true);
            f.drag_to(Vec2::new(60.0, y));
            f.step();
            assert_eq!(f.pushed_x(0), 0.0);
            assert_eq!(f.pushed_x(1), 90.0);
            assert_eq!(
                f.app.world().get::<Node>(f.held).unwrap().left,
                Val::Px(60.0)
            );
            assert_eq!(
                f.app
                    .world()
                    .resource::<PlacementPreview>()
                    .0
                    .as_ref()
                    .unwrap()
                    .cell
                    .x,
                80.0
            );
            f.set_key(key, false);
            for _ in 0..5 {
                f.step();
                assert_eq!(f.pushed_x(0), 0.0);
                assert_eq!(f.pushed_x(1), 160.0);
                assert_eq!(
                    f.app.world().get::<Node>(f.held).unwrap().left,
                    Val::Px(80.0)
                );
            }
            let expected = [
                (f.held, Vec2::new(80.0, y)),
                (f.neighbors[0], Vec2::new(0.0, y)),
                (f.neighbors[1], Vec2::new(160.0, y)),
            ];
            f.release_at(Vec2::new(70.0, y + 10.0));
            f.assert_placed(&expected);
            f.assert_settles_to(&expected);
        }
    }
}

#[test]
fn boundary_drop_uses_release_position_with_or_without_a_phase_preview() {
    for preview in [false, true] {
        for release_shift in [false, true] {
            let mut f = near_left_edge(1.5, 0.8);
            let y = 2.0 * LINE_PITCH;
            f.grab(Vec2::new(10.0, 410.0));
            f.set_key(KeyCode::ShiftLeft, true);
            if preview {
                f.drag_to(Vec2::new(60.0, y));
                f.step();
            }
            if release_shift {
                f.set_key(KeyCode::ShiftLeft, false);
            }
            f.release_at(Vec2::new(70.0, y + 10.0));
            let expected = [
                (f.held, Vec2::new(80.0, y)),
                (f.neighbors[0], Vec2::new(0.0, y)),
                (f.neighbors[1], Vec2::new(160.0, y)),
            ];
            f.assert_placed(&expected);
            f.assert_settles_to(&expected);
        }
    }
}

#[test]
fn stationary_boundary_adjustment_does_not_start_a_new_leftward_push() {
    for immediate_drop in [false, true] {
        let mut f = near_left_edge(1.0, 1.0);
        let y = 2.0 * LINE_PITCH;
        f.app
            .world_mut()
            .entity_mut(f.neighbors[1])
            .insert(PlacedTile(Vec2::new(100.0, y)));
        f.grab(Vec2::new(10.0, 410.0));
        f.set_key(KeyCode::ShiftLeft, true);
        f.drag_to(Vec2::new(150.0, y));
        f.step();
        f.set_key(KeyCode::ShiftLeft, false);
        f.step();
        if !immediate_drop {
            for _ in 0..5 {
                f.step();
            }
        }
        // Both neighbors are to the left of the intended insertion. Their gap
        // must survive the held tile's nudge from 150 to 180, even on release.
        assert_eq!(f.pushed_x(0), 0.0);
        assert_eq!(f.pushed_x(1), 100.0);
        assert_eq!(
            f.app.world().get::<Node>(f.held).unwrap().left,
            Val::Px(180.0)
        );
        f.release_at(Vec2::new(160.0, y + 10.0));
        f.assert_placed(&[
            (f.held, Vec2::new(180.0, y)),
            (f.neighbors[0], Vec2::new(0.0, y)),
            (f.neighbors[1], Vec2::new(100.0, y)),
        ]);
    }
}

#[test]
fn moving_right_after_a_boundary_nudge_never_pushes_left() {
    let mut f = near_left_edge(1.0, 1.0);
    let y = 2.0 * LINE_PITCH;
    f.app
        .world_mut()
        .entity_mut(f.neighbors[1])
        .insert(PlacedTile(Vec2::new(100.0, y)));
    f.grab(Vec2::new(10.0, 410.0));
    f.set_key(KeyCode::ShiftLeft, true);
    f.drag_to(Vec2::new(150.0, y));
    f.step();
    f.set_key(KeyCode::ShiftLeft, false);
    f.step();
    for (requested, resolved) in [
        (151.0, 180.0),
        (179.0, 180.0),
        (180.0, 180.0),
        (200.0, 200.0),
    ] {
        f.drag_to(Vec2::new(requested, y));
        f.step();
        assert_eq!(f.pushed_x(0), 0.0);
        assert_eq!(f.pushed_x(1), 100.0);
        assert_eq!(
            f.app.world().get::<Node>(f.held).unwrap().left,
            Val::Px(resolved)
        );
    }
    // A genuine leftward move can still push normally.
    f.drag_to(Vec2::new(170.0, y));
    f.step();
    assert_eq!(f.pushed_x(0), 0.0);
    assert_eq!(f.pushed_x(1), 90.0);
    assert_eq!(
        f.app.world().get::<Node>(f.held).unwrap().left,
        Val::Px(170.0)
    );
}

#[test]
fn rejected_boundary_drop_does_not_commit_partial_pushes() {
    for phased in [false, true] {
        let mut f = near_left_edge(1.0, 1.0);
        f.app
            .world_mut()
            .get_mut::<ComputedNode>(f.neighbors[1])
            .unwrap()
            .size
            .x = 500.0;
        f.grab(Vec2::new(10.0, 410.0));
        f.set_key(KeyCode::ShiftLeft, true);
        f.drag_to(Vec2::new(60.0, 2.0 * LINE_PITCH));
        f.step();
        f.set_key(KeyCode::ShiftLeft, phased);
        f.step();
        assert!(f.app.world().resource::<PlacementPreview>().0.is_none());
        assert_eq!(f.pushed_x(0), 0.0);
        assert_eq!(f.pushed_x(1), 90.0);
        f.release_at(Vec2::new(70.0, 2.0 * LINE_PITCH + 10.0));
        assert!(f.app.world().get::<DragFollow>(f.held).is_none());
        assert!(f.app.world().get::<PlacedTile>(f.held).is_none());
        let parent = f.app.world().get::<ChildOf>(f.held).unwrap().parent();
        assert!(f.app.world().get::<BoardTray>(parent).is_some());
        for (entity, x) in f.neighbors.into_iter().zip([0.0, 90.0]) {
            assert_eq!(f.app.world().get::<PlacedTile>(entity).unwrap().0.x, x);
        }
    }
}

#[test]
fn canceling_a_boundary_insertion_restores_both_neighbors() {
    let mut f = near_left_edge(1.0, 1.0);
    f.grab(Vec2::new(10.0, 410.0));
    f.set_key(KeyCode::ShiftLeft, true);
    f.drag_to(Vec2::new(60.0, 2.0 * LINE_PITCH));
    f.step();
    f.set_key(KeyCode::ShiftLeft, false);
    f.step();
    assert_eq!(f.pushed_x(1), 160.0);
    f.set_key(KeyCode::Escape, true);
    for _ in 0..90 {
        f.step();
    }
    assert!(f.app.world().get::<DragFollow>(f.held).is_none());
    assert!(f.app.world().get::<PlacedTile>(f.held).is_none());
    for (entity, x) in f.neighbors.into_iter().zip([0.0, 90.0]) {
        assert_eq!(f.app.world().get::<PlacedTile>(entity).unwrap().0.x, x);
        assert_eq!(f.app.world().get::<Node>(entity).unwrap().left, Val::Px(x));
    }
}
