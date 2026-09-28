use bevy::prelude::*;

/// Ordinary game configuration. Only the startup/devtools boundary reads env vars.
#[derive(Resource, Clone)]
pub struct GameOptions {
    pub word_font: String,
    pub show_signature: bool,
    pub seed: Option<u64>,
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            word_font: "fonts/Fraunces9ptSoft-Regular.ttf".into(),
            show_signature: true,
            seed: None,
        }
    }
}
