use super::super::*;
use super::harness::DragHarness;
use crate::test_support::input::pointer;

fn start(entity: Entity, owner: PointerId, button: PointerButton) -> Pointer<DragStart> {
    let mut event = pointer(
        entity,
        Vec2::new(10.0, 410.0),
        DragStart {
            button,
            hit: bevy::picking::backend::HitData::new(entity, 0.0, None, None),
        },
    );
    event.pointer_id = owner;
    event
}

#[test]
fn secondary_and_middle_buttons_cannot_pick_up_tiles() {
    for button in [PointerButton::Secondary, PointerButton::Middle] {
        let mut f = DragHarness::new();
        let parent = f.app.world().get::<ChildOf>(f.held).unwrap().parent();
        f.app
            .world_mut()
            .trigger(start(f.held, PointerId::Mouse, button));
        f.app.world_mut().flush();
        assert!(f.app.world().get::<DragFollow>(f.held).is_none());
        assert_eq!(
            f.app.world().get::<ChildOf>(f.held).unwrap().parent(),
            parent
        );
        assert_eq!(
            f.app.world().get::<Node>(f.held).unwrap().top,
            Val::Px(400.0)
        );
        assert_eq!(
            f.app
                .world_mut()
                .query_filtered::<Entity, With<TrayGap>>()
                .iter(f.app.world())
                .count(),
            0
        );
        f.grab(Vec2::new(10.0, 410.0));
    }
}

#[test]
fn simultaneous_touch_starts_have_one_owner_even_before_commands_flush() {
    for queued in [true, false] {
        let mut f = DragHarness::new();
        let first = start(f.held, PointerId::Touch(1), PointerButton::Primary);
        let second = start(f.neighbors[0], PointerId::Touch(2), PointerButton::Primary);
        if queued {
            // This is how Bevy picking normally dispatches its events.
            f.app.world_mut().commands().trigger(first);
            f.app.world_mut().commands().trigger(second);
        } else {
            // Direct/nested triggers can share one deferred-command boundary.
            f.app.world_mut().trigger(first);
            f.app.world_mut().trigger(second);
        }
        f.app.world_mut().flush();
        let owners: Vec<_> = f
            .app
            .world_mut()
            .query_filtered::<Entity, With<DragFollow>>()
            .iter(f.app.world())
            .collect();
        assert_eq!(owners, vec![f.held], "queued={queued}");
        assert_eq!(
            f.app.world().get::<DragFollow>(f.held).unwrap().owner,
            PointerId::Touch(1)
        );
        assert_eq!(
            f.app
                .world()
                .get::<ChildOf>(f.neighbors[0])
                .unwrap()
                .parent(),
            f.zone
        );
        f.cancel(PointerId::Touch(2));
        f.step();
        assert_eq!(f.app.world().resource::<ActiveDrag>().0, Some(f.held));
        f.cancel(PointerId::Touch(1));
        f.step();
        assert_eq!(f.app.world().resource::<ActiveDrag>().0, None);
    }
}

#[test]
fn every_drag_finish_path_releases_ownership_for_the_next_tile() {
    for finish in [
        "tray",
        "paper",
        "escape",
        "pointer_cancel",
        "focus_loss",
        "despawn",
    ] {
        let mut f = DragHarness::new();
        f.grab(Vec2::new(10.0, 410.0));
        assert_eq!(f.app.world().resource::<ActiveDrag>().0, Some(f.held));
        match finish {
            "tray" => f.release_at(Vec2::new(10.0, 410.0)),
            "paper" => f.release_at(Vec2::new(170.0, LINE_PITCH * 2.0 + 10.0)),
            "escape" => f.set_key(KeyCode::Escape, true),
            "pointer_cancel" => f.cancel(PointerId::Mouse),
            "focus_loss" => {
                f.app.world_mut().write_message(WindowFocused {
                    window: Entity::PLACEHOLDER,
                    focused: false,
                });
            }
            "despawn" => {
                f.app.world_mut().despawn(f.held);
            }
            _ => unreachable!(),
        }
        f.step();
        f.set_key(KeyCode::Escape, false);
        assert_eq!(f.app.world().resource::<ActiveDrag>().0, None, "{finish}");
        let next = f.neighbors[0];
        f.app
            .world_mut()
            .trigger(start(next, PointerId::Touch(3), PointerButton::Primary));
        f.app.world_mut().flush();
        assert!(f.app.world().get::<DragFollow>(next).is_some(), "{finish}");
        assert_eq!(
            f.app.world().resource::<ActiveDrag>().0,
            Some(next),
            "{finish}"
        );
    }
}
