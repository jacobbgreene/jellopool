//! Load all required assets before enabling play; failures exit explicitly.
use crate::config::GameOptions;
use crate::prelude::*;
use crate::word_bank::WordBank;
use bevy::app::AppExit;
use bevy::asset::LoadState;
use bevy_common_assets::ron::RonAssetPlugin;

#[derive(Resource)]
pub struct GameAssets {
    pub words: Handle<WordBank>,
    pub word_font: Handle<Font>,
    pub signature_font: Option<Handle<Font>>,
}

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOptions>()
            .add_plugins(RonAssetPlugin::<WordBank>::new(&["ron"]))
            .add_systems(Startup, load_assets)
            .add_systems(Update, finish_loading.run_if(in_state(AppState::Loading)));
    }
}

fn load_assets(mut commands: Commands, server: Res<AssetServer>, options: Res<GameOptions>) {
    commands.insert_resource(GameAssets {
        words: server.load("word_bank.ron"),
        word_font: server.load(options.word_font.clone()),
        signature_font: options
            .show_signature
            .then(|| server.load("fonts/Fraunces72ptSoft-Italic.ttf")),
    });
}

fn finish_loading(
    server: Res<AssetServer>,
    assets: Res<GameAssets>,
    word_banks: Res<Assets<WordBank>>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
    mut started: Local<Option<std::time::Instant>>,
) {
    let started = started.get_or_insert_with(std::time::Instant::now);
    let states = [
        ("word bank", Some(assets.words.id().untyped())),
        ("word font", Some(assets.word_font.id().untyped())),
        (
            "signature font",
            assets
                .signature_font
                .as_ref()
                .map(|font| font.id().untyped()),
        ),
    ]
    .into_iter()
    .filter_map(|(label, id)| id.map(|id| (label, server.get_load_state(id))));
    let ready = match all_required_loaded(states) {
        Ok(ready) => ready,
        Err(error) => {
            error!("Cannot start game: {error}");
            exit.write(AppExit::error());
            return;
        }
    };
    if ready {
        let validation = word_banks
            .get(&assets.words)
            .ok_or_else(|| "loaded word bank asset is missing".to_string())
            .and_then(WordBank::validate);
        if let Err(error) = validation {
            error!("Cannot start game: {error}");
            exit.write(AppExit::error());
            return;
        }
        next.set(AppState::Playing);
    } else if started.elapsed() > std::time::Duration::from_secs(30) {
        error!("Cannot start game: asset loading timed out");
        exit.write(AppExit::error());
    }
}

fn all_required_loaded(
    states: impl IntoIterator<Item = (&'static str, Option<LoadState>)>,
) -> Result<bool, String> {
    let mut ready = true;
    for (label, state) in states {
        match state {
            Some(LoadState::Loaded) => {}
            Some(LoadState::Failed(error)) => {
                return Err(format!("{label} failed to load: {error}"));
            }
            _ => ready = false,
        }
    }
    Ok(ready)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaded_word_bank_does_not_bypass_pending_fonts() {
        for font in [None, Some(LoadState::NotLoaded), Some(LoadState::Loading)] {
            assert_eq!(
                all_required_loaded([("word bank", Some(LoadState::Loaded)), ("word font", font)]),
                Ok(false)
            );
        }
        assert_eq!(
            all_required_loaded([
                ("word bank", Some(LoadState::Loaded)),
                ("word font", Some(LoadState::Loaded))
            ]),
            Ok(true)
        );
    }

    #[test]
    fn failure_is_reported_even_while_another_asset_is_loading() {
        let failure = bevy::asset::AssetLoadError::MissingAssetLoader {
            asset_type_id: None,
            asset_path: "bad-font.ttf".into(),
        };
        let result = all_required_loaded([
            ("word bank", Some(LoadState::Loading)),
            (
                "word font",
                Some(LoadState::Failed(std::sync::Arc::new(failure))),
            ),
        ]);
        assert!(result.unwrap_err().contains("word font failed to load"));
    }
}
