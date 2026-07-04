use super::WordTile;
use bevy::prelude::*;

pub fn on_tile_drag(
    event: On<Pointer<Drag>>,
    mut tiles_query: Query<(Entity, &WordTile, &mut Transform)>,
) {
    let tile_delta = event.delta;
    let mut z_position: f32 = 0.;
    let mut update: bool = false;

    // Moving the tiles position
    let (dragged_word_len, dragged_tile_pos) = {
        let (_, word_tile, mut transform) = tiles_query.get_mut(event.entity).unwrap();
        transform.translation.x += tile_delta.x;
        transform.translation.y += tile_delta.y * -1.0;
        (word_tile.unique_word.len(), transform.translation.clone())
    };

    //Checking if the tile is overlapping with any other tile
    // Setting update == true when we do find an overlap
    for (other_entity, other_tile, other_transform) in tiles_query.iter() {
        if event.entity == other_entity {
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
        //adding 10 to the z position to bring the tile to the front if update is true
        if let Ok((_, _, mut transform)) = tiles_query.get_mut(event.entity) {
            transform.translation.z = z_position + 10.;
        } else {
            println!(
                "Failed to get mutable transform for entity: {:?}",
                event.entity
            );
        }

        if z_position > 900. {
            // TODO(restack): unfinished. When the stack climbs past z=900, collect the
            //   tiles here and normalize their z back down so it doesn't run away.
            //   stored_tiles + the empty loop below are placeholders for that logic.
            let mut stored_tiles: Vec<(Entity, f32)> = Vec::new();
            for tile in tiles_query.iter_mut() {}
        }
    }
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
