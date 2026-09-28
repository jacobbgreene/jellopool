//! Durable poem data and local storage. No UI entities, animation, or Bevy scenes.
mod model;
mod session;
pub(crate) mod storage;
pub(crate) use session::{DraftSession, PoemsPlugin, SaveLocation};

pub(crate) use model::{
    DraftBook, LINE_COUNT, LINE_PITCH, MAX_DRAFTS, PAPER_WIDTH, PaperPosition, PoemDocument,
    PoemId, TITLE_LIMIT, TileId,
};

#[cfg(test)]
mod tests;
