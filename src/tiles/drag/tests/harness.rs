use super::super::*;
use crate::test_support::{input::pointer, ui};
use crate::tiles::animation::px_or_zero;

// Headless fixture: real observers, hierarchy commands and the production
// Update chain, with measured UI geometry supplied instead of a renderer.
pub(super) struct DragHarness {
    pub(super) app: App,
    pub(super) zone: Entity,
    pub(super) held: Entity,
    pub(super) neighbors: [Entity; 2],
    dpi: f32,
    ui_scale: f32,
    viewport_origin: Vec2,
}

pub(super) fn geometry(size: Vec2, top_left: Vec2) -> (ComputedNode, UiGlobalTransform) {
    (
        ComputedNode {
            size,
            inverse_scale_factor: 1.0,
            ..default()
        },
        bevy::math::Affine2::from_translation(top_left + size * 0.5).into(),
    )
}

impl DragHarness {
    pub(super) fn new() -> Self {
        Self::scaled(1.0, 1.0, UVec2::ZERO)
    }

    pub(super) fn scaled(dpi: f32, ui_scale: f32, viewport_origin: UVec2) -> Self {
        let mut app = ui::synthetic_app(ui_scale);
        ui::spawn_camera(
            &mut app,
            dpi,
            UVec2::splat(4096),
            bevy::camera::Viewport {
                physical_position: viewport_origin,
                physical_size: UVec2::splat(3000),
                ..default()
            },
        );
        crate::tiles::register_interaction_systems(&mut app);
        let zone = app
            .world_mut()
            .spawn((
                WritingZone,
                Node::default(),
                geometry(Vec2::new(640.0, 320.0), Vec2::ZERO),
            ))
            .id();
        let tray = app
            .world_mut()
            .spawn((
                BoardTray,
                Node::default(),
                geometry(Vec2::new(640.0, 160.0), Vec2::new(0.0, 400.0)),
            ))
            .id();
        // Keep Children present even after the only tray tile is picked up.
        app.world_mut().spawn(ChildOf(tray));
        app.world_mut().spawn((DragLayer, Node::default()));
        let mut spawn_tile = |pos: Vec2, parent: Entity| {
            app.world_mut()
                .spawn((
                    WordTile,
                    Node {
                        left: Val::Px(pos.x),
                        top: Val::Px(pos.y),
                        ..default()
                    },
                    geometry(Vec2::new(80.0, 40.0), pos),
                    ChildOf(parent),
                ))
                .observe(tile_drag_start)
                .observe(on_tile_drag)
                .observe(tile_drag_end)
                .id()
        };
        let held = spawn_tile(Vec2::new(0.0, 400.0), tray);
        let neighbors = [
            spawn_tile(Vec2::new(160.0, LINE_PITCH * 2.0), zone),
            spawn_tile(Vec2::new(240.0, LINE_PITCH * 2.0), zone),
        ];
        for (entity, x) in neighbors.into_iter().zip([160.0, 240.0]) {
            app.world_mut()
                .entity_mut(entity)
                .insert(PlacedTile(Vec2::new(x, LINE_PITCH * 2.0)));
        }
        let world = app.world_mut();
        let mut geometry = world.query::<(&mut ComputedNode, &mut UiGlobalTransform)>();
        for (mut node, mut transform) in geometry.iter_mut(world) {
            node.size *= dpi * ui_scale;
            node.inverse_scale_factor = 1.0 / (dpi * ui_scale);
            let translation = transform.translation * dpi * ui_scale;
            *transform = bevy::math::Affine2::from_translation(translation).into();
        }
        // Populate the same computed target-camera components as production.
        app.update();
        Self {
            app,
            zone,
            held,
            neighbors,
            dpi,
            ui_scale,
            viewport_origin: viewport_origin.as_vec2(),
        }
    }

    pub(super) fn location(&self, ui_position: Vec2) -> Vec2 {
        ui_position * self.ui_scale + self.viewport_origin / self.dpi
    }

    pub(super) fn grab(&mut self, position: Vec2) {
        use bevy::picking::backend::HitData;
        let position = self.location(position);
        self.app.world_mut().trigger(pointer(
            self.held,
            position,
            DragStart {
                button: PointerButton::Primary,
                hit: HitData::new(self.zone, 0.0, None, None),
            },
        ));
        self.app.world_mut().flush();
        assert!(self.app.world().get::<DragFollow>(self.held).is_some());
    }

    /// Dispatch and flush only: rapid-release tests must not get an implicit frame.
    pub(super) fn release_at(&mut self, point: Vec2) {
        let position = self.location(point);
        self.app.world_mut().trigger(pointer(
            self.held,
            position,
            DragEnd {
                button: PointerButton::Primary,
                distance: Vec2::ZERO,
            },
        ));
        self.app.world_mut().flush();
    }

    pub(super) fn step(&mut self) {
        crate::test_support::step(&mut self.app);
    }

    /// Place the held tile's top-left, preserving its actual grip offset.
    pub(super) fn drag_to(&mut self, top_left: Vec2) {
        let grip = self
            .app
            .world()
            .get::<DragFollow>(self.held)
            .unwrap()
            .grab_offset;
        let position = self.location(top_left + grip);
        self.app.world_mut().trigger(pointer(
            self.held,
            position,
            Drag {
                button: PointerButton::Primary,
                distance: Vec2::ZERO,
                delta: Vec2::ZERO,
            },
        ));
        self.app.world_mut().flush();
    }

    pub(super) fn set_key(&mut self, code: KeyCode, down: bool) {
        let mut keys = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        if down {
            keys.press(code);
        } else {
            keys.release(code);
        }
    }

    pub(super) fn cancel(&mut self, owner: PointerId) {
        crate::test_support::input::cancel(&mut self.app, owner);
    }

    pub(super) fn pushed_x(&self, index: usize) -> f32 {
        self.app
            .world()
            .get::<DragFollow>(self.held)
            .unwrap()
            .pushes
            .iter()
            .find(|tile| tile.entity == self.neighbors[index])
            .unwrap()
            .pos
            .x
    }

    pub(super) fn assert_placed(&self, expected: &[(Entity, Vec2)]) {
        assert!(self.app.world().get::<DragFollow>(self.held).is_none());
        for &(entity, position) in expected {
            assert_eq!(
                self.app.world().get::<ChildOf>(entity).unwrap().parent(),
                self.zone
            );
            assert_eq!(
                self.app.world().get::<PlacedTile>(entity).unwrap().0,
                position
            );
        }
    }

    /// Check that committed positions never drift while presentation settles.
    pub(super) fn assert_settles_to(&mut self, expected: &[(Entity, Vec2)]) {
        for _ in 0..90 {
            self.step();
            self.assert_placed(expected);
        }
        let world = self.app.world();
        let mut rects = Vec::new();
        for &(entity, expected_position) in expected {
            let node = world.get::<Node>(entity).unwrap();
            let pos = Vec2::new(px_or_zero(node.left), px_or_zero(node.top));
            assert_eq!(pos, expected_position);
            assert!(world.get::<SnapAnim>(entity).is_none());
            assert!(world.get::<TileFeel>(entity).is_none());
            let measured = world.get::<ComputedNode>(entity).unwrap();
            rects.push(Rect::from_corners(
                pos,
                pos + measured.size * measured.inverse_scale_factor,
            ));
        }
        for (index, a) in rects.iter().enumerate() {
            for b in &rects[index + 1..] {
                let intersection = a.intersect(*b);
                assert!(intersection.width() <= 0.0 || intersection.height() <= 0.0);
            }
        }
    }
}
