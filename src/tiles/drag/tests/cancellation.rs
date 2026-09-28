use super::super::*;
use super::harness::DragHarness;
use crate::test_support::input::pointer;
use crate::tiles::animation::push_preview_system;

#[test]
fn escape_restores_held_and_rolls_back_neighbors() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<PlacementPreview>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<bevy::picking::pointer::PointerInput>()
        .add_message::<bevy::window::WindowFocused>()
        .add_systems(Update, (cancel_drag_system, push_preview_system).chain());
    let zone = app.world_mut().spawn((WritingZone, Node::default())).id();
    app.world_mut().spawn(BoardTray);
    let origin = Vec2::new(80.0, LINE_PITCH * 2.0);
    let held = app
        .world_mut()
        .spawn((
            WordTile,
            Node::default(),
            PlacedTile(origin),
            DragFollow {
                owner: PointerId::Mouse,
                button: PointerButton::Primary,
                grab_offset: Vec2::ZERO,
                target: Vec2::ZERO,
                pointer: Vec2::ZERO,
                origin: Some(origin),
                tray_index: 0,
                tray_slots: Vec::new(),
                pushes: Vec::new(),
                last_cell: Some(origin),
                phased: false,
            },
        ))
        .id();
    let neighbor = app
        .world_mut()
        .spawn((
            WordTile,
            Node {
                left: Val::Px(240.0),
                top: Val::Px(LINE_PITCH * 2.0),
                ..default()
            },
            PlacedTile(Vec2::new(160.0, LINE_PITCH * 2.0)),
        ))
        .id();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(app.world().get::<DragFollow>(held).is_none());
    assert_eq!(app.world().get::<ChildOf>(held).unwrap().parent(), zone);
    assert_eq!(app.world().get::<PlacedTile>(held).unwrap().0, origin);
    assert_eq!(
        app.world().get::<Node>(neighbor).unwrap().left,
        Val::Px(160.0)
    );
    assert_eq!(
        app.world().get::<PlacedTile>(neighbor).unwrap().0,
        Vec2::new(160.0, LINE_PITCH * 2.0)
    );
}

#[test]
fn raw_pointer_cancel_restores_tray_and_clears_preview_without_hover() {
    let mut fixture = DragHarness::new();
    let expected = [
        (fixture.held, Vec2::new(160.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[0], Vec2::new(240.0, LINE_PITCH * 2.0)),
        (fixture.neighbors[1], Vec2::new(320.0, LINE_PITCH * 2.0)),
    ];
    let parent = fixture
        .app
        .world()
        .get::<ChildOf>(fixture.held)
        .unwrap()
        .parent();
    fixture.grab(Vec2::new(10.0, 410.0));
    fixture.app.world_mut().trigger(pointer(
        fixture.held,
        Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0),
        Drag {
            button: PointerButton::Primary,
            distance: Vec2::ZERO,
            delta: Vec2::ZERO,
        },
    ));
    fixture.step();
    assert!(
        fixture
            .app
            .world()
            .resource::<PlacementPreview>()
            .0
            .is_some()
    );
    fixture.cancel(PointerId::Mouse);
    fixture.step();
    assert!(
        fixture
            .app
            .world()
            .get::<DragFollow>(fixture.held)
            .is_none()
    );
    assert!(
        fixture
            .app
            .world()
            .resource::<PlacementPreview>()
            .0
            .is_none()
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(fixture.held)
            .unwrap()
            .parent(),
        parent
    );
    assert_eq!(
        fixture
            .app
            .world_mut()
            .query_filtered::<Entity, With<TrayGap>>()
            .iter(fixture.app.world())
            .count(),
        0
    );
    for _ in 0..90 {
        fixture.step();
    }
    assert_eq!(
        fixture
            .app
            .world()
            .get::<Node>(fixture.neighbors[0])
            .unwrap()
            .left,
        Val::Px(160.0)
    );
    fixture.grab(Vec2::new(10.0, 410.0));
    fixture.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
    fixture.assert_placed(&expected);
}

#[test]
fn focus_loss_restores_a_committed_tile() {
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
    fixture.grab(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
    fixture.app.world_mut().write_message(WindowFocused {
        window: Entity::PLACEHOLDER,
        focused: false,
    });
    fixture.step();
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
fn missing_camera_on_release_cancels_instead_of_stranding_drag() {
    let mut fixture = DragHarness::new();
    let parent = fixture
        .app
        .world()
        .get::<ChildOf>(fixture.held)
        .unwrap()
        .parent();
    fixture.grab(Vec2::new(10.0, 410.0));
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
        Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0),
        DragEnd {
            button: PointerButton::Primary,
            distance: Vec2::ZERO,
        },
    ));
    fixture.step();
    assert_eq!(fixture.app.world().resource::<ActiveDrag>().0, None);
    assert!(
        fixture
            .app
            .world()
            .get::<DragFollow>(fixture.held)
            .is_none()
    );
    assert!(
        fixture
            .app
            .world()
            .get::<CancelDrag>(fixture.held)
            .is_none()
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(fixture.held)
            .unwrap()
            .parent(),
        parent
    );
}
