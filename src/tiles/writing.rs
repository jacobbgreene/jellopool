//! Scrolling and viewport-aware coordinates for the numbered writing sheet.
use super::drag::pointer_in_viewport;
use super::placement::LINE_PITCH;
use crate::prelude::*;
use bevy::input::mouse::MouseScrollUnit;
use bevy::picking::pointer::{PointerAction, PointerInput};

#[derive(Component)]
pub(crate) struct WritingViewport;
#[derive(Component)]
pub(super) struct ScrollTrack;
#[derive(Component)]
pub(super) struct ScrollThumb;
#[derive(Component)]
pub(super) struct LineGuide;

pub(super) type Viewports<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedNode,
        &'static UiGlobalTransform,
        &'static ScrollPosition,
    ),
    With<WritingViewport>,
>;

#[derive(Clone, Copy)]
pub(super) struct WritingGeometry {
    pub bounds: Vec2,
    /// Physical, viewport-relative origin, including any pending scroll.
    pub origin: Vec2,
    pub inverse_scale: f32,
    pub visible: Rect,
}

pub(super) fn geometry(
    node: &ComputedNode,
    transform: &UiGlobalTransform,
    viewport: Option<(&ComputedNode, &UiGlobalTransform, &ScrollPosition)>,
) -> WritingGeometry {
    let mut origin = transform.translation - node.size * 0.5;
    let mut clip = Rect::from_center_size(transform.translation, node.size);
    if let Some((view, view_transform, scroll)) = viewport {
        // Layout resolves scroll in physical whole pixels. Include a change
        // made this Update without waiting for PostUpdate's transforms.
        let requested = scroll.0.y.clamp(0.0, max_scroll(view));
        origin.y -= (requested / view.inverse_scale_factor).floor() - view.scroll_position.y;
        clip = Rect::from_center_size(view_transform.translation, view.size);
    }
    let content = Rect::from_corners(origin, origin + node.size);
    WritingGeometry {
        bounds: node.size * node.inverse_scale_factor,
        origin,
        inverse_scale: node.inverse_scale_factor,
        visible: content.intersect(clip),
    }
}

fn max_scroll(node: &ComputedNode) -> f32 {
    (node.content_size.y - node.size.y).max(0.0) * node.inverse_scale_factor
}

/// Raw input reaches the sheet even while a held tile covers the pointer.
/// No scrolling when the pointer is over the tray or outside the paper.
pub(super) fn scroll_input_system(
    mut input: MessageReader<PointerInput>,
    cameras: Query<&Camera>,
    mut viewport: Single<
        (
            &mut ScrollPosition,
            &ComputedNode,
            &UiGlobalTransform,
            &ComputedUiTargetCamera,
        ),
        With<WritingViewport>,
    >,
) {
    let (scroll, node, transform, target) = &mut *viewport;
    for event in input.read() {
        let PointerAction::Scroll { y, unit, .. } = event.action else {
            continue;
        };
        let Some(pointer) = pointer_in_viewport(event.location.position, target, &cameras) else {
            continue;
        };
        if !node.contains_point(**transform, pointer) || !y.is_finite() {
            continue;
        }
        let amount = match unit {
            MouseScrollUnit::Line => y * LINE_PITCH,
            // Winit passes physical pixel wheel deltas through unchanged;
            // unlike pointer locations these must not be multiplied by DPI.
            MouseScrollUnit::Pixel => y * node.inverse_scale_factor,
        };
        let next = (scroll.0.y - amount).clamp(0.0, max_scroll(node));
        if scroll.0.y != next {
            scroll.0.y = next;
        }
    }
}

fn thumb_metrics(view: &ComputedNode, track: &ComputedNode) -> (f32, f32) {
    let height = track.size.y * track.inverse_scale_factor;
    let thumb =
        (height * view.size.y / view.content_size.y.max(1.0)).clamp(24.0_f32.min(height), height);
    (thumb, height - thumb)
}

pub(super) fn scrollbar_system(
    viewport: Single<(&ComputedNode, &ScrollPosition), With<WritingViewport>>,
    track: Single<&ComputedNode, With<ScrollTrack>>,
    mut thumb: Single<&mut Node, With<ScrollThumb>>,
) {
    let (view, scroll) = *viewport;
    let (height, travel) = thumb_metrics(view, &track);
    let top = travel * scroll.0.y.clamp(0.0, max_scroll(view)) / max_scroll(view).max(1.0);
    if thumb.height != Val::Px(height) {
        thumb.height = Val::Px(height);
    }
    if thumb.top != Val::Px(top) {
        thumb.top = Val::Px(top);
    }
}

pub(super) fn scroll_thumb_drag(
    mut event: On<Pointer<Drag>>,
    mut viewport: Single<(&ComputedNode, &mut ScrollPosition), With<WritingViewport>>,
    track: Single<(&ComputedNode, &ComputedUiTargetCamera), With<ScrollTrack>>,
    cameras: Query<&Camera>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    event.propagate(false);
    let (node, target) = *track;
    let (view, scroll) = &mut *viewport;
    let (_, travel) = thumb_metrics(view, node);
    if travel <= 0.0 {
        return;
    }
    let Some(scale) = target
        .get()
        .and_then(|id| cameras.get(id).ok())
        .and_then(Camera::target_scaling_factor)
    else {
        return;
    };
    let delta = event.delta.y * scale * node.inverse_scale_factor;
    scroll.0.y = (scroll.0.y + delta * max_scroll(view) / travel).clamp(0.0, max_scroll(view));
}

pub(super) fn scroll_track_press(
    mut event: On<Pointer<Press>>,
    mut viewport: Single<(&ComputedNode, &mut ScrollPosition), With<WritingViewport>>,
    track: Single<(&ComputedNode, &UiGlobalTransform, &ComputedUiTargetCamera), With<ScrollTrack>>,
    thumbs: Query<(), With<ScrollThumb>>,
    cameras: Query<&Camera>,
) {
    if event.button != PointerButton::Primary || thumbs.contains(event.entity) {
        return;
    }
    event.propagate(false);
    let (node, transform, target) = *track;
    let Some(pointer) = pointer_in_viewport(event.pointer_location.position, target, &cameras)
    else {
        return;
    };
    let (view, scroll) = &mut *viewport;
    let (height, travel) = thumb_metrics(view, node);
    if travel <= 0.0 {
        return;
    }
    let y = (pointer.y - transform.translation.y + node.size.y * 0.5) * node.inverse_scale_factor;
    scroll.0.y = ((y - height * 0.5) / travel * max_scroll(view)).clamp(0.0, max_scroll(view));
}

#[cfg(test)]
mod tests;
