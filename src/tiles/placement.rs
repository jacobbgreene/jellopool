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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "opt-in CPU measurement, not a timing assertion"]
    fn performance_placement_plans() {
        use crate::performance::measure;
        use std::hint::black_box;
        for count in [0, 10, 39] {
            let neighbors: Vec<_> = (0..count)
                .map(|index| TileRect {
                    entity: Entity::from_raw_u32(index + 1).unwrap(),
                    pos: Vec2::new((index % 13) as f32 * 80.0, (index / 13) as f32 * LINE_PITCH),
                    size: Vec2::new(80.0, 40.0),
                })
                .collect();
            for (label, pos) in [
                ("clear", Vec2::new(0.0, 10.0 * LINE_PITCH)),
                ("cascade", Vec2::ZERO),
            ] {
                let held = TileRect {
                    entity: Entity::from_raw_u32(0).unwrap(),
                    pos,
                    size: Vec2::new(80.0, 40.0),
                };
                let bounds = Vec2::new(1248.0, 693.0);
                let succeeds = plan(held, &neighbors, bounds).is_some();
                measure(
                    &format!("{count} neighbors / {label} / success={succeeds}"),
                    1000,
                    || {
                        black_box(plan(
                            black_box(held),
                            black_box(&neighbors),
                            black_box(bounds),
                        ));
                    },
                );
            }
        }
        let neighbors: Vec<_> = (0..39)
            .map(|index| TileRect {
                entity: Entity::from_raw_u32(index + 1).unwrap(),
                pos: Vec2::new((index % 13) as f32 * 80.0, (index / 13) as f32 * LINE_PITCH),
                size: Vec2::new(80.0, 40.0),
            })
            .collect();
        let held = TileRect {
            entity: Entity::from_raw_u32(0).unwrap(),
            pos: Vec2::ZERO,
            size: Vec2::new(80.0, 40.0),
        };
        let bounds = Vec2::new(1040.0, 3.0 * LINE_PITCH);
        assert!(plan(held, &neighbors, bounds).is_none());
        measure("39 neighbors / infeasible full board", 100, || {
            black_box(plan(
                black_box(held),
                black_box(&neighbors),
                black_box(bounds),
            ));
        });

        // Unequal measured widths and changing contact positions exercise more
        // than the best-case uniform rows. Generate once, outside the timer.
        use rand::{RngExt, SeedableRng, rngs::SmallRng};
        let mut rng = SmallRng::seed_from_u64(0xBEE5);
        let bounds = Vec2::new(1248.0, 693.0);
        let mut corpus = Vec::new();
        for _ in 0..1024 {
            let mut pos = Vec2::ZERO;
            let neighbors: Vec<_> = (1..40)
                .map(|id| {
                    let width = rng.random_range(120..760) as f32 / 4.0;
                    if pos.x + width > bounds.x {
                        pos = Vec2::new(0.0, pos.y + LINE_PITCH);
                    }
                    let tile = TileRect {
                        entity: Entity::from_raw_u32(id).unwrap(),
                        pos,
                        size: Vec2::new(width, 40.0),
                    };
                    pos.x += (width / GRID).ceil() * GRID;
                    tile
                })
                .collect();
            let contact = neighbors[rng.random_range(0..neighbors.len())];
            let held = TileRect {
                entity: Entity::from_raw_u32(0).unwrap(),
                pos: contact.pos,
                size: Vec2::new(rng.random_range(120..760) as f32 / 4.0, 40.0),
            };
            corpus.push((held, neighbors));
        }
        let mut index = 0;
        measure(
            "39 variable-width neighbors / 1024 seeded contact cases",
            1024,
            || {
                let (held, neighbors) = &corpus[index % corpus.len()];
                black_box(plan(
                    black_box(*held),
                    black_box(neighbors),
                    black_box(bounds),
                ));
                index += 1;
            },
        );
        // A mean hides uneven costs between contacts. Time each seeded case
        // repeatedly and report the distribution of per-case medians as well.
        let mut case_times = Vec::new();
        let mut rejected = 0;
        for (index, (held, neighbors)) in corpus.iter().enumerate() {
            let mut times = Vec::new();
            let mut succeeds = false;
            for _ in 0..9 {
                let started = std::time::Instant::now();
                succeeds = black_box(plan(
                    black_box(*held),
                    black_box(neighbors),
                    black_box(bounds),
                ))
                .is_some();
                times.push(started.elapsed().as_secs_f64() * 1_000_000.0);
            }
            times.sort_by(f64::total_cmp);
            rejected += usize::from(!succeeds);
            case_times.push((times[4], index));
        }
        case_times.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (slowest, index) = case_times[1023];
        println!(
            "Variable-width per-case medians: p50 {:.3}, p95 {:.3}, p99 {:.3}, max {:.3} us; slowest case {index}; {rejected}/1024 rejected",
            case_times[511].0, case_times[972].0, case_times[1013].0, slowest
        );
        let (held, neighbors) = &corpus[index];
        println!("Slowest seeded case: held={held:?}; neighbors={neighbors:?}");
    }
    fn rect(id: u32, x: f32, row: usize, width: f32) -> TileRect {
        TileRect {
            entity: Entity::from_raw_u32(id).unwrap(),
            pos: Vec2::new(x, row as f32 * LINE_PITCH),
            size: Vec2::new(width, 40.0),
        }
    }

    #[test]
    fn empty_space_and_touching_edges_never_shift_neighbors() {
        let neighbors = [
            rect(1, 0.0, 0, 80.0),
            rect(2, 240.0, 0, 80.0),
            rect(3, 80.0, 1, 160.0),
        ];
        let held = rect(0, 80.0, 0, 160.0);
        let result = plan(held, &neighbors, Vec2::splat(640.0)).unwrap();
        for tile in result {
            assert_eq!(
                tile.pos,
                neighbors
                    .iter()
                    .find(|other| other.entity == tile.entity)
                    .unwrap()
                    .pos
            );
        }
    }

    #[test]
    fn a_small_overlap_only_opens_the_missing_space() {
        let held = rect(0, 80.0, 1, 82.25);
        let neighbors = [rect(1, 160.0, 1, 80.0), rect(2, 300.0, 1, 80.0)];
        let result = plan(held, &neighbors, Vec2::splat(640.0)).unwrap();
        assert_eq!(
            result
                .iter()
                .find(|tile| tile.entity == neighbors[0].entity)
                .unwrap()
                .pos
                .x,
            162.25
        );
        assert_eq!(
            result
                .iter()
                .find(|tile| tile.entity == neighbors[1].entity)
                .unwrap()
                .pos
                .x,
            300.0
        );
        assert!(valid_result(held, &result, Vec2::splat(640.0)));
    }

    #[test]
    fn overlap_can_shift_left_without_repacking_gaps() {
        let held = rect(0, 150.0, 0, 80.0);
        let neighbors = [rect(1, 0.0, 0, 40.0), rect(2, 80.0, 0, 80.0)];
        let result = plan(held, &neighbors, Vec2::splat(640.0)).unwrap();
        assert_eq!(
            result
                .iter()
                .find(|tile| tile.entity == neighbors[1].entity)
                .unwrap()
                .pos
                .x,
            70.0
        );
        assert_eq!(
            result
                .iter()
                .find(|tile| tile.entity == neighbors[0].entity)
                .unwrap()
                .pos
                .x,
            0.0
        );
    }

    #[test]
    fn only_contacting_neighbors_on_the_same_line_can_follow_a_push() {
        let held = rect(0, 80.0, 1, 90.0);
        let neighbors = [
            rect(1, 160.0, 1, 80.0),
            rect(2, 240.0, 1, 80.0),
            rect(3, 160.0, 2, 80.0),
            rect(4, 400.0, 1, 80.0),
        ];
        let result = plan(held, &neighbors, Vec2::splat(640.0)).unwrap();
        for (before, x) in neighbors.iter().zip([170.0, 250.0, 160.0, 400.0]) {
            let after = result
                .iter()
                .find(|tile| tile.entity == before.entity)
                .unwrap();
            assert_eq!(after.pos, Vec2::new(x, before.pos.y));
        }
    }

    #[test]
    fn full_line_rejects_without_wrapping_or_mutating_input() {
        let held = rect(0, 0.0, 0, 80.0);
        let neighbors = [rect(1, 0.0, 0, 160.0), rect(2, 160.0, 0, 160.0)];
        assert!(plan(held, &neighbors, Vec2::new(320.0, 1000.0)).is_none());
        assert_eq!(neighbors[0].pos, Vec2::ZERO);
    }

    #[test]
    fn blocked_side_can_open_space_on_the_other_side_of_the_word() {
        let held = rect(0, 100.0, 0, 80.0);
        let neighbor = rect(1, 0.0, 0, 150.0);
        let result = plan(held, &[neighbor], Vec2::splat(640.0)).unwrap();
        assert_eq!(result[0].pos.x, 180.0);
    }

    #[test]
    fn invalid_arrangements_are_never_reported_as_success() {
        let held = rect(0, 0.0, 3, 80.0);
        let bounds = Vec2::splat(400.0);
        assert!(plan(held, &[rect(1, 360.0, 0, 80.0)], bounds).is_none());
        assert!(
            plan(
                held,
                &[rect(1, 80.0, 0, 80.0), rect(2, 120.0, 0, 80.0)],
                bounds
            )
            .is_none()
        );
        assert!(plan(rect(0, 0.0, 0, 500.0), &[], bounds).is_none());
    }

    #[test]
    fn randomized_plans_preserve_geometry_lines_and_empty_space() {
        use rand::{RngExt, SeedableRng, rngs::SmallRng};
        let mut rng = SmallRng::seed_from_u64(0xCA5CADE);
        let bounds = Vec2::new(804.0, LINE_COUNT as f32 * LINE_PITCH);
        for _ in 0..10_000 {
            let mut make = |id| {
                rect(
                    id,
                    rng.random_range(0..19) as f32 * GRID,
                    rng.random_range(0..8),
                    rng.random_range(120..640) as f32 / 4.0,
                )
            };
            let held = make(0);
            let mut neighbors = Vec::new();
            for id in 1..40 {
                let tile = make(id);
                if fits(tile, bounds) && neighbors.iter().all(|other| !overlaps(tile, *other)) {
                    neighbors.push(tile);
                }
            }
            let touching = neighbors.iter().any(|tile| overlaps(held, *tile));
            if let Some(result) = plan(held, &neighbors, bounds) {
                assert!(valid_result(held, &result, bounds));
                assert_eq!(result.len(), neighbors.len());
                for after in &result {
                    let before = neighbors
                        .iter()
                        .find(|tile| tile.entity == after.entity)
                        .unwrap();
                    assert_eq!(after.size, before.size);
                    assert_eq!(after.pos.y, before.pos.y);
                    if !touching || before.pos.y != held.pos.y {
                        assert_eq!(after.pos, before.pos);
                    }
                }
            }
        }
    }
}
