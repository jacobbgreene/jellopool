//! Input construction only: helpers neither advance frames nor assert outcomes.
use crate::prelude::*;
use bevy::camera::{ManualTextureViewHandle, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};

/// Positions are window-logical coordinates, after the fixture's DPI/UI conversion.
pub(crate) fn location(position: Vec2) -> Location {
    Location {
        target: NormalizedRenderTarget::TextureView(ManualTextureViewHandle(5)),
        position,
    }
}

pub(crate) fn pointer<E: std::fmt::Debug + Clone + Reflect>(
    entity: Entity,
    position: Vec2,
    event: E,
) -> Pointer<E> {
    Pointer::new(PointerId::Mouse, location(position), event, entity)
}

pub(crate) fn cancel(app: &mut App, owner: PointerId) {
    app.world_mut().write_message(PointerInput::new(
        owner,
        location(Vec2::ZERO),
        PointerAction::Cancel,
    ));
}
