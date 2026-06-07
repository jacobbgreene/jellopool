use std::hash;

use bevy::{
    color::palettes::basic::{BLACK, WHITE},
    prelude::*,
    window::WindowResolution,
};
use hashbrown;
use random_word::Lang;

const _MAX_BOARD_WIDTH: f32 = 2000.;
const _MAX_BOARD_HEIGHT: f32 = 2000.;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: WindowResolution::new(1080, 1440).with_scale_factor_override(1.0),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, spawn_board)
        .add_systems(Startup, spawn_all_tiles)
        .run();
}

//TODO: Make these draggable
#[derive(Component)]
struct WordTile {
    id: f32,
    unique_word: String,
    pos_x: f32,
    pos_y: f32,
}

fn spawn_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(materials.add(Color::from(BLACK))),
        Transform::default().with_scale(Vec3::new(_MAX_BOARD_WIDTH, _MAX_BOARD_HEIGHT, 1.)),
    ));
}

fn spawn_all_tiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut word_tile_collection: Vec<WordTile> = Vec::new();
    for i in 0..35 {
        let word = random_word::get(Lang::En);
        let tile_position: (f32, f32);
        if word_tile_collection.is_empty() != true {
            let last_tile = word_tile_collection.last().unwrap();
            let prev_word_len = last_tile.unique_word.len();
            let prev_word_pos_x = last_tile.pos_x;
            tile_position = create_tile_position(i, prev_word_pos_x, prev_word_len, word.len());
        } else {
            tile_position = create_tile_position(i, -300 as f32, 0, word.len());
        }
        let tile = WordTile {
            id: (i as f32),
            unique_word: String::from(word),
            pos_x: tile_position.0,
            pos_y: tile_position.1,
        };
        word_tile_collection.push(tile);
    }

    for word_tile in word_tile_collection.iter_mut() {
        spawn_word_tile(
            &mut commands,
            &mut meshes,
            &mut materials,
            &word_tile.unique_word,
            word_tile.pos_x,
            word_tile.pos_y,
        );
    }
}

fn create_tile_position(i: usize, pos: f32, previous_len: usize, current_len: usize) -> (f32, f32) {
    let row: f32 = pos + (previous_len * 20 / 2) as f32 + (current_len * 20 / 2) as f32;
    let column: f32 = if i < 6 {
        300.0
    } else if i < 12 {
        200.0
    } else if i < 18 {
        100.0
    } else {
        0.0
    };

    return (row as f32, column as f32);
}

fn spawn_word_tile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    word: &String,
    pos_x: f32,
    pos_y: f32,
) {
    commands
        .spawn((
            Mesh2d(meshes.add(Rectangle::new((word.len() * 20) as f32, 50.))),
            MeshMaterial2d(materials.add(Color::from(WHITE))),
            Transform::from_xyz(pos_x, pos_y, 2.),
        ))
        .with_child((Text2d::new(word.as_str()), TextColor(Color::from(BLACK))));
}
