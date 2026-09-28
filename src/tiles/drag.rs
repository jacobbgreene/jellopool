use super::animation::{HELD_SCALE, SnapAnim, TileFeel};
use super::placement::{LINE_PITCH, TileRect, plan};
use super::tray::{TrayGap, TrayMotion, TraySlot, child_index, insertion_slot, spawn_gap};
use super::writing::{Viewports, WritingGeometry, geometry as writing_geometry};
use crate::prelude::*;
use bevy::picking::pointer::{PointerAction, PointerId, PointerInput};
use bevy::window::WindowFocused;

// Query shapes shared (or just too wide to inline) across the drag systems.
/// Tiles with full layout info and an editable `Node`, grabbed in `tile_drag_start`.
type TileGrabQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedNode,
        &'static ComputedUiTargetCamera,
        &'static UiGlobalTransform,
        &'static mut Node,
        Option<&'static PlacedTile>,
    ),
    With<WordTile>,
>;
/// Same layout info but reporting drag state instead of placement, for `tile_drag_end`.
type TileDropQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedNode,
        &'static ComputedUiTargetCamera,
        &'static UiGlobalTransform,
        Option<&'static DragFollow>,
        &'static mut Node,
    ),
    With<WordTile>,
>;
/// All tiles currently snapped into the writing zone, with their sizes.
type PlacedTiles<'w, 's> =
    Query<'w, 's, (Entity, &'static PlacedTile, &'static ComputedNode), With<WordTile>>;
type CancelTiles<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static DragFollow,
        &'static mut Node,
        &'static ComputedNode,
        &'static UiGlobalTransform,
        Option<&'static CancelDrag>,
    ),
>;
/// Present only on the tile currently being dragged. The tile's `Node`
/// offset follows `target` directly every frame.
#[derive(Component)]
pub struct DragFollow {
    owner: PointerId,
    button: PointerButton,
    /// Pointer position minus tile top-left at grab time (logical units),
    /// so the tile keeps the same grip point while following.
    grab_offset: Vec2,
    /// Where the tile's top-left should be right now (logical units).
    target: Vec2,
    /// Latest pointer position in physical viewport pixels, for region hit tests.
    pub(super) pointer: Vec2,
    origin: Option<Vec2>,
    tray_index: usize,
    pub(super) tray_slots: Vec<TraySlot>,
}

/// Both visual consumers read one plan, computed after pointer updates.
/// Release deliberately recomputes from its own event instead of using this cache.
#[derive(Resource, Default)]
pub(super) struct PlacementPreview(pub Option<PreviewPlan>);

pub(super) struct PreviewPlan {
    pub cell: Vec2,
    pub size: Vec2,
    pub moved: Vec<TileRect>,
}

#[derive(Component)]
pub(super) struct CancelDrag;

pub(super) fn update_placement_preview(
    zone: Single<(&ComputedNode, &UiGlobalTransform), With<WritingZone>>,
    viewports: Viewports,
    dragged: Query<(Entity, &DragFollow, &ComputedNode), With<WordTile>>,
    placed: PlacedTiles,
    mut preview: ResMut<PlacementPreview>,
) {
    let Ok((entity, follow, node)) = dragged.single() else {
        if preview.0.is_some() {
            preview.0 = None;
        }
        return;
    };
    let committed: Vec<_> = placed
        .iter()
        .filter(|(id, _, _)| *id != entity)
        .map(|(entity, pos, node)| TileRect {
            entity,
            pos: pos.0,
            size: node.size * node.inverse_scale_factor,
        })
        .collect();
    let size = node.size * node.inverse_scale_factor;
    preview.0 = placement_at(
        entity,
        follow.pointer,
        follow.grab_offset,
        size,
        writing_geometry(zone.0, zone.1, viewports.single().ok()),
        &committed,
    )
    .map(|(cell, moved)| PreviewPlan { cell, size, moved });
}

/// Authoritative zone-relative position, never changed by preview animation.
#[derive(Component)]
pub struct PlacedTile(pub Vec2);

fn placement_at(
    entity: Entity,
    pointer: Vec2,
    grab_offset: Vec2,
    size: Vec2,
    zone: WritingGeometry,
    committed: &[TileRect],
) -> Option<(Vec2, Vec<TileRect>)> {
    if !zone.visible.contains(pointer) {
        return None;
    }
    let inv = zone.inverse_scale;
    let bounds = zone.bounds;
    if size.cmpgt(bounds).any() {
        return None;
    }
    let top_left = zone.origin * inv;
    let cell = snap_to_line(pointer * inv - grab_offset - top_left, bounds - size);
    plan(
        TileRect {
            entity,
            pos: cell,
            size,
        },
        committed,
        bounds,
    )
    .map(|plan| (cell, plan))
}

/// Lines guide vertical placement; x remains under the writer's control.
/// No coarse horizontal snap that could push a word before actual contact.
fn snap_to_line(pos: Vec2, max: Vec2) -> Vec2 {
    Vec2::new(
        pos.x.round().clamp(0.0, max.x),
        ((pos.y / LINE_PITCH).round() * LINE_PITCH)
            .clamp(0.0, (max.y / LINE_PITCH).floor() * LINE_PITCH),
    )
}

/// Picking locations are logical render-target coordinates, but UI geometry is
/// physical and viewport-relative. UiScale belongs only in the later conversion
/// via ComputedNode::inverse_scale_factor, not in this conversion.
pub(super) fn pointer_in_viewport(
    position: Vec2,
    target: &ComputedUiTargetCamera,
    cameras: &Query<&Camera>,
) -> Option<Vec2> {
    let converted = target.get().and_then(|entity| {
        let camera = cameras.get(entity).ok()?;
        let scale = camera.target_scaling_factor()?;
        let viewport = camera.physical_viewport_rect()?;
        Some(position * scale - viewport.min.as_vec2())
    });
    if converted.is_none() {
        // Never guess DPI from the UI scale or treat logical input as physical.
        // Ignore this event until camera geometry is ready; Escape still cancels.
        warn!("Ignoring tile drag event: UI target camera geometry is unavailable");
    }
    converted
}

/// On grab: move the tile into the drag layer without it visibly jumping,
/// and start the float animation (slight grow + shadow fade-in).
#[allow(clippy::too_many_arguments)] // Bevy injects each system param.
pub fn tile_drag_start(
    event: On<Pointer<DragStart>>,
    mut commands: Commands,
    drag_layer: Single<Entity, With<DragLayer>>,
    cameras: Query<&Camera>,
    mut tiles: TileGrabQuery,
    active: Query<Entity, With<DragFollow>>,
    tray: Single<(Entity, Option<&Children>), With<BoardTray>>,
    snapping: Query<Entity, (With<PlacedTile>, With<SnapAnim>)>,
) {
    if !active.is_empty() {
        return;
    }
    let (tray_entity, tray_children) = tray.into_inner();
    let tray_index = tray_children
        .into_iter()
        .flat_map(|children| children.iter())
        .take_while(|child| *child != event.entity)
        .filter(|child| tiles.contains(*child))
        .count();
    let tray_slots = tray_children
        .into_iter()
        .flat_map(|children| children.iter())
        .filter_map(|entity| {
            let (node, _, transform, _, _) = tiles.get(entity).ok()?;
            Some(TraySlot {
                entity,
                rect: Rect::from_center_size(transform.translation, node.size),
                is_held: entity == event.entity,
            })
        })
        .collect();
    let Ok((computed, camera, transform, mut node, placed)) = tiles.get_mut(event.entity) else {
        return;
    };

    let Some(pointer) = pointer_in_viewport(event.pointer_location.position, camera, &cameras)
    else {
        return;
    };

    // Hand any in-flight drop animation to the preview writer.
    for entity in &snapping {
        commands.entity(entity).remove::<SnapAnim>();
    }

    // UiGlobalTransform is the node's center in physical viewport pixels;
    // convert to a logical top-left offset for the drag layer, which
    // covers the whole viewport.
    let inv = computed.inverse_scale_factor;
    let top_left = (transform.translation - computed.size * 0.5) * inv;

    // Reserve the source slot before layout sees the tile leave, so pickup
    // itself cannot collapse the row and move all the insertion anchors.
    if placed.is_none() {
        let index = tray_children
            .and_then(|children| children.iter().position(|child| child == event.entity))
            .unwrap_or(0);
        spawn_gap(
            &mut commands,
            tray_entity,
            index,
            tray_index,
            computed.size * inv,
        );
    }

    node.position_type = PositionType::Absolute;
    node.left = Val::Px(top_left.x);
    node.top = Val::Px(top_left.y);

    let pointer_logical = pointer * inv;

    commands
        .entity(event.entity)
        .insert(ChildOf(*drag_layer))
        .remove::<(SnapAnim, TrayMotion)>()
        .insert(DragFollow {
            owner: event.pointer_id,
            button: event.button,
            grab_offset: pointer_logical - top_left,
            target: top_left,
            pointer,
            origin: placed.map(|placed| placed.0),
            tray_index,
            tray_slots,
        })
        .insert(TileFeel {
            scale: 1.0,
            scale_target: HELD_SCALE,
            shadow: 0.0,
            shadow_target: 1.0,
        });
}

/// While dragging: update where the tile should be. The follow system
/// applies the actual offset directly to this target.
pub fn on_tile_drag(
    event: On<Pointer<Drag>>,
    cameras: Query<&Camera>,
    mut tiles: Query<(&ComputedNode, &ComputedUiTargetCamera, &mut DragFollow), With<WordTile>>,
) {
    let Ok((computed, camera, mut follow)) = tiles.get_mut(event.entity) else {
        return;
    };
    if follow.owner != event.pointer_id || follow.button != event.button {
        return;
    }

    let Some(pointer) = pointer_in_viewport(event.pointer_location.position, camera, &cameras)
    else {
        return;
    };
    // Convert physical viewport pixels to logical UI units (including UiScale).
    let inv = computed.inverse_scale_factor;
    follow.pointer = pointer;
    follow.target = follow.pointer * inv - follow.grab_offset;
}

/// On release: drop into the writing zone if the pointer is inside it,
/// snapping to the previewed line position, otherwise return to the tray —
/// into the gap the tray is currently previewing, if there is one.
#[allow(clippy::too_many_arguments)] // Bevy injects each system param.
pub fn tile_drag_end(
    event: On<Pointer<DragEnd>>,
    cameras: Query<&Camera>,
    mut commands: Commands,
    tray: Single<(Entity, &ComputedNode, &UiGlobalTransform, Option<&Children>), With<BoardTray>>,
    gaps: Query<Entity, With<TrayGap>>,
    zone: Single<(Entity, &ComputedNode, &UiGlobalTransform), With<WritingZone>>,
    viewports: Viewports,
    placed: PlacedTiles,
    mut tiles: TileDropQuery,
) {
    let Ok((computed, camera, transform, follow, mut node)) = tiles.get_mut(event.entity) else {
        return;
    };
    let (zone_entity, zone_node, zone_transform) = zone.into_inner();
    let zone_geometry = writing_geometry(zone_node, zone_transform, viewports.single().ok());
    let (tray_entity, tray_node, tray_transform, tray_children) = tray.into_inner();

    let Some(follow) = follow else {
        return;
    };
    if follow.owner != event.pointer_id || follow.button != event.button {
        return;
    }
    let Some(pointer) = pointer_in_viewport(event.pointer_location.position, camera, &cameras)
    else {
        commands.entity(event.entity).insert(CancelDrag);
        return;
    };
    let committed: Vec<_> = placed
        .iter()
        .filter(|(entity, _, _)| *entity != event.entity)
        .map(|(entity, pos, node)| TileRect {
            entity,
            pos: pos.0,
            size: node.size * node.inverse_scale_factor,
        })
        .collect();
    let placement = placement_at(
        event.entity,
        pointer,
        follow.grab_offset,
        computed.size * computed.inverse_scale_factor,
        zone_geometry,
        &committed,
    );

    for gap in &gaps {
        commands.entity(gap).despawn();
    }

    // Start the settle animation; the shadow fades out via TileFeel.
    commands
        .entity(event.entity)
        .remove::<DragFollow>()
        .insert(TileFeel {
            scale: HELD_SCALE,
            scale_target: 1.0,
            shadow: 1.0,
            shadow_target: 0.0,
        });

    if let Some((to, plan)) = placement {
        // Keep the tile where it appears on screen, expressed as an offset
        // from the writing zone's top-left, then play the snap into the
        // highlighted cell.
        let inv = computed.inverse_scale_factor;
        let tile_top_left = transform.translation - computed.size * 0.5;
        let zone_top_left = zone_geometry.origin;
        let relative = (tile_top_left - zone_top_left) * inv;

        // Clamp so the tile stays fully inside the zone; an oversized tile
        // clamps against a zeroed range rather than an inverted one.
        let max = ((zone_node.size - computed.size) * inv).max(Vec2::ZERO);
        let from = relative.clamp(Vec2::ZERO, max);
        for tile in plan {
            commands
                .entity(tile.entity)
                .remove::<SnapAnim>()
                .insert(PlacedTile(tile.pos));
        }

        node.position_type = PositionType::Absolute;
        node.left = Val::Px(from.x);
        node.top = Val::Px(from.y);

        commands
            .entity(event.entity)
            .insert(ChildOf(zone_entity))
            .insert(PlacedTile(to))
            .insert(SnapAnim { from, to, t: 0.0 });
    } else {
        commands.entity(event.entity).remove::<PlacedTile>();
        // Recompute from the release event, even if no preview frame ran.
        let slot = if tray_node.contains_point(*tray_transform, pointer) {
            insertion_slot(&follow.tray_slots, pointer)
        } else {
            follow.tray_slots.len()
        };
        let index = child_index(tray_children, &gaps, &follow.tray_slots, slot);
        node.position_type = PositionType::Relative;
        node.left = Val::Auto;
        node.top = Val::Auto;
        commands.entity(event.entity).insert(TrayMotion {
            center: pointer * computed.inverse_scale_factor - follow.grab_offset
                + computed.size * computed.inverse_scale_factor * 0.5,
        });
        commands
            .entity(tray_entity)
            .insert_children(index, &[event.entity]);
    }
}

/// Applies a dragged tile's pointer-derived target directly each frame.
pub fn tile_follow_system(mut tiles: Query<(&DragFollow, &mut Node), With<WordTile>>) {
    for (follow, mut node) in &mut tiles {
        node.left = Val::Px(follow.target.x);
        node.top = Val::Px(follow.target.y);
    }
}

/// Restore the original placement on Escape, pointer cancellation, lost focus,
/// or a release whose camera became unavailable. Raw pointer input also covers
/// cancellations with no hovered entity, where no Pointer<Cancel> is emitted.
#[allow(clippy::too_many_arguments)]
pub fn cancel_drag_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut pointer_inputs: MessageReader<PointerInput>,
    mut focus: MessageReader<WindowFocused>,
    mut commands: Commands,
    zone: Single<Entity, With<WritingZone>>,
    tray: Single<(Entity, Option<&Children>), With<BoardTray>>,
    gaps: Query<Entity, With<TrayGap>>,
    mut dragged: CancelTiles,
) {
    let canceled: Vec<_> = pointer_inputs
        .read()
        .filter(|event| matches!(event.action, PointerAction::Cancel))
        .map(|event| event.pointer_id)
        .collect();
    // Consume all messages; don't leave an old focus loss queued after a match.
    let mut lost_focus = false;
    for event in focus.read() {
        lost_focus |= !event.focused;
    }
    let (tray_entity, tray_children) = tray.into_inner();
    for (entity, follow, mut node, computed, transform, requested) in &mut dragged {
        if !keys.just_pressed(KeyCode::Escape)
            && !lost_focus
            && !canceled.contains(&follow.owner)
            && requested.is_none()
        {
            continue;
        }
        for gap in &gaps {
            commands.entity(gap).despawn();
        }
        commands
            .entity(entity)
            .remove::<(DragFollow, SnapAnim, CancelDrag)>()
            .insert(TileFeel {
                scale: HELD_SCALE,
                scale_target: 1.0,
                shadow: 1.0,
                shadow_target: 0.0,
            });
        if let Some(origin) = follow.origin {
            node.left = Val::Px(origin.x);
            node.top = Val::Px(origin.y);
            commands
                .entity(entity)
                .insert((ChildOf(*zone), PlacedTile(origin)));
        } else {
            node.position_type = PositionType::Relative;
            node.left = Val::Auto;
            node.top = Val::Auto;
            let index = child_index(tray_children, &gaps, &follow.tray_slots, follow.tray_index);
            commands.entity(entity).insert(TrayMotion {
                center: transform.translation * computed.inverse_scale_factor,
            });
            commands
                .entity(tray_entity)
                .insert_children(index, &[entity]);
        }
    }
}

#[cfg(test)]
mod tests;
