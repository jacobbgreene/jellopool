use super::super::*;
use super::harness::TrayHarness;

fn place(fixture: &mut TrayHarness, tile: Entity, point: Vec2) {
    fixture.grab(tile);
    fixture.release(tile, point);
    for _ in 0..60 {
        fixture.step();
    }
    assert!(fixture.app.world().get::<PlacedTile>(tile).is_some());
}

#[test]
fn right_click_returns_to_latest_tray_slot_after_reordering_and_paper_moves() {
    let mut fixture = TrayHarness::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 1.0, 1.0);
    let tile = fixture.tiles[1];
    let destination = fixture.center(fixture.tiles[3]) - Vec2::X;
    fixture.grab(tile);
    fixture.release(tile, destination);
    let reordered = fixture.order();
    assert_eq!(reordered[2], tile);
    for _ in 0..60 {
        fixture.step();
    }
    place(&mut fixture, tile, Vec2::new(200.0, 150.0));
    place(&mut fixture, tile, Vec2::new(300.0, 250.0));
    fixture.click(tile, PointerButton::Secondary);
    assert_eq!(fixture.order(), reordered);
    assert!(fixture.app.world().get::<PlacedTile>(tile).is_none());
    assert!(fixture.app.world().get::<SnapAnim>(tile).is_none());
    assert!(fixture.app.world().get::<TrayReturn>(tile).is_some());
}

#[test]
fn return_flies_from_displayed_position_then_settles_at_wrapped_slot() {
    for (dpi, ui_scale) in [(1.0, 1.0), (1.5, 0.8), (2.0, 1.25)] {
        let mut fixture = TrayHarness::new(&[80.0, 140.0, 60.0, 110.0, 70.0], dpi, ui_scale);
        let tile = fixture.tiles[3];
        place(&mut fixture, tile, Vec2::new(200.0, 150.0));
        let from = fixture.center(tile);
        fixture.click(tile, PointerButton::Secondary);
        assert_eq!(fixture.center(tile), from);
        fixture.step();
        let target = fixture.target(tile);
        assert!(fixture.center(tile).distance(from) < 8.0);
        assert!(fixture.center(tile).distance(target) > 100.0);
        let child = fixture.app.world().get::<Children>(tile).unwrap()[0];
        assert!(fixture.center(child).distance(fixture.center(tile)) < 70.0);
        for _ in 0..6 {
            fixture.step();
        }
        assert!(fixture.center(tile).distance(from) > 50.0);
        assert!(fixture.center(tile).distance(target) > 20.0);
        assert!(fixture.app.world().get::<TrayReturn>(tile).is_some());
        for _ in 0..8 {
            fixture.step();
        }
        assert!(fixture.app.world().get::<TrayReturn>(tile).is_none());
        assert!(fixture.center(tile).distance(target) < 0.01);
        assert!(fixture.app.world().get::<TileFeel>(tile).is_some());
        for _ in 0..60 {
            fixture.step();
        }
        assert!(fixture.app.world().get::<TileFeel>(tile).is_none());
        assert!(fixture.app.world().get::<GlobalZIndex>(tile).is_none());
        assert_eq!(
            fixture.app.world().get::<UiTransform>(tile).unwrap().scale,
            Vec2::ONE
        );
        assert_eq!(fixture.order(), fixture.tiles);
    }
}

#[test]
fn regrab_interrupts_return_without_jumping_and_can_cancel_into_its_slot() {
    let mut fixture = TrayHarness::new(&[80.0, 140.0, 60.0], 1.5, 1.25);
    let tile = fixture.tiles[1];
    place(&mut fixture, tile, Vec2::new(200.0, 150.0));
    fixture.click(tile, PointerButton::Secondary);
    for _ in 0..8 {
        fixture.step();
    }
    let before = fixture.center(tile);
    fixture.grab(tile);
    fixture.step();
    assert!(fixture.center(tile).distance(before) < 1.0);
    assert!(fixture.app.world().get::<TrayReturn>(tile).is_none());
    assert!(fixture.app.world().get::<GlobalZIndex>(tile).is_none());
    fixture
        .app
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    fixture.step();
    assert_eq!(fixture.order(), fixture.tiles);
    assert!(fixture.gaps().is_empty());
}

#[test]
fn right_click_ignores_tray_tiles_primary_clicks_and_active_drags() {
    let mut fixture = TrayHarness::new(&[80.0, 140.0, 60.0], 1.0, 1.0);
    let tile = fixture.tiles[1];
    fixture.click(tile, PointerButton::Secondary);
    assert!(fixture.app.world().get::<TrayReturn>(tile).is_none());
    place(&mut fixture, tile, Vec2::new(200.0, 150.0));
    fixture.click(tile, PointerButton::Primary);
    assert!(fixture.app.world().get::<PlacedTile>(tile).is_some());
    fixture.grab(fixture.tiles[0]);
    fixture.click(tile, PointerButton::Secondary);
    assert!(fixture.app.world().get::<PlacedTile>(tile).is_some());
    assert!(fixture.app.world().get::<TrayReturn>(tile).is_none());
}

#[test]
fn returns_to_empty_trays_and_clamps_slots_when_other_tiles_have_left() {
    for count in [1, 3] {
        let mut fixture = TrayHarness::new(&vec![80.0; count], 1.0, 1.0);
        let tile = fixture.tiles[count - 1];
        place(&mut fixture, tile, Vec2::new(200.0, 100.0));
        for index in 0..count - 1 {
            let other = fixture.tiles[index];
            place(&mut fixture, other, Vec2::new(200.0, 250.0));
        }
        assert!(fixture.order().is_empty());
        fixture.click(tile, PointerButton::Secondary);
        assert_eq!(fixture.order(), vec![tile]);
        for _ in 0..90 {
            fixture.step();
        }
        let target = fixture.target(tile);
        assert!(fixture.center(tile).distance(target) < 0.01);
    }
}

#[test]
fn older_placed_tiles_without_history_return_to_the_end() {
    let mut fixture = TrayHarness::new(&[80.0, 140.0, 60.0], 1.0, 1.0);
    let tile = fixture.tiles[0];
    place(&mut fixture, tile, Vec2::new(200.0, 150.0));
    fixture
        .app
        .world_mut()
        .entity_mut(tile)
        .remove::<LastTraySlot>();
    fixture.click(tile, PointerButton::Secondary);
    assert_eq!(
        fixture.order(),
        vec![fixture.tiles[1], fixture.tiles[2], tile]
    );
}
