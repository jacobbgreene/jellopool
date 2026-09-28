//! Gameplay composition and explicit system ordering.
mod animation;
mod drag;
pub(crate) mod placement;
mod presentation;
mod tray;
pub(crate) mod writing;

use crate::prelude::*;
use animation::{
    push_preview_system, snap_anim_system, tile_feel_system, zone_snap_highlight_system,
};
pub use drag::PlacedTile;
use drag::{PlacementPreview, cancel_drag_system, tile_follow_system, update_placement_preview};
use presentation::TILE_RADIUS;
#[cfg(test)]
use presentation::{get_board_tray, get_drag_layer};
pub use presentation::{spawn_all_tiles, spawn_board_root};
use tray::{tray_gap_system, tray_slide_system};
use writing::{scroll_input_system, scrollbar_system};

#[derive(Component)]
pub struct BoardTray;
#[derive(Component)]
pub struct WritingZone;
#[derive(Component)]
pub struct DragLayer;
#[derive(Component)]
pub struct WordTile;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TileInteraction;

pub struct TilesPlugin;

impl Plugin for TilesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlacementPreview>()
            .add_systems(PreUpdate, presentation::fit_board_to_viewport)
            .add_systems(
                OnEnter(AppState::Playing),
                (spawn_board_root, spawn_all_tiles).chain(),
            )
            .add_systems(
                Update,
                (
                    scroll_input_system,
                    cancel_drag_system,
                    tile_follow_system,
                    update_placement_preview,
                    tile_feel_system,
                    tray_gap_system,
                    zone_snap_highlight_system,
                    snap_anim_system,
                    push_preview_system,
                    scrollbar_system,
                )
                    .chain()
                    .in_set(TileInteraction)
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(
                PostUpdate,
                tray_slide_system
                    .after(bevy::ui::UiSystems::Layout)
                    .before(bevy::ui::UiSystems::PostLayout)
                    .run_if(in_state(AppState::Playing)),
            );
    }
}
