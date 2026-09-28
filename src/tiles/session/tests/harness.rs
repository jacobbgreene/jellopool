use super::super::*;
use crate::poems::DraftSession;
use bevy::text::{FontCx, LayoutCx, TextEdit, apply_text_edits};

pub(super) fn app() -> App {
    with_book(None)
}

pub(super) fn with_book(book: Option<DraftBook>) -> App {
    let mut app = crate::test_support::ui::layout_app(Vec2::new(1600.0, 900.0), 1.0, 1.0);
    app.init_resource::<GameOptions>()
        .init_resource::<Assets<crate::word_bank::WordBank>>()
        .init_resource::<DraftSession>()
        .init_resource::<InputFocus>()
        .init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<bevy::clipboard::Clipboard>()
        .add_message::<bevy::app::AppExit>()
        .add_systems(
            Startup,
            (
                crate::tiles::spawn_board_root,
                crate::tiles::spawn_all_tiles,
            )
                .chain(),
        )
        .add_systems(PostUpdate, apply_text_edits.in_set(EditableTextSystems));
    register_systems(&mut app);
    crate::tiles::register_interaction_systems(&mut app);
    let words = app
        .world_mut()
        .resource_mut::<Assets<crate::word_bank::WordBank>>()
        .add(
            ron::from_str::<crate::word_bank::WordBank>(include_str!(
                "../../../../assets/word_bank.ron"
            ))
            .unwrap(),
        );
    app.insert_resource(GameAssets {
        words,
        word_font: default(),
        signature_font: None,
    });
    app.world_mut().resource_mut::<DraftSession>().book = book;
    app.update();
    app.update();
    app
}

pub(super) fn single<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .unwrap()
}

pub(super) fn title(app: &mut App, value: &str) {
    let entity = single::<PoemTitle>(app);
    app.world_mut()
        .get_mut::<EditableText>(entity)
        .unwrap()
        .queue_edit(TextEdit::Insert(value.into()));
    app.update();
}

pub(super) fn active(app: &App) -> &PoemDocument {
    app.world()
        .resource::<DraftSession>()
        .book
        .as_ref()
        .unwrap()
        .active()
}
