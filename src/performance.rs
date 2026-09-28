//! Opt-in CPU benchmarks; run with `cargo test performance -- --ignored --nocapture --test-threads=1`.
use std::time::Instant;

pub(crate) fn measure(label: &str, iterations: u32, mut operation: impl FnMut()) {
    for _ in 0..100 {
        operation();
    }
    let mut samples = Vec::with_capacity(31);
    for _ in 0..31 {
        let started = Instant::now();
        for _ in 0..iterations {
            operation();
        }
        samples.push(started.elapsed().as_secs_f64() * 1_000_000.0 / f64::from(iterations));
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{label}: median {:.3} us/op; p95 batch mean {:.3} us/op ({iterations} ops/batch, 31 batches)",
        samples[15], samples[29]
    );
}
