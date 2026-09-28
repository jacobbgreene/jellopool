use super::super::*;
use super::harness::TrayHarness;

#[test]
#[ignore = "opt-in CPU measurement, not a timing assertion"]
fn performance_tray_with_real_layout_forty_tiles() {
    use crate::test_support::performance::measure;
    let widths: Vec<_> = (0..40)
        .map(|index| 40.0 + (index % 5) as f32 * 20.0)
        .collect();
    let mut fixture = TrayHarness::new(&widths, 1.0, 1.0);
    fixture
        .app
        .world_mut()
        .get_mut::<Node>(fixture.tray)
        .unwrap()
        .width = Val::Px(1248.0);
    for _ in 0..90 {
        fixture.step();
    }
    measure("40 tray tiles / idle with UI layout", 100, || {
        fixture.step()
    });
    let held = fixture.tiles[3];
    fixture.grab(held);
    let mut tick = 0;
    measure("40 tray tiles / moving gap with UI layout", 100, || {
        fixture.drag_and_step(
            held,
            Vec2::new(
                120.0 + (tick % 30) as f32 * 40.0,
                430.0 + ((tick / 30) % 3) as f32 * 48.0,
            ),
        );
        tick += 1;
    });
}
