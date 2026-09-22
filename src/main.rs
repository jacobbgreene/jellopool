mod prelude;
use bevy::prelude::*;
mod board;
mod states;
mod tiles;
mod word_bank;

use crate::{
    board::spawn_board,
    states::AppState,
    tiles::{spawn_all_tiles, spawn_board_root, GameFont},
    word_bank::{WordBank, load_word_bank, switch_to_playing_state},
};
use bevy::window::{MonitorSelection, WindowMode};
use bevy_common_assets::ron::RonAssetPlugin;

fn load_game_font(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(GameFont(asset_server.load("fonts/EBGaramond-Regular.ttf")));
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resizable: false,
                    mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                    ..default()
                }),
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
                    tiles::tile_follow_system,
                    tiles::tile_feel_system,
                    tiles::tray_gap_system,
                    tiles::tray_gap_anim_system,
                    tiles::zone_snap_highlight_system,
                    tiles::snap_anim_system,
                )
                    .run_if(in_state(AppState::Playing)),
            ),
        )
        // Chained so the root (and its tray) exist before tiles spawn into it.
        .add_systems(
            OnEnter(AppState::Playing),
            (spawn_board_root, spawn_all_tiles).chain(),
        )
        .run();
}
