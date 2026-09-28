//! Pure, atomic, line-local placement. Empty space never reflows a line.
use bevy::prelude::*;

/// Horizontal spacing unit for authored fixtures, not a gameplay snap.
pub(crate) const GRID: f32 = 40.0;
pub(crate) const LINE_PITCH: f32 = 56.0;
pub(crate) const LINE_COUNT: usize = 24;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TileRect {
    pub entity: Entity,
    pub pos: Vec2,
    pub size: Vec2,
}

fn overlaps(a: TileRect, b: TileRect) -> bool {
    a.pos.x < b.pos.x + b.size.x
        && a.pos.x + a.size.x > b.pos.x
        && a.pos.y < b.pos.y + b.size.y
        && a.pos.y + a.size.y > b.pos.y
}

fn fits(tile: TileRect, bounds: Vec2) -> bool {
    tile.size.cmpgt(Vec2::ZERO).all()
        && tile.pos.cmpge(Vec2::ZERO).all()
        && (tile.pos + tile.size).cmple(bounds).all()
}

fn valid_result(held: TileRect, result: &[TileRect], bounds: Vec2) -> bool {
    result.iter().enumerate().all(|(index, tile)| {
        tile.entity != held.entity
            && fits(*tile, bounds)
            && !overlaps(held, *tile)
            && result[..index]
                .iter()
                .all(|other| other.entity != tile.entity && !overlaps(*other, *tile))
    })
}

/// Open only the space needed for an actual overlap, preserving reading order
/// and every tile's line. Try insertion boundaries through the covered words,
/// preferring the side indicated by the word's center, with the smallest
/// displacement fallback if that side is blocked. No wrapping, repacking, or push
/// into another line; a full line rejects the entire plan.
pub(crate) fn plan(held: TileRect, committed: &[TileRect], bounds: Vec2) -> Option<Vec<TileRect>> {
    if !fits(held, bounds) {
        return None;
    }
    let mut result = committed.to_vec();
    result.sort_by_key(|tile| tile.entity.to_bits());
    if !result.iter().any(|tile| overlaps(held, *tile)) {
        return valid_result(held, &result, bounds).then_some(result);
    }
    let mut row: Vec<_> = result
        .iter()
        .enumerate()
        .filter(|(_, tile)| tile.pos.y == held.pos.y)
        .map(|(index, _)| index)
        .collect();
    row.sort_by(|&a, &b| {
        result[a]
            .pos
            .x
            .total_cmp(&result[b].pos.x)
            .then_with(|| result[a].entity.to_bits().cmp(&result[b].entity.to_bits()))
    });
    let first = row
        .iter()
        .position(|&index| overlaps(held, result[index]))?;
    let last = row
        .iter()
        .rposition(|&index| overlaps(held, result[index]))?
        + 1;
    let center = held.pos.x + held.size.x * 0.5;
    let preferred = row
        .iter()
        .take_while(|&&index| result[index].pos.x + result[index].size.x * 0.5 < center)
        .count();
    let mut best: Option<(f32, Vec<TileRect>)> = None;
    for split in
        std::iter::once(preferred).chain((first..=last).filter(|split| *split != preferred))
    {
        let mut candidate = result.clone();
        let mut edge = held.pos.x;
        for &index in row[..split].iter().rev() {
            let tile = &mut candidate[index];
            tile.pos.x = tile.pos.x.min(edge - tile.size.x);
            edge = tile.pos.x;
        }
        edge = held.pos.x + held.size.x;
        for &index in &row[split..] {
            let tile = &mut candidate[index];
            tile.pos.x = tile.pos.x.max(edge);
            edge = tile.pos.x + tile.size.x;
        }
        if candidate.iter().any(|tile| !fits(*tile, bounds)) {
            continue;
        }
        if split == preferred {
            return valid_result(held, &candidate, bounds).then_some(candidate);
        }
        let distance: f32 = candidate
            .iter()
            .zip(&result)
            .map(|(after, before)| (after.pos.x - before.pos.x).abs())
            .sum();
        if best.as_ref().is_none_or(|(cost, _)| distance < *cost) {
            best = Some((distance, candidate));
        }
    }
    let (_, result) = best?;
    valid_result(held, &result, bounds).then_some(result)
}

/// Continue a live push without letting a fast horizontal move tunnel through
/// words. Keep reading order and stop against the page edge when a chain fills
/// the line. Entering a new line (or leaving phase mode) uses ordinary insertion.
pub(crate) fn push(
    mut held: TileRect,
    previous: Option<Vec2>,
    neighbors: &[TileRect],
    bounds: Vec2,
) -> Option<(Vec2, Vec<TileRect>)> {
    let Some(from) = previous.filter(|from| from.y == held.pos.y && from.x != held.pos.x) else {
        return plan(held, neighbors, bounds).map(|result| (held.pos, result));
    };
    let start = TileRect { pos: from, ..held };
    if !fits(start, bounds) || !valid_result(start, neighbors, bounds) {
        return plan(held, neighbors, bounds).map(|result| (held.pos, result));
    }
    let right = held.pos.x > from.x;
    let mut result = neighbors.to_vec();
    let mut ahead: Vec<_> = result
        .iter()
        .enumerate()
        .filter(|(_, tile)| {
            tile.pos.y == from.y
                && if right {
                    tile.pos.x >= from.x + held.size.x
                } else {
                    tile.pos.x + tile.size.x <= from.x
                }
        })
        .map(|(index, _)| index)
        .collect();
    ahead.sort_by(|&a, &b| {
        let order = result[a].pos.x.total_cmp(&result[b].pos.x);
        if right { order } else { order.reverse() }
    });
    let occupied: f32 = ahead.iter().map(|&index| result[index].size.x).sum();
    held.pos.x = if right {
        held.pos.x.min(bounds.x - held.size.x - occupied)
    } else {
        held.pos.x.max(occupied)
    };
    let mut edge = if right {
        held.pos.x + held.size.x
    } else {
        held.pos.x
    };
    for index in ahead {
        let tile = &mut result[index];
        if right {
            tile.pos.x = tile.pos.x.max(edge);
            edge = tile.pos.x + tile.size.x;
        } else {
            tile.pos.x = tile.pos.x.min(edge - tile.size.x);
            edge = tile.pos.x;
        }
    }
    (fits(held, bounds) && valid_result(held, &result, bounds)).then_some((held.pos, result))
}

#[cfg(test)]
mod tests;
