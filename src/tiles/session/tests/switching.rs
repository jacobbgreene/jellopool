use super::{super::*, harness::*};
use bevy::text::TextEdit;

#[test]
fn new_and_switch_restore_title_placed_words_and_tray_order_without_duplicate_roots() {
    let mut app = app();
    title(&mut app, "First poem");
    let zone = single::<WritingZone>(&mut app);
    let tray = single::<BoardTray>(&mut app);
    let tiles: Vec<_> = app
        .world()
        .get::<Children>(tray)
        .unwrap()
        .iter()
        .filter(|entity| app.world().get::<DocumentTile>(*entity).is_some())
        .collect();
    app.world_mut().entity_mut(tiles[0]).insert((
        ChildOf(zone),
        PlacedTile(Vec2::new(120.25, LINE_PITCH * 3.0)),
    ));
    app.world_mut()
        .entity_mut(tray)
        .insert_children(0, &[tiles[3], tiles[2], tiles[1]]);
    app.update();
    let original = active(&app).clone();
    app.world_mut().resource_mut::<DraftRequest>().action = Some(DraftAction::New);
    app.update();
    app.update();
    assert_ne!(active(&app).id, original.id);
    assert!(active(&app).title.is_empty());
    title(&mut app, "Second poem");
    app.world_mut().resource_mut::<DraftRequest>().action =
        Some(DraftAction::Open(original.id.clone()));
    app.update();
    app.update();
    assert_eq!(active(&app), &original);
    assert_eq!(
        app.world()
            .resource::<DraftSession>()
            .book
            .as_ref()
            .unwrap()
            .drafts
            .len(),
        2
    );
    for marker in ["board_root", "writing_zone", "board_tray", "poem_title"] {
        assert_eq!(
            app.world_mut()
                .query::<&Name>()
                .iter(app.world())
                .filter(|name| name.as_str() == marker)
                .count(),
            1
        );
    }
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<WordTile>>()
            .iter(app.world())
            .count(),
        crate::word_bank::WORD_COUNT
    );
    let placed = app
        .world_mut()
        .query::<&PlacedTile>()
        .single(app.world())
        .unwrap();
    assert_eq!(placed.0, Vec2::new(120.25, LINE_PITCH * 3.0));
    let title = single::<PoemTitle>(&mut app);
    assert_eq!(
        app.world().get::<EditableText>(title).unwrap().value(),
        "First poem"
    );
}

#[test]
fn toolbar_buttons_create_and_switch_drafts_through_the_real_observers() {
    let mut app = app();
    let first = active(&app).id.clone();
    let click = |app: &mut App, label: &str| {
        let entity = app
            .world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, name)| name.as_str() == label)
            .unwrap()
            .0;
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity });
        app.update();
        app.update();
    };
    click(&mut app, "New draft +");
    let second = active(&app).id.clone();
    assert_ne!(first, second);
    click(&mut app, "Previous draft");
    assert_eq!(active(&app).id, first);
    click(&mut app, "Next draft");
    assert_eq!(active(&app).id, second);
}

#[test]
fn switching_mid_push_cancels_the_gesture_and_restores_committed_data() {
    use crate::test_support::input::pointer;
    use bevy::picking::backend::HitData;
    let mut app = app();
    let zone = single::<WritingZone>(&mut app);
    let tray = single::<BoardTray>(&mut app);
    let tiles: Vec<_> = app
        .world()
        .get::<Children>(tray)
        .unwrap()
        .iter()
        .filter(|entity| app.world().get::<DocumentTile>(*entity).is_some())
        .collect();
    let neighbor = tiles[0];
    let held = tiles[1];
    for entity in [held, neighbor] {
        let mut node = app.world_mut().get_mut::<Node>(entity).unwrap();
        node.width = Val::Px(80.0);
        node.height = Val::Px(40.0);
    }
    app.world_mut()
        .entity_mut(neighbor)
        .insert((ChildOf(zone), PlacedTile(Vec2::new(160.0, LINE_PITCH))));
    {
        let mut node = app.world_mut().get_mut::<Node>(neighbor).unwrap();
        node.position_type = PositionType::Absolute;
        node.left = Val::Px(160.0);
        node.top = Val::Px(LINE_PITCH);
    }
    app.update();
    let before = active(&app).clone();
    let grip = app
        .world()
        .get::<UiGlobalTransform>(held)
        .unwrap()
        .translation;
    app.world_mut().trigger(pointer(
        held,
        grip,
        DragStart {
            button: PointerButton::Primary,
            hit: HitData::new(zone, 0.0, None, None),
        },
    ));
    app.world_mut().flush();
    assert!(
        app.world().get::<DragFollow>(held).is_some(),
        "the scene retained its drag observer"
    );
    let origin = app
        .world()
        .get::<UiGlobalTransform>(zone)
        .unwrap()
        .translation
        - app.world().get::<ComputedNode>(zone).unwrap().size * 0.5;
    app.world_mut().trigger(pointer(
        held,
        origin + Vec2::new(195.0, LINE_PITCH + 20.0),
        Drag {
            button: PointerButton::Primary,
            distance: Vec2::ZERO,
            delta: Vec2::ZERO,
        },
    ));
    app.world_mut().flush();
    app.update();
    let follow = app.world().get::<DragFollow>(held).unwrap();
    assert!(
        follow
            .pushes
            .iter()
            .any(|tile| tile.entity == neighbor && tile.pos.x != 160.0)
    );
    assert_eq!(active(&app), &before, "preview pushes are never captured");
    app.world_mut().resource_mut::<DraftRequest>().action = Some(DraftAction::New);
    app.update();
    assert!(app.world().get::<DragFollow>(held).is_none());
    app.update();
    let book = app
        .world()
        .resource::<DraftSession>()
        .book
        .as_ref()
        .unwrap();
    assert_eq!(
        book.drafts
            .iter()
            .find(|draft| draft.id == before.id)
            .unwrap(),
        &before
    );
    assert!(app.world().resource::<PlacementPreview>().0.is_none());
    app.world_mut().resource_mut::<DraftRequest>().action =
        Some(DraftAction::Open(before.id.clone()));
    app.update();
    app.update();
    assert_eq!(active(&app), &before);
    assert_eq!(
        app.world_mut()
            .query::<&PlacedTile>()
            .single(app.world())
            .unwrap()
            .0,
        Vec2::new(160.0, LINE_PITCH)
    );
}

#[test]
fn pending_title_edits_are_captured_before_switching() {
    let mut app = app();
    let title = single::<PoemTitle>(&mut app);
    app.world_mut()
        .get_mut::<EditableText>(title)
        .unwrap()
        .queue_edit(TextEdit::Insert("last keystroke".into()));
    app.world_mut().resource_mut::<DraftRequest>().action = Some(DraftAction::New);
    app.update();
    app.update();
    assert_eq!(
        app.world()
            .resource::<DraftSession>()
            .book
            .as_ref()
            .unwrap()
            .drafts[0]
            .title,
        "last keystroke"
    );
}

#[test]
fn incomplete_workspace_never_overwrites_its_document_or_switches_away_silently() {
    let mut app = app();
    let before = active(&app).clone();
    let tile = app
        .world_mut()
        .query_filtered::<Entity, With<WordTile>>()
        .iter(app.world())
        .next()
        .unwrap();
    app.world_mut().despawn(tile);
    app.world_mut().resource_mut::<DraftRequest>().action = Some(DraftAction::New);
    app.update();
    app.update();
    assert_eq!(active(&app), &before);
    assert!(app.world().resource::<CaptureError>().0.is_some());
    let (warning, node) = app
        .world_mut()
        .query::<(&Name, &Text, &Node)>()
        .iter(app.world())
        .find_map(|(name, text, node)| (name.as_str() == "save_warning").then_some((text, node)))
        .unwrap();
    assert!(warning.0.contains("incomplete"));
    assert_eq!(node.display, Display::Flex);
}
