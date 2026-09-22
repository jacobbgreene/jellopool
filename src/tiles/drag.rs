use super::{BoardTray, WordTile, WritingZone};
use bevy::prelude::*;

/// Eased follow speed for the dragged tile trailing the pointer.
const FOLLOW_SPEED: f32 = 28.0;
/// Ease speed for pickup/drop scale and shadow.
const FEEL_SPEED: f32 = 16.0;
/// Ease speed for the tray gap opening/closing.
const GAP_SPEED: f32 = 14.0;
/// Scale a tile eases to while held.
const HELD_SCALE: f32 = 1.06;

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
}

/// Invisible spacer that reserves room in the tray for the dragged tile.
/// Its width eases open/closed, so neighboring tiles slide apart smoothly.
#[derive(Component)]
pub struct TrayGap {
    width: f32,
    target_width: f32,
    height: f32,
}

fn shadow(intensity: f32) -> BoxShadow {
    BoxShadow::new(
        Color::srgba(0.0, 0.0, 0.0, 0.4 * intensity),
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
        .insert(DragFollow {
            grab_offset: pointer_logical - top_left,
            target: top_left,
            pointer: event.pointer_location.position,
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

/// On release: drop into the writing zone if the pointer is inside it
/// (clamped to its bounds), otherwise return to the tray — into the gap
/// the tray is currently previewing, if there is one.
pub fn tile_drag_end(
    event: On<Pointer<DragEnd>>,
    mut commands: Commands,
    tray: Single<(Entity, &Children), With<BoardTray>>,
    gaps: Query<Entity, With<TrayGap>>,
    zone: Single<(Entity, &ComputedNode, &UiGlobalTransform), With<WritingZone>>,
    mut tiles: Query<(&ComputedNode, &UiGlobalTransform, &mut Node), With<WordTile>>,
) {
    let Ok((computed, transform, mut node)) = tiles.get_mut(event.entity) else {
        return;
    };
    let (zone_entity, zone_node, zone_transform) = zone.into_inner();
    let (tray_entity, tray_children) = tray.into_inner();

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
        // Keep the tile where it appears on screen, but express it as an
        // offset from the writing zone's top-left instead of the viewport's.
        let inv = computed.inverse_scale_factor;
        let tile_top_left = transform.translation - computed.size * 0.5;
        let zone_top_left = zone_transform.translation - zone_node.size * 0.5;
        let relative = (tile_top_left - zone_top_left) * inv;

        // Clamp so the tile stays fully inside the zone; an oversized tile
        // clamps against a zeroed range rather than an inverted one.
        let max = ((zone_node.size - computed.size) * inv).max(Vec2::ZERO);
        node.left = Val::Px(relative.x.clamp(0.0, max.x));
        node.top = Val::Px(relative.y.clamp(0.0, max.y));
        node.position_type = PositionType::Absolute;

        commands.entity(event.entity).insert(ChildOf(zone_entity));
    } else {
        // Back to the tray: rejoin normal flex layout and let it reflow.
        node.position_type = PositionType::Relative;
        node.left = Val::Auto;
        node.top = Val::Auto;

        // If the tray is previewing a gap, the tile takes that exact slot;
        // the gap spawned at the same size, so nothing visibly jumps.
        let mut insert_at = None;
        if let Ok(gap) = gaps.single() {
            insert_at = tray_children
                .iter()
                .position(|child| child == gap)
                .map(|position| (gap, position));
        }

        match insert_at {
            Some((gap, position)) => {
                commands.entity(gap).despawn();
                commands
                    .entity(tray_entity)
                    .insert_children(position, &[event.entity]);
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

/// While a tile is dragged over the tray, keeps an invisible gap open at
/// the slot the tile would occupy, so tray tiles slide aside in advance.
pub fn tray_gap_system(
    mut commands: Commands,
    tray: Single<(Entity, &ComputedNode, &UiGlobalTransform, &Children), With<BoardTray>>,
    dragged: Query<(&DragFollow, &ComputedNode), With<WordTile>>,
    tiles: Query<(&ComputedNode, &UiGlobalTransform), With<WordTile>>,
    mut gaps: Query<(Entity, &mut TrayGap)>,
) {
    let (tray_entity, tray_node, tray_transform, tray_children) = tray.into_inner();

    // Shrink the gap when nothing is being dragged.
    let Ok((follow, dragged_node)) = dragged.single() else {
        for (_, mut gap) in &mut gaps {
            gap.target_width = 0.0;
        }
        return;
    };

    // Shrink the gap when the pointer isn't over the tray.
    if !tray_node.contains_point(*tray_transform, follow.pointer) {
        for (_, mut gap) in &mut gaps {
            gap.target_width = 0.0;
        }
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

    match gaps.single_mut() {
        Ok((gap_entity, mut gap)) => {
            gap.target_width = tile_width;
            gap.height = tile_height;
            // Only move the gap when its tile-count position changed.
            let tiles_before = tray_children
                .iter()
                .take_while(|child| *child != gap_entity)
                .filter(|child| tiles.contains(*child))
                .count();
            if tiles_before != index {
                commands.entity(tray_entity).detach_children(&[gap_entity]);
                commands
                    .entity(tray_entity)
                    .insert_children(index, &[gap_entity]);
            }
        }
        Err(_) => {
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
}

/// Eases the tray gap's width; neighbors slide as layout reflows each
/// frame. Despawns the gap once it has fully closed.
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
