use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub(super) const SAVE_VERSION: u32 = 1;
pub(crate) const MAX_DRAFTS: usize = 256;
pub(crate) const TITLE_LIMIT: usize = 120;
// These define the document's logical paper, not viewport pixels. Keep the
// renderer and planner on this same format; changes require a save migration.
pub(crate) const LINE_COUNT: usize = 24;
pub(crate) const LINE_PITCH: f32 = 56.0;
pub(crate) const PAPER_WIDTH: f32 = 804.0;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct PoemId(pub String);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct TileId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum PoemStatus {
    Draft,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct PaperPosition {
    pub line: u16,
    pub x: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PoemTile {
    pub id: TileId,
    pub word: String,
    pub position: Option<PaperPosition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PoemDocument {
    pub id: PoemId,
    pub title: String,
    pub status: PoemStatus,
    pub tiles: Vec<PoemTile>,
    /// IDs, not words: repeated words are distinct, independently movable tiles.
    pub tray: Vec<TileId>,
}

impl PoemDocument {
    pub fn new(words: Vec<String>) -> Result<Self, String> {
        let tiles: Vec<_> = words
            .into_iter()
            .enumerate()
            .map(|(index, word)| PoemTile {
                id: TileId(index as u32),
                word,
                position: None,
            })
            .collect();
        let document = Self {
            id: PoemId(format!("{:032x}", rand::random::<u128>())),
            title: String::new(),
            status: PoemStatus::Draft,
            tray: tiles.iter().map(|tile| tile.id).collect(),
            tiles,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.0.len() != 32 || !self.id.0.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("invalid poem ID".into());
        }
        if self.title.chars().count() > TITLE_LIMIT
            || self
                .title
                .chars()
                .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
        {
            return Err("invalid poem title".into());
        }
        if self.tiles.is_empty() || self.tiles.len() > 256 {
            return Err("a poem must contain between 1 and 256 tiles".into());
        }
        let mut ids = HashSet::new();
        let mut unplaced = HashSet::new();
        for tile in &self.tiles {
            if !ids.insert(tile.id) {
                return Err("duplicate tile ID".into());
            }
            if tile.word.trim().is_empty()
                || tile.word.chars().count() > 128
                || tile.word.chars().any(char::is_control)
            {
                return Err("invalid tile text".into());
            }
            if let Some(pos) = tile.position {
                if usize::from(pos.line) >= LINE_COUNT
                    || !pos.x.is_finite()
                    || !(0.0..PAPER_WIDTH).contains(&pos.x)
                {
                    return Err("invalid tile position".into());
                }
            } else {
                unplaced.insert(tile.id);
            }
        }
        let tray: HashSet<_> = self.tray.iter().copied().collect();
        if tray.len() != self.tray.len() || tray != unplaced {
            return Err("tray must contain each unplaced tile exactly once".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct DraftBook {
    pub version: u32,
    pub active: PoemId,
    pub drafts: Vec<PoemDocument>,
}

impl DraftBook {
    pub fn new(document: PoemDocument) -> Self {
        Self {
            version: SAVE_VERSION,
            active: document.id.clone(),
            drafts: vec![document],
        }
    }

    pub fn active(&self) -> &PoemDocument {
        // All constructors/loads validate this relationship; changing the active
        // document goes through switch(), never an unchecked external index.
        self.drafts
            .iter()
            .find(|draft| draft.id == self.active)
            .expect("validated active poem")
    }

    pub fn replace_active(&mut self, document: PoemDocument) -> Result<bool, String> {
        document.validate()?;
        if document.id != self.active {
            return Err("cannot replace a different poem".into());
        }
        let target = self
            .drafts
            .iter_mut()
            .find(|draft| draft.id == self.active)
            .ok_or("active poem missing")?;
        if *target == document {
            return Ok(false);
        }
        *target = document;
        Ok(true)
    }

    pub fn switch(&mut self, id: &PoemId) -> Result<(), String> {
        if !self.drafts.iter().any(|draft| &draft.id == id) {
            return Err("unknown poem".into());
        }
        self.active = id.clone();
        Ok(())
    }

    pub fn add(&mut self, document: PoemDocument) -> Result<(), String> {
        document.validate()?;
        if self.drafts.len() >= MAX_DRAFTS {
            return Err("draft limit reached (256)".into());
        }
        if self.drafts.iter().any(|draft| draft.id == document.id) {
            return Err("duplicate poem ID".into());
        }
        self.active = document.id.clone();
        self.drafts.push(document);
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != SAVE_VERSION {
            return Err(format!("unsupported save version {}", self.version));
        }
        if self.drafts.is_empty() || self.drafts.len() > MAX_DRAFTS {
            return Err("invalid number of drafts".into());
        }
        let mut ids = HashSet::new();
        for draft in &self.drafts {
            draft.validate()?;
            if !ids.insert(&draft.id) {
                return Err("duplicate poem ID".into());
            }
        }
        if !ids.contains(&self.active) {
            return Err("active poem missing".into());
        }
        Ok(())
    }
}
