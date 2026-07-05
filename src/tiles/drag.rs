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

    //This shit just gives a corrected position, based on board layout and with easing
    motion.target = layout.clamp_tile(
        motion.target + Vec2::new(event.delta.x, -event.delta.y),
        tile.size,
    );
}
