use super::{super::*, harness::*};

#[test]
fn scene_components_can_be_customized_without_losing_children_or_tile_behavior() {
    let mut app = app();
    let tile = app
        .world_mut()
        .spawn_scene(bsn! {
            crate::tiles::scenes::word_tile("example".into(), default(), default())
            Node { border_radius: BorderRadius::all(px(8)) }
            BackgroundColor(Color::srgb(0.2, 0.3, 0.4))
        })
        .unwrap()
        .id();
    assert!(app.world().get::<WordTile>(tile).is_some());
    let node = app.world().get::<Node>(tile).unwrap();
    assert_eq!(node.padding, UiRect::axes(Val::Px(14.0), Val::Px(7.0)));
    assert_eq!(node.border_radius, BorderRadius::all(Val::Px(8.0)));
    let label = app.world().get::<Children>(tile).unwrap()[0];
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "example");
}

#[test]
fn reopening_a_document_uses_saved_words_and_duplicate_word_ids_not_a_new_selection() {
    let mut document = PoemDocument::new(vec!["the".into(), "the".into(), "café".into()]).unwrap();
    document.title = "Café — 詩".into();
    document.tiles[0].position = Some(PaperPosition { line: 4, x: 120.25 });
    document.tray = vec![TileId(2), TileId(1)];
    let mut app = with_book(Some(DraftBook::new(document.clone())));
    assert_eq!(active(&app), &document);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<WordTile>>()
            .iter(app.world())
            .count(),
        3
    );
    let tray = single::<BoardTray>(&mut app);
    let ids: Vec<_> = app
        .world()
        .get::<Children>(tray)
        .unwrap()
        .iter()
        .filter_map(|entity| app.world().get::<DocumentTile>(entity).map(|id| id.0))
        .collect();
    assert_eq!(ids, document.tray);
    let title = single::<PoemTitle>(&mut app);
    assert_eq!(
        app.world().get::<EditableText>(title).unwrap().value(),
        document.title.as_str()
    );
    assert_eq!(
        app.world_mut()
            .query::<&PlacedTile>()
            .single(app.world())
            .unwrap()
            .0,
        Vec2::new(120.25, 4.0 * LINE_PITCH)
    );
}

#[test]
fn returning_a_restored_tile_captures_its_tray_order_before_animation_finishes() {
    let mut document = PoemDocument::new(vec!["one".into(), "two".into(), "three".into()]).unwrap();
    document.tiles[0].position = Some(PaperPosition { line: 0, x: 120.0 });
    document.tiles[0].last_tray_slot = Some(1);
    document.tray = vec![TileId(2), TileId(1)];
    let mut app = with_book(Some(DraftBook::new(document)));
    let tile = single::<PlacedTile>(&mut app);
    app.world_mut().trigger(crate::test_support::input::pointer(
        tile,
        Vec2::ZERO,
        Click {
            button: PointerButton::Secondary,
            hit: bevy::picking::backend::HitData::new(tile, 0.0, None, None),
            duration: std::time::Duration::from_millis(80),
            count: 1,
        },
    ));
    app.update();
    assert!(
        app.world()
            .get::<crate::tiles::tray::TrayReturn>(tile)
            .is_some()
    );
    assert!(app.world().resource::<CaptureError>().0.is_none());
    assert_eq!(active(&app).tray, vec![TileId(2), TileId(0), TileId(1)]);
    assert_eq!(active(&app).tiles[0].position, None);
    active(&app).validate().unwrap();
}
