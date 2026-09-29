//! Adapter between durable documents and the one active writing workspace.
//! Switching cancels unfinished gestures, captures committed state, then rebuilds
//! the scene. Animations, preview pushes, focus and entity IDs are never saved.
use super::{
    drag::{CancelDrag, DragFollow, PlacementPreview},
    scenes::WorkspaceRoot,
    title::PoemTitle,
    tray::LastTraySlot,
};
pub(super) use crate::poems::DraftSession;
use crate::{
    config::GameOptions,
    poems::{DraftBook, LINE_PITCH, PaperPosition, PoemDocument, PoemId, TileId},
    prelude::*,
};
use bevy::{
    input_focus::InputFocus,
    text::{EditableText, EditableTextSystems},
};
use std::collections::HashMap;

#[derive(Component, Clone, Copy, Default)]
pub(super) struct DocumentTile(pub TileId);

#[derive(Clone)]
pub(super) enum DraftAction {
    New,
    Open(PoemId),
}

#[derive(Resource, Default)]
pub(super) struct DraftRequest {
    pub action: Option<DraftAction>,
    captured: bool,
}

#[derive(Resource, Default, PartialEq)]
pub(super) struct CaptureError(pub Option<String>);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DraftWorkspace;

#[derive(Default)]
struct CaptureBuffer {
    positions: HashMap<TileId, (Option<PaperPosition>, Option<usize>)>,
    tray: Vec<TileId>,
}

type CaptureTiles<'w, 's> = Query<
    'w,
    's,
    (
        &'static DocumentTile,
        Option<&'static PlacedTile>,
        Option<&'static LastTraySlot>,
    ),
    With<WordTile>,
>;

pub(super) fn register(app: &mut App) {
    app.configure_sets(
        PreUpdate,
        DraftWorkspace.run_if(in_state(AppState::Playing)),
    )
    .configure_sets(Update, DraftWorkspace.run_if(in_state(AppState::Playing)))
    .configure_sets(
        PostUpdate,
        DraftWorkspace.run_if(in_state(AppState::Playing)),
    );
    register_systems(app);
}

fn register_systems(app: &mut App) {
    app.init_resource::<DraftRequest>()
        .init_resource::<CaptureError>()
        .add_systems(
            PreUpdate,
            switch_document
                .in_set(DraftWorkspace)
                .before(bevy::input::InputSystems)
                .before(super::presentation::fit_board_to_viewport),
        )
        .add_systems(
            Update,
            prepare_switch
                .before(super::TileInteraction)
                .in_set(DraftWorkspace),
        )
        .add_systems(
            PostUpdate,
            (
                capture_document,
                approve_switch,
                super::toolbar::sync_toolbar,
            )
                .chain()
                .after(EditableTextSystems)
                .before(bevy::ui::UiSystems::Layout)
                .in_set(DraftWorkspace),
        );
}

pub(super) fn new_document(world: &World) -> Result<PoemDocument, String> {
    let assets = world.resource::<GameAssets>();
    let banks = world.resource::<Assets<crate::word_bank::WordBank>>();
    let bank = banks
        .get(&assets.words)
        .ok_or("required word bank asset is missing after loading")?;
    let words = crate::word_bank::select_words(bank, world.resource::<GameOptions>().seed)?;
    PoemDocument::new(words)
}

pub(super) fn active_document(world: &mut World) -> Result<PoemDocument, String> {
    if let Some(book) = world
        .get_resource::<DraftSession>()
        .and_then(|session| session.book.as_ref())
    {
        return Ok(book.active().clone());
    }
    let document = new_document(world)?;
    let now = world
        .get_resource::<Time>()
        .map_or(0.0, Time::elapsed_secs_f64);
    if let Some(mut session) = world.get_resource_mut::<DraftSession>() {
        session.book = Some(DraftBook::new(document.clone()));
        session.changed(now);
    }
    Ok(document)
}

fn prepare_switch(
    request: Res<DraftRequest>,
    titles: Query<&EditableText, With<PoemTitle>>,
    dragged: Query<Entity, With<DragFollow>>,
    mut commands: Commands,
) {
    if request.action.is_some() && !titles.iter().any(EditableText::is_composing) {
        for entity in &dragged {
            commands.entity(entity).insert(CancelDrag);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn capture_document(
    mut session: ResMut<DraftSession>,
    time: Res<Time>,
    titles: Query<&EditableText, With<PoemTitle>>,
    tiles: CaptureTiles,
    trays: Query<&Children, With<BoardTray>>,
    dragged: Query<(), With<DragFollow>>,
    mut problem: ResMut<CaptureError>,
    mut buffer: Local<CaptureBuffer>,
) {
    let Some(book) = session.book.as_ref() else {
        return;
    };
    let Ok(editor) = titles.single() else {
        problem.set_if_neq(CaptureError(Some(
            "Expected one title editor; the saved draft has not been replaced.".into(),
        )));
        return;
    };
    if editor.is_composing() {
        return;
    }
    let current = book.active();
    let title = editor.value().to_string();
    let CaptureBuffer { positions, tray } = &mut *buffer;
    positions.clear();
    tray.clear();
    if dragged.is_empty() {
        for (id, placement, last_slot) in &tiles {
            if let Some(placement) = placement {
                let line = placement.0.y / LINE_PITCH;
                if !placement.0.is_finite()
                    || line < 0.0
                    || line >= crate::poems::LINE_COUNT as f32
                    || (line - line.round()).abs() > 0.001
                {
                    problem.set_if_neq(CaptureError(Some("A tile has an invalid line position; the saved draft has not been replaced.".into())));
                    return;
                }
            }
            let pos = placement.map(|pos| PaperPosition {
                line: (pos.0.y / LINE_PITCH).round() as u16,
                x: pos.0.x,
            });
            positions.insert(id.0, (pos, last_slot.map(|slot| slot.0)));
        }
        let Ok(children) = trays.single() else {
            problem.set_if_neq(CaptureError(Some(
                "Expected one word tray; the saved draft has not been replaced.".into(),
            )));
            return;
        };
        tray.extend(
            children
                .iter()
                .filter_map(|entity| tiles.get(entity).ok().map(|(id, _, _)| id.0)),
        );
        if positions.len() != current.tiles.len()
            || tiles.iter().count() != positions.len()
            || !current
                .tiles
                .iter()
                .all(|tile| positions.contains_key(&tile.id))
        {
            problem.set_if_neq(CaptureError(Some(
                "The workspace is incomplete; its saved draft has not been replaced.".into(),
            )));
            return;
        }
    }
    let positions_changed = dragged.is_empty()
        && (*tray != current.tray
            || current
                .tiles
                .iter()
                .any(|tile| positions[&tile.id] != (tile.position, tile.last_tray_slot)));
    if title == current.title && !positions_changed {
        problem.set_if_neq(CaptureError(None));
        return;
    }
    // Clone the words only when something committed actually changes, not on
    // every animation frame. During a drag only the title can be captured.
    let mut document = current.clone();
    document.title = title;
    if positions_changed {
        document.tray.clone_from(tray);
        for tile in &mut document.tiles {
            (tile.position, tile.last_tray_slot) = positions[&tile.id];
        }
    }
    match session
        .book
        .as_mut()
        .expect("active book")
        .replace_active(document)
    {
        Ok(true) => {
            session.changed(time.elapsed_secs_f64());
            problem.set_if_neq(CaptureError(None));
        }
        Ok(false) => {
            problem.set_if_neq(CaptureError(None));
        }
        Err(error) => {
            problem.set_if_neq(CaptureError(Some(format!("Draft not captured: {error}"))));
        }
    }
}

fn approve_switch(
    mut request: ResMut<DraftRequest>,
    problem: Res<CaptureError>,
    dragged: Query<(), With<DragFollow>>,
    titles: Query<&EditableText, With<PoemTitle>>,
) {
    if request.action.is_some() {
        request.captured = problem.0.is_none()
            && dragged.is_empty()
            && !titles.iter().any(EditableText::is_composing);
    }
}

fn switch_document(world: &mut World) {
    // Text edits run during UI content preparation. Capture them there, but
    // rebuild only at the start of the next frame, before input/camera/layout
    // propagation. Replacing the hierarchy mid-layout leaves stale geometry.
    if !world.resource::<DraftRequest>().captured {
        return;
    }
    world.resource_mut::<DraftRequest>().captured = false;
    let action = world
        .resource_mut::<DraftRequest>()
        .action
        .take()
        .expect("pending action");
    let result = match action {
        DraftAction::New => new_document(world).and_then(|document| {
            world
                .resource_mut::<DraftSession>()
                .book
                .as_mut()
                .ok_or("no draft library")?
                .add(document)
        }),
        DraftAction::Open(id) => world
            .resource_mut::<DraftSession>()
            .book
            .as_mut()
            .ok_or_else(|| "no draft library".to_string())
            .and_then(|book| book.switch(&id)),
    };
    if let Err(error) = result {
        world.resource_mut::<DraftSession>().error = Some(error);
        return;
    }
    let now = world.resource::<Time>().elapsed_secs_f64();
    world.resource_mut::<DraftSession>().changed(now);
    if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
        focus.clear();
    }
    world.resource_mut::<PlacementPreview>().0 = None;
    let roots: Vec<_> = world
        .query_filtered::<Entity, With<WorkspaceRoot>>()
        .iter(world)
        .collect();
    for root in roots {
        world.despawn(root);
    }
    super::spawn_board_root(world);
    super::spawn_all_tiles(world);
}

#[cfg(test)]
mod tests;
