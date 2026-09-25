mod prelude;
use bevy::prelude::*;
mod board;
mod fixture;
mod states;
mod tiles;
mod word_bank;

use crate::{
    board::spawn_board,
    states::AppState,
    tiles::{GameFont, spawn_all_tiles, spawn_board_root},
    word_bank::{WordBank, load_word_bank, switch_to_playing_state},
};
use bevy::window::{MonitorSelection, WindowMode, WindowResolution};
use bevy_common_assets::ron::RonAssetPlugin;

fn load_game_font(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(GameFont(asset_server.load("fonts/EBGaramond-Regular.ttf")));
}

/// Primary window config.
///
/// Default (no env vars): byte-for-byte the historical behavior — a
/// non-resizable borderless fullscreen window on the primary monitor.
///
/// Test-only override: when `JELLOPOOL_WINDOWED` is set (any value), use a
/// fixed window with a 1600x900 LOGICAL viewport for reproducible fixtures.
/// `JELLOPOOL_SCALE_FACTOR` (f32) additionally forces the display scale
/// factor so the logical size stays 1600x900 at any DPI.
///
/// Bevy 0.19 API note (verified against bevy_window 0.19.1):
/// `WindowResolution::new(physical_width: u32, physical_height: u32)` takes
/// PHYSICAL pixels, and logical size = physical / scale_factor, where
/// `with_scale_factor_override(f32)` pins the scale factor. Hence physical =
/// logical * factor below.
fn primary_window() -> Window {
    if std::env::var("JELLOPOOL_WINDOWED").is_err() {
        return Window {
            resizable: false,
            mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
            ..default()
        };
    }
    const LOGICAL: Vec2 = Vec2::new(1600.0, 900.0);
    let scale_factor = std::env::var("JELLOPOOL_SCALE_FACTOR")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| *value > 0.0)
        .unwrap_or(1.0);
    Window {
        resizable: false,
        mode: WindowMode::Windowed,
        resolution: WindowResolution::new(
            (LOGICAL.x * scale_factor) as u32,
            (LOGICAL.y * scale_factor) as u32,
        )
        .with_scale_factor_override(scale_factor),
        ..default()
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(primary_window()),
            ..default()
        }),
        RonAssetPlugin::<WordBank>::new(&["ron"]),
    ))
    .init_state::<AppState>()
    .add_systems(Startup, (spawn_board, load_word_bank, load_game_font))
    .add_systems(
        Update,
        (
            switch_to_playing_state.run_if(in_state(AppState::Loading)),
            (
                tiles::cancel_drag_system,
                tiles::tile_follow_system,
                tiles::tile_feel_system,
                tiles::tray_gap_system,
                tiles::tray_gap_anim_system,
                tiles::zone_snap_highlight_system,
                tiles::snap_anim_system,
                tiles::push_preview_system,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
        ),
    )
    // Chained so the root (and its tray) exist before tiles spawn into it.
    .add_systems(
        OnEnter(AppState::Playing),
        (spawn_board_root, spawn_all_tiles).chain(),
    );

    // Stage-0 test-only fixture (see fixture.rs): registered only when
    // JELLOPOOL_SCENE names a scene; unset env var means normal launches get
    // exactly the systems above.
    if let Some(scene) = fixture::scene_from_env() {
        app.insert_resource(scene)
            // Ordered after spawn_all_tiles so the fixture arms only once
            // the scene's tiles exist.
            .add_systems(
                OnEnter(AppState::Playing),
                fixture::arm_fixture_scene.after(spawn_all_tiles),
            )
            .add_systems(
                Update,
                fixture::apply_fixture_scene.run_if(
                    in_state(AppState::Playing)
                        .and_then(resource_exists::<fixture::PendingFixture>),
                ),
            );
    }

    app.run();
}
