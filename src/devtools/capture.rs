//! Opt-in, windowless screenshots and strict visual validation.
//! JELLOPOOL_CAPTURE=/absolute/path.png; optional JELLOPOOL_CAPTURE_SIZE=1600x900 and
//! JELLOPOOL_SCALE_FACTOR=1.5. Uses the ordinary scene/seed fixture flags.

use crate::prelude::*;
use bevy::app::{AppExit, ScheduleRunnerPlugin};
use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::text::TextLayoutInfo;
use std::time::{Duration, Instant};

#[derive(Resource)]
pub struct Capture {
    path: std::path::PathBuf,
    size: UVec2,
    scale: f32,
    image: Handle<Image>,
    started: Instant,
}

impl Capture {
    pub fn from_env(scale: f32) -> Result<Option<Self>, String> {
        let Some(path) = std::env::var_os("JELLOPOOL_CAPTURE") else {
            return Ok(None);
        };
        if path.is_empty() {
            return Err("JELLOPOOL_CAPTURE must be a nonempty image path".into());
        }
        let size = parse_size(
            std::env::var("JELLOPOOL_CAPTURE_SIZE").ok().as_deref(),
            scale,
        )?;
        Ok(Some(Self {
            path: path.into(),
            size,
            scale,
            image: default(),
            started: Instant::now(),
        }))
    }
}

fn parse_size(value: Option<&str>, scale: f32) -> Result<UVec2, String> {
    let size = value
        .unwrap_or("1600x900")
        .split_once('x')
        .and_then(|(width, height)| Some(UVec2::new(width.parse().ok()?, height.parse().ok()?)))
        .ok_or_else(|| "JELLOPOOL_CAPTURE_SIZE must be WIDTHxHEIGHT".to_string())?;
    let physical = size.as_vec2() * scale;
    if !physical.is_finite() || physical.min_element() < 1.0 || physical.max_element() > 8192.0 {
        return Err(
            "Capture dimensions must be between 1 and 8192 physical pixels per axis".into(),
        );
    }
    Ok(size)
}

pub fn configure(app: &mut App, capture: Capture) {
    app.insert_resource(capture)
        .add_plugins(ScheduleRunnerPlugin::run_loop(
            std::time::Duration::from_secs_f64(1.0 / 60.0),
        ))
        .add_systems(Startup, setup.after(crate::board::spawn_camera))
        .add_systems(Update, capture_frame.after(crate::tiles::TileInteraction));
}

fn setup(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    mut images: ResMut<Assets<Image>>,
    camera: Single<Entity, With<Camera2d>>,
) {
    let size = (capture.size.as_vec2() * capture.scale).as_uvec2();
    let mut image = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    capture.image = images.add(image);
    commands.entity(*camera).insert((
        IsDefaultUiCamera,
        RenderTarget::Image(ImageRenderTarget {
            handle: capture.image.clone(),
            scale_factor: capture.scale,
        }),
    ));
}

#[allow(clippy::too_many_arguments)]
fn capture_frame(
    mut commands: Commands,
    capture: Res<Capture>,
    state: Res<State<AppState>>,
    pending: Option<Res<super::fixture::PendingFixture>>,
    pending_drag: Option<Res<super::drag_fixture::PendingDrag>>,
    tiles: Query<(&Name, &ComputedNode, &UiGlobalTransform, &ChildOf), With<WordTile>>,
    regions: Query<(&ComputedNode, &UiGlobalTransform)>,
    text: Query<(&Text, &TextLayoutInfo)>,
    mut exit: MessageWriter<AppExit>,
    mut frames: Local<u32>,
) {
    if capture.started.elapsed() > Duration::from_secs(30) {
        error!("Capture timed out waiting for loaded text, a valid scene, or screenshot readback");
        exit.write(AppExit::error());
        return;
    }
    if *state.get() != AppState::Playing
        || pending.is_some()
        || pending_drag.is_some()
        || tiles.iter().count() != crate::word_bank::WORD_COUNT
        || text.is_empty()
        || text.iter().any(|(value, text)| {
            !value.is_empty() && (text.glyphs.is_empty() || text.size.min_element() <= 0.0)
        })
        || tiles
            .iter()
            .any(|(_, node, _, _)| node.size.min_element() <= 0.0)
    {
        *frames = 0;
        return;
    }
    *frames += 1;
    // Allow font measurement, shader compilation and tray motion to settle.
    if *frames != 90 {
        return;
    }
    let path = capture.path.clone();
    info!(
        "Capturing {} tiles at {:?}, scale {}",
        tiles.iter().count(),
        capture.size,
        capture.scale
    );
    for (name, node, transform, parent) in &tiles {
        if node.border.min_inset.min_element() <= 0.0 || node.border.max_inset.min_element() <= 0.0
        {
            error!("Capture: tile {name} has a missing border edge");
            exit.write(AppExit::error());
            return;
        }
        if let Ok((region, region_transform)) = regions.get(parent.parent()) {
            let bounds = Rect::from_center_size(region_transform.translation, region.size);
            let tile = Rect::from_center_size(transform.translation, node.size);
            if tile.min.cmplt(bounds.min - Vec2::ONE).any()
                || tile.max.cmpgt(bounds.max + Vec2::ONE).any()
            {
                error!("Capture: tile {name} extends outside its region: {tile:?} vs {bounds:?}");
                exit.write(AppExit::error());
                return;
            }
        } else {
            error!("Capture: tile {name} has no measured parent region");
            exit.write(AppExit::error());
            return;
        }
    }
    for [(name_a, a, ta, pa), (name_b, b, tb, pb)] in tiles.iter_combinations() {
        if pa.parent() == pb.parent()
            && rects_overlap(
                Rect::from_center_size(ta.translation, a.size),
                Rect::from_center_size(tb.translation, b.size),
            )
        {
            error!("Capture: tiles {name_a} and {name_b} overlap");
            exit.write(AppExit::error());
            return;
        }
    }
    commands
        // The scale is part of the target identity. Screenshot::image assumes
        // 1.0, which would capture a different (blank) target at fractional DPI.
        .spawn(Screenshot(RenderTarget::Image(ImageRenderTarget {
            handle: capture.image.clone(),
            scale_factor: capture.scale,
        })))
        .observe(
            move |event: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                let result = event
                    .image
                    .clone()
                    .try_into_dynamic()
                    .map_err(|error| error.to_string())
                    .and_then(|image| image.save(&path).map_err(|error| error.to_string()));
                match result {
                    Ok(()) => {
                        info!("Screenshot saved to {}", path.display());
                        exit.write(AppExit::Success);
                    }
                    Err(error) => {
                        error!("Screenshot failed: {error}");
                        exit.write(AppExit::error());
                    }
                }
            },
        );
}

fn rects_overlap(a: Rect, b: Rect) -> bool {
    // Ignore subpixel edge rounding, but not a visible overlap.
    (a.max.min(b.max) - a.min.max(b.min)).min_element() > 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_size_is_strict_and_bounded() {
        assert_eq!(parse_size(None, 1.5), Ok(UVec2::new(1600, 900)));
        for value in ["bad", "0x900", "100x0", "-1x10", "999999x900", "10x10x10"] {
            assert!(parse_size(Some(value), 1.0).is_err());
        }
        assert!(parse_size(Some("8192x8192"), 1.5).is_err());
    }

    #[test]
    fn shared_edges_are_not_overlaps() {
        let a = Rect::new(0.0, 0.0, 80.0, 40.0);
        assert!(!rects_overlap(a, Rect::new(80.0, 0.0, 160.0, 40.0)));
        assert!(rects_overlap(a, Rect::new(78.0, 0.0, 160.0, 40.0)));
    }
}
