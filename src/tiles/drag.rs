use super::{BoardTray, WordTile, WritingZone};
use bevy::prelude::*;

/// Eased follow speed for the dragged tile trailing the pointer.
const FOLLOW_SPEED: f32 = 28.0;
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
/// Snap highlight accent: warm amber lamplight that reads beautifully on
/// the parchment writing zone. Alpha scales with fade intensity.
const HIGHLIGHT_FILL: Srgba = Srgba::new(0.60, 0.48, 0.28, 0.12);
const HIGHLIGHT_BORDER: Srgba = Srgba::new(0.60, 0.48, 0.28, 0.55);

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
/// offset eases toward `target` every frame instead of snapping, which is
/// what makes the drag feel smooth rather than rigid.
#[derive(Component)]
pub struct DragFollow {
    /// Pointer position minus tile top-left at grab time (logical units),
    /// so the tile keeps the same grip point while following.
    grab_offset: Vec2,
    /// Where the tile's top-left should be right now (logical units).
    target: Vec2,
    /// Latest pointer position in physical pixels, for region hit tests.
    pointer: Vec2,
    /// The grid cell the tile would snap to if dropped right now
    /// (writing-zone-relative logical units), while hovering the zone.
    snap_target: Option<Vec2>,
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

fn shadow(intensity: f32) -> BoxShadow {
    BoxShadow::new(
        Color::srgba(0.08, 0.05, 0.03, 0.35 * intensity),
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
    snapped.clamp(Vec2::ZERO, max)
}

/// Ease-out with a slight overshoot — the "click" into place.
fn ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    1.0 + C3 * (t - 1.0).powi(3) + C1 * (t - 1.0).powi(2)
}

/// On grab: move the tile into the drag layer without it visibly jumping,
/// and start the float animation (slight grow + shadow fade-in).
pub fn tile_drag_start(
    event: On<Pointer<DragStart>>,
    mut commands: Commands,
    drag_layer: Single<Entity, With<super::DragLayer>>,
    mut tiles: Query<(&ComputedNode, &UiGlobalTransform, &mut Node), With<WordTile>>,
) {
    let Ok((computed, transform, mut node)) = tiles.get_mut(event.entity) else {
        return;
    };

    // UiGlobalTransform is the node's center in physical viewport pixels;
    // convert to a logical top-left offset for the drag layer, which
    // covers the whole viewport.
    let inv = computed.inverse_scale_factor;
    let top_left = (transform.translation - computed.size * 0.5) * inv;

    node.position_type = PositionType::Absolute;
    node.left = Val::Px(top_left.x);
    node.top = Val::Px(top_left.y);

    let pointer_logical = event.pointer_location.position * inv;

    commands
        .entity(event.entity)
        .insert(ChildOf(*drag_layer))
        .remove::<SnapAnim>()
        .insert(DragFollow {
            grab_offset: pointer_logical - top_left,
            target: top_left,
            pointer: event.pointer_location.position,
            snap_target: None,
        })
        .insert(TileFeel {
            scale: 1.0,
            scale_target: HELD_SCALE,
            shadow: 0.0,
            shadow_target: 1.0,
        });
}

/// While dragging: update where the tile should be. The follow system
/// eases the actual offset toward this target.
pub fn on_tile_drag(
    event: On<Pointer<Drag>>,
    mut tiles: Query<(&ComputedNode, &mut DragFollow), With<WordTile>>,
) {
    let Ok((computed, mut follow)) = tiles.get_mut(event.entity) else {
        return;
    };

    // Pointer positions are physical pixels; logical UI units differ at
    // non-default DPI or UiScale.
    let inv = computed.inverse_scale_factor;
    follow.pointer = event.pointer_location.position;
    follow.target = follow.pointer * inv - follow.grab_offset;
}

/// On release: drop into the writing zone if the pointer is inside it,
/// snapping to the highlighted grid cell, otherwise return to the tray —
/// into the gap the tray is currently previewing, if there is one.
pub fn tile_drag_end(
    event: On<Pointer<DragEnd>>,
    mut commands: Commands,
    tray: Single<(Entity, &Children), With<BoardTray>>,
    gaps: Query<(Entity, &TrayGap)>,
    zone: Single<(Entity, &ComputedNode, &UiGlobalTransform), With<WritingZone>>,
    mut tiles: Query<
        (
            &ComputedNode,
            &UiGlobalTransform,
            Option<&DragFollow>,
            &mut Node,
        ),
        With<WordTile>,
    >,
) {
    let Ok((computed, transform, follow, mut node)) = tiles.get_mut(event.entity) else {
        return;
    };
    let (zone_entity, zone_node, zone_transform) = zone.into_inner();
    let (tray_entity, tray_children) = tray.into_inner();

    let snap_target = follow.and_then(|follow| follow.snap_target);

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

    if zone_node.contains_point(*zone_transform, event.pointer_location.position) {
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
        // Prefer the cell the highlight is showing; fall back to snapping
        // the release position directly.
        let to = snap_target
            .unwrap_or_else(|| snap_to_grid(from, max))
            .clamp(Vec2::ZERO, max);

        node.position_type = PositionType::Absolute;
        node.left = Val::Px(from.x);
        node.top = Val::Px(from.y);

        commands
            .entity(event.entity)
            .insert(ChildOf(zone_entity))
            .insert(SnapAnim { from, to, t: 0.0 });
    } else {
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

/// Eases a dragged tile's offset toward its pointer-derived target.
pub fn tile_follow_system(
    time: Res<Time>,
    mut tiles: Query<(&DragFollow, &mut Node), With<WordTile>>,
) {
    let ease = ease_factor(FOLLOW_SPEED, time.delta_secs());
    for (follow, mut node) in &mut tiles {
        let left = px_or_zero(node.left);
        let top = px_or_zero(node.top);
        node.left = Val::Px(left + (follow.target.x - left) * ease);
        node.top = Val::Px(top + (follow.target.y - top) * ease);
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
    mut dragged: Query<(&mut DragFollow, &ComputedNode), With<WordTile>>,
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
    let inv = zone_node.inverse_scale_factor;

    // Work out the snap cell and preview size while a drag hovers the zone.
    let mut snap = None;
    if let Ok((mut follow, tile_node)) = dragged.single_mut() {
        if zone_node.contains_point(*zone_transform, follow.pointer) {
            let zone_top_left = (zone_transform.translation - zone_node.size * 0.5) * inv;
            let tile_size = tile_node.size * tile_node.inverse_scale_factor;
            let max = (zone_node.size * inv - tile_size).max(Vec2::ZERO);
            let cell = snap_to_grid(follow.target - zone_top_left, max);
            follow.snap_target = Some(cell);
            snap = Some((cell, tile_size));
        } else {
            follow.snap_target = None;
        }
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
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
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
