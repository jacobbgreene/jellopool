use bevy::{
    color::palettes::basic::{BLACK, WHITE},
    prelude::*,
    window::{MonitorSelection, WindowMode},
};
use bevy_common_assets::ron::RonAssetPlugin;
use rand::seq::{IndexedRandom, SliceRandom};

#[derive(Asset, TypePath, serde::Deserialize)]
pub struct WordBank {
    pub nouns: Vec<String>,
    pub verbs: Vec<String>,
    pub adjectives: Vec<String>,
    pub adverbs: Vec<String>,
    pub pronouns: Vec<String>,
    pub prepositions: Vec<String>,
    pub conjunctions: Vec<String>,
    pub articles: Vec<String>,
}

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

#[derive(Component)]
struct WordTile {
    id: f32,
    unique_word: String,
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
}

#[derive(Resource)]
struct WordBankHandle(Handle<WordBank>);

fn load_word_bank(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset: Handle<WordBank> = asset_server.load("word_bank.ron");
    commands.insert_resource(WordBankHandle(asset));
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
    ));
}

fn spawn_all_tiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut spawned: Local<bool>,
    word_bank_handle: Res<WordBankHandle>,
    word_banks: Res<Assets<WordBank>>,
) {
    let spawned_word_bank = word_banks.get(&word_bank_handle.0);
    if *spawned || spawned_word_bank.is_none() {
        return;
        // TODO: "Address the AppState/Loading Screen setup later");
    }
    let spawned_word_bank = spawned_word_bank.unwrap();
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
            id: (i as f32),
            unique_word: String::from(word),
            pos_x: tile_position.0,
            pos_y: tile_position.1,
            pos_z: i as f32,
        };
        word_tile_collection.push(tile);
    }

    for word_tile in word_tile_collection.into_iter() {
        spawn_word_tile(&mut commands, &mut meshes, &mut materials, word_tile);
    }
    *spawned = true;
}

fn create_tile_position(i: usize, pos: f32, previous_len: usize, current_len: usize) -> (f32, f32) {
    let row: f32 = pos + (previous_len * 12 / 2) as f32 + (current_len * 6 / 2) as f32 + 40 as f32;
    let column = 300.0 - ((i / 6) as f32 * 30.0);
    return (row as f32, column as f32);
}

fn select_words(word_bank: &WordBank) -> Vec<String> {
    let mut selected_words: Vec<String> = Vec::new();
    let mut rng = rand::rng();

    let plan = [
        (&word_bank.nouns, 6),
        (&word_bank.verbs, 6),
        (&word_bank.adjectives, 4),
        (&word_bank.adverbs, 2),
        (&word_bank.pronouns, 6),
        (&word_bank.prepositions, 10),
        (&word_bank.conjunctions, 3),
        (&word_bank.articles, 3),
    ];

    for (category, count) in &plan {
        let picked: Vec<String> = category.sample(&mut rng, *count).cloned().collect();
        selected_words.extend(picked);
    }

    selected_words.shuffle(&mut rng);
    return selected_words;
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
            Mesh2d(meshes.add(Rectangle::new((word.len() * 10) as f32, 25.))),
            MeshMaterial2d(materials.add(Color::from(WHITE))),
            Transform::from_xyz(word_tile.pos_x, word_tile.pos_y, 2.),
            word_tile,
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
        .observe(
            |event: On<Pointer<Drag>>, mut tiles_query: Query<(&WordTile, &mut Transform)>| {
                let tile_delta = event.delta;
                let mut z_position: f32 = 0.;
                let mut update: bool = false;

                let (dragged_tile_id, dragged_word_len, dragged_tile_pos) = {
                    let (word_tile, mut transform) = tiles_query.get_mut(event.entity).unwrap();
                    transform.translation.x += tile_delta.x;
                    transform.translation.y += tile_delta.y * -1.0;
                    (
                        word_tile.id,
                        word_tile.unique_word.len(),
                        transform.translation.clone(),
                    )
                };

                for (other_tile, other_transform) in tiles_query.iter() {
                    if other_tile.id == dragged_tile_id {
                        continue;
                    } else if overlap(
                        dragged_tile_pos,
                        dragged_word_len,
                        other_tile,
                        other_transform,
                    ) {
                        z_position = z_position.max(other_transform.translation.z);
                        update = true;
                    }
                }

                if update {
                    if let Ok((_, mut transform)) = tiles_query.get_mut(event.entity) {
                        println!(
                            "Updating z from {} to {}",
                            transform.translation.z,
                            z_position + 1.0
                        );
                        transform.translation.z = z_position + 10.0;
                    }
                }
            },
        );
}

struct TileRange {
    left_edge: f32,
    right_edge: f32,
    bottom_edge: f32,
    top_edge: f32,
}

fn overlap(
    dragged_tile_position: Vec3,
    dragged_word_len: usize,
    word_tile_b: &WordTile,
    word_tile_b_tranform: &Transform,
) -> bool {
    let range_a: TileRange = get_tile_range(dragged_tile_position, dragged_word_len);
    let range_b: TileRange = get_tile_range(
        word_tile_b_tranform.translation,
        word_tile_b.unique_word.len(),
    );
    if (range_a.left_edge < range_b.right_edge && range_b.left_edge < range_a.right_edge)
        && (range_a.bottom_edge < range_b.top_edge && range_b.bottom_edge < range_a.top_edge)
    {
        return true;
    }
    false
}

fn get_tile_range(position: Vec3, word_len: usize) -> TileRange {
    let width = (word_len * 10) as f32;
    let height = 25.0;
    TileRange {
        left_edge: position.x - width / 2.0,
        right_edge: position.x + width / 2.0,
        bottom_edge: position.y - height / 2.0,
        top_edge: position.y + height / 2.0,
    }
}
