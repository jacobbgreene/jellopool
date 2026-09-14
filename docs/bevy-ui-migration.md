# Bevy UI migration: board and word tiles

**End state:** `Playing` owns one full-screen Bevy UI board with a wrapping word
tray and draggable UI tiles; mesh board/tile rendering and world-space drag
motion are gone, while loading and word selection remain unchanged.

## 1. Replace the current board and tiles in one cutover

Make this the first implementation change—do not add a mini-panel or run mesh
and UI boards in parallel.

- `src/board.rs` / `spawn_board`: keep only the persistent `Camera2d` in
  `Startup`; remove the mesh background and `BoardLayout` resource.
- `src/main.rs`: keep `load_word_bank` at `Startup`, keep
  `switch_to_playing_state.run_if(in_state(AppState::Loading))`, and replace the
  mesh tile update registration with the UI systems introduced below.
- `src/tiles/mod.rs` / `spawn_all_tiles`: continue calling `select_words`; it
  already supplies 40 random words. Replace every mesh tile spawn with one
  `Playing`-owned UI tree.
- Give the root `DespawnOnExit(AppState::Playing)`. Its descendants must be the
  board background, composition area, tray, and drag layer, so state exit
  removes them together.
- Do not attach `DespawnOnExit` to the startup camera. A later `Playing` entry
  must get a fresh board root beneath the same camera.
- Remove mesh assets/components and their imports: `Mesh2d`, `MeshMaterial2d`,
  `ColorMaterial`, mesh `Text2d`, and tile `Transform` rendering.
- Remove `src/tiles/layout.rs` once its centered-world `create_tile_position`
  has no callers.

Step 1 is the cutover boundary implemented by the following steps, not an
independent done milestone: its completion is verified by the lifecycle and
regression checks in step 5.

## 2. Build the UI tree and static tray

In `src/tiles/mod.rs`, create named UI entities under the board root:

1. a full-screen root with intentional background color;
2. a composition region for absolutely placed tiles;
3. a tray region using flex row wrapping, gap, and padding;
4. an absolute drag layer above both regions. Set the drag-layer container's
   `Pickable` to `Pickable::IGNORE` so it does not block pointer targeting of
   underlying tiles.

- Spawn each word as a UI tile node with UI text as a child. Let padding and
  text determine its normal size; do not derive width from `word.len()`.
- Put an ignored picking behavior on text and decorative children so the outer
  tile is the drag target.
- Keep idle tray tiles in normal flex layout. The tray, not
  `create_tile_position`, determines their initial arrangement.
- Assign UI stacking deliberately: the drag layer must render above tray and
  composition; a dragged tile must be above siblings in that layer.
- Treat tray and composition as visual regions only. Do not add new drop rules
  in this migration.

**Done:** short and long words wrap in the tray, every visible word is readable,
and pointer targeting selects the tile rather than its text child.

## 3. Replace world drag with UI drag

Rewrite `src/tiles/drag.rs` around UI entities and parent-relative UI offsets;
delete `TileMotion`, `move_tiles`, `Transform` scale/Z handling, and
`src/board.rs::BoardLayout` usage together.

- Store each tile's logical position separately from its measured extent. Rename
  or remove `WordTile.position`; it is an initial world position today, not tile
  size.
- On drag start, move/reparent the tile to the drag layer and raise it using UI
  order/stacking. Preserve its visible screen position during reparenting by
  converting the old parent-relative top-left position through screen space into
  the new parent's coordinates.
- On drag, convert the pointer delta from window physical pixels to UI logical
  units by dividing by the active `UiScale` (with UI X growing right and UI Y
  growing down), then add it to the logical top-left position. Do not imply that
  adding `event.delta` directly works at arbitrary UI scale or retain the mesh Y
  inversion.
- On release, the current release only resets scale/Z; migration must choose a
  minimal visual policy: release onto the composition places the tile absolutely
  and clamps it, otherwise return it to the tray. This is an implementation
  policy, not a new acceptance rule. Preserve the displayed position during
  drag-layer/absolute reparenting; returning to flex intentionally lets the tray
  rearrange, so screen position is not promised there.
- Use absolute positioning only for tiles in the drag layer or composition.
  Returning a tile to the tray restores normal flex participation.
- Use UI styling for the held appearance if wanted; do not port the old
  transform-easing/scale path merely because it exists.

**Done:** a tile follows the pointer without a reversed Y axis, remains visible
while moving between parents, paints above other tiles while dragged, and
returns to normal tray layout when returned there.

## 4. Measure and clamp in one coordinate system

Add the drag bounds logic in `src/tiles/drag.rs` after UI layout has produced
measurements.

- Use `ComputedNode` for actual tile and destination-parent extents. Do not use
  the old `WordTile.position` as a clamp size.
- Keep stored offsets and pointer deltas in logical UI units. Convert
  `Pointer<Drag>::delta` window physical pixels to logical UI offsets by
  dividing by `UiScale`. Convert `ComputedNode`'s physical-pixel measurements to
  UI logical units using the inverse scale factor before comparing them with
  logical offsets; do not imply that direct `event.delta` addition is valid for
  arbitrary `UiScale`.
- Clamp an absolute tile's top-left position to the usable parent rectangle:
  `0..parent_width - tile_width` and `0..parent_height - tile_height`, after
  applying chosen padding/insets.
- If a tile is larger than its usable region or either measurement is
  unavailable, keep the last valid position and return/retain it in the tray;
  never clamp against an inverted range.
- Include any intentional visual overflow (for example, a shadow) in the chosen
  usable bounds policy.
- Remove the old `BoardLayout` calculation entirely. It currently stores
  `±window.width()` and `±window.height()` rather than centered half-extents, so
  it is not a valid replacement for UI bounds.

**Done:** fitting tiles remain inside composition bounds at every edge and
corner, long tiles behave deliberately, and drag/clamp behavior remains aligned
at non-1x DPI or non-default UI scale.

## 5. Lifecycle and regression pass

Keep migration scope limited to presentation and interaction.

- Preserve the current loading gate: `switch_to_playing_state` runs only while
  `Loading` and requests `Playing` after the word bank is available. Do not add
  a transition loop or change word selection during this work.
- Confirm the `OnEnter(AppState::Playing)` board/tile spawn has one owner and
  all of its UI is descendant-owned by that root.
- Remove dead mesh-picking observers/components and mesh-specific constants once
  UI drag is wired. Keep `MeshPickingPlugin` only if the final UI pointer path
  still requires it; otherwise remove it with the old mesh path.
- Do not add menus, finish behavior, semantic poem ordering, scoring, or
  drop-rule changes here.

**Done:** no mesh board/tile code or transform-motion path remains, and repeated
`Playing` state entries create one clean UI board with one selected word set.

## Verification

For this documentation-only plan, no build is required. When implementing it,
run:

```text
cargo fmt --check
cargo check
cargo test
```

Then use a dev-only state switch (because the app has no current UI to leave
`Playing`) to manually verify:

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
   readable text. Make clear whether a drag changes reading order or only visual
   placement.
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
