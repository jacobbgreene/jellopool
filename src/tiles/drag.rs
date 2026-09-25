use super::placement::{TileRect, plan};
use super::{BoardTray, WordTile, WritingZone};
use bevy::prelude::*;

/// Ease speed for neighboring tiles sliding during drag previews.
const PUSH_SPEED: f32 = 28.0;
/// Ease speed for pickup/drop scale and shadow.
const FEEL_SPEED: f32 = 16.0;
/// Ease speed for tray gaps opening/closing.
const GAP_SPEED: f32 = 14.0;
/// Scale a tile eases to while held.
const HELD_SCALE: f32 = 1.06;
/// Writing-zone snap grid: tile top-lefts land on multiples of this
/// (logical pixels), measured from the zone's top-left.
const GRID_CELL: f32 = 40.0;
/// Seconds the snap-into-place animation takes.
const SNAP_DURATION: f32 = 0.16;
/// Ease speeds for the snap highlight's position, size, and fade.
const HIGHLIGHT_POS_SPEED: f32 = 22.0;
const HIGHLIGHT_FADE_SPEED: f32 = 12.0;
/// Snap highlight accent: vermilion #C44732, the palette's sole accent,
/// reserved for the active landing preview over the paper writing zone.
/// Alpha scales with fade intensity.
const HIGHLIGHT_FILL: Srgba = Srgba::new(0.769, 0.278, 0.196, 0.12);
const HIGHLIGHT_BORDER: Srgba = Srgba::new(0.769, 0.278, 0.196, 0.55);

/// Per-frame eased "juice" for a tile: visual scale and shadow intensity.
/// Inserted on drag start, removed once the drop animation settles.
#[derive(Component)]
pub struct TileFeel {
    scale: f32,
    scale_target: f32,
    /// Shadow intensity, 0.0 (none) to 1.0 (fully floating).
    shadow: f32,
    shadow_target: f32,
}

/// Present only on the tile currently being dragged. The tile's `Node`
/// offset follows `target` directly every frame.
#[derive(Component)]
pub struct DragFollow {
    /// Pointer position minus tile top-left at grab time (logical units),
    /// so the tile keeps the same grip point while following.
    grab_offset: Vec2,
    /// Where the tile's top-left should be right now (logical units).
    target: Vec2,
    /// Latest pointer position in physical viewport pixels, for region hit tests.
    pointer: Vec2,
    /// The grid cell the tile would snap to if dropped right now
    /// (writing-zone-relative logical units), while hovering the zone.
    snap_target: Option<Vec2>,
    origin: Option<Vec2>,
    tray_index: usize,
}

/// Invisible spacer that reserves room in the tray for the dragged tile.
/// Its width eases open/closed, so neighboring tiles slide apart smoothly.
/// When the pointer moves to a new slot, the old gap closes while a new
/// one opens — the tray never reflows abruptly.
#[derive(Component)]
pub struct TrayGap {
    width: f32,
    target_width: f32,
    height: f32,
}

/// Translucent landing preview shown in the writing zone where the dragged
/// tile would snap. Sized to match the dragged tile, so the landing spot is
/// unambiguous.
#[derive(Component)]
pub struct SnapHighlight {
    pos: Vec2,
    target: Vec2,
    size: Vec2,
    target_size: Vec2,
    alpha: f32,
    target_alpha: f32,
}

/// Plays after a tile is dropped into the writing zone: eases the tile
/// from its release position into the snapped grid cell.
#[derive(Component)]
pub struct SnapAnim {
    from: Vec2,
    to: Vec2,
    /// 0.0 → 1.0 over SNAP_DURATION seconds.
    t: f32,
}

/// Authoritative zone-relative position, never changed by preview animation.
#[derive(Component)]
pub struct PlacedTile(pub Vec2);

fn placement_at(
    entity: Entity,
    pointer: Vec2,
    grab_offset: Vec2,
    size: Vec2,
    zone: (&ComputedNode, &UiGlobalTransform),
    committed: &[TileRect],
) -> Option<(Vec2, Vec<TileRect>)> {
    let (node, transform) = zone;
    if !node.contains_point(*transform, pointer) {
        return None;
    }
    let inv = node.inverse_scale_factor;
    let bounds = node.size * inv;
    if size.cmpgt(bounds).any() {
        return None;
    }
    let top_left = (transform.translation - node.size * 0.5) * inv;
    let cell = snap_to_grid(pointer * inv - grab_offset - top_left, bounds - size);
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

fn shadow(intensity: f32) -> BoxShadow {
    BoxShadow::new(
        // Neutral-warm gray, retuned for the light paper surround: the old
        // near-black mix was tuned against a dark desk and reads muddy here.
        // Alpha structure and geometry are unchanged.
        Color::srgba(0.16, 0.15, 0.13, 0.35 * intensity),
        Val::Px(0.0),
        Val::Px(6.0 * intensity),
        Val::Px(2.0 * intensity),
        Val::Px(10.0 * intensity),
    )
}

fn px_or_zero(value: Val) -> f32 {
    match value {
        Val::Px(px) => px,
        _ => 0.0,
    }
}

fn ease_factor(speed: f32, dt: f32) -> f32 {
    1.0 - (-speed * dt).exp()
}

/// Round a position to the nearest grid cell, clamped to the usable area.
fn snap_to_grid(pos: Vec2, max: Vec2) -> Vec2 {
    let snapped = (pos / GRID_CELL).round() * GRID_CELL;
    snapped.clamp(Vec2::ZERO, (max / GRID_CELL).floor() * GRID_CELL)
}

/// Ease-out with a slight overshoot — the "click" into place.
fn ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    1.0 + C3 * (t - 1.0).powi(3) + C1 * (t - 1.0).powi(2)
}

/// Picking locations are logical render-target coordinates, but UI geometry is
/// physical and viewport-relative. UiScale belongs only in the later conversion
/// via ComputedNode::inverse_scale_factor, not in this conversion.
fn pointer_in_viewport(
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
pub fn tile_drag_start(
    event: On<Pointer<DragStart>>,
    mut commands: Commands,
    drag_layer: Single<Entity, With<super::DragLayer>>,
    cameras: Query<&Camera>,
    mut tiles: Query<
        (
            &ComputedNode,
            &ComputedUiTargetCamera,
            &UiGlobalTransform,
            &mut Node,
            Option<&PlacedTile>,
        ),
        With<WordTile>,
    >,
    active: Query<Entity, With<DragFollow>>,
    tray: Single<&Children, With<BoardTray>>,
    snapping: Query<Entity, (With<PlacedTile>, With<SnapAnim>)>,
) {
    if !active.is_empty() {
        return;
    }
    let tray_index = tray
        .iter()
        .take_while(|child| *child != event.entity)
        .filter(|child| tiles.contains(*child))
        .count();
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

    node.position_type = PositionType::Absolute;
    node.left = Val::Px(top_left.x);
    node.top = Val::Px(top_left.y);

    let pointer_logical = pointer * inv;

    commands
        .entity(event.entity)
        .insert(ChildOf(*drag_layer))
        .remove::<SnapAnim>()
        .insert(DragFollow {
            grab_offset: pointer_logical - top_left,
            target: top_left,
            pointer,
            snap_target: None,
            origin: placed.map(|placed| placed.0),
            tray_index,
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
/// snapping to the highlighted grid cell, otherwise return to the tray —
/// into the gap the tray is currently previewing, if there is one.
pub fn tile_drag_end(
    event: On<Pointer<DragEnd>>,
    cameras: Query<&Camera>,
    mut commands: Commands,
    tray: Single<(Entity, &Children), With<BoardTray>>,
    gaps: Query<(Entity, &TrayGap)>,
    zone: Single<(Entity, &ComputedNode, &UiGlobalTransform), With<WritingZone>>,
    placed: Query<(Entity, &PlacedTile, &ComputedNode), With<WordTile>>,
    mut tiles: Query<
        (
            &ComputedNode,
            &ComputedUiTargetCamera,
            &UiGlobalTransform,
            Option<&DragFollow>,
            &mut Node,
        ),
        With<WordTile>,
    >,
) {
    let Ok((computed, camera, transform, follow, mut node)) = tiles.get_mut(event.entity) else {
        return;
    };
    let (zone_entity, zone_node, zone_transform) = zone.into_inner();
    let (tray_entity, tray_children) = tray.into_inner();

    let Some(follow) = follow else {
        return;
    };
    let Some(pointer) = pointer_in_viewport(event.pointer_location.position, camera, &cameras)
    else {
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
        (zone_node, zone_transform),
        &committed,
    );

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
        let zone_top_left = zone_transform.translation - zone_node.size * 0.5;
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
        // Back to the tray: rejoin normal flex layout and let it reflow.
        node.position_type = PositionType::Relative;
        node.left = Val::Auto;
        node.top = Val::Auto;

        // If the tray is previewing an open gap, the tile takes that exact
        // slot; the gap reserved the tile's size, so nothing visibly jumps.
        let open_gap = gaps
            .iter()
            .find(|(_, gap)| gap.target_width > 0.0)
            .map(|(entity, _)| entity);

        match open_gap {
            Some(gap) => {
                // Count only real tiles before the gap; other (closing)
                // gaps are despawned below and don't take a slot.
                let index = tray_children
                    .iter()
                    .take_while(|child| *child != gap)
                    .filter(|child| !gaps.contains(*child))
                    .count();
                for (gap_entity, _) in &gaps {
                    commands.entity(gap_entity).despawn();
                }
                commands
                    .entity(tray_entity)
                    .insert_children(index, &[event.entity]);
            }
            None => {
                commands.entity(event.entity).insert(ChildOf(tray_entity));
            }
        }
    }
}

/// Applies a dragged tile's pointer-derived target directly each frame.
pub fn tile_follow_system(mut tiles: Query<(&DragFollow, &mut Node), With<WordTile>>) {
    for (follow, mut node) in &mut tiles {
        node.left = Val::Px(follow.target.x);
        node.top = Val::Px(follow.target.y);
    }
}

/// Eases pickup/drop scale and shadow, and cleans up once settled.
pub fn tile_feel_system(
    mut commands: Commands,
    time: Res<Time>,
    mut tiles: Query<(Entity, &mut TileFeel, &mut UiTransform), With<WordTile>>,
) {
    let ease = ease_factor(FEEL_SPEED, time.delta_secs());
    for (entity, mut feel, mut ui_transform) in &mut tiles {
        feel.scale += (feel.scale_target - feel.scale) * ease;
        feel.shadow += (feel.shadow_target - feel.shadow) * ease;

        if (feel.scale - feel.scale_target).abs() < 0.001 {
            feel.scale = feel.scale_target;
        }
        if (feel.shadow - feel.shadow_target).abs() < 0.01 {
            feel.shadow = feel.shadow_target;
        }

        ui_transform.scale = Vec2::splat(feel.scale);
        if feel.shadow > 0.0 {
            commands.entity(entity).insert(shadow(feel.shadow));
        } else {
            commands.entity(entity).remove::<BoxShadow>();
        }

        // Back at rest: animation done, stop running it for this tile.
        if feel.scale == 1.0 && feel.shadow == 0.0 {
            commands.entity(entity).remove::<TileFeel>();
        }
    }
}

/// While a tile is dragged over the tray, keeps a gap open at the slot the
/// tile would occupy. Moving between slots closes the old gap and opens a
/// new one, so tray tiles always slide instead of jumping.
pub fn tray_gap_system(
    mut commands: Commands,
    tray: Single<(Entity, &ComputedNode, &UiGlobalTransform, &Children), With<BoardTray>>,
    dragged: Query<(&DragFollow, &ComputedNode), With<WordTile>>,
    tiles: Query<(&ComputedNode, &UiGlobalTransform), With<WordTile>>,
    mut gaps: Query<(Entity, &mut TrayGap)>,
) {
    let (tray_entity, tray_node, tray_transform, tray_children) = tray.into_inner();

    let close_all = |gaps: &mut Query<(Entity, &mut TrayGap)>| {
        for (_, mut gap) in gaps.iter_mut() {
            gap.target_width = 0.0;
        }
    };

    // Close all gaps when nothing is being dragged.
    let Ok((follow, dragged_node)) = dragged.single() else {
        close_all(&mut gaps);
        return;
    };

    // Close all gaps when the pointer isn't over the tray.
    if !tray_node.contains_point(*tray_transform, follow.pointer) {
        close_all(&mut gaps);
        return;
    }

    let inv = tray_node.inverse_scale_factor;
    let tile_width = dragged_node.size.x * inv;
    let tile_height = dragged_node.size.y * inv;

    // The gap's slot: how many tray tiles come "before" the pointer in
    // reading order (rows top-to-bottom, left-to-right within a row).
    let pointer = follow.pointer;
    let mut index = 0;
    for child in tray_children.iter() {
        if gaps.contains(child) {
            continue;
        }
        let Ok((child_node, child_transform)) = tiles.get(child) else {
            continue;
        };
        let center = child_transform.translation;
        let half_height = child_node.size.y * 0.5;
        let row_above = center.y < pointer.y - half_height;
        let same_row_left = (pointer.y - center.y).abs() <= half_height && center.x < pointer.x;
        if row_above || same_row_left {
            index += 1;
        }
    }

    // Keep open whichever gap already sits at the desired slot; every
    // other gap closes. This is what makes slot changes smooth.
    let mut slot_covered = false;
    for (gap_entity, mut gap) in &mut gaps {
        let tiles_before = tray_children
            .iter()
            .take_while(|child| *child != gap_entity)
            .filter(|child| tiles.contains(*child))
            .count();
        if !slot_covered && tiles_before == index && gap.target_width > 0.0 {
            gap.target_width = tile_width;
            gap.height = tile_height;
            slot_covered = true;
        } else {
            gap.target_width = 0.0;
        }
    }

    // No gap at the desired slot yet: open a fresh one there.
    if !slot_covered {
        let gap = commands
            .spawn((
                Name::new("tray_gap"),
                TrayGap {
                    width: 0.0,
                    target_width: tile_width,
                    height: tile_height,
                },
                Node {
                    width: Val::Px(0.0),
                    height: Val::Px(tile_height),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(tray_entity).insert_children(index, &[gap]);
    }
}

/// Eases each tray gap's width; neighbors slide as layout reflows every
/// frame. Despawns gaps once fully closed.
pub fn tray_gap_anim_system(
    mut commands: Commands,
    time: Res<Time>,
    mut gaps: Query<(Entity, &mut TrayGap, &mut Node)>,
) {
    let ease = ease_factor(GAP_SPEED, time.delta_secs());
    for (entity, mut gap, mut node) in &mut gaps {
        gap.width += (gap.target_width - gap.width) * ease;
        if (gap.target_width - gap.width).abs() < 0.5 {
            gap.width = gap.target_width;
        }
        if gap.width == 0.0 && gap.target_width == 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        node.width = Val::Px(gap.width);
        node.height = Val::Px(gap.height);
    }
}

/// Shows and positions the snap highlight square while a dragged tile
/// hovers the writing zone, and records the cell on the tile so a drop
/// lands exactly where the highlight promised.
pub fn zone_snap_highlight_system(
    mut commands: Commands,
    time: Res<Time>,
    zone: Single<(Entity, &ComputedNode, &UiGlobalTransform), With<WritingZone>>,
    mut dragged: Query<(Entity, &mut DragFollow, &ComputedNode), With<WordTile>>,
    placed: Query<(Entity, &PlacedTile, &ComputedNode), With<WordTile>>,
    mut highlight: Query<
        (
            &mut SnapHighlight,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Without<WordTile>,
    >,
) {
    let (zone_entity, zone_node, zone_transform) = zone.into_inner();
    let mut snap = None;
    if let Ok((entity, mut follow, tile_node)) = dragged.single_mut() {
        let committed: Vec<_> = placed
            .iter()
            .filter(|(id, _, _)| *id != entity)
            .map(|(entity, pos, node)| TileRect {
                entity,
                pos: pos.0,
                size: node.size * node.inverse_scale_factor,
            })
            .collect();
        let size = tile_node.size * tile_node.inverse_scale_factor;
        let placement = placement_at(
            entity,
            follow.pointer,
            follow.grab_offset,
            size,
            (zone_node, zone_transform),
            &committed,
        );
        follow.snap_target = placement.as_ref().map(|(cell, _)| *cell);
        snap = follow.snap_target.map(|cell| (cell, size));
    }

    // Ensure the highlight exists, as the zone's first child so dropped
    // tiles paint above it.
    let Some((mut highlight, mut node, mut background, mut border)) = highlight.single_mut().ok()
    else {
        let entity = commands
            .spawn((
                Name::new("snap_highlight"),
                SnapHighlight {
                    pos: Vec2::ZERO,
                    target: Vec2::ZERO,
                    size: Vec2::ZERO,
                    target_size: Vec2::ZERO,
                    alpha: 0.0,
                    target_alpha: 0.0,
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    // Silhouette must track the tile's border width and
                    // corner radius (currently 1px / 3px; see mod.rs).
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::from(Color::NONE),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(zone_entity).insert_children(0, &[entity]);
        return;
    };

    match snap {
        Some((cell, tile_size)) => {
            highlight.target = cell;
            highlight.target_size = tile_size;
            highlight.target_alpha = 1.0;
        }
        None => {
            highlight.target_alpha = 0.0;
        }
    }

    // Ease position, size, and fade, then apply.
    let pos_ease = ease_factor(HIGHLIGHT_POS_SPEED, time.delta_secs());
    highlight.pos = highlight.pos.lerp(highlight.target, pos_ease);
    if (highlight.target - highlight.pos).length() < 0.5 {
        highlight.pos = highlight.target;
    }
    highlight.size = highlight.size.lerp(highlight.target_size, pos_ease);
    if (highlight.target_size - highlight.size).length() < 0.5 {
        highlight.size = highlight.target_size;
    }
    let fade_ease = ease_factor(HIGHLIGHT_FADE_SPEED, time.delta_secs());
    highlight.alpha += (highlight.target_alpha - highlight.alpha) * fade_ease;
    if (highlight.target_alpha - highlight.alpha).abs() < 0.01 {
        highlight.alpha = highlight.target_alpha;
    }

    let mut fill = HIGHLIGHT_FILL;
    fill.alpha *= highlight.alpha;
    let mut outline = HIGHLIGHT_BORDER;
    outline.alpha *= highlight.alpha;

    node.left = Val::Px(highlight.pos.x);
    node.top = Val::Px(highlight.pos.y);
    node.width = Val::Px(highlight.size.x);
    node.height = Val::Px(highlight.size.y);
    *background = BackgroundColor(Color::from(fill));
    *border = BorderColor::from(Color::from(outline));
}

/// Plays the snap-into-place animation for a tile dropped in the writing
/// zone: ease-out-back from the release point into the grid cell.
pub fn snap_anim_system(
    mut commands: Commands,
    time: Res<Time>,
    mut tiles: Query<(Entity, &mut SnapAnim, &mut Node), With<WordTile>>,
) {
    for (entity, mut anim, mut node) in &mut tiles {
        anim.t = (anim.t + time.delta_secs() / SNAP_DURATION).min(1.0);
        let eased = ease_out_back(anim.t);
        let pos = anim.from.lerp(anim.to, eased);
        node.left = Val::Px(pos.x);
        node.top = Val::Px(pos.y);
        if anim.t >= 1.0 {
            node.left = Val::Px(anim.to.x);
            node.top = Val::Px(anim.to.y);
            commands.entity(entity).remove::<SnapAnim>();
        }
    }
}

/// One writer for neighbor preview/rollback motion. Drop snaps are excluded.
pub fn push_preview_system(
    time: Res<Time>,
    zone: Single<(&ComputedNode, &UiGlobalTransform), With<WritingZone>>,
    dragged: Query<(Entity, &DragFollow, &ComputedNode), With<WordTile>>,
    placed: Query<(Entity, &PlacedTile, &ComputedNode), With<WordTile>>,
    mut nodes: Query<(Entity, &PlacedTile, &mut Node), (Without<DragFollow>, Without<SnapAnim>)>,
) {
    let held = dragged.single().ok();
    let committed: Vec<_> = placed
        .iter()
        .filter(|(id, _, _)| held.is_none_or(|(entity, _, _)| entity != *id))
        .map(|(entity, pos, node)| TileRect {
            entity,
            pos: pos.0,
            size: node.size * node.inverse_scale_factor,
        })
        .collect();
    let preview = held.and_then(|(entity, follow, node)| {
        placement_at(
            entity,
            follow.pointer,
            follow.grab_offset,
            node.size * node.inverse_scale_factor,
            *zone,
            &committed,
        )
    });
    for (entity, placed, mut node) in &mut nodes {
        let target = preview
            .as_ref()
            .and_then(|(_, plan)| plan.iter().find(|tile| tile.entity == entity))
            .map_or(placed.0, |tile| tile.pos);
        let current = Vec2::new(px_or_zero(node.left), px_or_zero(node.top));
        let pos = if current.distance(target) < 0.5 {
            target
        } else {
            current.lerp(target, ease_factor(PUSH_SPEED, time.delta_secs()))
        };
        node.left = Val::Px(pos.x);
        node.top = Val::Px(pos.y);
    }
}

/// Escape restores the held tile's original zone position or tray slot.
pub fn cancel_drag_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    zone: Single<Entity, With<WritingZone>>,
    tray: Single<Entity, With<BoardTray>>,
    gaps: Query<Entity, With<TrayGap>>,
    mut dragged: Query<(Entity, &DragFollow, &mut Node)>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    for gap in &gaps {
        commands.entity(gap).despawn();
    }
    for (entity, follow, mut node) in &mut dragged {
        commands
            .entity(entity)
            .remove::<(DragFollow, SnapAnim)>()
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
            commands
                .entity(*tray)
                .insert_children(follow.tray_index, &[entity]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Headless fixture: real observers, hierarchy commands and the production
    // Update chain, with measured UI geometry supplied instead of a renderer.
    struct Lifecycle {
        app: App,
        zone: Entity,
        held: Entity,
        neighbors: [Entity; 2],
        dpi: f32,
        ui_scale: f32,
        viewport_origin: Vec2,
    }

    fn geometry(size: Vec2, top_left: Vec2) -> (ComputedNode, UiGlobalTransform) {
        (
            ComputedNode {
                size,
                inverse_scale_factor: 1.0,
                ..default()
            },
            bevy::math::Affine2::from_translation(top_left + size * 0.5).into(),
        )
    }

    fn pointer<E: std::fmt::Debug + Clone + Reflect>(
        entity: Entity,
        position: Vec2,
        event: E,
    ) -> Pointer<E> {
        use bevy::camera::{ManualTextureViewHandle, NormalizedRenderTarget};
        use bevy::picking::pointer::{Location, PointerId};
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::TextureView(ManualTextureViewHandle(5)),
                position,
            },
            event,
            entity,
        )
    }

    impl Lifecycle {
        fn new() -> Self {
            Self::scaled(1.0, 1.0, UVec2::ZERO)
        }

        fn scaled(dpi: f32, ui_scale: f32, viewport_origin: UVec2) -> Self {
            use bevy::app::HierarchyPropagatePlugin;
            use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
            let mut app = App::new();
            app.insert_resource(UiScale(ui_scale))
                .add_plugins(HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(
                    PostUpdate,
                ))
                .add_systems(Update, bevy::ui::update::propagate_ui_target_cameras);
            app.world_mut().spawn((
                Camera2d,
                IsDefaultUiCamera,
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::splat(4096),
                            scale_factor: dpi,
                        }),
                        ..default()
                    },
                    viewport: Some(Viewport {
                        physical_position: viewport_origin,
                        physical_size: UVec2::splat(3000),
                        ..default()
                    }),
                    ..default()
                },
            ));
            app.init_resource::<Time>()
                .init_resource::<ButtonInput<KeyCode>>()
                .add_systems(
                    Update,
                    (
                        cancel_drag_system,
                        tile_follow_system,
                        tile_feel_system,
                        tray_gap_system,
                        tray_gap_anim_system,
                        zone_snap_highlight_system,
                        snap_anim_system,
                        push_preview_system,
                    )
                        .chain(),
                );
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
            app.world_mut()
                .spawn((super::super::DragLayer, Node::default()));
            let mut spawn_tile = |pos: Vec2, parent: Entity| {
                app.world_mut()
                    .spawn((
                        WordTile {
                            unique_word: "test".into(),
                        },
                        Node {
                            left: Val::Px(pos.x),
                            top: Val::Px(pos.y),
                            ..default()
                        },
                        geometry(Vec2::new(80.0, 60.0), pos),
                        ChildOf(parent),
                    ))
                    .observe(tile_drag_start)
                    .observe(on_tile_drag)
                    .observe(tile_drag_end)
                    .id()
            };
            let held = spawn_tile(Vec2::new(0.0, 400.0), tray);
            let neighbors = [
                spawn_tile(Vec2::new(160.0, 80.0), zone),
                spawn_tile(Vec2::new(240.0, 80.0), zone),
            ];
            for (entity, x) in neighbors.into_iter().zip([160.0, 240.0]) {
                app.world_mut()
                    .entity_mut(entity)
                    .insert(PlacedTile(Vec2::new(x, 80.0)));
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

        fn location(&self, ui_position: Vec2) -> Vec2 {
            ui_position * self.ui_scale + self.viewport_origin / self.dpi
        }

        fn grab(&mut self, position: Vec2) {
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

        fn release(&mut self) {
            self.release_scaled();
            self.assert_committed();
        }

        fn release_scaled(&mut self) {
            let position = self.location(Vec2::new(170.0, 90.0));
            self.app.world_mut().trigger(pointer(
                self.held,
                position,
                DragEnd {
                    button: PointerButton::Primary,
                    distance: Vec2::ZERO,
                },
            ));
            self.app.world_mut().flush();
            assert!(self.app.world().get::<DragFollow>(self.held).is_none());
            assert_eq!(
                self.app.world().get::<ChildOf>(self.held).unwrap().parent(),
                self.zone
            );
            assert_eq!(
                self.app.world().get::<PlacedTile>(self.held).unwrap().0,
                Vec2::new(160.0, 80.0)
            );
        }

        fn step(&mut self) {
            self.app
                .world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            self.app.update();
        }

        fn assert_committed(&self) {
            for (entity, x) in [self.held, self.neighbors[0], self.neighbors[1]]
                .into_iter()
                .zip([160.0, 240.0, 320.0])
            {
                assert_eq!(
                    self.app.world().get::<PlacedTile>(entity).unwrap().0,
                    Vec2::new(x, 80.0)
                );
            }
        }

        fn assert_settled(&mut self) {
            for _ in 0..90 {
                self.step();
                self.assert_committed();
            }
            let mut positions = Vec::new();
            for entity in [self.held, self.neighbors[0], self.neighbors[1]] {
                let world = self.app.world();
                let node = world.get::<Node>(entity).unwrap();
                let pos = Vec2::new(px_or_zero(node.left), px_or_zero(node.top));
                assert_eq!(pos, world.get::<PlacedTile>(entity).unwrap().0);
                assert!(world.get::<SnapAnim>(entity).is_none());
                assert!(world.get::<TileFeel>(entity).is_none());
                positions.push(pos);
            }
            for pair in positions.windows(2) {
                assert!(pair[0].x + 80.0 <= pair[1].x);
            }
        }
    }

    #[test]
    fn scaled_pointer_tracks_without_drift_and_drives_preview_and_release() {
        for dpi in [1.0, 1.5, 2.0] {
            for ui_scale in [1.0, 1.25] {
                for origin in [UVec2::ZERO, UVec2::new(120, 60)] {
                    let mut fixture = Lifecycle::scaled(dpi, ui_scale, origin);
                    let grip = Vec2::new(10.0, 10.0);
                    fixture.grab(Vec2::new(0.0, 400.0) + grip);
                    for ui_pointer in [
                        Vec2::new(100.0, 420.0),
                        Vec2::new(330.0, 150.0),
                        Vec2::new(170.0, 90.0),
                    ] {
                        let position = fixture.location(ui_pointer);
                        fixture.app.world_mut().trigger(pointer(
                            fixture.held,
                            position,
                            Drag {
                                button: PointerButton::Primary,
                                distance: Vec2::ZERO,
                                delta: Vec2::ZERO,
                            },
                        ));
                        fixture.step();
                        let world = fixture.app.world();
                        let follow = world.get::<DragFollow>(fixture.held).unwrap();
                        assert!((follow.grab_offset - grip).length() < 0.001);
                        assert!((follow.pointer - ui_pointer * dpi * ui_scale).length() < 0.001);
                        let node = world.get::<Node>(fixture.held).unwrap();
                        let actual = Vec2::new(px_or_zero(node.left), px_or_zero(node.top));
                        assert!((actual + grip - ui_pointer).length() < 0.001);
                        if ui_pointer.y == 420.0 {
                            let mut gaps = fixture.app.world_mut().query::<&TrayGap>();
                            assert!(
                                gaps.iter(fixture.app.world())
                                    .any(|gap| (gap.target_width - 80.0).abs() < 0.001)
                            );
                        }
                    }
                    assert_eq!(
                        fixture
                            .app
                            .world()
                            .get::<DragFollow>(fixture.held)
                            .unwrap()
                            .snap_target,
                        Some(Vec2::new(160.0, 80.0))
                    );
                    // Release must independently convert its position, not rely on
                    // the previous move. This also covers release without any move.
                    fixture.release_scaled();
                    let mut immediate = Lifecycle::scaled(dpi, ui_scale, origin);
                    immediate.grab(Vec2::new(10.0, 410.0));
                    immediate.release_scaled();
                }
            }
        }
    }

    #[test]
    fn missing_camera_ignores_grab_without_mutating_tile() {
        let mut fixture = Lifecycle::new();
        let camera = fixture
            .app
            .world()
            .get::<ComputedUiTargetCamera>(fixture.held)
            .unwrap()
            .get()
            .unwrap();
        fixture.app.world_mut().despawn(camera);
        fixture.app.world_mut().trigger(pointer(
            fixture.held,
            Vec2::new(10.0, 410.0),
            DragStart {
                button: PointerButton::Primary,
                hit: bevy::picking::backend::HitData::new(fixture.zone, 0.0, None, None),
            },
        ));
        fixture.app.world_mut().flush();
        assert!(
            fixture
                .app
                .world()
                .get::<DragFollow>(fixture.held)
                .is_none()
        );
        assert_eq!(
            fixture.app.world().get::<Node>(fixture.held).unwrap().top,
            Val::Px(400.0)
        );
    }

    #[test]
    fn rapid_release_commits_without_a_preview_frame_and_settles() {
        let mut fixture = Lifecycle::new();
        fixture.grab(Vec2::new(10.0, 410.0));
        // Release is authoritative even before Drag/highlight/preview runs.
        fixture.release();
        assert_eq!(
            fixture
                .app
                .world()
                .get::<Node>(fixture.neighbors[0])
                .unwrap()
                .left,
            Val::Px(160.0)
        );
        fixture.step();
        fixture.assert_committed();
        assert!(fixture.app.world().get::<SnapAnim>(fixture.held).is_some());
        fixture.assert_settled();
    }

    #[test]
    fn rapid_release_after_one_preview_frame_keeps_committed_push() {
        let mut fixture = Lifecycle::new();
        fixture.grab(Vec2::new(10.0, 410.0));
        fixture.app.world_mut().trigger(pointer(
            fixture.held,
            Vec2::new(170.0, 90.0),
            Drag {
                button: PointerButton::Primary,
                distance: Vec2::new(160.0, -320.0),
                delta: Vec2::new(160.0, -320.0),
            },
        ));
        fixture.step();
        let neighbor = fixture.neighbors[0];
        let world = fixture.app.world();
        let preview_x = px_or_zero(world.get::<Node>(neighbor).unwrap().left);
        assert!(preview_x > 160.0 && preview_x < 240.0);
        assert_eq!(world.get::<PlacedTile>(neighbor).unwrap().0.x, 160.0);
        fixture.release();
        fixture.step();
        assert!(px_or_zero(fixture.app.world().get::<Node>(neighbor).unwrap().left) > preview_x);
        fixture.assert_settled();
    }

    #[test]
    fn regrab_during_snap_then_escape_preserves_last_commit() {
        let mut fixture = Lifecycle::new();
        fixture.grab(Vec2::new(10.0, 410.0));
        fixture.release();
        fixture.step();
        assert!(fixture.app.world().get::<SnapAnim>(fixture.held).is_some());
        // Stand in for layout's transform propagation after this animation frame.
        let node = fixture.app.world().get::<Node>(fixture.held).unwrap();
        let pos = Vec2::new(px_or_zero(node.left), px_or_zero(node.top));
        fixture
            .app
            .world_mut()
            .entity_mut(fixture.held)
            .insert(geometry(Vec2::new(80.0, 60.0), pos));
        fixture.grab(pos + Vec2::splat(10.0));
        assert!(fixture.app.world().get::<SnapAnim>(fixture.held).is_none());
        assert_eq!(
            fixture
                .app
                .world()
                .get::<DragFollow>(fixture.held)
                .unwrap()
                .origin,
            Some(Vec2::new(160.0, 80.0))
        );
        fixture
            .app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        fixture.step();
        fixture
            .app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        assert!(
            fixture
                .app
                .world()
                .get::<DragFollow>(fixture.held)
                .is_none()
        );
        assert_eq!(
            fixture
                .app
                .world()
                .get::<ChildOf>(fixture.held)
                .unwrap()
                .parent(),
            fixture.zone
        );
        fixture.assert_settled();
    }

    #[test]
    fn escape_restores_held_and_rolls_back_neighbors() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, (cancel_drag_system, push_preview_system).chain());
        let zone = app.world_mut().spawn((WritingZone, Node::default())).id();
        app.world_mut().spawn(BoardTray);
        let origin = Vec2::new(80.0, 80.0);
        let held = app
            .world_mut()
            .spawn((
                WordTile {
                    unique_word: "held".into(),
                },
                Node::default(),
                PlacedTile(origin),
                DragFollow {
                    grab_offset: Vec2::ZERO,
                    target: Vec2::ZERO,
                    pointer: Vec2::ZERO,
                    snap_target: None,
                    origin: Some(origin),
                    tray_index: 0,
                },
            ))
            .id();
        let neighbor = app
            .world_mut()
            .spawn((
                WordTile {
                    unique_word: "neighbor".into(),
                },
                Node {
                    left: Val::Px(240.0),
                    top: Val::Px(80.0),
                    ..default()
                },
                PlacedTile(Vec2::new(160.0, 80.0)),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(app.world().get::<DragFollow>(held).is_none());
        assert_eq!(app.world().get::<ChildOf>(held).unwrap().parent(), zone);
        assert_eq!(app.world().get::<PlacedTile>(held).unwrap().0, origin);
        assert_eq!(
            app.world().get::<Node>(neighbor).unwrap().left,
            Val::Px(160.0)
        );
        assert_eq!(
            app.world().get::<PlacedTile>(neighbor).unwrap().0,
            Vec2::new(160.0, 80.0)
        );
    }

    #[test]
    fn edge_snap_stays_on_grid() {
        assert_eq!(
            snap_to_grid(Vec2::splat(999.0), Vec2::new(123.0, 79.0)),
            Vec2::new(120.0, 40.0)
        );
    }
}
