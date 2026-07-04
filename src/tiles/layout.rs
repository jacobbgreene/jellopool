pub fn create_tile_position(
    i: usize,
    pos: f32,
    previous_len: usize,
    current_len: usize,
) -> (f32, f32) {
    let row: f32 = pos + (previous_len * 12 / 2) as f32 + (current_len * 6 / 2) as f32 + 40 as f32;
    let column = 300.0 - ((i / 6) as f32 * 30.0);
    return (row as f32, column as f32);
}
