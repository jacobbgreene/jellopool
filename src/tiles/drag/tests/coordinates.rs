use super::super::*;
use super::harness::DragHarness;
use crate::test_support::input::pointer;
use crate::tiles::animation::px_or_zero;

#[test]
fn scaled_pointer_tracks_without_drift_and_drives_preview_and_release() {
    for dpi in [1.0, 1.5, 2.0] {
        for ui_scale in [0.8, 1.0, 1.25] {
            for origin in [UVec2::ZERO, UVec2::new(120, 60)] {
                let mut fixture = DragHarness::scaled(dpi, ui_scale, origin);
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
                fixture.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
                fixture.assert_placed(&[(fixture.held, Vec2::new(160.0, LINE_PITCH * 2.0))]);
                let mut immediate = DragHarness::scaled(dpi, ui_scale, origin);
                immediate.grab(Vec2::new(10.0, 410.0));
                immediate.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0));
                immediate.assert_placed(&[(immediate.held, Vec2::new(160.0, LINE_PITCH * 2.0))]);
            }
        }
    }
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
                let mut fixture = DragHarness::scaled(dpi, ui_scale, UVec2::new(120, 60));
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
