//! The document library and its save lifecycle. No UI entities or drag geometry.
use super::{DraftBook, storage::DraftStorage};
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, block_on, poll_once};
use std::{path::PathBuf, sync::Arc};

#[derive(Resource, Default)]
pub(crate) enum SaveLocation {
    /// Tests, seeded review runs, and captures must never touch personal drafts.
    #[default]
    Memory,
    Directory(PathBuf),
    Unavailable(String),
}

type SaveTask = Task<(u64, Result<(), String>)>;

#[derive(Resource, Default)]
pub(crate) struct DraftSession {
    pub book: Option<DraftBook>,
    storage: Option<Arc<DraftStorage>>,
    pending: Option<SaveTask>,
    revision: u64,
    saved_revision: u64,
    changed_at: f64,
    dirty_since: f64,
    retry_at: f64,
    pub notice: Option<String>,
    pub error: Option<String>,
}

impl DraftSession {
    pub fn changed(&mut self, now: f64) {
        if self.revision == self.saved_revision {
            self.dirty_since = now;
        }
        self.revision += 1;
        self.changed_at = now;
    }

    pub fn status(&self) -> &str {
        if self.error.is_some() {
            "Not saved — see warning"
        } else if self.storage.is_none() {
            "Temporary session · not saved"
        } else if self.pending.is_some() {
            "Saving…"
        } else if self.revision != self.saved_revision {
            "Unsaved changes…"
        } else {
            "All drafts saved"
        }
    }

    fn finish_save(&mut self, revision: u64, result: Result<(), String>, now: f64) {
        match result {
            Ok(()) => {
                self.saved_revision = revision;
                self.error = None;
                if self.revision != revision {
                    self.dirty_since = now;
                }
            }
            Err(error) => {
                error!("{error}");
                self.error = Some(error);
                self.retry_at = now + 5.0;
            }
        }
    }
}

pub(crate) struct PoemsPlugin;
impl Plugin for PoemsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveLocation>()
            .init_resource::<DraftSession>()
            .add_systems(Startup, load_drafts)
            .add_systems(Last, (autosave, save_on_exit).chain());
    }
}

fn load_drafts(location: Res<SaveLocation>, mut session: ResMut<DraftSession>) {
    let directory = match &*location {
        SaveLocation::Memory => return,
        SaveLocation::Unavailable(error) => {
            session.error = Some(error.clone());
            return;
        }
        SaveLocation::Directory(directory) => directory,
    };
    let loaded = DraftStorage::open(directory.clone()).and_then(|storage| {
        let loaded = storage.load()?;
        Ok((Arc::new(storage), loaded))
    });
    match loaded {
        Ok((storage, loaded)) => {
            info!("Drafts stored in {}", directory.display());
            session.storage = Some(storage);
            session.book = loaded.book;
            session.notice = loaded.notice;
        }
        Err(error) => {
            error!("{error}");
            // Stay usable in memory, but do not replace existing unreadable data.
            session.error = Some(format!(
                "{error}. This session is temporary; resolve the problem and restart to enable saving."
            ));
        }
    }
}

fn autosave(time: Res<Time>, mut session: ResMut<DraftSession>) {
    let now = time.elapsed_secs_f64();
    // Do not mark the session changed merely by checking an empty task slot.
    if session.pending.is_some()
        && let Some(task) = &mut session.pending
        && let Some((revision, result)) = block_on(poll_once(task))
    {
        session.pending = None;
        session.finish_save(revision, result, now);
    }
    let ready = session.revision != session.saved_revision
        && session.pending.is_none()
        && now >= session.retry_at
        && (now - session.changed_at >= 0.75 || now - session.dirty_since >= 5.0);
    if !ready {
        return;
    }
    let Some(storage) = session.storage.clone() else {
        return;
    };
    let Some(book) = session.book.clone() else {
        return;
    };
    let revision = session.revision;
    // One write at a time; edits made while this snapshot saves retain a newer
    // revision and are saved next. File I/O never stalls ordinary interaction.
    session.pending = Some(IoTaskPool::get().spawn(async move { (revision, storage.save(&book)) }));
}

fn save_on_exit(
    mut exit: MessageReader<AppExit>,
    mut session: ResMut<DraftSession>,
    time: Res<Time>,
) {
    if exit.read().next().is_none() {
        return;
    }
    if let Some(task) = session.pending.take() {
        let (revision, result) = block_on(task);
        session.finish_save(revision, result, time.elapsed_secs_f64());
    }
    if session.revision != session.saved_revision
        && let (Some(storage), Some(book)) = (session.storage.clone(), session.book.as_ref())
    {
        let result = storage.save(book);
        let revision = session.revision;
        session.finish_save(revision, result, time.elapsed_secs_f64());
    }
}

#[cfg(test)]
mod tests;
