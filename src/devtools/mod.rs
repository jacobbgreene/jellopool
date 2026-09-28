//! Startup-only environment parsing and opt-in review systems.
//! Gameplay consumes GameOptions; it never imports these diagnostic resources.
mod capture;
mod fixture;

use crate::config::GameOptions;
use crate::prelude::*;
use bevy::window::{MonitorSelection, WindowMode, WindowResolution};

pub struct RunOptions {
    game: GameOptions,
    scene: Option<fixture::FixtureScene>,
    capture: Option<capture::Capture>,
    windowed: bool,
    scale: f32,
}

impl RunOptions {
    pub fn from_env() -> Result<Self, String> {
        let scene = std::env::var("JELLOPOOL_SCENE")
            .ok()
            .map(|value| fixture::parse_scene(&value))
            .transpose()?;
        let scale = parse_scale(std::env::var("JELLOPOOL_SCALE_FACTOR").ok().as_deref())?;
        let capture = capture::Capture::from_env(scale)?;
        let seed = std::env::var("JELLOPOOL_SEED")
            .ok()
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| "JELLOPOOL_SEED must be a u64".to_string())
            })
            .transpose()?
            .or(scene.map(|_| fixture::FIXTURE_SEED));
        let mut game = GameOptions { seed, ..default() };
        if capture.is_some() {
            if let Ok(font) = std::env::var("JELLOPOOL_CAPTURE_FONT") {
                game.word_font = font;
            }
            game.show_signature = std::env::var_os("JELLOPOOL_CAPTURE_NO_SIGNATURE").is_none();
        }
        Ok(Self {
            game,
            scene,
            capture,
            scale,
            windowed: std::env::var_os("JELLOPOOL_WINDOWED").is_some(),
        })
    }

    pub fn headless(&self) -> bool {
        self.capture.is_some()
    }

    pub fn window(&self) -> Window {
        if !self.windowed {
            return Window {
                resizable: false,
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                ..default()
            };
        }
        Window {
            resizable: false,
            mode: WindowMode::Windowed,
            resolution: WindowResolution::new(
                (1600.0 * self.scale) as u32,
                (900.0 * self.scale) as u32,
            )
            .with_scale_factor_override(self.scale),
            ..default()
        }
    }

    pub fn configure(self, app: &mut App) {
        app.insert_resource(self.game);
        if let Some(scene) = self.scene {
            app.insert_resource(scene)
                .add_systems(
                    OnEnter(AppState::Playing),
                    fixture::arm_fixture_scene.after(crate::tiles::spawn_all_tiles),
                )
                .add_systems(
                    Update,
                    fixture::apply_fixture_scene
                        .before(crate::tiles::TileInteraction)
                        .run_if(
                            in_state(AppState::Playing)
                                .and_then(resource_exists::<fixture::PendingFixture>),
                        ),
                );
        }
        if let Some(capture) = self.capture {
            capture::configure(app, capture);
        }
    }
}

fn parse_scale(value: Option<&str>) -> Result<f32, String> {
    match value {
        None => Ok(1.0),
        Some(value) => value
            .parse::<f32>()
            .ok()
            .filter(|scale| scale.is_finite() && *scale >= 0.25 && *scale <= 4.0)
            .ok_or_else(|| "JELLOPOOL_SCALE_FACTOR must be finite and between 0.25 and 4".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scale_validation_rejects_unrenderable_values() {
        assert_eq!(parse_scale(None), Ok(1.0));
        assert_eq!(parse_scale(Some("1.5")), Ok(1.5));
        for value in ["NaN", "inf", "0", "-1", "0.0001", "100000", "oops"] {
            assert!(parse_scale(Some(value)).is_err());
        }
    }
}
