mod drag;
mod layout;
use crate::states::AppState;
use crate::word_bank::{WordBank, WordBankHandle, select_words};
use bevy::color::palettes::basic::{BLACK, WHITE};
use bevy::prelude::*;
use drag::on_tile_drag;
use layout::create_tile_position;

const BOARD_COLOR: Srgba = BLACK;
const TILE_COLOR: Srgba = WHITE;

#[derive(Component)]
pub struct WordTile {
    unique_word: String,
    //size has the x and y positions for the tile
    size: Vec2,
}

#[derive(Component)]
pub struct TileMotion {
    target: Vec2,
    target_scale: f32,
    base_rotation: f32,
}

pub fn spawn_all_tiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    word_bank_handle: Res<WordBankHandle>,
    word_banks: Res<Assets<WordBank>>,
) {
    //For each tile in our word bank, spawn a tile and it's respective motion
    //motion is just tracking it's actual position and scale
    let Some(spawned_word_bank) = word_banks.get(&word_bank_handle.0) else {
        return;
    };

    let selected_words = select_words(spawned_word_bank);

    let mut word_tile_collection: Vec<(WordTile, TileMotion)> = Vec::new();
    for (i, word) in selected_words.iter().enumerate() {
        let tile_position: (f32, f32);
        if i % 6 == 0 {
            tile_position = create_tile_position(i, -800., 0, word.len());
        } else {
            let Some(last_tile) = word_tile_collection.last().map(|(tile, _)| tile) else {
                continue;
            };
            let prev_word_len = last_tile.unique_word.len();
            let prev_word_pos_x = last_tile.size.x;
            tile_position = create_tile_position(i, prev_word_pos_x, prev_word_len, word.len());
        }
        let tile = WordTile {
            unique_word: String::from(word),
            size: Vec2::new(tile_position.0, tile_position.1),
        };
        let motion = TileMotion {
            target: Vec2::new(tile_position.0, tile_position.1),
            target_scale: 1.,
            base_rotation: 0.,
        };
        word_tile_collection.push((tile, motion));
    }

    for (word_tile, motion) in word_tile_collection.into_iter() {
        spawn_word_tile(
            &mut commands,
            &mut meshes,
            &mut materials,
            word_tile,
            motion,
        );
    }
}

fn spawn_word_tile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    word_tile: WordTile,
    motion: TileMotion,
) {
    let word = String::from(&word_tile.unique_word);
    commands
        .spawn((
            Name::new(word.clone()),
            DespawnOnExit(AppState::Playing),
            Mesh2d(meshes.add(Rectangle::new((&word.len() * 10 + 2) as f32, 27.))),
            MeshMaterial2d(materials.add(Color::from(BOARD_COLOR))),
            Transform::from_xyz(word_tile.size.x.clone(), word_tile.size.y.clone(), 2.),
            word_tile,
            motion,
        ))
        .with_child((
            Mesh2d(meshes.add(Rectangle::new((&word.len() * 10) as f32, 25.))),
            MeshMaterial2d(materials.add(Color::from(TILE_COLOR))),
        ))
        .with_child((
            Text2d::new(word),
            Transform::from_xyz(0., 0., 1.),
            TextFont {
                font_size: 14.,
                ..default()
            },
            TextColor(Color::from(BOARD_COLOR)),
        ))
        .observe(on_tile_drag);
}

pub fn move_tiles(time: Res<Time>, mut tiles: Query<(&TileMotion, &mut Transform)>) {
    let dt = time.delta_secs();
    let speed = 38.0;
    let ease = 1.0 - (-speed * dt).exp();
    for (motion, mut transform) in &mut tiles {
        transform.translation.x += (motion.target.x - transform.translation.x) * ease;
        transform.translation.y += (motion.target.y - transform.translation.y) * ease;
    }
}
