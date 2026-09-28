use super::super::*;
use super::harness::{DragHarness, geometry};
use crate::test_support::input::pointer;
use crate::tiles::animation::px_or_zero;

#[test]
fn missing_camera_ignores_grab_without_mutating_tile() {
    let mut fixture = DragHarness::new();
    let camera = fixture
        .app
        .world()
        .get::<ComputedUiTargetCamera>(fixture.held)
        .unwrap()
        .get()
        .unwrap();
    fixture.app.world_mut().despawn(camera);
    fixture.app.world_mut().trigger(pointer(
        fixture.held,
        Vec2::new(10.0, 410.0),
        DragStart {
            button: PointerButton::Primary,
            hit: bevy::picking::backend::HitData::new(fixture.zone, 0.0, None, None),
        },
    ));
    fixture.app.world_mut().flush();
    assert_eq!(fixture.app.world().resource::<ActiveDrag>().0, None);
    assert!(
        fixture
            .app
            .world()
            .get::<DragFollow>(fixture.held)
            .is_none()
    );
    assert_eq!(
        fixture.app.world().get::<Node>(fixture.held).unwrap().top,
        Val::Px(400.0)
    );
}

#[test]
fn rapid_release_commits_without_a_preview_frame_and_settles() {
    let mut fixture = DragHarness::new();
    let expected = [
        (fixture.held, Vec2::new(160.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[0], Vec2::new(240.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[1], Vec2::new(320.0, LINE_PITCH * 2.0)),
    ];
    fixture.grab(Vec2::new(10.0, 410.0));
    // Release is authoritative even before Drag/highlight/preview runs.
    fixture.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
    fixture.assert_placed(&expected);
    assert_eq!(
        fixture
            .app
            .world()
            .get::<Node>(fixture.neighbors[0])
            .unwrap()
            .left,
        Val::Px(160.0)
    );
    fixture.step();
    fixture.assert_placed(&expected);
    assert!(fixture.app.world().get::<SnapAnim>(fixture.held).is_some());
    fixture.assert_settles_to(&expected);
}

#[test]
fn rapid_release_after_one_preview_frame_keeps_committed_push() {
    let mut fixture = DragHarness::new();
    let expected = [
        (fixture.held, Vec2::new(160.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[0], Vec2::new(240.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[1], Vec2::new(320.0, LINE_PITCH * 2.0)),
    ];
    fixture.grab(Vec2::new(10.0, 410.0));
    fixture.app.world_mut().trigger(pointer(
        fixture.held,
        Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0),
        Drag {
            button: PointerButton::Primary,
            distance: Vec2::new(160.0, -320.0),
            delta: Vec2::new(160.0, -320.0),
        },
    ));
    fixture.step();
    let neighbor = fixture.neighbors[0];
    let world = fixture.app.world();
    let preview_x = px_or_zero(world.get::<Node>(neighbor).unwrap().left);
    assert!(preview_x > 160.0 && preview_x < 240.0);
    assert_eq!(world.get::<PlacedTile>(neighbor).unwrap().0.x, 160.0);
    fixture.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
    fixture.assert_placed(&expected);
    fixture.step();
    assert!(px_or_zero(fixture.app.world().get::<Node>(neighbor).unwrap().left) > preview_x);
    fixture.assert_settles_to(&expected);
}

#[test]
fn regrab_during_snap_then_escape_preserves_last_commit() {
    let mut fixture = DragHarness::new();
    let expected = [
        (fixture.held, Vec2::new(160.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[0], Vec2::new(240.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[1], Vec2::new(320.0, LINE_PITCH * 2.0)),
    ];
    fixture.grab(Vec2::new(10.0, 410.0));
    fixture.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
    fixture.assert_placed(&expected);
    fixture.step();
    assert!(fixture.app.world().get::<SnapAnim>(fixture.held).is_some());
    // Stand in for layout's transform propagation after this animation frame.
    let node = fixture.app.world().get::<Node>(fixture.held).unwrap();
    let pos = Vec2::new(px_or_zero(node.left), px_or_zero(node.top));
    fixture
        .app
        .world_mut()
        .entity_mut(fixture.held)
        .insert(geometry(Vec2::new(80.0, 40.0), pos));
    fixture.grab(pos + Vec2::splat(10.0));
    assert!(fixture.app.world().get::<SnapAnim>(fixture.held).is_none());
    assert_eq!(
        fixture
            .app
            .world()
            .get::<DragFollow>(fixture.held)
            .unwrap()
            .origin,
        Some(Vec2::new(160.0, LINE_PITCH * 2.0))
    );
    fixture
        .app
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    fixture.step();
    fixture
        .app
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    assert!(
        fixture
            .app
            .world()
            .get::<DragFollow>(fixture.held)
            .is_none()
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(fixture.held)
            .unwrap()
            .parent(),
        fixture.zone
    );
    fixture.assert_settles_to(&expected);
}

#[test]
fn foreign_pointer_and_button_cannot_move_or_release_an_owned_drag() {
    let mut fixture = DragHarness::new();
    let expected = [
        (fixture.held, Vec2::new(160.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[0], Vec2::new(240.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[1], Vec2::new(320.0, LINE_PITCH * 2.0)),
    ];
    fixture.grab(Vec2::new(10.0, 410.0));
    for (owner, button) in [
        (PointerId::Touch(7), PointerButton::Primary),
        (PointerId::Mouse, PointerButton::Secondary),
    ] {
        let mut event = pointer(
            fixture.held,
            Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0),
            Drag {
                button,
                distance: Vec2::ZERO,
                delta: Vec2::ZERO,
            },
        );
        event.pointer_id = owner;
        fixture.app.world_mut().trigger(event);
        let mut event = pointer(
            fixture.held,
            Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0),
            DragEnd {
                button,
                distance: Vec2::ZERO,
            },
        );
        event.pointer_id = owner;
        fixture.app.world_mut().trigger(event);
        fixture.app.world_mut().flush();
        let follow = fixture.app.world().get::<DragFollow>(fixture.held).unwrap();
        assert_eq!(follow.pointer, Vec2::new(10.0, 410.0));
    }
    fixture.cancel(PointerId::Touch(7));
    fixture.step();
    assert!(
        fixture
            .app
            .world()
            .get::<DragFollow>(fixture.held)
            .is_some()
    );
    assert_eq!(
        fixture
            .app
            .world_mut()
            .query_filtered::<Entity, With<TrayGap>>()
            .iter(fixture.app.world())
            .count(),
        1
    );
    fixture.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
    fixture.assert_placed(&expected);
}

#[test]
fn settled_frames_do_not_dirty_tile_nodes() {
    let mut fixture = DragHarness::new();
    let expected = [
        (fixture.held, Vec2::new(160.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[0], Vec2::new(240.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[1], Vec2::new(320.0, LINE_PITCH * 2.0)),
    ];
    fixture.grab(Vec2::new(10.0, 410.0));
    fixture.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
    fixture.assert_placed(&expected);
    fixture.assert_settles_to(&expected);
    fixture.app.world_mut().clear_trackers();
    fixture.step();
    assert_eq!(
        fixture
            .app
            .world_mut()
            .query_filtered::<Entity, (With<WordTile>, Changed<Node>)>()
            .iter(fixture.app.world())
            .count(),
        0
    );
}
