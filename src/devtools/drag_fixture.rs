//! Opt-in push/phase screenshots exercise the real drag observers.
use crate::prelude::*;
use bevy::camera::RenderTarget;
use bevy::picking::{
    backend::HitData,
    pointer::{Location, PointerId},
};
use bevy::window::PrimaryWindow;

#[derive(Resource)]
pub(super) struct PendingDrag(pub bool);

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_drag(
    mut commands: Commands,
    pending: Res<PendingDrag>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    tiles: Query<(Entity, &Name, &ComputedNode, &UiGlobalTransform), With<WordTile>>,
    zone: Single<(&ComputedNode, &UiGlobalTransform), With<WritingZone>>,
    camera: Single<(Entity, &Camera, &RenderTarget), With<Camera2d>>,
    window: Query<Entity, With<PrimaryWindow>>,
    mut exit: MessageWriter<bevy::app::AppExit>,
    mut stage: Local<u8>,
) {
    // Allow the authored scene and tray layout to settle before measuring grip.
    if *stage < 2 {
        *stage += 1;
        return;
    }
    let Some((entity, _, node, transform)) =
        tiles.iter().find(|(_, name, _, _)| name.as_str() == "just")
    else {
        error!("drag fixture: the selected word bank has no 'just' tile");
        exit.write(bevy::app::AppExit::error());
        commands.remove_resource::<PendingDrag>();
        return;
    };
    let (camera_entity, camera, target) = *camera;
    let Some(target) = target.normalize(window.single().ok()) else {
        return;
    };
    let Some(viewport) = camera.physical_viewport_rect() else {
        return;
    };
    let Some(dpi) = camera.target_scaling_factor() else {
        return;
    };
    let physical = if *stage == 2 {
        transform.translation
    } else {
        zone.1.translation - zone.0.size * 0.5
            + Vec2::new(30.0, 0.0) / zone.0.inverse_scale_factor
            + node.size * 0.5
    };
    let location = Location {
        target,
        position: (physical + viewport.min.as_vec2()) / dpi,
    };
    if *stage == 2 {
        commands.trigger(Pointer::new(
            PointerId::Mouse,
            location,
            DragStart {
                button: PointerButton::Primary,
                hit: HitData::new(camera_entity, 0.0, None, None),
            },
            entity,
        ));
        *stage += 1;
    } else {
        if pending.0 {
            keys.press(KeyCode::ShiftLeft);
        }
        commands.trigger(Pointer::new(
            PointerId::Mouse,
            location,
            Drag {
                button: PointerButton::Primary,
                distance: Vec2::ZERO,
                delta: Vec2::ZERO,
            },
            entity,
        ));
        commands.remove_resource::<PendingDrag>();
    }
}
