mod prelude;
use bevy::prelude::*;
mod board;
mod states;
mod tiles;
mod word_bank;

use crate::{
    board::spawn_board,
    states::AppState,
    tiles::{move_tiles, spawn_all_tiles},
    word_bank::{WordBank, load_word_bank, switch_to_playing_state},
};
use bevy::window::{MonitorSelection, WindowMode};
use bevy_common_assets::ron::RonAssetPlugin;

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
            MeshPickingPlugin,
            RonAssetPlugin::<WordBank>::new(&["ron"]),
        ))
        .init_state::<AppState>()
        .add_systems(Startup, (spawn_board, load_word_bank))
        .add_systems(
            Update,
            (
                switch_to_playing_state.run_if(in_state(AppState::Loading)),
                move_tiles.run_if(in_state(AppState::Playing)),
            ),
        )
        .add_systems(OnEnter(AppState::Playing), spawn_all_tiles)
        .run();
}
