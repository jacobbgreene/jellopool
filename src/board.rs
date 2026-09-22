use bevy::prelude::*;

pub fn spawn_board(mut commands: Commands) {
    commands.spawn((Name::new("Camera"), Camera2d));
}
