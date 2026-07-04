mod prelude;
use bevy::prelude::*;
mod board;
mod states;
mod tiles;
mod word_bank;

use crate::{
    board::spawn_board, tiles::spawn_all_tiles, word_bank::WordBank, word_bank::load_word_bank,
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
        .add_systems(Startup, (spawn_board, load_word_bank))
        .add_systems(Update, spawn_all_tiles)
        .run();
}
