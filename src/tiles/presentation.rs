//! Board typography, surfaces and tile construction. No drag decisions.
#[cfg(test)]
mod tests;
use crate::prelude::*;
use bevy::app::AppExit;

// Off-white paper on a cool, pale stock. Tray and gutters share one surface;
// the words carry the character. Vermilion remains exclusive to the live preview.
pub(super) const INK: Srgba = Srgba::new(0.129, 0.145, 0.137, 1.0); // #212523
pub(super) const PAPER: Srgba = Srgba::new(0.965, 0.961, 0.941, 1.0); // #F6F5F0
pub(super) const TILE_FACE: Srgba = Srgba::new(0.984, 0.980, 0.961, 1.0); // #FBFAF5
pub(super) const BANK: Srgba = Srgba::new(0.914, 0.918, 0.894, 1.0); // #E9EAE4
pub(super) const OUTLINE: Srgba = Srgba::new(0.600, 0.616, 0.584, 1.0); // #999D95
pub(super) const RULE: Srgba = Srgba::new(0.773, 0.784, 0.745, 1.0); // #C5C8BE
pub(super) const SIGNATURE: Srgba = Srgba::new(0.392, 0.416, 0.384, 1.0); // #646A62
pub(super) const TILE_RADIUS: f32 = 2.0;
pub(super) const MENU_HEIGHT: f32 = 64.0;
pub(super) const PAGE_INSET: f32 = 24.0;
pub(super) const PAGE_LEFT: f32 = 64.0;
pub(super) const PAGE_RIGHT: f32 = 32.0;
const TRAY_WIDTH_PERCENT: f32 = 78.0;

/// Fixed menu strip outside the scrolling page.
#[derive(Component, Default, Clone)]
pub(super) struct MenuSpace;
#[derive(Component, Default, Clone)]
pub(super) struct PageLead;

pub(super) fn page_lead_height(screen_height: f32) -> f32 {
    // Center the first 40px tile on the screen, not the space above the tray.
    (screen_height * 0.5 - MENU_HEIGHT - PAGE_INSET - 20.0).max(0.0)
}

pub(super) fn fit_page_lead(
    camera: Single<&Camera, With<Camera2d>>,
    scale: Res<UiScale>,
    mut leads: Query<&mut Node, With<PageLead>>,
) {
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    if scale.0 <= 0.0 || size.y <= 0.0 {
        return;
    }
    let height = Val::Px(page_lead_height(size.y / scale.0));
    for mut node in &mut leads {
        if node.height != height {
            node.height = height;
        }
    }
}

/// Keep the full word bank within its allotted area on smaller displays.
/// Scale the entire UI together so tile layout, grid and drag coordinates use
/// the same units. Larger viewports retain the normal 20px word size.
pub fn fit_board_to_viewport(camera: Single<&Camera, With<Camera2d>>, mut scale: ResMut<UiScale>) {
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    if size.min_element() <= 0.0 {
        return;
    }
    let fit = (size.x / 1600.0).min(size.y / 900.0).min(1.0);
    if scale.0 != fit {
        scale.0 = fit;
    }
}

pub fn spawn_board_root(world: &mut World) {
    let assets = world.resource::<GameAssets>();
    let title = world
        .get_resource::<super::session::DraftSession>()
        .and_then(|session| session.book.as_ref())
        .map(|book| book.active().title.clone())
        .unwrap_or_default();
    let scene = super::scenes::writing_workspace(
        assets.word_font.clone(),
        assets.signature_font.clone(),
        title,
        default(),
    );
    match world.spawn_scene(scene) {
        Ok(mut root) => {
            root.insert(DespawnOnExit(AppState::Playing));
        }
        Err(error) => {
            error!("Cannot create writing workspace: {error}");
            world.write_message(AppExit::error());
        }
    }
}

pub fn get_drag_layer() -> (Name, DragLayer, Pickable, Node) {
    (
        Name::new("drag_layer"),
        DragLayer,
        // The layer renders and holds dragged tiles, but never blocks the
        // pointer from reaching tiles and regions beneath it.
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
    )
}

pub fn get_board_tray() -> (Name, BoardTray, BackgroundColor, Node) {
    let name = Name::new("board_tray");
    let color = BackgroundColor(Color::from(BANK));
    let node = Node {
        width: Val::Percent(TRAY_WIDTH_PERCENT),
        // Preserve the normal footprint, but let measured wrapped rows grow it.
        // Even a valid selection can need more than four rows with this font.
        min_height: Val::Percent(23.0),
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        justify_content: JustifyContent::Center,
        align_content: AlignContent::Center,
        row_gap: Val::Px(8.0),
        column_gap: Val::Px(12.0),
        padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
        ..default()
    };

    (name, BoardTray, color, node)
}

/// The quiet page/bank seam is an overlay, so it doesn't alter drop coordinates.
pub fn get_separator() -> (Name, BackgroundColor, Pickable, Node) {
    (
        Name::new("bank_separator"),
        BackgroundColor(Color::from(RULE)),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            ..default()
        },
    )
}

/// Populate the scene from a durable document, never from transient drag state.
pub fn spawn_all_tiles(world: &mut World) {
    let document = match super::session::active_document(world) {
        Ok(document) => document,
        Err(error) => {
            error!("Cannot spawn tiles: {error}");
            world.write_message(AppExit::error());
            return;
        }
    };
    let tray = world
        .query_filtered::<Entity, With<BoardTray>>()
        .single(world)
        .ok();
    let zone = world
        .query_filtered::<Entity, With<WritingZone>>()
        .single(world)
        .ok();
    let Some(tray) = tray else {
        error!("Cannot populate workspace: expected exactly one word tray");
        world.write_message(AppExit::error());
        return;
    };
    let font = world.resource::<GameAssets>().word_font.clone();
    // Spawn tray words in their saved order; placed words have no tray slot.
    for id in document.tray.iter().chain(
        document
            .tiles
            .iter()
            .filter(|tile| tile.position.is_some())
            .map(|tile| &tile.id),
    ) {
        let tile = document
            .tiles
            .iter()
            .find(|tile| &tile.id == id)
            .expect("validated tile ID");
        match world.spawn_scene(super::scenes::word_tile(
            tile.word.clone(),
            font.clone(),
            default(),
        )) {
            Ok(mut entity) => {
                entity.insert(super::session::DocumentTile(tile.id));
                if let Some(slot) = tile.last_tray_slot {
                    entity.insert(super::tray::LastTraySlot(slot));
                }
                if let Some(position) = tile.position {
                    let Some(zone) = zone else {
                        error!("Cannot restore placed tiles without a writing zone");
                        world.write_message(AppExit::error());
                        return;
                    };
                    let pos = Vec2::new(
                        position.x,
                        f32::from(position.line) * crate::poems::LINE_PITCH,
                    );
                    entity.insert((ChildOf(zone), PlacedTile(pos)));
                    let mut node = entity.get_mut::<Node>().expect("tile scene node");
                    node.position_type = PositionType::Absolute;
                    node.left = Val::Px(pos.x);
                    node.top = Val::Px(pos.y);
                } else {
                    entity.insert(ChildOf(tray));
                }
            }
            Err(error) => {
                error!("Cannot create word tile: {error}");
                world.write_message(AppExit::error());
                return;
            }
        }
    }
}
