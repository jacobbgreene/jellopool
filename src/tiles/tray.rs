use super::animation::{HELD_SCALE, SnapAnim, TileFeel};
use super::drag::{ActiveDrag, DragFollow};
use crate::prelude::*;

#[cfg(test)]
mod tests;

const SLIDE_SPEED: f32 = 18.0;
const RETURN_DURATION: f32 = 0.22;

#[derive(Component, Clone, Copy)]
pub(super) struct LastTraySlot(pub usize);

#[derive(Component)]
pub(super) struct TrayReturn {
    from: Vec2,
    elapsed: f32,
}

type ReturnTiles<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedNode,
        &'static UiGlobalTransform,
        &'static mut Node,
        Option<&'static LastTraySlot>,
    ),
    (With<WordTile>, With<PlacedTile>, Without<DragFollow>),
>;

pub(super) fn return_tile_to_tray(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    active: Res<ActiveDrag>,
    tray: Single<(Entity, Option<&Children>), With<BoardTray>>,
    words: Query<(), With<WordTile>>,
    mut tiles: ReturnTiles,
) {
    if event.button != PointerButton::Secondary || active.is_active() {
        return;
    }
    let Ok((computed, transform, mut node, slot)) = tiles.get_mut(event.entity) else {
        return;
    };
    event.propagate(false);
    let (tray_entity, children) = tray.into_inner();
    let index = children
        .into_iter()
        .flat_map(|children| children.iter().enumerate())
        .filter(|(_, child)| words.contains(*child))
        .nth(slot.map_or(usize::MAX, |slot| slot.0))
        .map_or(children.map_or(0, Children::len), |(index, _)| index);
    let from = transform.translation * computed.inverse_scale_factor;
    node.position_type = PositionType::Relative;
    node.left = Val::Auto;
    node.top = Val::Auto;
    commands
        .entity(event.entity)
        .remove::<(PlacedTile, SnapAnim)>()
        .insert((
            TrayMotion::new(from),
            TrayReturn { from, elapsed: 0.0 },
            GlobalZIndex(10),
            TileFeel {
                scale: 1.0,
                scale_target: HELD_SCALE,
                shadow: 0.0,
                shadow_target: 1.0,
            },
        ));
    commands
        .entity(tray_entity)
        .insert_children(index, &[event.entity]);
}

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
    /// Unanimated flex position, also logical. A fresh return to the tray has
    /// no layout target until PostUpdate; later grabs freeze these row anchors.
    pub layout_center: Option<Vec2>,
}

impl TrayMotion {
    pub(super) fn new(center: Vec2) -> Self {
        Self {
            center,
            layout_center: None,
        }
    }
}

pub(super) fn insertion_slot(slots: &[TraySlot], pointer: Vec2) -> usize {
    // Slots must stay in flex child order: rows top-to-bottom, tiles left-to-right.
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
        .filter(|(follow, _)| node.contains_point(*transform, follow.pointer))
        .map(|(follow, tile)| {
            let index = follow.tray_slot(follow.pointer);
            (follow, tile, index)
        });
    if let Ok(gap) = gap_slots.single()
        && preview.is_some_and(|(_, _, index)| index == gap.index)
    {
        return;
    }

    // Remove the old gap outright: even a zero-width flex item adds spacing
    // and can force another row. The visual slide is handled after layout.
    for gap in &gaps {
        commands.entity(gap).despawn();
    }
    if let Some((follow, tile, index)) = preview {
        spawn_gap(
            &mut commands,
            tray_entity,
            child_index(children, &gaps, &follow.tray_slots, index),
            index,
            tile.size * tile.inverse_scale_factor,
        );
    }
}

type SlidingTiles<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ChildOf,
        &'static ComputedNode,
        Option<&'static mut TrayMotion>,
        Option<&'static mut TrayReturn>,
        Option<&'static mut TileFeel>,
    ),
    With<WordTile>,
>;

/// Layout computes final wrapped positions each frame. Ease only the displayed
/// transforms afterward, including tile text, before clipping/rendering/picking.
/// Keeping Node and UiTransform untouched avoids changing flex measurements or
/// carrying a tray animation offset into the drag layer on a quick re-grab.
pub fn tray_slide_system(
    mut commands: Commands,
    time: Res<Time>,
    tray: Single<Entity, With<BoardTray>>,
    mut tiles: SlidingTiles,
    children: Query<&Children>,
    mut transforms: Query<&mut UiGlobalTransform>,
) {
    let ease = 1.0 - (-SLIDE_SPEED * time.delta_secs()).exp();
    for (entity, parent, node, motion, returning, feel) in &mut tiles {
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
            commands.entity(entity).insert(TrayMotion {
                center: target,
                layout_center: Some(target),
            });
            continue;
        };
        motion.layout_center = Some(target);
        motion.center = if let Some(mut returning) = returning {
            returning.elapsed += time.delta_secs();
            let progress = (returning.elapsed / RETURN_DURATION).min(1.0);
            let eased = progress * progress * (3.0 - 2.0 * progress);
            let arc = 24.0 * (std::f32::consts::PI * progress).sin();
            if progress >= 0.65
                && let Some(mut feel) = feel
            {
                feel.scale_target = 1.0;
                feel.shadow_target = 0.0;
            }
            if progress >= 1.0 {
                commands
                    .entity(entity)
                    .remove::<(TrayReturn, GlobalZIndex)>();
                target
            } else {
                returning.from.lerp(target, eased) - Vec2::Y * arc
            }
        } else if motion.center.distance(target) < 0.5 {
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
