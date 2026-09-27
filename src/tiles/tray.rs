use super::drag::DragFollow;
use crate::prelude::*;

#[cfg(test)]
mod tests;

const SLIDE_SPEED: f32 = 18.0;

/// A single, full-size placeholder participates in flex layout. Animation
/// belongs to the tiles, since animating widths makes wrapped rows jump.
#[derive(Component)]
pub struct TrayGap {
    index: usize,
}

/// Capture insertion anchors before lifting the tile. Preview reflow must not
/// move these hit targets and feed back into the next insertion decision.
pub(super) struct TraySlot {
    pub entity: Entity,
    pub rect: Rect,
    /// Keep the source rectangle as a row anchor, but not an insertion item.
    pub is_held: bool,
}

#[derive(Component)]
pub struct TrayMotion {
    /// Last displayed center, in logical viewport coordinates.
    pub center: Vec2,
}

pub(super) fn insertion_slot(slots: &[TraySlot], pointer: Vec2) -> usize {
    // Pick the nearest row first, including the whitespace between rows.
    // Testing each tile's vertical bounds independently treats row gaps as
    // the end of a row regardless of the pointer's horizontal position.
    let mut best = (f32::INFINITY, 0);
    let mut start = 0;
    let mut tiles_before = 0;
    while start < slots.len() {
        let mut end = start + 1;
        let mut top = slots[start].rect.min.y;
        let mut bottom = slots[start].rect.max.y;
        while end < slots.len() && slots[end].rect.min.y < bottom {
            top = top.min(slots[end].rect.min.y);
            bottom = bottom.max(slots[end].rect.max.y);
            end += 1;
        }
        let distance = (top - pointer.y).max(pointer.y - bottom).max(0.0);
        if distance < best.0 {
            let index = tiles_before
                + slots[start..end]
                    .iter()
                    .filter(|slot| !slot.is_held)
                    .take_while(|slot| slot.rect.center().x < pointer.x)
                    .count();
            best = (distance, index);
        }
        tiles_before += slots[start..end]
            .iter()
            .filter(|slot| !slot.is_held)
            .count();
        start = end;
    }
    best.1
}

/// Translate a tile-only slot to a child index *after* removing placeholders.
/// Empty trays have no Children component, and decorative children don't
/// consume tile slots.
pub(super) fn child_index(
    children: Option<&Children>,
    gaps: &Query<Entity, With<TrayGap>>,
    slots: &[TraySlot],
    slot: usize,
) -> usize {
    let children: Vec<_> = children
        .into_iter()
        .flat_map(|children| children.iter())
        .filter(|child| !gaps.contains(*child))
        .collect();
    slots
        .iter()
        .filter(|slot| !slot.is_held)
        .nth(slot)
        .and_then(|slot| children.iter().position(|child| *child == slot.entity))
        .unwrap_or(children.len())
}

pub(super) fn spawn_gap(
    commands: &mut Commands,
    tray: Entity,
    child_index: usize,
    index: usize,
    size: Vec2,
) {
    let gap = commands
        .spawn((
            Name::new("tray_gap"),
            TrayGap { index },
            Node {
                width: Val::Px(size.x),
                height: Val::Px(size.y),
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(tray).insert_children(child_index, &[gap]);
}

pub fn tray_gap_system(
    mut commands: Commands,
    tray: Single<(Entity, &ComputedNode, &UiGlobalTransform, Option<&Children>), With<BoardTray>>,
    dragged: Query<(&DragFollow, &ComputedNode), With<WordTile>>,
    gaps: Query<Entity, With<TrayGap>>,
    gap_slots: Query<&TrayGap>,
) {
    let (tray_entity, node, transform, children) = tray.into_inner();
    let preview = dragged
        .single()
        .ok()
        .filter(|(follow, _)| node.contains_point(*transform, follow.pointer));
    let index = preview.map(|(follow, _)| insertion_slot(&follow.tray_slots, follow.pointer));
    if let Ok(gap) = gap_slots.single()
        && index == Some(gap.index)
    {
        return;
    }

    // Remove the old gap outright: even a zero-width flex item adds spacing
    // and can force another row. The visual slide is handled after layout.
    for gap in &gaps {
        commands.entity(gap).despawn();
    }
    if let Some((follow, tile)) = preview {
        let index = index.unwrap();
        spawn_gap(
            &mut commands,
            tray_entity,
            child_index(children, &gaps, &follow.tray_slots, index),
            index,
            tile.size * tile.inverse_scale_factor,
        );
    }
}

/// Layout computes final wrapped positions each frame. Ease only the displayed
/// transforms afterward, including tile text, before clipping/rendering/picking.
/// Keeping Node and UiTransform untouched avoids changing flex measurements or
/// carrying a tray animation offset into the drag layer on a quick re-grab.
pub fn tray_slide_system(
    mut commands: Commands,
    time: Res<Time>,
    tray: Single<Entity, With<BoardTray>>,
    mut tiles: Query<(Entity, &ChildOf, &ComputedNode, Option<&mut TrayMotion>), With<WordTile>>,
    children: Query<&Children>,
    mut transforms: Query<&mut UiGlobalTransform>,
) {
    let ease = 1.0 - (-SLIDE_SPEED * time.delta_secs()).exp();
    for (entity, parent, node, motion) in &mut tiles {
        if parent.parent() != *tray {
            if motion.is_some() {
                commands.entity(entity).remove::<TrayMotion>();
            }
            continue;
        }
        if node.size == Vec2::ZERO {
            continue;
        }
        let Ok(transform) = transforms.get(entity) else {
            continue;
        };
        let target = transform.translation * node.inverse_scale_factor;
        let Some(mut motion) = motion else {
            commands
                .entity(entity)
                .insert(TrayMotion { center: target });
            continue;
        };
        motion.center = if motion.center.distance(target) < 0.5 {
            target
        } else {
            motion.center.lerp(target, ease)
        };
        let offset = (motion.center - target) / node.inverse_scale_factor;
        if offset == Vec2::ZERO {
            continue;
        }
        let translation = bevy::math::Affine2::from_translation(offset);
        for child in std::iter::once(entity).chain(children.iter_descendants(entity)) {
            if let Ok(mut transform) = transforms.get_mut(child) {
                *transform = (translation * transform.affine()).into();
            }
        }
    }
}
