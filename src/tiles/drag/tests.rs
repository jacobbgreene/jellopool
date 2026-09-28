use super::*;
use crate::tiles::animation::{
    push_preview_system, px_or_zero, snap_anim_system, tile_feel_system, zone_snap_highlight_system,
};
use crate::tiles::tray_gap_system;

// Headless fixture: real observers, hierarchy commands and the production
// Update chain, with measured UI geometry supplied instead of a renderer.
struct Lifecycle {
    app: App,
    zone: Entity,
    held: Entity,
    neighbors: [Entity; 2],
    dpi: f32,
    ui_scale: f32,
    viewport_origin: Vec2,
}

fn geometry(size: Vec2, top_left: Vec2) -> (ComputedNode, UiGlobalTransform) {
    (
        ComputedNode {
            size,
            inverse_scale_factor: 1.0,
            ..default()
        },
        bevy::math::Affine2::from_translation(top_left + size * 0.5).into(),
    )
}

fn pointer<E: std::fmt::Debug + Clone + Reflect>(
    entity: Entity,
    position: Vec2,
    event: E,
) -> Pointer<E> {
    use bevy::camera::{ManualTextureViewHandle, NormalizedRenderTarget};
    use bevy::picking::pointer::{Location, PointerId};
    Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::TextureView(ManualTextureViewHandle(5)),
            position,
        },
        event,
        entity,
    )
}

impl Lifecycle {
    fn new() -> Self {
        Self::scaled(1.0, 1.0, UVec2::ZERO)
    }

    fn scaled(dpi: f32, ui_scale: f32, viewport_origin: UVec2) -> Self {
        use bevy::app::HierarchyPropagatePlugin;
        use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
        let mut app = App::new();
        app.insert_resource(UiScale(ui_scale))
            .add_plugins(HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(
                PostUpdate,
            ))
            .add_systems(Update, bevy::ui::update::propagate_ui_target_cameras);
        app.world_mut().spawn((
            Camera2d,
            IsDefaultUiCamera,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::splat(4096),
                        scale_factor: dpi,
                    }),
                    ..default()
                },
                viewport: Some(Viewport {
                    physical_position: viewport_origin,
                    physical_size: UVec2::splat(3000),
                    ..default()
                }),
                ..default()
            },
        ));
        app.init_resource::<Time>()
            .init_resource::<PlacementPreview>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<bevy::picking::pointer::PointerInput>()
            .add_message::<bevy::window::WindowFocused>()
            .add_systems(
                Update,
                (
                    cancel_drag_system,
                    tile_follow_system,
                    update_placement_preview,
                    tile_feel_system,
                    tray_gap_system,
                    zone_snap_highlight_system,
                    snap_anim_system,
                    push_preview_system,
                )
                    .chain(),
            );
        let zone = app
            .world_mut()
            .spawn((
                WritingZone,
                Node::default(),
                geometry(Vec2::new(640.0, 320.0), Vec2::ZERO),
            ))
            .id();
        let tray = app
            .world_mut()
            .spawn((
                BoardTray,
                Node::default(),
                geometry(Vec2::new(640.0, 160.0), Vec2::new(0.0, 400.0)),
            ))
            .id();
        // Keep Children present even after the only tray tile is picked up.
        app.world_mut().spawn(ChildOf(tray));
        app.world_mut()
            .spawn((super::super::DragLayer, Node::default()));
        let mut spawn_tile = |pos: Vec2, parent: Entity| {
            app.world_mut()
                .spawn((
                    WordTile,
                    Node {
                        left: Val::Px(pos.x),
                        top: Val::Px(pos.y),
                        ..default()
                    },
                    geometry(Vec2::new(80.0, 40.0), pos),
                    ChildOf(parent),
                ))
                .observe(tile_drag_start)
                .observe(on_tile_drag)
                .observe(tile_drag_end)
                .id()
        };
        let held = spawn_tile(Vec2::new(0.0, 400.0), tray);
        let neighbors = [
            spawn_tile(Vec2::new(160.0, LINE_PITCH * 2.0), zone),
            spawn_tile(Vec2::new(240.0, LINE_PITCH * 2.0), zone),
        ];
        for (entity, x) in neighbors.into_iter().zip([160.0, 240.0]) {
            app.world_mut()
                .entity_mut(entity)
                .insert(PlacedTile(Vec2::new(x, LINE_PITCH * 2.0)));
        }
        let world = app.world_mut();
        let mut geometry = world.query::<(&mut ComputedNode, &mut UiGlobalTransform)>();
        for (mut node, mut transform) in geometry.iter_mut(world) {
            node.size *= dpi * ui_scale;
            node.inverse_scale_factor = 1.0 / (dpi * ui_scale);
            let translation = transform.translation * dpi * ui_scale;
            *transform = bevy::math::Affine2::from_translation(translation).into();
        }
        // Populate the same computed target-camera components as production.
        app.update();
        Self {
            app,
            zone,
            held,
            neighbors,
            dpi,
            ui_scale,
            viewport_origin: viewport_origin.as_vec2(),
        }
    }

    fn location(&self, ui_position: Vec2) -> Vec2 {
        ui_position * self.ui_scale + self.viewport_origin / self.dpi
    }

    fn grab(&mut self, position: Vec2) {
        use bevy::picking::backend::HitData;
        let position = self.location(position);
        self.app.world_mut().trigger(pointer(
            self.held,
            position,
            DragStart {
                button: PointerButton::Primary,
                hit: HitData::new(self.zone, 0.0, None, None),
            },
        ));
        self.app.world_mut().flush();
        assert!(self.app.world().get::<DragFollow>(self.held).is_some());
    }

    fn release(&mut self) {
        self.release_scaled();
        self.assert_committed();
    }

    fn release_scaled(&mut self) {
        let position = self.location(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
        self.app.world_mut().trigger(pointer(
            self.held,
            position,
            DragEnd {
                button: PointerButton::Primary,
                distance: Vec2::ZERO,
            },
        ));
        self.app.world_mut().flush();
        assert!(self.app.world().get::<DragFollow>(self.held).is_none());
        assert_eq!(
            self.app.world().get::<ChildOf>(self.held).unwrap().parent(),
            self.zone
        );
        assert_eq!(
            self.app.world().get::<PlacedTile>(self.held).unwrap().0,
            Vec2::new(160.0, LINE_PITCH * 2.0)
        );
    }

    fn step(&mut self) {
        self.app
            .world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
        self.app.update();
    }

    fn assert_committed(&self) {
        for (entity, x) in [self.held, self.neighbors[0], self.neighbors[1]]
            .into_iter()
            .zip([160.0, 240.0, 320.0])
        {
            assert_eq!(
                self.app.world().get::<PlacedTile>(entity).unwrap().0,
                Vec2::new(x, LINE_PITCH * 2.0)
            );
        }
    }

    fn assert_settled(&mut self) {
        for _ in 0..90 {
            self.step();
            self.assert_committed();
        }
        let mut positions = Vec::new();
        for entity in [self.held, self.neighbors[0], self.neighbors[1]] {
            let world = self.app.world();
            let node = world.get::<Node>(entity).unwrap();
            let pos = Vec2::new(px_or_zero(node.left), px_or_zero(node.top));
            assert_eq!(pos, world.get::<PlacedTile>(entity).unwrap().0);
            assert!(world.get::<SnapAnim>(entity).is_none());
            assert!(world.get::<TileFeel>(entity).is_none());
            positions.push(pos);
        }
        for pair in positions.windows(2) {
            assert!(pair[0].x + 80.0 <= pair[1].x);
        }
    }
}

#[test]
fn scaled_pointer_tracks_without_drift_and_drives_preview_and_release() {
    for dpi in [1.0, 1.5, 2.0] {
        for ui_scale in [0.8, 1.0, 1.25] {
            for origin in [UVec2::ZERO, UVec2::new(120, 60)] {
                let mut fixture = Lifecycle::scaled(dpi, ui_scale, origin);
                let grip = Vec2::new(10.0, 10.0);
                fixture.grab(Vec2::new(0.0, 400.0) + grip);
                for ui_pointer in [
                    Vec2::new(100.0, 420.0),
                    Vec2::new(330.0, 150.0),
                    Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0),
                ] {
                    let position = fixture.location(ui_pointer);
                    fixture.app.world_mut().trigger(pointer(
                        fixture.held,
                        position,
                        Drag {
                            button: PointerButton::Primary,
                            distance: Vec2::ZERO,
                            delta: Vec2::ZERO,
                        },
                    ));
                    fixture.step();
                    let world = fixture.app.world();
                    let follow = world.get::<DragFollow>(fixture.held).unwrap();
                    assert!((follow.grab_offset - grip).length() < 0.001);
                    assert!((follow.pointer - ui_pointer * dpi * ui_scale).length() < 0.001);
                    let node = world.get::<Node>(fixture.held).unwrap();
                    let actual = Vec2::new(px_or_zero(node.left), px_or_zero(node.top));
                    assert!((actual + grip - ui_pointer).length() < 0.001);
                    if ui_pointer.y == 420.0 {
                        let mut gaps = fixture
                            .app
                            .world_mut()
                            .query_filtered::<&Node, With<TrayGap>>();
                        assert!(
                            gaps.iter(fixture.app.world())
                                .any(|gap| (px_or_zero(gap.width) - 80.0).abs() < 0.001)
                        );
                    }
                }
                assert_eq!(
                    fixture
                        .app
                        .world()
                        .resource::<PlacementPreview>()
                        .0
                        .as_ref()
                        .map(|plan| plan.cell),
                    Some(Vec2::new(160.0, LINE_PITCH * 2.0))
                );
                // Release must independently convert its position, not rely on
                // the previous move. This also covers release without any move.
                fixture.release_scaled();
                let mut immediate = Lifecycle::scaled(dpi, ui_scale, origin);
                immediate.grab(Vec2::new(10.0, 410.0));
                immediate.release_scaled();
            }
        }
    }
}

#[test]
fn missing_camera_ignores_grab_without_mutating_tile() {
    let mut fixture = Lifecycle::new();
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
    let mut fixture = Lifecycle::new();
    fixture.grab(Vec2::new(10.0, 410.0));
    // Release is authoritative even before Drag/highlight/preview runs.
    fixture.release();
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
    fixture.assert_committed();
    assert!(fixture.app.world().get::<SnapAnim>(fixture.held).is_some());
    fixture.assert_settled();
}

#[test]
fn rapid_release_after_one_preview_frame_keeps_committed_push() {
    let mut fixture = Lifecycle::new();
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
    fixture.release();
    fixture.step();
    assert!(px_or_zero(fixture.app.world().get::<Node>(neighbor).unwrap().left) > preview_x);
    fixture.assert_settled();
}

#[test]
fn regrab_during_snap_then_escape_preserves_last_commit() {
    let mut fixture = Lifecycle::new();
    fixture.grab(Vec2::new(10.0, 410.0));
    fixture.release();
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
    fixture.assert_settled();
}

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
fn edge_snap_stays_on_a_numbered_line_without_snapping_x() {
    assert_eq!(
        snap_to_line(Vec2::splat(999.0), Vec2::new(123.0, 79.0)),
        Vec2::new(123.0, LINE_PITCH)
    );
}

#[test]
fn scrolled_and_pending_scroll_drops_use_content_coordinates_and_clip_to_paper() {
    use crate::tiles::writing::WritingViewport;
    for dpi in [1.0, 1.5, 2.0] {
        for ui_scale in [0.8, 1.0] {
            for run_preview in [false, true] {
                let mut fixture = Lifecycle::scaled(dpi, ui_scale, UVec2::new(120, 60));
                let scale = dpi * ui_scale;
                let content = Vec2::new(640.0, 24.0 * LINE_PITCH);
                fixture.app.world_mut().entity_mut(fixture.zone).insert((
                    ComputedNode {
                        size: content * scale,
                        inverse_scale_factor: 1.0 / scale,
                        ..default()
                    },
                    UiGlobalTransform::from(bevy::math::Affine2::from_translation(
                        content * scale * 0.5,
                    )),
                ));
                fixture.app.world_mut().spawn((
                    WritingViewport,
                    ScrollPosition(Vec2::new(0.0, 6.0 * LINE_PITCH)),
                    ComputedNode {
                        size: Vec2::new(640.0, 320.0) * scale,
                        content_size: content * scale,
                        inverse_scale_factor: 1.0 / scale,
                        ..default()
                    },
                    UiGlobalTransform::from(bevy::math::Affine2::from_translation(
                        Vec2::new(320.0, 160.0) * scale,
                    )),
                ));
                fixture.grab(Vec2::new(10.0, 410.0));
                let point = fixture.location(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
                if run_preview {
                    fixture.app.world_mut().trigger(pointer(
                        fixture.held,
                        point,
                        Drag {
                            button: PointerButton::Primary,
                            distance: Vec2::ZERO,
                            delta: Vec2::ZERO,
                        },
                    ));
                    fixture.step();
                    assert_eq!(
                        fixture
                            .app
                            .world()
                            .resource::<PlacementPreview>()
                            .0
                            .as_ref()
                            .unwrap()
                            .cell,
                        Vec2::new(160.0, LINE_PITCH * 8.0)
                    );
                }
                fixture.app.world_mut().trigger(pointer(
                    fixture.held,
                    point,
                    DragEnd {
                        button: PointerButton::Primary,
                        distance: Vec2::ZERO,
                    },
                ));
                fixture.app.world_mut().flush();
                assert_eq!(
                    fixture
                        .app
                        .world()
                        .get::<PlacedTile>(fixture.held)
                        .unwrap()
                        .0,
                    Vec2::new(160.0, LINE_PITCH * 8.0)
                );
                // The tall content extends under the tray, but that area is clipped.
                fixture.grab(Vec2::new(170.0, 120.0));
                let outside = fixture.location(Vec2::new(170.0, 450.0));
                fixture.app.world_mut().trigger(pointer(
                    fixture.held,
                    outside,
                    DragEnd {
                        button: PointerButton::Primary,
                        distance: Vec2::ZERO,
                    },
                ));
                fixture.app.world_mut().flush();
                assert!(
                    fixture
                        .app
                        .world()
                        .get::<PlacedTile>(fixture.held)
                        .is_none()
                );
            }
        }
    }
}

fn raw_cancel(fixture: &mut Lifecycle, owner: PointerId) {
    let location = pointer(
        fixture.held,
        Vec2::ZERO,
        DragEnd {
            button: PointerButton::Primary,
            distance: Vec2::ZERO,
        },
    )
    .pointer_location;
    fixture.app.world_mut().write_message(PointerInput::new(
        owner,
        location,
        PointerAction::Cancel,
    ));
    fixture.step();
}

#[test]
fn foreign_pointer_and_button_cannot_move_or_release_an_owned_drag() {
    let mut fixture = Lifecycle::new();
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
    raw_cancel(&mut fixture, PointerId::Touch(7));
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
    fixture.release();
}

#[test]
fn raw_pointer_cancel_restores_tray_and_clears_preview_without_hover() {
    let mut fixture = Lifecycle::new();
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
    raw_cancel(&mut fixture, PointerId::Mouse);
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
    fixture.release();
}

#[test]
fn focus_loss_restores_a_committed_tile() {
    let mut fixture = Lifecycle::new();
    fixture.grab(Vec2::new(10.0, 410.0));
    fixture.release();
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
    fixture.assert_settled();
}

#[test]
fn missing_camera_on_release_cancels_instead_of_stranding_drag() {
    let mut fixture = Lifecycle::new();
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

#[test]
fn settled_frames_do_not_dirty_tile_nodes() {
    let mut fixture = Lifecycle::new();
    fixture.grab(Vec2::new(10.0, 410.0));
    fixture.release();
    fixture.assert_settled();
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

#[test]
#[ignore = "opt-in CPU measurement, not a timing assertion"]
fn performance_drag_systems_forty_tiles() {
    use crate::performance::measure;
    let mut fixture = Lifecycle::new();
    fixture
        .app
        .world_mut()
        .entity_mut(fixture.zone)
        .insert(geometry(Vec2::new(1248.0, 693.0), Vec2::ZERO));
    for index in 0..37 {
        let pos = Vec2::new(
            (index % 12) as f32 * 80.0,
            LINE_PITCH * 3.0 + (index / 12) as f32 * LINE_PITCH,
        );
        fixture.app.world_mut().spawn((
            WordTile,
            PlacedTile(pos),
            Node {
                left: Val::Px(pos.x),
                top: Val::Px(pos.y),
                ..default()
            },
            geometry(Vec2::new(80.0, 40.0), pos),
            ChildOf(fixture.zone),
        ));
    }
    measure("40 tiles / idle interaction schedule", 100, || {
        fixture.step()
    });
    fixture.grab(Vec2::new(10.0, 410.0));
    fixture.app.world_mut().trigger(pointer(
        fixture.held,
        Vec2::new(170.0, LINE_PITCH * 3.0 + 10.0),
        Drag {
            button: PointerButton::Primary,
            distance: Vec2::ZERO,
            delta: Vec2::ZERO,
        },
    ));
    measure("40 tiles / stationary held preview", 100, || fixture.step());
    let mut tick = 0;
    measure("40 tiles / moving held preview + observer", 100, || {
        let pos = Vec2::new(
            (tick % 12) as f32 * 80.0 + 10.0,
            LINE_PITCH * 3.0 + 10.0 + ((tick / 12) % 3) as f32 * LINE_PITCH,
        );
        fixture.app.world_mut().trigger(pointer(
            fixture.held,
            pos,
            Drag {
                button: PointerButton::Primary,
                distance: Vec2::ZERO,
                delta: Vec2::ZERO,
            },
        ));
        fixture.step();
        tick += 1;
    });
}
