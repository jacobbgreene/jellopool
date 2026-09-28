//! Versioned local saves with atomic replacement, one previous-good backup,
//! and an exclusive process lock. Never overwrite an unreadable/newer save.
use super::model::{DraftBook, SAVE_VERSION};
use serde::Deserialize;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_SAVE_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) struct DraftStorage {
    directory: PathBuf,
    _lock: File,
}

pub(crate) struct LoadedDrafts {
    pub book: Option<DraftBook>,
    pub notice: Option<String>,
}

impl DraftStorage {
    pub fn open(directory: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("drafts.lock"))
            .map_err(|error| format!("Cannot open draft lock: {error}"))?;
        lock.try_lock().map_err(|error| {
            format!("Cannot lock drafts (another Jellopool may be open): {error}")
        })?;
        Ok(Self {
            directory,
            _lock: lock,
        })
    }

    pub fn load(&self) -> Result<LoadedDrafts, String> {
        let primary = read_optional(&self.directory.join("drafts.ron"))?;
        let problem = match primary.as_deref().map(decode) {
            Some(Ok(book)) => {
                return Ok(LoadedDrafts {
                    book: Some(book),
                    notice: None,
                });
            }
            Some(Err(DecodeError::NewerVersion(version))) => {
                return Err(format!(
                    "Save version {version} is not supported; existing files were left untouched"
                ));
            }
            Some(Err(DecodeError::Invalid(error))) => Some(error),
            None => None,
        };
        if let Some(backup) = read_optional(&self.directory.join("drafts.ron.bak"))? {
            let book = decode(&backup)
                .map_err(|error| format!("Cannot recover saved drafts: {error:?}"))?;
            return Ok(LoadedDrafts { book: Some(book), notice: Some("Recovered the previous saved drafts from backup; the original will be preserved on the next save.".into()) });
        }
        if let Some(error) = problem {
            return Err(format!(
                "Cannot read saved drafts: {error}. No usable backup; existing files were left untouched"
            ));
        }
        Ok(LoadedDrafts {
            book: None,
            notice: None,
        })
    }

    pub fn save(&self, book: &DraftBook) -> Result<(), String> {
        book.validate()?;
        let bytes = ron::ser::to_string_pretty(book, ron::ser::PrettyConfig::default())
            .map_err(|error| error.to_string())?
            .into_bytes();
        if bytes.len() as u64 > MAX_SAVE_BYTES {
            return Err("Draft collection exceeds the save size limit".into());
        }
        let primary_path = self.directory.join("drafts.ron");
        if let Some(previous) = read_optional(&primary_path)? {
            match decode(&previous) {
                Ok(_) => self.atomic_write("drafts.ron.bak", &previous)?,
                Err(DecodeError::NewerVersion(version)) => {
                    return Err(format!("Refusing to overwrite save version {version}"));
                }
                Err(DecodeError::Invalid(_)) => {
                    // Recovery may load the backup, but the damaged primary is
                    // still someone's data. Keep it separately before replacing.
                    let archive = format!("drafts.recovery-{:032x}.ron", rand::random::<u128>());
                    self.atomic_write(&archive, &previous)?;
                }
            }
        }
        self.atomic_write("drafts.ron", &bytes)
    }

    fn atomic_write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let temporary = self
            .directory
            .join(format!(".drafts-{:032x}.tmp", rand::random::<u128>()));
        let result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, self.directory.join(name))?;
            #[cfg(unix)]
            File::open(&self.directory)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            // Only our unique incomplete file; never the original or backup.
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|error| {
            format!(
                "Cannot save {}: {error}",
                self.directory.join(name).display()
            )
        })
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot read {}: {error}", path.display())),
    };
    let mut bytes = Vec::new();
    file.take(MAX_SAVE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_SAVE_BYTES {
        return Err(format!("{} exceeds the save size limit", path.display()));
    }
    Ok(Some(bytes))
}

#[derive(Debug)]
enum DecodeError {
    NewerVersion(u32),
    Invalid(String),
}

fn decode(bytes: &[u8]) -> Result<DraftBook, DecodeError> {
    #[derive(Deserialize)]
    struct Header {
        version: u32,
    }
    let header: Header =
        ron::de::from_bytes(bytes).map_err(|error| DecodeError::Invalid(error.to_string()))?;
    if header.version != SAVE_VERSION {
        return Err(DecodeError::NewerVersion(header.version));
    }
    let book: DraftBook =
        ron::de::from_bytes(bytes).map_err(|error| DecodeError::Invalid(error.to_string()))?;
    book.validate().map_err(DecodeError::Invalid)?;
    Ok(book)
}
