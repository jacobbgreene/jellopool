# Jellopool: a restrained typographic publishing system

**Status:** Proposal only; source-informed, not implemented.
No rendered prototype, contrast verification, or playtest has been completed.
This document authorizes no implementation and makes no historical authenticity claim.

## Recommended direction

Make the player's arrangement the publication: warm light paper, dark ink,
quietly bounded word tiles, and one subordinate piece of custom identity.
Borrow from Emigre's deliberate typographic relationships and its treatment of
the page as a designed composition rather than a neutral container. Concretely,
that attitude is the four commitments named below—consistent word typography,
content-sized rectangles, open space, and a fixed gutter wordmark—and nothing
beyond them. Within this system's exclusions, that expressiveness lives only in
the gutter wordmark, never in word tiles—a borrowing of attitude, not an
assertion that Emigre had one historical style.
Restraint here means a coherent frame for unpredictable play, not literary refinement.
Absurd phrases, fragments, jokes, and scattered compositions should feel equally welcome.
The player owns composition; the interface must not imply a preferred reading order.

The distinguishing move is the relationship between consistent word typography,
content-sized rectangles, open space, and a fixed gutter wordmark. Because word
typography never performs, expressiveness concentrates in one drawn mark and one
accent color; that concentration, not any surface color, is the identity.
A beige recolor alone would remain generic stationery; extra ornament is not the remedy.
If the wordmark fails its gate, the concentration discipline stands alone and the
system ships without a mark.

## Current baseline, verified in source

- `src/main.rs` loads `assets/fonts/EBGaramond-Regular.ttf` through `GameFont`.
- `src/tiles/mod.rs` builds Bevy UI tiles with uniform 20px text, ivory faces,
  warm dark ink, 3px brass borders, and 4px corner radii.
- Tile dimensions follow text plus 16px horizontal / 10px vertical padding per side,
  plus the 3px border on every side (Bevy `UiRect` values are per-edge);
  these are content-sized rectangles, not fixed square cells.
- The parchment writing zone and dark brown tray are both 78% of root width.
  Centering leaves existing side gutters of exactly 11% each.
- The tray reserves 23% of root height, with wrapping, 12px gaps, 20px padding,
  and a 6px radius; the writing zone grows into the remaining vertical space.
- `src/tiles/drag.rs` supplies pickup scale (1.06), shadow, an amber landing
  highlight (a distinct constant from the tiles' brass border), tray gap previews,
  return/reorder, and Escape cancellation.
- `src/tiles/placement.rs` plans neighbor pushes on a 40px grid and rejects
  plans that cannot fit. Drag preview and drop each call this planning path
  independently; drop recomputes at release rather than reusing the previewed cell.
  This is not an unrestricted-overlap board; preserve this current behavior.
- `src/word_bank.rs` randomly samples fixed per-category counts summing to exactly
  40 words and shuffles them.
- The window is non-resizable borderless fullscreen on the primary monitor.

These observations include existing dirty working-tree changes, not only committed code.
In particular, the placement-planning behavior (`src/tiles/placement.rs` and its
consumers in `drag.rs`) is uncommitted and untracked; commit before baseline capture.
They describe source behavior, not demonstrated runtime correctness.

## Visual system

### Paper, ink, and word bank
Use warm light paper for the composition field and a related, slightly darker
light neutral for the word bank. Replace the dark-desk framing with a quiet light
surround. Distinguish the bank by value and a fine neutral separator, not a label.
Implement the separator as an absolutely positioned, non-pickable overlay sibling
(the drag-layer pattern)—never a tray child, never an in-flow sibling with height,
and never a top or left border on the writing zone: placement math references the
zone's border box while absolutely positioned children inset from its padding box,
so a top/left zone border would add a constant preview-to-commit offset that outer
measurements cannot detect (see Technical boundaries).

Keep word faces subtly distinct from both surfaces and text consistently dark;
bounds come from outlines, so face separation may stay quiet.
Initial palette candidates: paper `#F3EFE5`, bank `#E6E0D4`, tile face between
them (candidate `#EEE9DE`), ink `#25231F`, outline `#969187`, and vermilion
`#C44732`. These are provisional comparison swatches, not verified contrast
values or a locked production palette. Brass and amber are retired as a decision:
their roles pass to the neutral outline and the vermilion preview.
Tune the face/surface separation and outlines together after rendering.

### Tiles and type
Retain EB Garamond at 20px for the initial comparison; use one font and size for
all words, regardless of category, length, or placement.
Keep content-sized width and height. “Nearly square” describes the corner
treatment only: reduce rounding toward a crisp rectangle, never force square
tile dimensions.

Recommend fine, perceptible neutral outlines rather than brass frames.
Do not rely on a barely visible face-color difference to communicate tile bounds.
A 1px border is a candidate only after compensating padding and checking rendering
at scale factor 1.0 and at least one fractional scale factor (e.g. 1.25 or 1.5),
where 1px strokes rasterize unevenly. If fine strokes disappear, first strengthen
the outline by darkening the neutral, not by thickening the stroke; a thicker
stroke changes the stage-2 padding compensation and must be re-derived, not
assumed. If no 1–2px weight is reliable, reject the fine-outline direction and
keep the stage-1 outline.
No individual word styling, random rotations, part-of-speech colors, textures,
decorative board text, or grunge effects.

### Interaction feedback
Reserve vermilion exclusively for the active landing preview: it appears only
while a drag hovers a valid board landing—never on resting tile borders, the
wordmark, tray cues, or any persistent element. It should identify the single
action the player is about to take; at rest, no vermilion is visible anywhere.
The tray gap preview remains a hueless geometric spacer; reorder is signaled by
motion alone. Any further vermilion use requires explicit approval.
Preview, neighbor push-preview, and drop commit are three separate `placement_at`
call sites in `drag.rs`; keep their inputs identical. The visible highlight eases
toward the plan target and the drop recomputes from the release pointer, so
consistency must be verified with both slow and fast releases.
An invalid plan shows no landing preview at all: the current behavior, in which
the highlight fades out when planning fails, is the invalid signal. Introduce no
second hue for invalid states, and never let an invalid plan gain a valid-looking
destination through styling.

Keep non-color pickup/lift and shadow cues, plus existing preview motion and geometry.
The active tile and landing location must remain understandable without hue alone.
Initially preserve animation timing and scale; do not bundle feel or mechanics tuning
with the palette comparison. Feedback should support play without dominating the words.

### One fixed identity exploration
Explore one custom `jellopool` wordmark with a coherent round-counter construction
across the o/o/p forms. This is a direction to draw and assess, not an approved
final letterform or permission to mix unrelated novelty glyphs. “Coherent” is
checkable: the same counter construction must repeat across the o, o, and p forms;
a gesture appearing in a single glyph is rejected as the novelty this proposal forbids.

Place it in one side gutter only with a stable, asymmetrical relationship to the
board edge; the stage-3 drawing must name the gutter and the vertical anchor
explicitly. Render it in the same ink as the words, never vermilion. Keep it
subordinate, non-pickable, and outside the composition area.
It must not take board space, become a header, or act like another movable word.
Should a resizable window ever be approved, omit the mark below the width where it
no longer fits the gutter rather than shrink the board; under the current fixed
fullscreen this clause requires no work.
Judge its character beside absurd and sparse arrangements as well as dense poems.

## Staged implementation proposal

Each stage requires separate approval to implement and a comparison before advancing.
Stage 0 (fixture, defined under Controlled validation) is the shared prerequisite:
no stage captures without it.

1. **Palette, radius, and feedback; geometry fixed.**
   Change surface/ink/outline colors and landing-feedback colors. Corner radius may
   be reduced no further than the retained 3px border width—Bevy renders the fill's
   inner radius as radius minus border, clamped at zero—so crisper corners are
   deferred to stage 2. Keep border thickness, padding, font, gaps, zone dimensions,
   and mechanics fixed. Radius still reshapes the pick region's corner cutouts, so
   include corners in picking checks. Update the snap highlight's own border/radius
   literals in `drag.rs` alongside the tile constants so the preview keeps the
   tile's silhouette.
   Capture matched before/after scenes, including mid-drag interaction states, only
   after the font has finished loading and measuring; assess legibility and
   interaction hierarchy.
2. **Fine borders with compensated padding.**
   Moving from 3px to 1px borders removes 4px of total width and height unless
   compensated. Add 2px padding on every side: horizontal 16→18px, vertical 10→12px.
   This also preserves the text inset (border plus padding is 19px horizontal and
   13px vertical before and after). Verify against stage 1: computed tile outer
   bounds and text positions; snap-highlight border weight and corner radius; tray
   wrap points and gap-preview dimensions; hit regions including rounded-corner
   cutouts; and placement predictions at board-edge cells, where the size-dependent
   snap clamp changes first. Check at scale factor 1.0 and at fractional factors;
   physical-pixel rounding may shift compensated bounds by ≤1px. Arithmetic alone
   is not validation. If fine strokes disappear or bounds drift beyond tolerance at
   any tested scale factor, revert to the stage-1 state (3px neutral outline,
   16/10px padding)—not to brass.
3. **Identity in the existing gutter.**
   Compare the single wordmark exploration with the same scene without it.
   Verify subordination, gutter fit, and non-picking before accepting its construction.
   Implementation shape: an absolutely positioned child of the board root (the
   drag-layer pattern), inserted before the drag layer, with explicit
   `Pickable::IGNORE`—Bevy's default `Pickable` blocks lower entities. A drawn mark
   is an image asset and needs the same conditional approval as font plumbing.
   Do not compensate for a weak mark with decoration or changes to word typography.
   Rejection ships stages 1–2 without a wordmark; the mark may be redrawn and
   re-gated once.

## Technical boundaries

- Concentrate approved styling in `src/tiles/mod.rs` and presentation constants
  in `src/tiles/drag.rs`; any necessary presentation-only wiring needs explicit scope.
  The drag-shadow color is currently a function-body literal in `drag.rs`, tuned
  against the dark surround, and likely needs retuning on light paper.
- Preserve `src/tiles/placement.rs` and all current placement/grid, neighbor-preview,
  rollback, tray-return, reorder, and cancellation behavior—including dirty changes.
  The 40px grid is duplicated as independent constants in `drag.rs` and
  `placement.rs`; do not let them diverge, and do not unify them under this proposal.
- No root padding or header changes: do not disturb drag coordinate assumptions,
  move the writing-zone origin, or reduce the available board area.
- Do not insert decorative children into the tray's indexing sequence or introduce
  pointer-blocking overlays. Tray slot math mixes tile-filtered and non-gap-filtered
  child counts with raw child indices, so a decorative tray child desynchronizes the
  gap preview from the committed drop. Keep tile text and all identity decoration
  non-pickable.
- Font assets and loading plumbing are conditional on later approval only.
  The font handle is cloned into every tile at spawn and nothing re-reads it
  afterward, so only a same-path asset swap is contained. A font change would
  require fresh geometry validation—re-measured tile bounds, tray wrap and row
  heights, reachable grid cells, push distances, and tray vertical overflow, which
  paints visibly rather than clipping—not a silent substitution.

## Controlled validation and decision gates

Freeze the **same words, tray order, tile positions, viewport, and DPI** for each
comparison. Independent random launches are not comparable evidence: word selection
samples and shuffles from an unseeded RNG, so a pinned word bank alone cannot freeze
tray order.

**Stage 0 — fixture (prerequisite to stage 1).** Add an explicitly approved,
test-only fixture: a seeded or fixed word list; a debug spawner that commits fixed
board scenes as word + 40px-grid coordinate pairs through the existing placement
path; and a window override in `src/main.rs` (windowed mode, explicit resolution,
optional scale-factor override) selected by the same flag. This fixture is the only
approved change outside `src/tiles/` in this proposal; it ships behind a feature or
environment flag and never affects normal launches. The headless lifecycle tests in
`src/tiles/drag.rs` already cover much of the interaction surface; run them per stage
so live time concentrates on rendering.

Record with each screenshot and live check: commit hash and working-tree state,
fixture id, window mode, logical viewport size, scale factor, and the stage constants
in effect. Captures from a dirty tree are labeled as such and retaken after commit.
Interactive resizing is never used as evidence.

Scenes: empty board, dense composition, scattered neighbor placement, long-word
40-tile tray, short/long adjacency, crowded placements at all board edges including
the zone/tray seam, one narrow-viewport capture, one fractional-scale capture, and
one matched mid-drag capture per interaction state (pickup, valid landing preview,
rejected drop).

For each stage, inspect screenshots **and** test live picking at text, tile edges,
and tile corners; landing predictions versus committed results, including a drop made
before the preview appears and both slow and fast releases; neighbor push/rollback;
regrabbing a tile during its snap animation; a second grab attempt while a drag is
active; tray return and reorder; invalid/edge drops; Escape from tray and board drags;
and rendering at scale factor 1.0 and at least one fractional factor.

**Aesthetic gate:** matched screenshots show zero vermilion at rest and the wordmark
fully gutter-contained; and a reviewer shown each scene cold, without design context,
names the player's arrangement—not the wordmark or chrome—as the screen's focus and
does not read the scattered or nonsense arrangement as a bug or broken render.
**Usability gate:** no new clipping, missed hits, misleading feedback, tray-wrap
regressions, or loss of usable board area; computed tile outer bounds and writing-zone
origin within ±1px of the prior stage's captures at the same scale factor (text
measurement is scale-dependent, so cross-DPI equality is not required). Both gates
must pass independently for a stage to advance, judged by the stage's approver
against the frozen baseline scenes; on disagreement the stage does not advance.
A failed gate reverts that stage to the prior approved state; stage 3 may proceed
only on stages it does not depend on. If the aesthetic gate fails only on identity,
the proposal settles at the stage-2 appearance with no wordmark.
Record baseline defects separately; do not redesign mechanics to hide them in this pass.
Known candidates: unclipped tray overflow at small fullscreen heights, and the unused
`snap_target` field, whose comment overstates its role in drop consistency.

## Decisions and remaining questions

Settled: player-led composition, uniform initial typography, content-sized tiles,
light value-separated bank, neutral outlines, vermilion exclusive to the active
landing preview, retirement of brass and amber, and no added ornament.
Open after rendering: exact surface and face values, reliable outline weight within
the bounded 1–2px envelope, vermilion strength, near-square radius (coupled to the
stage-2 border width), and the wordmark's final construction/anchor.
A later font comparison is optional and separately approved, not a prerequisite.

## Acceptance checklist

Each item is checked only after the corresponding stage's separate approval.

- [ ] Matched scenes with the full capture record (commit, fixture, viewport, scale
  factor) for all approved stages.
- [ ] Fine outlines and word text remain readable on both surfaces at tested scale
  factors, including one fractional factor.
- [ ] Computed tile bounds, zone bounds and origin, tray wrapping, and available board
  area match the prior stage within tolerance; tray children remain tiles and gaps only.
- [ ] Picking (text, edges, corners), landing predictions, neighbor placement,
  return/reorder, interrupted drags, and Escape pass.
- [ ] Identity is subordinate, gutter-contained, non-pickable, takes no board space,
  and reads as neither header nor movable tile.
- [ ] Vermilion appears only on the active landing preview; pickup and landing cues
  read without hue; animation timing and scale are unchanged.
- [ ] Aesthetic and usability gates pass independently; unresolved issues are documented.
