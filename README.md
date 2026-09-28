# Jellopool

A word-tile composition game built with Rust and Bevy.

## Getting started

### Prerequisites

- Rust 1.95 or newer (Bevy's minimum); validation currently uses Rust 1.98.1.
- A working graphics driver. Linux builds need the X11/Wayland and Vulkan
  development libraries; `nix develop` supplies them on NixOS. The Nix shell
  uses your system Rust toolchain rather than pinning its own.

### Building

```bash
cargo build --locked
```

### Running

```bash
cargo run --locked
```

The normal window is borderless fullscreen. Drag tiles from the fixed tray onto
the centered writing sheet. Its 24 numbered lines scroll with the mouse wheel,
trackpad, or scrollbar. Words snap to a line vertically; horizontal placement is
free. Empty space never reflows neighbors: only an overlapping drop opens the
needed space on that line, without moving words onto other lines. A full line
rejects the drop and returns the tile to the tray.

Escape cancels a drag. Pointer cancellation and focus loss also restore the
tile's original position.

`Cargo.lock` is intentionally included: use `--locked` for repeatable dependency
resolution. Updating dependencies is a separate, deliberate operation.

## Checks and diagnostics

```bash
bash scripts/check.sh
# Once dependencies are cached:
CARGO_NET_OFFLINE=true bash scripts/check.sh
# Optional CPU benchmarks (not GPU/frame-rate measurements):
cargo test --locked performance -- --ignored --nocapture --test-threads=1
```

The following notes are local-only in the ignored `docs/` folder.
For deterministic scenes, screenshots, and module boundaries, see
[development notes](docs/development.md). The current visual direction is
described in the [visual review](docs/visual-review.md).
The [performance review](docs/performance-review.md) records the earlier
freeform-board baseline. The numbered-line planner replaces that cascade;
run the benchmark command above for current measurements.
