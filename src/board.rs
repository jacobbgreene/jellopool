use crate::states::AppState::Playing;
use bevy::color::palettes::basic::BLACK;
use bevy::{prelude::*, window};

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
    window_query: Query<&Window>,
) {
    commands.spawn((Name::new("Camera"), Camera2d));
    commands.spawn((
        Name::new("Background"),
        DespawnOnExit(Playing),
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(materials.add(Color::from(BLACK))),
    ));

    let Ok(window) = window_query.single() else {
        return;
    };

    let width = (window.width() / 2.) as f32;
    let height = (window.height() / 2.) as f32;

    commands.insert_resource(BoardLayout {
        min: Vec2::new(-width, -height),
        max: Vec2::new(width, height),
    });
}
