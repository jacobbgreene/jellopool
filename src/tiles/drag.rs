use super::{TileMotion, WordTile};
use crate::board::BoardLayout;
use bevy::prelude::*;

pub fn on_tile_drag(
    event: On<Pointer<Drag>>,
    mut tiles_query: Query<(Entity, &WordTile, &mut TileMotion, &mut Transform)>,
    layout: Res<BoardLayout>,
) {
    //Get the delta of the tile drag and update the motion first
    let Ok((_, tile, mut motion, _)) = tiles_query.get_mut(event.entity) else {
        println!(
            "Failed to get mutable tile motion for entity: {:?}",
            event.entity
        );
        return;
    };

    motion.target = layout.clamp_tile(
        motion.target + Vec2::new(event.delta.x, -event.delta.y),
        tile.size,
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
