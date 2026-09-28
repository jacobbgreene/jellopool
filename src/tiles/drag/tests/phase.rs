use super::super::*;
use super::harness::DragHarness;

#[test]
fn pushes_persist_after_moving_on_and_returning_to_the_tray() {
    let mut f = DragHarness::new();
    f.grab(Vec2::new(10.0, 410.0));
    f.drag_to(Vec2::new(0.0, 2.0 * LINE_PITCH));
    f.step();
    f.drag_to(Vec2::new(330.0, 2.0 * LINE_PITCH));
    f.step();
    assert_eq!(f.pushed_x(0), 410.0);
    assert_eq!(f.pushed_x(1), 490.0);
    f.drag_to(Vec2::ZERO);
    f.step();
    assert_eq!(f.pushed_x(0), 410.0);
    f.release_at(Vec2::new(10.0, 410.0));
    f.step();
    assert!(f.app.world().get::<PlacedTile>(f.held).is_none());
    assert_eq!(
        f.app.world().get::<PlacedTile>(f.neighbors[0]).unwrap().0.x,
        410.0
    );
    assert_eq!(
        f.app.world().get::<PlacedTile>(f.neighbors[1]).unwrap().0.x,
        490.0
    );
}

#[test]
fn either_shift_phases_without_motion_and_resumes_without_replaying_the_path() {
    let mut f = DragHarness::new();
    f.grab(Vec2::new(10.0, 410.0));
    f.drag_to(Vec2::new(0.0, 2.0 * LINE_PITCH));
    f.step();
    f.set_key(KeyCode::ShiftLeft, true);
    f.step();
    f.drag_to(Vec2::new(330.0, 2.0 * LINE_PITCH));
    f.step();
    assert_eq!(f.pushed_x(0), 160.0);
    assert_eq!(f.pushed_x(1), 240.0);
    f.set_key(KeyCode::ShiftRight, true);
    f.step();
    f.set_key(KeyCode::ShiftLeft, false);
    f.step();
    assert!(f.app.world().get::<DragFollow>(f.held).unwrap().phased);
    f.set_key(KeyCode::ShiftRight, false);
    f.step();
    assert!(!f.app.world().get::<DragFollow>(f.held).unwrap().phased);
    assert_eq!(f.pushed_x(0), 160.0);
    // Moving left now pushes in that direction, not along the skipped path.
    f.drag_to(Vec2::new(270.0, 2.0 * LINE_PITCH));
    f.step();
    assert_eq!(f.pushed_x(0), 110.0);
    assert_eq!(f.pushed_x(1), 190.0);
}

#[test]
fn phase_toggle_preserves_prior_pushes_and_escape_rolls_them_back() {
    let mut f = DragHarness::new();
    f.grab(Vec2::new(10.0, 410.0));
    f.drag_to(Vec2::new(160.0, 2.0 * LINE_PITCH));
    f.step();
    assert_eq!(f.pushed_x(0), 240.0);
    f.set_key(KeyCode::ShiftLeft, true);
    f.step();
    f.drag_to(Vec2::new(400.0, 2.0 * LINE_PITCH));
    f.step();
    assert_eq!(f.pushed_x(0), 240.0);
    f.app
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    f.step();
    for _ in 0..90 {
        f.step();
    }
    for (entity, x) in f.neighbors.into_iter().zip([160.0, 240.0]) {
        assert_eq!(f.app.world().get::<PlacedTile>(entity).unwrap().0.x, x);
        assert_eq!(f.app.world().get::<Node>(entity).unwrap().left, Val::Px(x));
    }
}

#[test]
fn phased_overlap_drop_still_makes_room_even_without_an_update() {
    for dpi in [1.0, 1.5, 2.0] {
        let mut f = DragHarness::scaled(dpi, 0.8, UVec2::new(120, 60));
        let expected = [
            (f.held, Vec2::new(160.0, LINE_PITCH * 2.0)),
            (f.neighbors[0], Vec2::new(240.0, LINE_PITCH * 2.0)),
            (f.neighbors[1], Vec2::new(320.0, LINE_PITCH * 2.0)),
        ];
        f.grab(Vec2::new(10.0, 410.0));
        f.drag_to(Vec2::new(0.0, 2.0 * LINE_PITCH));
        f.step();
        // Press Shift then release over words before the next Update.
        f.app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        f.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
        f.assert_placed(&expected);
        f.assert_settles_to(&expected);
    }
}

#[test]
fn releasing_shift_while_over_a_word_pushes_it_without_pointer_motion() {
    let mut f = DragHarness::new();
    let expected = [
        (f.held, Vec2::new(160.0, LINE_PITCH * 2.0)),
        (f.neighbors[0], Vec2::new(240.0, LINE_PITCH * 2.0)),
        (f.neighbors[1], Vec2::new(320.0, LINE_PITCH * 2.0)),
    ];
    f.grab(Vec2::new(10.0, 410.0));
    f.set_key(KeyCode::ShiftLeft, true);
    f.step();
    f.drag_to(Vec2::new(160.0, 2.0 * LINE_PITCH));
    f.step();
    assert_eq!(f.pushed_x(0), 160.0);
    f.set_key(KeyCode::ShiftLeft, false);
    f.step();
    assert_eq!(f.pushed_x(0), 240.0);
    f.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
    f.assert_placed(&expected);
    f.assert_settles_to(&expected);
}

#[test]
fn phase_dims_the_whole_tile_and_restores_colors_on_toggle_drop_and_cancel() {
    for cancel in [false, true] {
        let mut f = DragHarness::new();
        let expected = [
            (f.held, Vec2::new(160.0, LINE_PITCH * 2.0)),
            (f.neighbors[0], Vec2::new(240.0, LINE_PITCH * 2.0)),
            (f.neighbors[1], Vec2::new(320.0, LINE_PITCH * 2.0)),
        ];
        let background = BackgroundColor(Color::srgba(0.9, 0.8, 0.7, 0.8));
        let border = BorderColor::from(Color::srgba(0.4, 0.3, 0.2, 0.9));
        let ink = TextColor(Color::srgba(0.1, 0.2, 0.3, 0.7));
        f.app
            .world_mut()
            .entity_mut(f.held)
            .insert((background, border));
        let label = f.app.world_mut().spawn((ink, ChildOf(f.held))).id();
        f.grab(Vec2::new(10.0, 410.0));
        for _ in 0..3 {
            f.set_key(KeyCode::ShiftLeft, true);
            f.step();
            assert!(
                (f.app
                    .world()
                    .get::<BackgroundColor>(f.held)
                    .unwrap()
                    .0
                    .alpha()
                    - 0.8 * 0.35)
                    .abs()
                    < 0.001
            );
            assert!(
                (f.app.world().get::<TextColor>(label).unwrap().0.alpha() - 0.7 * 0.35).abs()
                    < 0.001
            );
            f.set_key(KeyCode::ShiftLeft, false);
            f.step();
            assert_eq!(
                *f.app.world().get::<BackgroundColor>(f.held).unwrap(),
                background
            );
            assert_eq!(*f.app.world().get::<BorderColor>(f.held).unwrap(), border);
            assert_eq!(*f.app.world().get::<TextColor>(label).unwrap(), ink);
        }
        f.set_key(KeyCode::ShiftLeft, true);
        f.step();
        if cancel {
            f.cancel(PointerId::Mouse);
            f.step();
        } else {
            f.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
            f.assert_placed(&expected);
            f.step();
        }
        assert_eq!(
            *f.app.world().get::<BackgroundColor>(f.held).unwrap(),
            background
        );
        assert_eq!(*f.app.world().get::<BorderColor>(f.held).unwrap(), border);
        assert_eq!(*f.app.world().get::<TextColor>(label).unwrap(), ink);
    }
}

#[test]
fn phase_releases_a_blocked_tile_without_moving_the_pointer() {
    let mut f = DragHarness::new();
    f.grab(Vec2::new(10.0, 410.0));
    f.drag_to(Vec2::new(0.0, 2.0 * LINE_PITCH));
    f.step();
    f.drag_to(Vec2::new(550.0, 2.0 * LINE_PITCH));
    f.step();
    assert_eq!(
        f.app.world().get::<Node>(f.held).unwrap().left,
        Val::Px(400.0)
    );
    f.set_key(KeyCode::ShiftLeft, true);
    f.step();
    assert_eq!(
        f.app.world().get::<Node>(f.held).unwrap().left,
        Val::Px(550.0)
    );
    assert_eq!(f.pushed_x(0), 480.0);
    assert_eq!(f.pushed_x(1), 560.0);
}
