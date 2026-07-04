use bevy::color::palettes::basic::BLACK;
use bevy::prelude::*;

use crate::states::AppState::Playing;

pub fn spawn_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((Name::new("Camera"), Camera2d));
    commands.spawn((
        Name::new("Background"),
        DespawnOnExit(Playing),
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(materials.add(Color::from(BLACK))),
    ));
}
