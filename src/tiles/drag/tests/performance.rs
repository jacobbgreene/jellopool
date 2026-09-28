use super::super::*;
use super::harness::{DragHarness, geometry};
use crate::test_support::input::pointer;

#[test]
#[ignore = "opt-in CPU measurement, not a timing assertion"]
fn performance_drag_systems_forty_tiles() {
    use crate::test_support::performance::measure;
    let mut fixture = DragHarness::new();
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
