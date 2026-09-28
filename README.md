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

The normal window is borderless fullscreen. Drag tiles with the primary mouse
button (or touch) from the bottom tray onto
the centered writing sheet. Its 24 numbered lines scroll with the mouse wheel,
trackpad, or scrollbar. Words snap to a line vertically; horizontal placement is
free. Dragging pushes contacted words along their line, keeping their order and
stopping at the page edge. Pushed words stay where you move them, even if you
return the dragged tile to the tray; other lines never reflow.

The tray stays outside the scrolling sheet. It retains its usual minimum height
and grows when longer word selections need extra rows. Word-bank categories
are validated before play; missing or undersized data exits with an explicit
error instead of silently producing fewer than 40 tiles.

Hold either Shift key at any point to phase the held tile: it dims and passes
over words without pushing. Release Shift to resume pushing. A drop still makes
room for the word even while Shift is held. If inserting between words would
push a left neighbor off the page, the left-side words stay fixed: the held tile
moves just clear of them and pushes only the right-side chain. The intended word
order never changes to squeeze in a drop; if that insertion cannot fit, the held
tile returns to the tray.

The top strip has **New draft**, **Previous draft**, and **Next draft** controls,
the current draft's title/number, and save status. Line 1 starts at the screen's
vertical center; the whitespace and title scroll away with the page. Click the
title field (or press Tab) to name the poem. It supports selection, copy/paste,
and Unicode input, up to 120 characters on one line. Enter, Escape, or clicking
outside finishes editing. Controls also support Tab and Enter/Space.

Each draft keeps its title, actual word selection, placed tiles, and tray order.
New draft starts with a fresh selection; switching does not reshuffle existing
drafts. Changes autosave after a short pause, and the last active draft reopens on
launch. Switching cancels an unfinished drag and its preview pushes. Only committed
edits are saved. There is no delete button or submitted-poem collection yet.

Escape cancels the entire drag, including its pushes. Pointer cancellation and
focus loss likewise restore the held tile and all neighbors to their positions
before the gesture.

`Cargo.lock` is intentionally included: use `--locked` for repeatable dependency
resolution. Updating dependencies is a separate, deliberate operation.

## Local drafts and architecture

On Linux, drafts are stored in `$XDG_DATA_HOME/jellopool/drafts.ron`, or
`$HOME/.local/share/jellopool/drafts.ron` when XDG_DATA_HOME is unset. macOS uses
`~/Library/Application Support/jellopool`; Windows uses `%LOCALAPPDATA%/jellopool`.
Set `JELLOPOOL_DATA_DIR` to choose another directory. Saves are local, not cloud
backups. The current library allows up to 256 drafts.

Writes are debounced and run off the interaction thread, with a final flush on
normal exit. A force-kill or power loss can still lose edits since the last save.
Atomic replacement preserves a `drafts.ron.bak` previous-save backup. If recovery
uses that backup, a warning is shown and damaged primary bytes are archived before
the next save. Unsupported save versions, unreadable saves, or another running
instance disable saving for that session; the warning remains visible and existing
files are not overwritten. In that case, resolve the warning and relaunch before
doing work you need to keep. Ordinary write failures retain edits in memory and
retry automatically. Copy the entire data directory for a manual backup.

`JELLOPOOL_SCENE`, `JELLOPOOL_CAPTURE`, and `JELLOPOOL_SEED` runs are deliberately
temporary: they neither load nor save personal drafts, even with a data-directory
override. This keeps diagnostics reproducible and protects normal work.

`src/poems/` owns durable documents and storage. `src/tiles/scenes.rs` contains
actual, composable Bevy scene blueprints; interaction behavior remains in systems,
with `src/tiles/session.rs` adapting the active document to the workspace.
See [ROADMAP.md](ROADMAP.md) for the architecture and the **documented-only**
exploration/story direction. No adventure-space assets or gameplay are required
for this milestone.

## Checks and diagnostics

```bash
bash scripts/check.sh
# Once dependencies are cached:
CARGO_NET_OFFLINE=true bash scripts/check.sh
# Optional CPU benchmarks (not GPU/frame-rate measurements):
cargo test --locked performance -- --ignored --nocapture --test-threads=1
```

### Test organization

Tests remain private unit-test modules beside the code they exercise. Larger
suites split by behavior, with fixtures in a dedicated `harness.rs`:

- `src/test_support/`: deterministic clock, pointer input, camera/layout setup,
  and the timing helper. No feature-specific entities or expected outcomes.
- `src/tiles/drag/tests/`: lifecycle, ownership, cancellation, coordinates,
  Shift phasing, boundary insertion, and an opt-in performance suite, all using
  one drag harness.
- `src/tiles/tray/tests/`: real wrapping-layout checks and an opt-in performance
  suite, sharing a separate tray harness. `word_selection.rs` adds shipped-font
  shaping/layout checks for 1,024 seeds at three size/DPI combinations and for
  the widest valid category selections.
- `src/tiles/placement/tests/`: pure geometry checks and seeded randomized cases;
  CPU measurements live in their own `performance.rs`.
- `src/poems/tests/`: document invariants and isolated on-disk save/recovery tests.
  `src/poems/session/tests.rs` covers autosave timing, exit, and loading failures.
- `src/tiles/session/tests/`: scene composition and draft-switching integration,
  sharing a workspace harness.

Drag, tray, and writing behavior harnesses register the same interaction chain
as the game. Synthetic drag tests supply measured geometry and deliberately omit
layout-dependent tray animation; tray/writing tests run Bevy's real layout and
post-layout animation. Neither starts a renderer. Focused system unit tests may
register only the systems they are isolating. Title-input tests retain their
own editor setup because they exercise a different Bevy subsystem.

Keep reusable actions free of scenario-specific expectations. For example,
`release_at(point)` dispatches and flushes a drop without advancing time; the test
then asserts its expected positions and explicitly steps frames. Add shared
helpers only when multiple suites need them, not to make every fixture identical.

Run a focused suite by module path, for example:

```bash
cargo test --locked tiles::drag::tests::phase
cargo test --locked tiles::tray::tests::layout
```

### Rendered smoke checks

```bash
bash scripts/check-visuals.sh
# Optional explicit output directory (must not already exist):
CARGO_NET_OFFLINE=true bash scripts/check-visuals.sh /tmp/jellopool-review
```

This separate, opt-in command renders eight deterministic scenes plus a
fractional-scale phase scene. Each capture checks loaded text, tile geometry,
containment, and unintended overlaps, then saves a PNG and log. A failed capture
stops the command with a nonzero exit code; artifacts remain for diagnosis.
It requires a working graphics backend (hardware or software; use `nix develop`
for the Linux libraries). Driver variables such as `WGPU_BACKEND` and Vulkan ICD
selection are inherited, not hardcoded.

These are rendered smoke checks and human-review artifacts, **not pixel-baseline
comparisons**. They are separate from the fast correctness suite and the opt-in
CPU benchmarks above.

Optional local notes live in the ignored `docs/` folder and are not included in a
fresh clone: `docs/development.md` (development workflow), `docs/visual-review.md`
(visual direction), and `docs/performance-review.md` (the earlier freeform-board
baseline). The numbered-line planner replaces that baseline's cascade; run the
benchmark command above for current measurements.
