//! Startup-only environment parsing and opt-in review systems.
//! Gameplay consumes GameOptions; it never imports these diagnostic resources.
mod capture;
mod drag_fixture;
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
    saves: crate::poems::SaveLocation,
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
        let saves = if scene.is_some() || capture.is_some() || seed.is_some() {
            crate::poems::SaveLocation::Memory
        } else {
            save_location()
        };
        Ok(Self {
            game,
            scene,
            capture,
            scale,
            saves,
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
        app.insert_resource(self.game).insert_resource(self.saves);
        if let Some(scene) = self.scene {
            if let Some(phased) = scene.2 {
                app.insert_resource(drag_fixture::PendingDrag(phased))
                    .add_systems(
                        Update,
                        drag_fixture::apply_drag
                            .after(fixture::apply_fixture_scene)
                            .before(crate::tiles::TileInteraction)
                            .run_if(in_state(AppState::Playing))
                            .run_if(resource_exists::<drag_fixture::PendingDrag>)
                            .run_if(|pending: Option<Res<fixture::PendingFixture>>| {
                                pending.is_none()
                            }),
                    );
            }
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

fn save_location() -> crate::poems::SaveLocation {
    use crate::poems::SaveLocation;
    use std::path::PathBuf;
    let value = |key| {
        std::env::var_os(key)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let directory = value("JELLOPOOL_DATA_DIR").or_else(|| {
        #[cfg(target_os = "windows")]
        {
            value("LOCALAPPDATA").map(|path| path.join("jellopool"))
        }
        #[cfg(target_os = "macos")]
        {
            value("HOME").map(|path| path.join("Library/Application Support/jellopool"))
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            value("XDG_DATA_HOME")
                .filter(|path| path.is_absolute())
                .or_else(|| value("HOME").map(|path| path.join(".local/share")))
                .map(|path| path.join("jellopool"))
        }
    });
    match directory {
        Some(path) => SaveLocation::Directory(path),
        None => SaveLocation::Unavailable("Cannot locate your data directory. Set JELLOPOOL_DATA_DIR to enable saving; this session is temporary.".into()),
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
