use super::super::*;
use super::harness::TrayHarness;

#[test]
fn pickup_preserves_layout_and_slot_changes_stay_stable_at_all_scales() {
    for dpi in [1.0, 1.5, 2.0] {
        for ui_scale in [0.8, 1.0, 1.25] {
            let mut f = TrayHarness::new(&[82.3, 140.6, 66.7, 112.1, 72.9], dpi, ui_scale);
            for &tile in &f.tiles {
                let node = f.app.world().get::<ComputedNode>(tile).unwrap();
                assert!(node.border.min_inset.min_element() > 0.0);
                assert!(node.border.max_inset.min_element() > 0.0);
            }
            let held = f.tiles[1];
            let initial: Vec<_> = f.tiles.iter().map(|&tile| f.center(tile)).collect();
            f.grab(held);
            f.step();
            f.assert_gap(1);
            for (&tile, &center) in f
                .tiles
                .iter()
                .zip(&initial)
                .filter(|(tile, _)| **tile != held)
            {
                assert!(f.center(tile).distance(center) < 0.01);
            }
            let anchors = [initial[0], initial[2], initial[3], initial[4]];
            // Reversals used to accumulate closing gaps and confuse child indices.
            for slot in [4, 0, 2, 4, 1] {
                let point = if slot == 4 {
                    anchors[3] + Vec2::X
                } else {
                    anchors[slot] - Vec2::X
                };
                f.drag_and_step(held, point);
                for _ in 0..40 {
                    f.assert_gap(slot);
                    f.step();
                }
            }
        }
    }
}

#[test]
fn wrapped_neighbors_and_their_text_slide_toward_final_layout() {
    let mut f = TrayHarness::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 1.5, 1.25);
    let held = f.tiles[1];
    let neighbor = f.tiles[2];
    let text = f.app.world().get::<Children>(neighbor).unwrap()[0];
    let start = f.center(neighbor);
    let text_offset = f.center(text) - start;
    let destination = f.center(f.tiles[4]) + Vec2::X;
    f.grab(held);
    f.drag_and_step(held, destination);
    let target = f.target(neighbor);
    assert!(
        (start.y - target.y).abs() > 30.0,
        "the neighbor must change rows"
    );
    let first = f.center(neighbor);
    assert!(first.distance(start) > 0.1);
    assert!(first.distance(target) > 0.1, "must slide, not jump");
    assert!(first.distance(target) < start.distance(target));
    // Layout rounds each child to physical pixels at fractional scales.
    assert!(
        (f.center(text) - first).distance(text_offset) <= 1.0 / (f.dpi * f.ui_scale),
        "text offset: before={text_offset:?}, after={:?}",
        f.center(text) - first
    );
    for _ in 0..60 {
        let before = f.center(neighbor).distance(target);
        f.step();
        assert!(f.center(neighbor).distance(target) <= before + 0.01);
    }
    assert!(f.center(neighbor).distance(target) < 0.01);
    let before_drop = f.center(neighbor);
    f.release(held, destination);
    f.assert_released(held);
    f.step();
    assert!(f.center(neighbor).distance(before_drop) < 0.01);
}

#[test]
fn release_uses_latest_pointer_even_without_a_preview_frame() {
    for preview in [false, true] {
        let mut f = TrayHarness::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 1.5, 1.25);
        let held = f.tiles[1];
        let last = f.center(f.tiles[4]) + Vec2::X;
        let first = f.center(f.tiles[0]) - Vec2::X;
        f.grab(held);
        if preview {
            f.drag_and_step(held, first);
            f.assert_gap(0);
        }
        f.release(held, last);
        f.assert_released(held);
        assert_eq!(
            f.order(),
            vec![f.tiles[0], f.tiles[2], f.tiles[3], f.tiles[4], held]
        );
    }
}

#[test]
fn empty_tray_accepts_last_tile_after_leaving_and_returning() {
    let mut f = TrayHarness::new(&[80.0], 1.0, 1.0);
    let held = f.tiles[0];
    let origin = f.center(held);
    f.grab(held);
    f.drag_and_step(held, Vec2::new(200.0, 200.0));
    assert!(f.gaps().is_empty());
    assert!(f.order().is_empty());
    f.drag_and_step(held, origin);
    f.assert_gap(0);
    f.release(held, origin);
    f.assert_released(held);
    assert_eq!(f.order(), vec![held]);
    f.step();
    // Also return to a tray that has no placeholder or Children component.
    f.grab(held);
    f.drag_and_step(held, Vec2::new(200.0, 200.0));
    f.release(held, origin);
    f.assert_released(held);
    assert_eq!(f.order(), vec![held]);
}

#[test]
fn escape_restores_original_order_and_cleans_up_placeholder() {
    let mut f = TrayHarness::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 1.0, 1.0);
    let held = f.tiles[1];
    let last = f.center(f.tiles[4]) + Vec2::X;
    f.grab(held);
    f.drag_and_step(held, last);
    f.app
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    f.step();
    assert!(f.gaps().is_empty());
    assert_eq!(f.order(), f.tiles);
    assert!(f.app.world().get::<DragFollow>(held).is_none());
}

#[test]
fn regrab_during_slide_keeps_the_visible_grip_point() {
    let mut f = TrayHarness::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 2.0, 1.25);
    let held = f.tiles[1];
    let last = f.center(f.tiles[4]) + Vec2::X;
    f.grab(held);
    f.release(held, last);
    f.assert_released(held);
    f.step();
    let before = f.center(held);
    f.grab(held);
    f.step();
    assert!(
        f.center(held).distance(before) <= 1.0 / (f.dpi * f.ui_scale),
        "grip: before={before:?}, after={:?}",
        f.center(held)
    );
}

#[test]
fn regrab_during_wrapping_preserves_the_source_slot_until_moved() {
    for frames in [1, 3, 8] {
        let mut f = TrayHarness::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 1.5, 0.8);
        let held = f.tiles[1];
        let destination = f.center(f.tiles[4]) + Vec2::X;
        f.grab(held);
        f.release(held, destination);
        for _ in 0..frames {
            f.step();
        }
        let order = f.order();
        // Re-grab each word at its displayed center while other words are
        // crossing rows; child order and displayed geometry need not agree.
        for (index, &tile) in order.iter().enumerate() {
            let point = f.center(tile);
            let targets: Vec<_> = order.iter().map(|&entity| f.target(entity)).collect();
            f.grab(tile);
            let follow = f.app.world().get::<DragFollow>(tile).unwrap();
            for (slot, target) in follow.tray_slots.iter().zip(targets) {
                let anchor = slot.rect.center() / (f.dpi * f.ui_scale);
                assert!(
                    anchor.distance(target) <= 1.0 / (f.dpi * f.ui_scale),
                    "frames={frames}, index={index}, tile={:?}, anchor={anchor:?}, layout={target:?}",
                    slot.entity
                );
            }
            f.step();
            f.assert_gap(index);
            f.release(tile, point);
            f.assert_released(tile);
            assert_eq!(f.order(), order);
            f.step();
        }
    }
}

#[test]
fn moving_after_a_regrab_uses_frozen_row_major_layout_anchors() {
    let mut f = TrayHarness::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 1.5, 0.8);
    let held = f.tiles[1];
    let destination = f.center(f.tiles[4]) + Vec2::X;
    f.grab(held);
    f.release(held, destination);
    f.step();
    let last = f.target(held) + Vec2::X;
    let regrabbed = f.tiles[3];
    f.grab(regrabbed);
    f.drag_and_step(regrabbed, last);
    for _ in 0..30 {
        f.assert_gap(4);
        f.step();
    }
    f.release(regrabbed, last);
    f.assert_released(regrabbed);
    assert_eq!(
        f.order(),
        vec![f.tiles[0], f.tiles[2], f.tiles[4], held, regrabbed]
    );
}

#[test]
fn whitespace_between_rows_uses_the_nearest_row_and_horizontal_slot() {
    let slots: Vec<_> = [(0.0, 0.0), (100.0, 0.0), (0.0, 52.0), (100.0, 52.0)]
        .into_iter()
        .map(|(x, y)| TraySlot {
            entity: Entity::PLACEHOLDER,
            is_held: false,
            rect: Rect::from_corners(Vec2::new(x, y), Vec2::new(x + 80.0, y + 40.0)),
        })
        .collect();
    assert_eq!(insertion_slot(&slots, Vec2::new(0.0, 43.0)), 0);
    assert_eq!(insertion_slot(&slots, Vec2::new(100.0, 43.0)), 1);
    assert_eq!(insertion_slot(&slots, Vec2::new(0.0, 49.0)), 2);
    assert_eq!(insertion_slot(&slots, Vec2::new(100.0, 49.0)), 3);
}

#[test]
fn picking_up_the_only_tile_in_a_row_keeps_its_source_slot() {
    for (widths, held_index) in [([280.0, 80.0, 60.0], 0), ([80.0, 60.0, 280.0], 2)] {
        let mut f = TrayHarness::new(&widths, 1.0, 1.0);
        let held = f.tiles[held_index];
        let before: Vec<_> = f.tiles.iter().map(|&tile| f.center(tile)).collect();
        f.grab(held);
        for _ in 0..30 {
            f.step();
            f.assert_gap(held_index);
            for (&tile, &center) in f
                .tiles
                .iter()
                .zip(&before)
                .filter(|(tile, _)| **tile != held)
            {
                assert!(f.center(tile).distance(center) < 0.01);
            }
        }
    }
}
