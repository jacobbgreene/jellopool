use super::super::*;

#[test]
#[ignore = "opt-in CPU measurement, not a timing assertion"]
fn performance_placement_plans() {
    use crate::test_support::performance::measure;
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
