//! Gameplay composition and explicit system ordering.
mod animation;
mod drag;
pub(crate) mod placement;
mod presentation;
pub(crate) mod title;
mod tray;
pub(crate) mod writing;

use crate::prelude::*;
use animation::{
    phase_appearance_system, push_preview_system, snap_anim_system, tile_feel_system,
    zone_snap_highlight_system,
};
pub use drag::PlacedTile;
use drag::{
    ActiveDrag, PlacementPreview, cancel_drag_system, tile_follow_system, update_placement_preview,
};
use presentation::TILE_RADIUS;
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

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TrayAnimation;

pub struct TilesPlugin;

impl Plugin for TilesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(title::TitlePlugin)
            .add_systems(
                PreUpdate,
                (
                    presentation::fit_board_to_viewport,
                    presentation::fit_page_lead,
                )
                    .chain(),
            )
            .add_systems(
                OnEnter(AppState::Playing),
                (spawn_board_root, spawn_all_tiles).chain(),
            )
            .configure_sets(Update, TileInteraction.run_if(in_state(AppState::Playing)))
            .configure_sets(
                PostUpdate,
                TrayAnimation.run_if(in_state(AppState::Playing)),
            );
        register_interaction_systems(app);
        register_tray_animation(app);
    }
}

/// Shared by the game and behavior harnesses; state gating belongs to TilesPlugin.
fn register_interaction_systems(app: &mut App) {
    app.init_resource::<ActiveDrag>()
        .init_resource::<PlacementPreview>()
        .add_systems(
            Update,
            (
                scroll_input_system,
                cancel_drag_system,
                update_placement_preview,
                tile_follow_system,
                phase_appearance_system,
                tile_feel_system,
                tray_gap_system,
                zone_snap_highlight_system,
                snap_anim_system,
                push_preview_system,
                scrollbar_system,
            )
                .chain()
                .in_set(TileInteraction),
        );
}

/// Requires real layout each frame. Synthetic-geometry tests deliberately omit it.
fn register_tray_animation(app: &mut App) {
    app.add_systems(
        PostUpdate,
        tray_slide_system
            .in_set(TrayAnimation)
            .after(bevy::ui::UiSystems::Layout)
            .before(bevy::ui::UiSystems::PostLayout),
    );
}
