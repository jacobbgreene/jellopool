//! Two intentionally distinct harnesses: supplied geometry or Bevy's real layout.
//! Neither starts a window, renderer, asset loader, or real-time clock.
use crate::prelude::*;
use bevy::app::{HierarchyPropagatePlugin, PropagateSet};
use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
use bevy::ui::{
    UiSystems, ui_layout_system, ui_surface::UiSurface, update::propagate_ui_target_cameras,
};

fn headless_app(ui_scale: f32) -> App {
    let mut app = App::new();
    app.insert_resource(UiScale(ui_scale))
        .init_resource::<Time>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<bevy::picking::pointer::PointerInput>()
        .add_message::<bevy::window::WindowFocused>()
        .add_plugins(HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(
            PostUpdate,
        ));
    app
}

/// Preserves explicitly supplied ComputedNode/UiGlobalTransform values.
pub(crate) fn synthetic_app(ui_scale: f32) -> App {
    let mut app = headless_app(ui_scale);
    app.add_systems(PreUpdate, propagate_ui_target_cameras);
    app
}

/// Runs actual flex layout, including target propagation and scale conversion.
/// Feature fixtures add their own systems and entities before the first update.
pub(crate) fn layout_app(size: Vec2, dpi: f32, ui_scale: f32) -> App {
    let mut app = headless_app(ui_scale);
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        HierarchyPropagatePlugin::<ComputedUiRenderTargetInfo>::new(PostUpdate),
    ))
    .init_resource::<UiSurface>()
    .init_resource::<bevy::text::FontCx>()
    .add_systems(PostUpdate, propagate_ui_target_cameras)
    .add_systems(PostUpdate, ui_layout_system.in_set(UiSystems::Layout))
    .configure_sets(
        PostUpdate,
        (UiSystems::Layout, UiSystems::PostLayout).chain(),
    )
    .configure_sets(
        PostUpdate,
        PropagateSet::<ComputedUiTargetCamera>::default()
            .after(propagate_ui_target_cameras)
            .before(UiSystems::Layout),
    )
    .configure_sets(
        PostUpdate,
        PropagateSet::<ComputedUiRenderTargetInfo>::default()
            .after(propagate_ui_target_cameras)
            .before(UiSystems::Layout),
    );
    let physical_size = (size * dpi * ui_scale).as_uvec2();
    spawn_camera(
        &mut app,
        dpi,
        physical_size,
        Viewport {
            physical_size,
            ..default()
        },
    );
    app
}

pub(crate) fn spawn_camera(
    app: &mut App,
    dpi: f32,
    physical_size: UVec2,
    viewport: Viewport,
) -> Entity {
    app.world_mut()
        .spawn((
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
                viewport: Some(viewport),
                ..default()
            },
        ))
        .id()
}
