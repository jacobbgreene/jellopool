use super::animation::{HELD_SCALE, SnapAnim, TileFeel};
use super::placement::{LINE_PITCH, TileRect, push};
use super::tray::{
    LastTraySlot, TrayGap, TrayMotion, TrayReturn, TraySlot, child_index, insertion_slot, spawn_gap,
};
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
        Option<&'static TrayMotion>,
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
#[component(on_remove = release_drag_claim)]
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
    start_pointer: Vec2,
    origin: Option<Vec2>,
    tray_index: usize,
    pub(super) tray_slots: Vec<TraySlot>,
    /// Transactional working arrangement: pushes persist during the gesture,
    /// but PlacedTile remains the rollback baseline until release.
    pub(super) pushes: Vec<TileRect>,
    last_placement: Option<DragPlacement>,
    pub(super) phased: bool,
}

impl DragFollow {
    pub(super) fn tray_slot(&self, pointer: Vec2) -> usize {
        // A re-grab can begin between animated rows. Picking up and releasing
        // without pointer motion must keep its source slot, not reinterpret it.
        if self.origin.is_none() && pointer == self.start_pointer {
            self.tray_index
        } else {
            insertion_slot(&self.tray_slots, pointer)
        }
    }
}

/// Keep pointer intent separate from the collision-resolved cell. A boundary
/// nudge is not pointer movement and must not start a reverse push next frame.
#[derive(Clone, Copy)]
struct DragPlacement {
    requested: Vec2,
    resolved: Vec2,
}

/// Reserve synchronously, before DragFollow's deferred insertion is visible.
#[derive(Resource, Default)]
pub(super) struct ActiveDrag(Option<Entity>);

impl ActiveDrag {
    pub(super) fn is_active(&self) -> bool {
        self.0.is_some()
    }
}

fn release_drag_claim(
    mut world: bevy::ecs::world::DeferredWorld,
    context: bevy::ecs::lifecycle::HookContext,
) {
    // One cleanup path for drop, cancellation, removal, and despawn. Do not
    // unlock until the old component is actually removed at a flush boundary.
    if let Some(mut active) = world.get_resource_mut::<ActiveDrag>()
        && active.0 == Some(context.entity)
    {
        active.0 = None;
    }
}

/// Landing highlight, computed after live pushes or a phase-mode hover.
/// Release deliberately recomputes from its own event instead of using this cache.
#[derive(Resource, Default)]
pub(super) struct PlacementPreview(pub Option<PreviewPlan>);

pub(super) struct PreviewPlan {
    pub cell: Vec2,
    pub size: Vec2,
}

#[derive(Component)]
pub(super) struct CancelDrag;

/// Resolve the gesture's working positions and pointer constraints, then publish
/// the landing highlight. Presentation systems apply these resolved targets.
pub(super) fn resolve_drag_placement(
    keys: Res<ButtonInput<KeyCode>>,
    zone: Single<(&ComputedNode, &UiGlobalTransform), With<WritingZone>>,
    viewports: Viewports,
    mut dragged: Query<(Entity, &mut DragFollow, &ComputedNode), With<WordTile>>,
    mut preview: ResMut<PlacementPreview>,
) {
    let Ok((entity, mut follow, node)) = dragged.single_mut() else {
        if preview.0.is_some() {
            preview.0 = None;
        }
        return;
    };
    follow.phased = phase_held(&keys);
    if follow.phased {
        follow.last_placement = None;
    }
    // Recompute even without pointer motion: Shift can release a blocked tile.
    follow.target = follow.pointer * node.inverse_scale_factor - follow.grab_offset;
    let size = node.size * node.inverse_scale_factor;
    let geometry = writing_geometry(zone.0, zone.1, viewports.single().ok());
    let placement = placement_at(
        entity,
        follow.pointer,
        follow.grab_offset,
        size,
        geometry,
        &follow.pushes,
        follow.last_placement,
    );
    preview.0 = if let Some((position, moved)) = placement {
        let cell = position.resolved;
        if !follow.phased {
            // Constrain horizontal following at a blocked chain or a boundary
            // insertion. Otherwise retain the exact pointer grip.
            if cell.x != position.requested.x {
                follow.target.x = geometry.origin.x * geometry.inverse_scale + cell.x;
            }
            follow.pushes = moved;
            follow.last_placement = Some(position);
        }
        Some(PreviewPlan { cell, size })
    } else {
        follow.last_placement = None;
        None
    };
}

fn phase_held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
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
    previous: Option<DragPlacement>,
) -> Option<(DragPlacement, Vec<TileRect>)> {
    if !zone.visible.contains(pointer) {
        return None;
    }
    let inv = zone.inverse_scale;
    let bounds = zone.bounds;
    if size.cmpgt(bounds).any() {
        return None;
    }
    let top_left = zone.origin * inv;
    let requested = snap_to_line(pointer * inv - grab_offset - top_left, bounds - size);
    let mut cell = requested;
    if let Some(previous) = previous.filter(|previous| previous.requested.y == requested.y) {
        // While the pointer catches up with a boundary nudge, keep the tile
        // still. Neither a stationary nor a rightward pointer should push left
        // merely because the resolved tile sits to the right of the pointer.
        cell.x = if requested.x > previous.requested.x {
            requested.x.max(previous.resolved.x)
        } else if requested.x < previous.requested.x {
            requested.x.min(previous.resolved.x)
        } else {
            previous.resolved.x
        };
    }
    push(
        TileRect {
            entity,
            pos: cell,
            size,
        },
        previous.map(|previous| previous.resolved),
        committed,
        bounds,
    )
    .map(|(resolved, moved)| {
        (
            DragPlacement {
                requested,
                resolved,
            },
            moved,
        )
    })
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
    mut claim: ResMut<ActiveDrag>,
    tray: Single<(Entity, Option<&Children>), With<BoardTray>>,
    snapping: Query<Entity, (With<PlacedTile>, With<SnapAnim>)>,
    placed_tiles: PlacedTiles,
) {
    if event.button != PointerButton::Primary || claim.0.is_some() || !active.is_empty() {
        return;
    }
    let pushes = placed_tiles
        .iter()
        .filter(|(id, _, _)| *id != event.entity)
        .map(|(entity, placed, node)| TileRect {
            entity,
            pos: placed.0,
            size: node.size * node.inverse_scale_factor,
        })
        .collect();
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
            let (node, _, transform, _, _, motion) = tiles.get(entity).ok()?;
            // Flex child order describes the unanimated rows, not the displayed
            // transforms of words still crossing rows after a previous drop.
            let center = motion
                .and_then(|motion| motion.layout_center)
                .map_or(transform.translation, |center| {
                    center / node.inverse_scale_factor
                });
            Some(TraySlot {
                entity,
                rect: Rect::from_center_size(center, node.size),
                is_held: entity == event.entity,
            })
        })
        .collect();
    let Ok((computed, camera, transform, mut node, placed, _)) = tiles.get_mut(event.entity) else {
        return;
    };

    let Some(pointer) = pointer_in_viewport(event.pointer_location.position, camera, &cameras)
    else {
        return;
    };

    claim.0 = Some(event.entity);

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
        .remove::<(SnapAnim, TrayMotion, TrayReturn, GlobalZIndex)>()
        .insert(DragFollow {
            owner: event.pointer_id,
            button: event.button,
            grab_offset: pointer_logical - top_left,
            target: top_left,
            pointer,
            start_pointer: pointer,
            origin: placed.map(|placed| placed.0),
            tray_index,
            tray_slots,
            pushes,
            last_placement: placed.map(|placed| DragPlacement {
                requested: placed.0,
                resolved: placed.0,
            }),
            phased: false,
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
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    tray: Single<(Entity, &ComputedNode, &UiGlobalTransform, Option<&Children>), With<BoardTray>>,
    gaps: Query<Entity, With<TrayGap>>,
    zone: Single<(Entity, &ComputedNode, &UiGlobalTransform), With<WritingZone>>,
    viewports: Viewports,
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
    let placement = placement_at(
        event.entity,
        pointer,
        follow.grab_offset,
        computed.size * computed.inverse_scale_factor,
        zone_geometry,
        &follow.pushes,
        if phase_held(&keys) {
            None
        } else {
            follow.last_placement
        },
    );

    // Returning the held word to the tray still keeps the pushes. Only an
    // explicit cancellation rolls the whole gesture back to PlacedTile.
    for tile in placement
        .as_ref()
        .map_or(follow.pushes.as_slice(), |(_, moved)| moved.as_slice())
    {
        commands
            .entity(tile.entity)
            .remove::<SnapAnim>()
            .insert(PlacedTile(tile.pos));
    }

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

    if let Some((position, _)) = placement {
        if follow.origin.is_none() {
            commands
                .entity(event.entity)
                .insert(LastTraySlot(follow.tray_index));
        }
        let to = position.resolved;
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
            follow.tray_slot(pointer)
        } else {
            follow.tray_slots.len()
        };
        let index = child_index(tray_children, &gaps, &follow.tray_slots, slot);
        node.position_type = PositionType::Relative;
        node.left = Val::Auto;
        node.top = Val::Auto;
        commands.entity(event.entity).insert(TrayMotion::new(
            pointer * computed.inverse_scale_factor - follow.grab_offset
                + computed.size * computed.inverse_scale_factor * 0.5,
        ));
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
            commands.entity(entity).insert(TrayMotion::new(
                transform.translation * computed.inverse_scale_factor,
            ));
            commands
                .entity(tray_entity)
                .insert_children(index, &[entity]);
        }
    }
}

#[cfg(test)]
mod tests;
