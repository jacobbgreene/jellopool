# Jellopool: growing beyond the writing desk

## Direction

The writing desk is the first part of a possible 2D adventure inspired by the
end of the author's grandfather's life. An elderly protagonist (working names:
Langston or Tuesday) lives in a care facility, carrying grief and uncertain
memories of a partner. A visiting poetry teacher offers a way into writing;
relationships with residents, staff, and family give the poems personal meaning.

The proposed longer arc is to collect 24 poems, keep unfinished work as drafts,
and eventually choose favorite poems for a reading attended by people the player
has invited. A facility library could teach forms and terminology; optional daily
challenges could introduce writing constraints. Reconnecting with family, and
whether to leave the facility with them, are possible later story choices.

These are design ideas, not implemented rules. In particular, completing or
scoring poems should not be treated as a medical test, a cure, or proof that the
protagonist has earned care or companionship. There is no poetry grading now.

## 1. Durable poems — implemented

`src/poems/` owns versioned document data and a locally saved draft library.
Documents contain stable poem/tile identities, the actual selected words, a title,
logical line positions, and ordered tray membership. They contain no Bevy entity
IDs, layout measurements, pointer state, animations, or UI hierarchy.

The workspace projects one active document into entities. New/Previous/Next
controls switch documents; each has its own word selection. Switching cancels an
unfinished drag rather than accepting a preview as a completed edit. Committed
changes autosave, and the active draft reopens on launch. No deletion, submission,
24-poem quota, or draft-status progression has been added yet.

## 2. Real reusable Bevy scenes — implemented

`src/tiles/scenes.rs` defines composed Bevy `Scene` blueprints with `bsn!`:
the workspace, writing page, numbered lines, scrollbar, and individual word tiles.
The title editor and draft toolbar are composed scenes in their owning modules.
These are actual scene definitions instantiated by Bevy, not merely a folder named
“workspace.” A scene can be included in another scene and its components patched:

```rust,ignore
bsn! {
    word_tile("remember".into(), font, TileStyle::default())
    Node { border_radius: BorderRadius::all(px(6)) }
}
```

Scene parameters configure content and visual style. The shared logical paper
format (24 lines, line pitch, writing width) is a document-format concern;
changing it requires an explicit save migration, not an unrelated styling edit.
Bevy 0.19's Rust-defined scenes are used here. No external `.bsn` asset loader,
editor, or serialized ECS world has been introduced.

Behavior remains in systems: dragging, collision planning, phasing, cancellation,
scrolling, text editing, and save/switch coordination. Saving a poem does not save
its scene. Only one interactive workspace is instantiated at a time; the other
drafts remain plain data. Later, entering/exiting the writing activity can reuse
this same document-to-scene boundary.

## 3. Exploration spaces — planned only

Wait for an initial art direction and a small set of usable assets. Start with one
room and one interaction rather than building the entire facility. Potential
places include the protagonist's room, the poetry-class space, shared areas, and
the library.

Room scene blueprints should describe environment composition: sprites, static
props, collision shapes, spawn points, and interaction markers. Shared systems
should handle movement, camera behavior, interactions, and room transitions.
Stable room/NPC identifiers belong in game data; runtime entity IDs do not.

At that point introduce an activity/state boundary for exploration versus writing.
Do not stretch the writing UI's singleton queries into multiple simultaneously
interactive workspaces. Keep background worlds or inactive activities from
receiving writing input.

There is deliberately no player movement, room map, NPC, dialogue, or placeholder
exploration implementation in this milestone.

## 4. Narrative progression — after exploration

Only once a small exploration loop works, add a separate persistent story model:
day/progression, relationships, invitations, dialogue choices, and the eventual
reading. It can reference poem IDs; it should not inspect the UI to determine
what the player wrote. Add collection/submission/trash rules, draft selection for
the final reading, and library challenges deliberately rather than encoding them
in today's scene or drag components.

Future save changes need versioned migrations and preservation of older files.
The current local draft library is not yet a whole-adventure save format.
