use bevy::prelude::*;

// Helps me set what mode the game is in
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Loading,
    // Placeholder for the future menu screen; nothing routes here yet.
    #[allow(dead_code)]
    MainMenu,
    Playing,
}
