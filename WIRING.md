# Module Wiring — Next Steps

The `poetry_game` code was split from a single `src/main.rs` into separate modules.
The **code moved verbatim**; the **wiring** (module declarations, imports, visibility,
and plugin registration) is intentionally left to do by hand. This doc lists every step
needed to get back to a compiling build.

Current files:

```
src/
├── main.rs        // use block + fn main (registrations still inline here)
├── prelude.rs     // shared re-exports (partially filled)
├── states.rs      // AppState (pub, not yet registered)
├── word_bank.rs   // WordBank, WordBankHandle, load_word_bank, select_words
├── board.rs       // spawn_board
└── tiles/
    ├── mod.rs     // WordTile, spawn_all_tiles, spawn_word_tile
    ├── layout.rs  // create_tile_position
    └── drag.rs    // on_tile_drag, overlap, TileRange, get_tile_range
```

Work top-to-bottom; each step assumes the previous ones are done.

---

## Step 1 — Declare the modules

**`src/main.rs`** — add near the top (before `fn main`):

```rust
mod prelude;
mod states;
mod word_bank;
mod board;
mod tiles;
```

**`src/tiles/mod.rs`** — add at the top:

```rust
mod layout;
mod drag;
```

Without these, none of the new files are part of the crate.

---

## Step 2 — Fix `prelude.rs`

Two problems in the current prelude:

1. `pub use crate::word_tile::WordTile;` references a module that does not exist.
   `WordTile` lives in `crate::tiles`. Change it to:
   ```rust
   pub use crate::tiles::WordTile;
   ```
2. `use bevy::prelude::*;` is a private import, so it does nothing for other modules.
   If you want the prelude to also forward Bevy's prelude, make it a re-export:
   ```rust
   pub use bevy::prelude::*;
   ```

For any type re-exported from the prelude to resolve, that type must itself be visible
(see Step 4): `AppState` is already `pub`, `WordBank` is already `pub`, but `WordTile`
is currently private and must be made `pub`.

---

## Step 3 — Add imports per file

None of the moved files have `use` lines yet. Minimum needed:

| File | Add |
| --- | --- |
| `word_bank.rs` | `use bevy::prelude::*;` and `use rand::seq::{IndexedRandom, SliceRandom};` (for `.sample` / `.shuffle`). `serde::Deserialize` is already fully-qualified — no import needed. |
| `board.rs` | `use bevy::prelude::*;` and `use bevy::color::palettes::basic::BLACK;` |
| `tiles/mod.rs` | `use bevy::prelude::*;`, `use bevy::color::palettes::basic::{BLACK, WHITE};`, plus paths to the items it calls: `use crate::word_bank::{WordBank, WordBankHandle, select_words};`, `use layout::create_tile_position;`, `use drag::on_tile_drag;` |
| `tiles/layout.rs` | **Nothing** — it uses only `usize`/`f32`, no Bevy types. |
| `tiles/drag.rs` | `use bevy::prelude::*;` and `use super::WordTile;` |
| `main.rs` | `use crate::{board::spawn_board, word_bank::load_word_bank, tiles::spawn_all_tiles};` — see Step 5 if you move these into plugin fns instead. Also **remove** the now-unused `BLACK`, `WHITE`, and `rand::seq::*` imports. |

(If you prefer, route shared types through `crate::prelude::*` instead of direct paths
once the prelude is populated.)

---

## Step 4 — Bump visibility for cross-module use

Rust child modules can see their parents' private items, but not vice-versa, and sibling
top-level modules can't see each other's private items. The following need widening:

| Item | File | Change to | Why |
| --- | --- | --- | --- |
| `WordBankHandle` (struct **and** its `.0` field) | `word_bank.rs` | `pub(crate)` struct + `pub(crate)` field | `spawn_all_tiles` in `tiles` takes `Res<WordBankHandle>` and reads `word_bank_handle.0`. |
| `select_words` | `word_bank.rs` | `pub(crate)` | Called from `tiles::spawn_all_tiles`. |
| `WordTile` (struct) | `tiles/mod.rs` | `pub` | Re-exported via the prelude (Step 2). Its fields can stay private — only `tiles` and its child `drag` touch them. |
| `create_tile_position` | `tiles/layout.rs` | `pub(super)` | Called by the parent `tiles/mod.rs`. |
| `on_tile_drag` | `tiles/drag.rs` | `pub(super)` | Referenced by `spawn_word_tile` in the parent `tiles/mod.rs`. |

Leave private (only used within their own module): `load_word_bank`, `spawn_board`,
`spawn_all_tiles`, `spawn_word_tile`, `overlap`, `get_tile_range`, `TileRange` — provided
you adopt the plugin pattern in Step 5 (which keeps their references in-module).

---

## Step 5 — Plugin functions + registration

Per the Bevy convention (function-plugins for a binary, not `struct … impl Plugin`),
give each module a `pub(super) fn plugin(app: &mut App)` and move the matching
registration lines out of `fn main`.

**`word_bank.rs`:**
```rust
pub(super) fn plugin(app: &mut App) {
    app.add_plugins(RonAssetPlugin::<WordBank>::new(&["ron"]));
    app.add_systems(Startup, load_word_bank);
}
```
(This module now also needs `use bevy_common_assets::ron::RonAssetPlugin;`.)

**`board.rs`:**
```rust
pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_board);
}
```

**`tiles/mod.rs`:**
```rust
pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, spawn_all_tiles);
}
```

**`main.rs`** — replace the inline `add_plugins`/`add_systems` for the moved systems
with plugin registration. The window/pick plugins stay; your app systems move into the
new plugins:
```rust
.add_plugins((
    DefaultPlugins.set(WindowPlugin { /* unchanged */ }),
    MeshPickingPlugin,
    word_bank::plugin,
    board::plugin,
    tiles::plugin,
))
```
The `RonAssetPlugin`, `Startup`, and `Update` lines currently in `fn main` are now
redundant — delete them once the plugin fns own them.

---

## Step 6 — (Optional) register `AppState`

`states.rs` defines `AppState` but nothing registers it yet. To wire it in:
```rust
app.init_state::<AppState>();
```
This is the prerequisite for the deferred structural cleanups (state-gating
`spawn_all_tiles`, adding `Name` + `DespawnOnExit` to spawned entities). Those are
separate follow-ups — see the pending items below — not required just to compile.

---

## Step 7 — Verify

```
cargo check
```
Expect it to pass once Steps 1–5 are done. Then `cargo run`; behavior should be identical
to before the split.

---

## Deferred (not part of wiring — behavior changes, do separately)

These were intentionally NOT touched during the move:

- `spawn_all_tiles` runs every frame gated by a `Local<bool>` instead of
  `run_if(in_state(AppState::Playing))` + a `SystemSet`.
- Spawned entities (camera, background, tiles) have no `Name` and no
  `DespawnOnExit`/`StateScoped` component.
- Dead code in `on_tile_drag`: the empty `for tile in tiles_query.iter_mut() {}` loop and
  the unused `stored_tiles` vec (`tiles/drag.rs`).
