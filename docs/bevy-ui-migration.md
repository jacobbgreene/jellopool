# Bevy UI migration: board and word tiles

**End state:** `Playing` owns one full-screen Bevy UI board with a wrapping word
tray and draggable UI tiles; mesh board/tile rendering and world-space drag
motion are gone, while loading and word selection remain unchanged.

**How to use this doc:** work the steps in order. Each step has three parts:

- **Do** — the concrete changes, in the order to make them.
- **Learn** — the Bevy UI concepts the step depends on. Read these *before*
  writing code; they are the background the step assumes.
- **Done when** — the check that lets you move on. Don't advance with a red
  check.

Steps 1–4 are one cutover: the app may not compile or look right between them.
Step 5 (drag) and step 6 (bounds) build on the UI tree once it renders.

## Key concept: UI entities vs. mesh entities

Everything so far has been world-space rendering: `Mesh2d` + `Transform` +
a `Camera2d`, positioned in pixels by hand. Bevy UI is a different system:

- UI entities are spawned as plain components — `Node` (layout box),
  `BackgroundColor`, `Text` — with **no mesh, no material, no `Transform`**.
- The `Camera2d` renders them automatically; you never position the camera.
- Sizes and positions are relative: `Val::Percent(100.0)` means "100% of my
  parent". The root node's parent is the viewport, so a 100%-sized root is
  full-screen **forever, including window resizes** — no `Window` queries, no
  resize systems.
- Parents arrange children: a parent with `flex_direction: Row` and
  `flex_wrap: Wrap` lays children out like words in a paragraph. Children with
  `position_type: Absolute` opt out and place themselves.
- Paint order comes from spawn order (later = on top) or explicit `ZIndex`.

## Step 1: Remove mesh rendering

**Do:**

- In `src/board.rs`: keep only the persistent `Camera2d` in `Startup`. Delete
  `BoardLayout` and its `Window` query — UI bounds come from measured nodes
  later (step 6), not window math.
- In `src/tiles/mod.rs`: delete the mesh/material/`Transform` pieces of
  `spawn_word_tile` and `TileMotion`, `move_tiles`, and the mesh imports
  (`Mesh2d`, `MeshMaterial2d`, `ColorMaterial`, `Text2d`).
- In `src/main.rs`: remove `move_tiles` from `Update`.
- Leave `spawn_all_tiles`'s call to `select_words` untouched — it already
  supplies 40 random words, and word selection does not change.
- `src/tiles/layout.rs` and its `create_tile_position`: keep for now if you
  want the app compiling step-by-step; delete it when the last caller is gone
  (its centered-world math has no place in UI).

**Learn:** this is intentionally a deletion-only step. Resist replacing
anything yet — the goal is to make the mesh path *gone* so the UI tree you
build next is the only board, not a parallel one.

**Done when:** `cargo check` shows no mesh-rendering types remain in
`board.rs` / `tiles/` (temporary dead-code warnings on `WordTile` are fine).

## Step 2: Spawn the full-screen root with a background

**Do:**

- Add a system (e.g. `spawn_board_root`) registered with
  `OnEnter(AppState::Playing)` — **not** `Startup`.
- Spawn one root entity carrying:
  - `Name::new("BoardRoot")`
  - `DespawnOnExit(AppState::Playing)`
  - `Node` sized `Val::Percent(100.0)` in both dimensions
  - a deliberate `BackgroundColor`
- Run the app. You should see a solid full-screen color.

**Learn:**

- *Why `OnEnter(Playing)` instead of `Startup`:* the root owns
  `DespawnOnExit(Playing)`, so leaving the state destroys it. Spawning at
  `Startup` would mean the first entry works but every later re-entry has no
  board. `OnEnter` guarantees a fresh root per entry, beneath the same
  persistent camera (which is why the camera must **not** get
  `DespawnOnExit`).
- *Why no `Window` query:* your old stub read `window.width()/height()` to
  size things. UI nodes sized in `Val::Percent` are recomputed by the layout
  engine every frame; querying the window re-introduces exactly the stale
  world-space thinking this migration removes.
- *Minimal shape:* a Bevy UI entity is just
  `commands.spawn((Node { .. }, BackgroundColor(..)))` — a bundle of plain
  components, like mesh entities but with `Node` where `Transform` used to be.

**Done when:** the app shows your background color full-screen on entering
`Playing`, with no window-size code involved.

## Step 3: Build the region tree

**Do:** add children under the root (`.with_children(...)`), in this order:

1. a **composition region** — the open area where placed tiles will live;
2. a **tray region** — a flex row with `flex_wrap: Wrap`, a `gap`, and
   `padding`;
3. a **drag layer** — an absolutely positioned node covering the screen, above
   both regions, with `Pickable::IGNORE` on the container so it never blocks
   pointer targeting of tiles underneath.

**Learn:**

- *Stacking:* later-spawned siblings paint on top. Spawning the drag layer
  last puts it above the tray and composition without any Z math.
- *Why an ignored drag layer:* picking walks the paint order top-down; an
  opaque layer over the screen would swallow every click. `Pickable::IGNORE`
  makes the layer and its decorative parts transparent to the pointer while
  still rendering (and its dragged-tile children get their own pickable
  overrides).
- *Tray as layout, not math:* the tray's flex row + wrap replaces
  `create_tile_position`. You will not compute tile positions for idle tiles
  at all — the tray arranges them, which is why varied word lengths wrap for
  free.

**Done when:** the regions are visible (give them temporary distinct
background colors while building — remove later), tray sits where you want it,
composition fills the rest.

## Step 4: Spawn words as UI tiles

**Do:**

- In `spawn_all_tiles`, keep `select_words`; replace the whole
  position-computing loop with: for each word, spawn a tile **as a child of
  the tray**.
- A tile is a `Node` with `padding`, a `BackgroundColor`, and a `Text` child
  (UI `Text`, not `Text2d`). Let padding + text size the tile — do not derive
  width from `word.len()`.
- Put `Pickable::IGNORE` on the text child so the outer tile is the drag
  target.
- Reattach the drag observers to the tile entity (they get rewritten in
  step 5; wiring them now keeps the structure in one place).
- Give the root `DespawnOnExit` ownership of everything by parenting: all of
  steps 2–4 are one tree under `BoardRoot`.

**Learn:**

- *Parenting is the ownership model:* `DespawnOnExit` on the root removes the
  whole subtree. You never despawn tiles individually on state exit.
- *Normal flex participation:* tray tiles have no position fields at all.
  Absolute positioning is reserved for tiles in the drag layer or composition
  (step 5).

**Done when:** short and long words wrap in the tray, every word is readable,
and clicking selects the tile rather than its text child.

## Step 5: Replace world drag with UI drag

**Do:** rewrite `src/tiles/drag.rs` around UI entities and parent-relative UI
offsets. Delete `TileMotion`, `move_tiles`, and all `Transform` scale/Z
handling together.

- Store each tile's logical position (its top-left offset) separately from its
  measured extent. Rename or remove `WordTile.position`; today it is an
  initial world position, not a tile size.
- **Drag start:** reparent the tile to the drag layer and raise it with UI
  stacking. Preserve its visible screen position during reparenting by
  converting the old parent-relative top-left through screen space into the
  drag layer's coordinates.
- **Dragging:** convert `Pointer<Drag>::delta` from window physical pixels to
  UI logical units by dividing by the active `UiScale` (UI X grows right, UI Y
  grows **down** — no mesh Y inversion), then add to the logical top-left.
  Adding `event.delta` directly is only valid at `UiScale` 1.0.
- **Release:** choose a minimal visual policy — release over the composition
  places the tile absolutely; otherwise return it to the tray (normal flex,
  tray rearranges intentionally, so screen position is not promised there).
- Use absolute positioning only in the drag layer or composition; returning to
  the tray restores flex participation.
- If you want a held appearance, use UI styling (e.g. a different
  `BackgroundColor`); do not port the old transform-easing/scale path.

**Learn:**

- *Reparenting moves the coordinate frame:* a tile at `(10, 10)` in the tray
  is at a different screen point than `(10, 10)` in the drag layer. The
  screen-space round-trip (old parent → screen → new parent) is what keeps the
  tile visually glued to the pointer across the move.
- *Physical vs. logical units:* pointer events report physical pixels; UI
  layout works in logical units scaled by `UiScale`. Every step that mixes
  them needs the divide-by-`UiScale` conversion, or dragging drifts at
  non-default scale.

**Done when:** a tile follows the pointer without a reversed Y axis, stays
visible while moving between parents, paints above other tiles while dragged,
and returns to normal tray layout when dropped outside the composition.

## Step 6: Measure and clamp in one coordinate system

**Do:** add drag bounds in `src/tiles/drag.rs`, after UI layout has produced
measurements.

- Read actual extents from `ComputedNode` (tile and destination parent). Never
  use the old `WordTile.position` as a clamp size.
- Keep stored offsets and pointer deltas in logical UI units. `ComputedNode`
  measurements are physical pixels — convert with the inverse `UiScale` factor
  before comparing against logical offsets.
- Clamp an absolute tile's top-left to the usable parent rectangle:
  `0..parent_width - tile_width` and `0..parent_height - tile_height`, after
  your chosen padding/insets.
- If the tile is larger than its usable region, or either measurement is
  unavailable, keep the last valid position (and return/retain the tile in the
  tray) — never clamp against an inverted range.
- Fold any intentional visual overflow (e.g. a shadow) into the usable-bounds
  policy deliberately.

**Learn:** the old `BoardLayout` stored `±window.width()` / `±window.height()`
— not even centered half-extents — so it was never a valid bounds source.
`ComputedNode` is authoritative because it is what the layout engine actually
produced, at the current scale, this frame.

**Done when:** fitting tiles stay inside composition bounds at every edge and
corner, oversized tiles behave deliberately, and drag/clamp stay aligned at
non-1x DPI or non-default `UiScale`.

## Step 7: Lifecycle and regression pass

**Do:**

- Preserve the loading gate exactly: `switch_to_playing_state` runs only while
  `Loading` and requests `Playing` after the word bank is available. No
  transition loop, no word-selection changes.
- Confirm the `OnEnter(Playing)` spawn has one owner root and every UI entity
  is its descendant.
- Remove dead mesh-picking observers/components and mesh-specific constants
  once UI drag is wired. Keep `MeshPickingPlugin` only if the final UI pointer
  path still requires it; otherwise remove it with the old mesh path.
- Do not add menus, finish behavior, semantic poem ordering, scoring, or
  drop-rule changes here.

**Done when:** no mesh board/tile code or transform-motion path remains, and
repeated `Playing` entries create one clean UI board with one selected word
set.

## Verification

Run after each step, and in full at the end:

```text
cargo fmt --check
cargo check
cargo test
```

Then use a dev-only state switch (the app has no UI to leave `Playing` yet) to
manually verify:

- resize/layout behavior where the window configuration permits it;
- tray wrapping for varied word lengths;
- text-child picking and overlapping-tile stacking;
- drag direction, edge/corner bounds, long-tile handling, and non-1x DPI/UI
  scale;
- leave and re-enter `Playing` without stale roots, tiles, or cameras.

## After migration

1. Add a visible menu and a free-verse loop: start, arrange, explicit **Finish
   poem**, then results. Preserve the finished visual arrangement as a session
   snapshot; restart/menu must clean up session UI.
2. Add semantic ordered lines and stanzas before rule checking or dependable
   readable text. Make clear whether a drag changes reading order or only
   visual placement.
3. Add one feasible required-word challenge beside rule-free free verse. Always
   supply required words, state the exact neutral condition, allow an unmet
   finish, and never score artistic quality.
4. Add further selectable challenges one at a time, with guaranteed vocabulary
   and transparent completion wording. Refrain selection/suggestions may be a
   constrained, optional experiment; make reuse and vocabulary effects explicit.
5. Add repetition/form activities only after semantic data is reliable. Label
   simplified exercises accurately; distinguish generic repetition from an
   authentic ghazal (including its couplets and rhyme/refrain conventions), and
   keep feedback factual rather than aesthetic.
6. Add optional, skippable contextual lessons and later a browsable library.
   Consider save/export, accessibility, vocabulary breadth, and opt-in
   progression only after the core loops are stable.
