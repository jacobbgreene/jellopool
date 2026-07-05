use crate::states::AppState::Playing;
use bevy::color::palettes::basic::BLACK;
use bevy::prelude::*;

#[derive(Resource)]
pub(crate) struct BoardLayout {
    min: Vec2,
    max: Vec2,
}

impl BoardLayout {
    pub(crate) fn clamp_tile(&self, pos: Vec2, size: Vec2) -> Vec2 {
        let half = size * 0.5;
        Vec2::new(
            pos.x.clamp(self.min.x + half.x, self.max.x - half.x),
            pos.y.clamp(self.min.y + half.y, self.max.y - half.y),
        )
    }
}

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
    commands.insert_resource(BoardLayout {
        min: Vec2::new(-2000., -2000.),
        max: Vec2::new(2000., 2000.),
    });
}
