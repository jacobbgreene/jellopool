use super::*;
use crate::poems::{PoemDocument, tests::TestDirectory};
use std::{fs, time::Duration};

fn app(directory: &TestDirectory) -> App {
    let mut app = App::new();
    app.add_plugins((bevy::app::TaskPoolPlugin::default(), PoemsPlugin))
        .init_resource::<Time>()
        .add_message::<AppExit>()
        .insert_resource(SaveLocation::Directory(directory.0.clone()));
    app.update();
    app
}

fn edit(app: &mut App, title: &str) {
    let now = app.world().resource::<Time>().elapsed_secs_f64();
    let mut session = app.world_mut().resource_mut::<DraftSession>();
    let mut document = PoemDocument::new(vec!["word".into()]).unwrap();
    document.title = title.into();
    session.book = Some(DraftBook::new(document));
    session.changed(now);
}

#[test]
fn saves_are_debounced_and_exit_flushes_the_latest_revision_after_an_in_flight_save() {
    let directory = TestDirectory::new();
    let mut app = app(&directory);
    edit(&mut app, "first snapshot");
    app.update();
    assert!(app.world().resource::<DraftSession>().pending.is_none());
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(1));
    app.update();
    assert!(app.world().resource::<DraftSession>().pending.is_some());
    edit(&mut app, "newer edit while writing");
    app.world_mut().write_message(AppExit::Success);
    app.update();
    let session = app.world().resource::<DraftSession>();
    assert_eq!(session.revision, session.saved_revision);
    assert!(session.pending.is_none());
    assert_eq!(
        session
            .storage
            .as_ref()
            .unwrap()
            .load()
            .unwrap()
            .book
            .unwrap()
            .active()
            .title,
        "newer edit while writing"
    );
}

#[test]
fn saved_library_reopens_without_creating_a_new_draft() {
    let directory = TestDirectory::new();
    let mut first = app(&directory);
    edit(&mut first, "poem one");
    {
        let mut session = first.world_mut().resource_mut::<DraftSession>();
        let second = PoemDocument::new(vec!["second".into()]).unwrap();
        session.book.as_mut().unwrap().add(second).unwrap();
        session.changed(0.0);
    }
    first.world_mut().write_message(AppExit::Success);
    first.update();
    let expected = first.world().resource::<DraftSession>().book.clone();
    drop(first);
    let second = app(&directory);
    assert_eq!(second.world().resource::<DraftSession>().book, expected);
    assert_eq!(
        second.world().resource::<DraftSession>().status(),
        "All drafts saved"
    );
}

#[test]
fn unreadable_library_disables_saving_and_leaves_original_untouched() {
    let directory = TestDirectory::new();
    fs::write(directory.0.join("drafts.ron"), b"damaged").unwrap();
    let mut app = app(&directory);
    assert!(app.world().resource::<DraftSession>().error.is_some());
    assert!(app.world().resource::<DraftSession>().storage.is_none());
    edit(&mut app, "temporary only");
    app.world_mut().write_message(AppExit::Success);
    app.update();
    assert_eq!(
        fs::read(directory.0.join("drafts.ron")).unwrap(),
        b"damaged"
    );
    assert_eq!(
        app.world().resource::<DraftSession>().status(),
        "Not saved — see warning"
    );
}

#[test]
fn failed_saves_retain_edits_and_retry_after_the_cooldown() {
    let directory = TestDirectory::new();
    let mut app = app(&directory);
    edit(&mut app, "first save");
    app.world_mut().write_message(AppExit::Success);
    app.update();
    let original = fs::read(directory.0.join("drafts.ron")).unwrap();
    fs::create_dir(directory.0.join("drafts.ron.bak")).unwrap();
    edit(&mut app, "retained in memory");
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(1));
    app.update();
    // Join the real background write deterministically, without sleeps/poll loops.
    let task = app
        .world_mut()
        .resource_mut::<DraftSession>()
        .pending
        .take()
        .unwrap();
    let (revision, result) = block_on(task);
    assert!(result.is_err());
    app.world_mut()
        .resource_mut::<DraftSession>()
        .finish_save(revision, result, 1.0);
    assert_eq!(fs::read(directory.0.join("drafts.ron")).unwrap(), original);
    assert_eq!(
        app.world()
            .resource::<DraftSession>()
            .book
            .as_ref()
            .unwrap()
            .active()
            .title,
        "retained in memory"
    );
    fs::remove_dir(directory.0.join("drafts.ron.bak")).unwrap();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(4));
    app.update();
    assert!(app.world().resource::<DraftSession>().pending.is_none());
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(1));
    app.update();
    assert!(app.world().resource::<DraftSession>().pending.is_some());
    app.world_mut().write_message(AppExit::Success);
    app.update();
    assert!(app.world().resource::<DraftSession>().error.is_none());
    let restored: DraftBook =
        ron::from_str(&fs::read_to_string(directory.0.join("drafts.ron")).unwrap()).unwrap();
    assert_eq!(restored.active().title, "retained in memory");
}
