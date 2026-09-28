use super::*;

fn node(size: Vec2, content: Vec2, scale: f32) -> ComputedNode {
    ComputedNode {
        size: size * scale,
        content_size: content * scale,
        inverse_scale_factor: 1.0 / scale,
        ..default()
    }
}

fn transform(top_left: Vec2, size: Vec2, scale: f32) -> UiGlobalTransform {
    bevy::math::Affine2::from_translation((top_left + size * 0.5) * scale).into()
}

#[test]
fn geometry_accounts_for_scrolling_and_clips_hidden_lines_at_every_scale() {
    for scale in [0.8, 1.0, 1.5, 2.0] {
        let view_size = Vec2::new(900.0, 500.0);
        let view = node(view_size, Vec2::new(900.0, 1392.0), scale);
        let view_transform = transform(Vec2::new(300.0, 24.0), view_size, scale);
        let content_size = Vec2::new(804.0, 1344.0);
        let content = node(content_size, content_size, scale);
        let content_transform = transform(Vec2::new(364.0, 48.0), content_size, scale);
        let scroll = ScrollPosition(Vec2::new(0.0, LINE_PITCH * 5.0));
        let g = geometry(
            &content,
            &content_transform,
            Some((&view, &view_transform, &scroll)),
        );
        let physical_point = Vec2::new(400.0, 64.0) * scale;
        assert!(g.visible.contains(physical_point));
        let local = (physical_point - g.origin) * g.inverse_scale;
        assert!((local.y - (16.0 + LINE_PITCH * 5.0)).abs() < 0.01);
        assert!(!g.visible.contains(Vec2::new(400.0, 600.0) * scale));
        assert!(!g.visible.contains(Vec2::new(320.0, 100.0) * scale));
        // Pending and already-laid-out scroll have identical coordinates.
        let mut resolved = view;
        resolved.scroll_position.y = (scroll.0.y * scale).floor();
        let moved_transform = transform(
            Vec2::new(364.0, 48.0) - Vec2::new(0.0, resolved.scroll_position.y / scale),
            content_size,
            scale,
        );
        let laid_out = geometry(
            &content,
            &moved_transform,
            Some((&resolved, &view_transform, &scroll)),
        );
        assert!((laid_out.origin - g.origin).length() < 0.01);
    }
}

#[test]
fn scrollbar_metrics_are_bounded_even_before_layout() {
    let view = node(Vec2::new(900.0, 500.0), Vec2::new(900.0, 1000.0), 1.5);
    let track = node(Vec2::new(16.0, 480.0), Vec2::ZERO, 1.5);
    assert_eq!(max_scroll(&view), 500.0);
    assert_eq!(thumb_metrics(&view, &track), (240.0, 240.0));
    assert_eq!(
        thumb_metrics(&ComputedNode::default(), &ComputedNode::default()),
        (0.0, 0.0)
    );
}

#[test]
fn real_layout_keeps_tray_fixed_and_scrolls_only_the_paper() {
    use crate::test_support::input::location;
    use bevy::input::touch::TouchPhase;
    use bevy::picking::pointer::PointerId;
    for dpi in [1.0, 1.5, 2.0] {
        for ui_scale in [0.8, 1.0] {
            let mut app =
                crate::test_support::ui::layout_app(Vec2::new(1600.0, 900.0), dpi, ui_scale);
            app.insert_resource(GameAssets {
                words: default(),
                word_font: default(),
                signature_font: None,
            })
            .add_systems(Startup, crate::tiles::spawn_board_root)
            .add_systems(PreUpdate, crate::tiles::presentation::fit_page_lead);
            crate::tiles::register_interaction_systems(&mut app);
            crate::tiles::register_tray_animation(&mut app);
            let physical_size = (Vec2::new(1600.0, 900.0) * dpi * ui_scale).as_uvec2();
            app.update();
            app.update();
            let view = app
                .world_mut()
                .query_filtered::<Entity, With<WritingViewport>>()
                .single(app.world())
                .unwrap();
            let zone = app
                .world_mut()
                .query_filtered::<Entity, With<WritingZone>>()
                .single(app.world())
                .unwrap();
            let tray = app
                .world_mut()
                .query_filtered::<Entity, With<BoardTray>>()
                .single(app.world())
                .unwrap();
            let tray_before = *app.world().get::<UiGlobalTransform>(tray).unwrap();
            let tray_size = app.world().get::<ComputedNode>(tray).unwrap().size / (dpi * ui_scale);
            assert!(
                (tray_size - Vec2::new(1248.0, 207.0)).length() < 1.0,
                "{tray_size:?}"
            );
            let view_size = app.world().get::<ComputedNode>(view).unwrap().size / (dpi * ui_scale);
            assert!(
                (view_size - Vec2::new(900.0, 581.0)).length() < 1.0,
                "{view_size:?}"
            );
            let start = app
                .world()
                .get::<UiGlobalTransform>(zone)
                .unwrap()
                .translation;
            let zone_size = app.world().get::<ComputedNode>(zone).unwrap().size;
            let first_line_center = start.y - zone_size.y * 0.5 + 20.0 * dpi * ui_scale;
            assert!(
                (first_line_center - physical_size.y as f32 * 0.5).abs() <= 1.0,
                "first center={first_line_center}, screen={physical_size:?}, dpi={dpi}, ui={ui_scale}"
            );
            let title = app
                .world_mut()
                .query_filtered::<Entity, With<crate::tiles::title::PoemTitle>>()
                .single(app.world())
                .unwrap();
            let title_start = app
                .world()
                .get::<UiGlobalTransform>(title)
                .unwrap()
                .translation;
            assert!(title_start.y < first_line_center - 60.0 * dpi * ui_scale);
            let bounds = geometry(
                app.world().get::<ComputedNode>(zone).unwrap(),
                app.world().get::<UiGlobalTransform>(zone).unwrap(),
                Some((
                    app.world().get::<ComputedNode>(view).unwrap(),
                    app.world().get::<UiGlobalTransform>(view).unwrap(),
                    app.world().get::<ScrollPosition>(view).unwrap(),
                )),
            );
            assert!(
                bounds
                    .visible
                    .contains(Vec2::new(start.x, first_line_center))
            );
            assert!(!bounds.visible.contains(title_start));
            let menu = app
                .world_mut()
                .query_filtered::<Entity, With<crate::tiles::presentation::MenuSpace>>()
                .single(app.world())
                .unwrap();
            let menu_start = app
                .world()
                .get::<UiGlobalTransform>(menu)
                .unwrap()
                .translation;
            let mut guides = app
                .world_mut()
                .query_filtered::<&ComputedNode, With<LineGuide>>();
            assert_eq!(
                guides.iter(app.world()).count(),
                crate::tiles::placement::LINE_COUNT
            );
            assert!(
                guides
                    .iter(app.world())
                    .all(|node| node.border.max_inset.y > 0.0)
            );
            let scroll_event = |app: &mut App, point: Vec2, y: f32, unit: MouseScrollUnit| {
                app.world_mut().write_message(PointerInput::new(
                    PointerId::Mouse,
                    location(point * ui_scale),
                    PointerAction::Scroll {
                        x: 0.0,
                        y,
                        unit,
                        phase: TouchPhase::Moved,
                    },
                ));
                app.update();
            };
            scroll_event(
                &mut app,
                Vec2::new(500.0, 300.0),
                -1.0,
                MouseScrollUnit::Line,
            );
            assert_eq!(
                app.world().get::<ScrollPosition>(view).unwrap().0.y,
                LINE_PITCH
            );
            let moved = app
                .world()
                .get::<UiGlobalTransform>(zone)
                .unwrap()
                .translation;
            assert!((start.y - moved.y - (LINE_PITCH * dpi * ui_scale).floor()).abs() < 1.0);
            let title_moved = app
                .world()
                .get::<UiGlobalTransform>(title)
                .unwrap()
                .translation;
            assert!(
                (title_start.y - title_moved.y - (LINE_PITCH * dpi * ui_scale).floor()).abs() < 1.0
            );
            assert_eq!(
                app.world()
                    .get::<UiGlobalTransform>(menu)
                    .unwrap()
                    .translation,
                menu_start
            );
            scroll_event(
                &mut app,
                Vec2::new(500.0, 800.0),
                -4.0,
                MouseScrollUnit::Line,
            );
            assert_eq!(
                app.world().get::<ScrollPosition>(view).unwrap().0.y,
                LINE_PITCH
            );
            scroll_event(
                &mut app,
                Vec2::new(500.0, 300.0),
                -14.0,
                MouseScrollUnit::Pixel,
            );
            assert!(
                (app.world().get::<ScrollPosition>(view).unwrap().0.y
                    - LINE_PITCH
                    - 14.0 / (dpi * ui_scale))
                    .abs()
                    < 0.01
            );
            scroll_event(
                &mut app,
                Vec2::new(500.0, 300.0),
                -10000.0,
                MouseScrollUnit::Line,
            );
            let max = max_scroll(app.world().get::<ComputedNode>(view).unwrap());
            // The entire title lead and all 24 lines contribute to scroll range.
            assert!((max - 1129.0).abs() < 1.0, "{max}");
            assert_eq!(app.world().get::<ScrollPosition>(view).unwrap().0.y, max);
            scroll_event(
                &mut app,
                Vec2::new(500.0, 300.0),
                10000.0,
                MouseScrollUnit::Line,
            );
            assert_eq!(app.world().get::<ScrollPosition>(view).unwrap().0.y, 0.0);
            assert_eq!(
                app.world()
                    .get::<UiGlobalTransform>(tray)
                    .unwrap()
                    .translation,
                tray_before.translation
            );

            let thumb = app
                .world_mut()
                .query_filtered::<Entity, With<ScrollThumb>>()
                .single(app.world())
                .unwrap();
            app.world_mut().trigger(Pointer::new(
                PointerId::Mouse,
                location(Vec2::new(1220.0, 100.0) * ui_scale),
                Drag {
                    button: PointerButton::Primary,
                    distance: Vec2::new(0.0, 20.0),
                    delta: Vec2::new(0.0, 20.0),
                },
                thumb,
            ));
            assert!(app.world().get::<ScrollPosition>(view).unwrap().0.y > 0.0);
            assert!(app.world().get::<ScrollPosition>(view).unwrap().0.y < max);
            let track = app
                .world_mut()
                .query_filtered::<Entity, With<ScrollTrack>>()
                .single(app.world())
                .unwrap();
            let track_node = app.world().get::<ComputedNode>(track).unwrap();
            let track_transform = app.world().get::<UiGlobalTransform>(track).unwrap();
            let bottom =
                (track_transform.translation + Vec2::new(0.0, track_node.size.y * 0.5)) / dpi;
            app.world_mut().trigger(Pointer::new(
                PointerId::Mouse,
                location(bottom),
                Press {
                    button: PointerButton::Primary,
                    hit: bevy::picking::backend::HitData::new(track, 0.0, None, None),
                    count: 1,
                },
                track,
            ));
            assert_eq!(app.world().get::<ScrollPosition>(view).unwrap().0.y, max);
            // Resize a live board: the leading space follows the full screen,
            // without resetting the reader's scroll position.
            app.world_mut().get_mut::<ScrollPosition>(view).unwrap().0.y = LINE_PITCH;
            let resized = (Vec2::new(1920.0, 1080.0) * dpi * ui_scale).as_uvec2();
            {
                let mut cameras = app
                    .world_mut()
                    .query_filtered::<&mut Camera, With<Camera2d>>();
                let mut camera = cameras.single_mut(app.world_mut()).unwrap();
                camera.viewport.as_mut().unwrap().physical_size = resized;
                camera.computed.target_info.as_mut().unwrap().physical_size = resized;
            }
            app.update();
            app.update();
            assert_eq!(
                app.world().get::<ScrollPosition>(view).unwrap().0.y,
                LINE_PITCH
            );
            let node = app.world().get::<ComputedNode>(zone).unwrap();
            let position = app
                .world()
                .get::<UiGlobalTransform>(zone)
                .unwrap()
                .translation;
            let first_line = position.y - node.size.y * 0.5 + 20.0 * dpi * ui_scale;
            assert!(
                (first_line + (LINE_PITCH * dpi * ui_scale).floor() - resized.y as f32 * 0.5).abs()
                    <= 1.0
            );
        }
    }
}
