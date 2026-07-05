mod drag;
mod layout;
use crate::states::AppState;
use crate::word_bank::{WordBank, WordBankHandle, select_words};
use bevy::color::palettes::basic::{BLACK, WHITE};
use bevy::prelude::*;
use drag::on_tile_drag;
use layout::create_tile_position;

#[derive(Component)]
pub struct WordTile {
    unique_word: String,
    pos_x: f32,
    pos_y: f32,
}

pub fn spawn_all_tiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    word_bank_handle: Res<WordBankHandle>,
    word_banks: Res<Assets<WordBank>>,
) {
    let Some(spawned_word_bank) = word_banks.get(&word_bank_handle.0) else {
        return;
    };

    let selected_words = select_words(spawned_word_bank);

    let mut word_tile_collection: Vec<WordTile> = Vec::new();
    for (i, word) in selected_words.iter().enumerate() {
        let tile_position: (f32, f32);
        if i % 6 == 0 {
            tile_position = create_tile_position(i, -800., 0, word.len());
        } else {
            let last_tile = word_tile_collection.last().unwrap();
            let prev_word_len = last_tile.unique_word.len();
            let prev_word_pos_x = last_tile.pos_x;
            tile_position = create_tile_position(i, prev_word_pos_x, prev_word_len, word.len());
        }
        let tile = WordTile {
            unique_word: String::from(word),
            pos_x: tile_position.0,
            pos_y: tile_position.1,
        };
        word_tile_collection.push(tile);
    }

    for word_tile in word_tile_collection.into_iter() {
        spawn_word_tile(&mut commands, &mut meshes, &mut materials, word_tile);
    }
}

fn spawn_word_tile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    word_tile: WordTile,
) {
    let word = String::from(&word_tile.unique_word);
    commands
        .spawn((
            Name::new(String::from(&word_tile.unique_word)),
            DespawnOnExit(AppState::Playing),
            Mesh2d(meshes.add(Rectangle::new((&word.len() * 10 + 2) as f32, 27.))),
            MeshMaterial2d(materials.add(Color::from(BLACK))),
            Transform::from_xyz(word_tile.pos_x.clone(), word_tile.pos_y.clone(), 2.),
            word_tile,
        ))
        .with_child((
            Mesh2d(meshes.add(Rectangle::new((&word.len() * 10) as f32, 25.))),
            MeshMaterial2d(materials.add(Color::from(WHITE))),
        ))
        .with_child((
            Text2d::new(word),
            Transform::from_xyz(0., 0., 1.),
            TextFont {
                font_size: 14.,
                ..default()
            },
            TextColor(Color::from(BLACK)),
        ))
        .observe(on_tile_drag);
}
