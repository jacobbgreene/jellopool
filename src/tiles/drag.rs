use super::{TileMotion, WordTile};
use crate::board::BoardLayout;
use bevy::prelude::*;

const Z_TOP: f32 = 10.;
const ON_BOARD_SCALE: f32 = 1.09;
const ON_TRAY_SCALE: f32 = 1.3;

const DEFAULT_SCALE: f32 = 1.;
const DROPPED_Z: f32 = 2.;

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

pub fn tile_drag_start(
    event: On<Pointer<DragStart>>,
    mut tiles_query: Query<(&mut TileMotion, &mut Transform)>,
) {
    let Ok((mut motion, mut transform)) = tiles_query.get_mut(event.entity) else {
        println!(
            "Failed to get either the entity, motion, or transform for {:?}",
            event.entity,
        );
        return;
    };
    motion.target_scale = ON_TRAY_SCALE;
    transform.translation.z = Z_TOP;
}

pub fn tile_drag_end(
    event: On<Pointer<DragEnd>>,
    mut tiles_query: Query<(&mut TileMotion, &mut Transform)>,
) {
    let Ok((mut motion, mut transform)) = tiles_query.get_mut(event.entity) else {
        println!(
            "Failed to get either the entity, motion, or transform for {:?}",
            event.entity,
        );
        return;
    };
    motion.target_scale = DEFAULT_SCALE;
    transform.translation.z = DROPPED_Z;
}
