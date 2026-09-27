use super::*;
use crate::tiles::drag::{
    cancel_drag_system, on_tile_drag, tile_drag_end, tile_drag_start, tile_feel_system,
    tile_follow_system,
};
use bevy::app::{HierarchyPropagatePlugin, PropagateSet};
use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
use bevy::ui::{ui_layout_system, ui_surface::UiSurface, update::propagate_ui_target_cameras};

// Exercise the production observers and real wrapping layout without a GPU.
struct TrayFixture {
    app: App,
    tray: Entity,
    tiles: Vec<Entity>,
    dpi: f32,
    ui_scale: f32,
}

impl TrayFixture {
    fn new(widths: &[f32], dpi: f32, ui_scale: f32) -> Self {
        let mut app = App::new();
        app.add_plugins((
            bevy::app::TaskPoolPlugin::default(),
            HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate),
            HierarchyPropagatePlugin::<ComputedUiRenderTargetInfo>::new(PostUpdate),
        ))
        .init_resource::<Time>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(UiScale(ui_scale))
        .init_resource::<UiSurface>()
        .init_resource::<bevy::text::FontCx>()
        .add_systems(
            Update,
            (
                cancel_drag_system,
                tile_follow_system,
                tile_feel_system,
                tray_gap_system,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (
                propagate_ui_target_cameras,
                ui_layout_system,
                tray_slide_system,
            )
                .chain(),
        )
        .configure_sets(
            PostUpdate,
            PropagateSet::<ComputedUiTargetCamera>::default()
                .after(propagate_ui_target_cameras)
                .before(ui_layout_system),
        )
        .configure_sets(
            PostUpdate,
            PropagateSet::<ComputedUiRenderTargetInfo>::default()
                .after(propagate_ui_target_cameras)
                .before(ui_layout_system),
        );
        let physical_size = (Vec2::new(800.0, 600.0) * dpi * ui_scale).as_uvec2();
        app.world_mut().spawn((
            Camera2d,
            IsDefaultUiCamera,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size,
                        scale_factor: dpi,
                    }),
                    ..default()
                },
                viewport: Some(Viewport {
                    physical_size,
                    ..default()
                }),
                ..default()
            },
        ));
        app.world_mut().spawn((
            WritingZone,
            Node {
                width: Val::Px(700.0),
                height: Val::Px(350.0),
                ..default()
            },
        ));
        app.world_mut().spawn(super::super::get_drag_layer());
        let mut tray_node = super::super::get_board_tray().3;
        tray_node.position_type = PositionType::Absolute;
        tray_node.left = Val::Px(100.0);
        tray_node.top = Val::Px(400.0);
        tray_node.width = Val::Px(320.0);
        tray_node.height = Val::Px(180.0);
        let tray = app.world_mut().spawn((BoardTray, tray_node)).id();
        let tiles = widths
            .iter()
            .enumerate()
            .map(|(index, &width)| {
                app.world_mut()
                    .spawn((
                        WordTile {
                            unique_word: index.to_string(),
                        },
                        Node {
                            width: Val::Px(width),
                            height: Val::Px(40.0),
                            ..default()
                        },
                        ChildOf(tray),
                    ))
                    // Stand-in for the text subtree: it must slide with the tile.
                    .with_child(Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        ..default()
                    })
                    .observe(tile_drag_start)
                    .observe(on_tile_drag)
                    .observe(tile_drag_end)
                    .id()
            })
            .collect();
        let mut fixture = Self {
            app,
            tray,
            tiles,
            dpi,
            ui_scale,
        };
        fixture.step();
        fixture.step();
        fixture
    }

    fn step(&mut self) {
        self.app
            .world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
        self.app.update();
    }

    fn center(&self, entity: Entity) -> Vec2 {
        self.app
            .world()
            .get::<UiGlobalTransform>(entity)
            .unwrap()
            .translation
            / (self.dpi * self.ui_scale)
    }

    fn target(&mut self, entity: Entity) -> Vec2 {
        let tray_size = self
            .app
            .world()
            .get::<ComputedNode>(self.tray)
            .unwrap()
            .size;
        let origin = self.center(self.tray) - tray_size / (self.dpi * self.ui_scale) * 0.5;
        let mut surface = self.app.world_mut().resource_mut::<UiSurface>();
        let (layout, _) = surface.get_layout(entity, true).unwrap();
        origin
            + Vec2::new(
                layout.location.x + layout.size.width * 0.5,
                layout.location.y + layout.size.height * 0.5,
            ) / (self.dpi * self.ui_scale)
    }

    fn trigger<E: std::fmt::Debug + Clone + Reflect>(
        &mut self,
        entity: Entity,
        point: Vec2,
        event: E,
    ) {
        use bevy::camera::{ManualTextureViewHandle, NormalizedRenderTarget};
        use bevy::picking::pointer::{Location, PointerId};
        self.app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::TextureView(ManualTextureViewHandle(5)),
                position: point * self.ui_scale,
            },
            event,
            entity,
        ));
        self.app.world_mut().flush();
    }

    fn grab(&mut self, entity: Entity) {
        self.trigger(
            entity,
            self.center(entity),
            DragStart {
                button: PointerButton::Primary,
                hit: bevy::picking::backend::HitData::new(entity, 0.0, None, None),
            },
        );
        assert!(self.app.world().get::<DragFollow>(entity).is_some());
    }

    fn drag(&mut self, entity: Entity, point: Vec2) {
        self.trigger(
            entity,
            point,
            Drag {
                button: PointerButton::Primary,
                distance: Vec2::ZERO,
                delta: Vec2::ZERO,
            },
        );
        self.step();
    }

    fn release(&mut self, entity: Entity, point: Vec2) {
        self.trigger(
            entity,
            point,
            DragEnd {
                button: PointerButton::Primary,
                distance: Vec2::ZERO,
            },
        );
        assert!(self.app.world().get::<DragFollow>(entity).is_none());
        assert_eq!(self.gaps().len(), 0);
    }

    fn order(&self) -> Vec<Entity> {
        self.app
            .world()
            .get::<Children>(self.tray)
            .into_iter()
            .flat_map(|children| children.iter())
            .filter(|child| self.app.world().get::<WordTile>(*child).is_some())
            .collect()
    }

    fn gaps(&mut self) -> Vec<Entity> {
        self.app
            .world_mut()
            .query_filtered::<Entity, With<TrayGap>>()
            .iter(self.app.world())
            .collect()
    }

    fn assert_gap(&mut self, index: usize) {
        let gaps = self.gaps();
        assert_eq!(gaps.len(), 1);
        let children = self.app.world().get::<Children>(self.tray).unwrap();
        assert_eq!(
            children.iter().position(|entity| entity == gaps[0]),
            Some(index)
        );
    }
}

#[test]
fn pickup_preserves_layout_and_slot_changes_stay_stable_at_all_scales() {
    for dpi in [1.0, 1.5, 2.0] {
        for ui_scale in [1.0, 1.25] {
            let mut f = TrayFixture::new(&[80.0, 140.0, 60.0, 110.0, 70.0], dpi, ui_scale);
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
                f.drag(held, point);
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
    let mut f = TrayFixture::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 1.5, 1.25);
    let held = f.tiles[1];
    let neighbor = f.tiles[2];
    let text = f.app.world().get::<Children>(neighbor).unwrap()[0];
    let start = f.center(neighbor);
    let text_offset = f.center(text) - start;
    let destination = f.center(f.tiles[4]) + Vec2::X;
    f.grab(held);
    f.drag(held, destination);
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
    f.step();
    assert!(f.center(neighbor).distance(before_drop) < 0.01);
}

#[test]
fn release_uses_latest_pointer_even_without_a_preview_frame() {
    for preview in [false, true] {
        let mut f = TrayFixture::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 1.5, 1.25);
        let held = f.tiles[1];
        let last = f.center(f.tiles[4]) + Vec2::X;
        let first = f.center(f.tiles[0]) - Vec2::X;
        f.grab(held);
        if preview {
            f.drag(held, first);
            f.assert_gap(0);
        }
        f.release(held, last);
        assert_eq!(
            f.order(),
            vec![f.tiles[0], f.tiles[2], f.tiles[3], f.tiles[4], held]
        );
    }
}

#[test]
fn empty_tray_accepts_last_tile_after_leaving_and_returning() {
    let mut f = TrayFixture::new(&[80.0], 1.0, 1.0);
    let held = f.tiles[0];
    let origin = f.center(held);
    f.grab(held);
    f.drag(held, Vec2::new(200.0, 200.0));
    assert!(f.gaps().is_empty());
    assert!(f.order().is_empty());
    f.drag(held, origin);
    f.assert_gap(0);
    f.release(held, origin);
    assert_eq!(f.order(), vec![held]);
    f.step();
    // Also return to a tray that has no placeholder or Children component.
    f.grab(held);
    f.drag(held, Vec2::new(200.0, 200.0));
    f.release(held, origin);
    assert_eq!(f.order(), vec![held]);
}

#[test]
fn escape_restores_original_order_and_cleans_up_placeholder() {
    let mut f = TrayFixture::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 1.0, 1.0);
    let held = f.tiles[1];
    let last = f.center(f.tiles[4]) + Vec2::X;
    f.grab(held);
    f.drag(held, last);
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
    let mut f = TrayFixture::new(&[80.0, 140.0, 60.0, 110.0, 70.0], 2.0, 1.25);
    let held = f.tiles[1];
    let last = f.center(f.tiles[4]) + Vec2::X;
    f.grab(held);
    f.release(held, last);
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
        let mut f = TrayFixture::new(&widths, 1.0, 1.0);
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
