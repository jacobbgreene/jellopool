use super::super::*;
use crate::test_support::input::pointer;
use crate::tiles::drag::{on_tile_drag, tile_drag_end, tile_drag_start};
use bevy::ui::ui_surface::UiSurface;

// Exercise the production observers and real wrapping layout without a GPU.
pub(super) struct TrayHarness {
    pub(super) app: App,
    pub(super) tray: Entity,
    pub(super) tiles: Vec<Entity>,
    pub(super) dpi: f32,
    pub(super) ui_scale: f32,
}

impl TrayHarness {
    pub(super) fn new(widths: &[f32], dpi: f32, ui_scale: f32) -> Self {
        let mut app = crate::test_support::ui::layout_app(Vec2::new(800.0, 600.0), dpi, ui_scale);
        crate::tiles::register_interaction_systems(&mut app);
        crate::tiles::register_tray_animation(&mut app);
        app.world_mut().spawn((
            WritingZone,
            Node {
                width: Val::Px(700.0),
                height: Val::Px(350.0),
                ..default()
            },
        ));
        app.world_mut()
            .spawn(crate::tiles::presentation::get_drag_layer());
        let mut tray_node = crate::tiles::presentation::get_board_tray().3;
        tray_node.position_type = PositionType::Absolute;
        tray_node.left = Val::Px(100.0);
        tray_node.top = Val::Px(400.0);
        tray_node.width = Val::Px(320.0);
        tray_node.height = Val::Px(180.0);
        let tray = app.world_mut().spawn((BoardTray, tray_node)).id();
        let tiles = widths
            .iter()
            .map(|&width| {
                app.world_mut()
                    .spawn((
                        WordTile,
                        LayoutConfig {
                            use_rounding: false,
                        },
                        Node {
                            width: Val::Px(width),
                            height: Val::Px(40.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        ChildOf(tray),
                    ))
                    // Stand-in for the text subtree: it must slide with the tile.
                    .with_child((
                        LayoutConfig::default(),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ))
                    .observe(tile_drag_start)
                    .observe(on_tile_drag)
                    .observe(tile_drag_end)
                    .observe(return_tile_to_tray)
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

    pub(super) fn step(&mut self) {
        crate::test_support::step(&mut self.app);
    }

    pub(super) fn center(&self, entity: Entity) -> Vec2 {
        self.app
            .world()
            .get::<UiGlobalTransform>(entity)
            .unwrap()
            .translation
            / (self.dpi * self.ui_scale)
    }

    pub(super) fn target(&mut self, entity: Entity) -> Vec2 {
        let tray_size = self
            .app
            .world()
            .get::<ComputedNode>(self.tray)
            .unwrap()
            .size;
        let origin = self.center(self.tray) - tray_size / (self.dpi * self.ui_scale) * 0.5;
        let mut surface = self.app.world_mut().resource_mut::<UiSurface>();
        let (layout, _) = surface.get_layout(entity, false).unwrap();
        origin
            + Vec2::new(
                layout.location.x + layout.size.width * 0.5,
                layout.location.y + layout.size.height * 0.5,
            ) / (self.dpi * self.ui_scale)
    }

    pub(super) fn trigger<E: std::fmt::Debug + Clone + Reflect>(
        &mut self,
        entity: Entity,
        point: Vec2,
        event: E,
    ) {
        self.app
            .world_mut()
            .trigger(pointer(entity, point * self.ui_scale, event));
        self.app.world_mut().flush();
    }

    pub(super) fn grab(&mut self, entity: Entity) {
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

    pub(super) fn drag_and_step(&mut self, entity: Entity, point: Vec2) {
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

    pub(super) fn release(&mut self, entity: Entity, point: Vec2) {
        self.trigger(
            entity,
            point,
            DragEnd {
                button: PointerButton::Primary,
                distance: Vec2::ZERO,
            },
        );
    }

    pub(super) fn click(&mut self, entity: Entity, button: PointerButton) {
        self.trigger(
            entity,
            self.center(entity),
            Click {
                button,
                hit: bevy::picking::backend::HitData::new(entity, 0.0, None, None),
                duration: std::time::Duration::from_millis(80),
                count: 1,
            },
        );
    }

    pub(super) fn assert_released(&mut self, entity: Entity) {
        assert!(self.app.world().get::<DragFollow>(entity).is_none());
        assert!(self.gaps().is_empty());
    }

    pub(super) fn order(&self) -> Vec<Entity> {
        self.app
            .world()
            .get::<Children>(self.tray)
            .into_iter()
            .flat_map(|children| children.iter())
            .filter(|child| self.app.world().get::<WordTile>(*child).is_some())
            .collect()
    }

    pub(super) fn gaps(&mut self) -> Vec<Entity> {
        self.app
            .world_mut()
            .query_filtered::<Entity, With<TrayGap>>()
            .iter(self.app.world())
            .collect()
    }

    pub(super) fn assert_gap(&mut self, index: usize) {
        let gaps = self.gaps();
        assert_eq!(gaps.len(), 1);
        let children = self.app.world().get::<Children>(self.tray).unwrap();
        assert_eq!(
            children.iter().position(|entity| entity == gaps[0]),
            Some(index)
        );
    }
}
