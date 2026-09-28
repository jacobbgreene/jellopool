//! Visual easing only; committed positions and drop decisions live in drag/placement.
use super::drag::{DragFollow, PlacementPreview};
use crate::prelude::*;

/// Ease speed for neighboring tiles sliding during drag previews.
const PUSH_SPEED: f32 = 28.0;
/// Ease speed for pickup/drop scale and shadow.
const FEEL_SPEED: f32 = 16.0;
/// Scale a tile eases to while held.
pub(super) const HELD_SCALE: f32 = 1.06;
/// Seconds the snap-into-place animation takes.
const SNAP_DURATION: f32 = 0.16;
/// Ease speeds for the snap highlight's position, size, and fade.
const HIGHLIGHT_POS_SPEED: f32 = 22.0;
const HIGHLIGHT_FADE_SPEED: f32 = 12.0;
/// Snap highlight accent: vermilion #C44732, the palette's sole accent,
/// reserved for the active landing preview over the paper writing zone.
/// Alpha scales with fade intensity.
const HIGHLIGHT_FILL: Srgba = Srgba::new(0.769, 0.278, 0.196, 0.12);
const HIGHLIGHT_BORDER: Srgba = Srgba::new(0.769, 0.278, 0.196, 0.55);

/// Original colors are captured once, so repeated Shift toggles never compound
/// alpha and release/cancel restores the exact theme (including the text).
#[derive(Component)]
pub(super) struct PhaseAppearance {
    background: BackgroundColor,
    border: BorderColor,
    text: Vec<(Entity, TextColor)>,
}

type PhaseTiles<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static DragFollow>,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
        Option<&'static Children>,
        Option<&'static PhaseAppearance>,
        Option<&'static mut TileFeel>,
    ),
    (
        With<WordTile>,
        Or<(With<DragFollow>, With<PhaseAppearance>)>,
    ),
>;

pub(super) fn phase_appearance_system(
    mut commands: Commands,
    mut tiles: PhaseTiles,
    mut text: Query<&mut TextColor>,
) {
    for (entity, follow, mut background, mut border, children, original, feel) in &mut tiles {
        let phased = follow.is_some_and(|follow| follow.phased);
        if follow.is_some()
            && let Some(mut feel) = feel
        {
            let target = if phased { 0.15 } else { 1.0 };
            if feel.shadow_target != target {
                feel.shadow_target = target;
            }
        }
        if phased && original.is_none() {
            let mut saved = PhaseAppearance {
                background: *background,
                border: *border,
                text: Vec::new(),
            };
            background.0 = background.0.with_alpha(background.0.alpha() * 0.35);
            let border = &mut *border;
            for color in [
                &mut border.top,
                &mut border.right,
                &mut border.bottom,
                &mut border.left,
            ] {
                *color = color.with_alpha(color.alpha() * 0.35);
            }
            for child in children.into_iter().flat_map(|children| children.iter()) {
                if let Ok(mut color) = text.get_mut(child) {
                    saved.text.push((child, *color));
                    color.0 = color.0.with_alpha(color.0.alpha() * 0.35);
                }
            }
            commands.entity(entity).insert(saved);
        } else if !phased && let Some(original) = original {
            *background = original.background;
            *border = original.border;
            for &(entity, color) in &original.text {
                if let Ok(mut current) = text.get_mut(entity) {
                    *current = color;
                }
            }
            commands.entity(entity).remove::<PhaseAppearance>();
        }
    }
}

/// Snapped tiles the preview writer may move: not dragged, not mid-snap.
type PlacedNodes<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static PlacedTile, &'static mut Node),
    (Without<DragFollow>, Without<SnapAnim>),
>;

/// Per-frame eased "juice" for a tile: visual scale and shadow intensity.
/// Inserted on drag start, removed once the drop animation settles.
#[derive(Component)]
pub struct TileFeel {
    pub(super) scale: f32,
    pub(super) scale_target: f32,
    /// Shadow intensity, 0.0 (none) to 1.0 (fully floating).
    pub(super) shadow: f32,
    pub(super) shadow_target: f32,
}

/// Translucent landing preview shown in the writing zone where the dragged
/// tile would snap. Sized to match the dragged tile, so the landing spot is
/// unambiguous.
#[derive(Component)]
pub struct SnapHighlight {
    pos: Vec2,
    target: Vec2,
    size: Vec2,
    target_size: Vec2,
    alpha: f32,
    target_alpha: f32,
}

/// Plays after a tile is dropped into the writing zone: eases the tile
/// from its release position into the snapped grid cell.
#[derive(Component)]
pub struct SnapAnim {
    pub(super) from: Vec2,
    pub(super) to: Vec2,
    /// 0.0 → 1.0 over SNAP_DURATION seconds.
    pub(super) t: f32,
}

fn shadow(intensity: f32) -> BoxShadow {
    BoxShadow::new(
        // Neutral-warm gray, retuned for the light paper surround: the old
        // near-black mix was tuned against a dark desk and reads muddy here.
        // Alpha structure and geometry are unchanged.
        Color::srgba(0.16, 0.15, 0.13, 0.35 * intensity),
        Val::Px(0.0),
        Val::Px(6.0 * intensity),
        Val::Px(2.0 * intensity),
        Val::Px(10.0 * intensity),
    )
}

pub(super) fn px_or_zero(value: Val) -> f32 {
    match value {
        Val::Px(px) => px,
        _ => 0.0,
    }
}

fn ease_factor(speed: f32, dt: f32) -> f32 {
    1.0 - (-speed * dt).exp()
}

/// Ease-out with a slight overshoot — the "click" into place.
fn ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    1.0 + C3 * (t - 1.0).powi(3) + C1 * (t - 1.0).powi(2)
}

/// Eases pickup/drop scale and shadow, and cleans up once settled.
pub fn tile_feel_system(
    mut commands: Commands,
    time: Res<Time>,
    mut tiles: Query<(Entity, &mut TileFeel, &mut UiTransform), With<WordTile>>,
) {
    let ease = ease_factor(FEEL_SPEED, time.delta_secs());
    for (entity, mut feel, mut ui_transform) in &mut tiles {
        feel.scale += (feel.scale_target - feel.scale) * ease;
        feel.shadow += (feel.shadow_target - feel.shadow) * ease;

        if (feel.scale - feel.scale_target).abs() < 0.001 {
            feel.scale = feel.scale_target;
        }
        if (feel.shadow - feel.shadow_target).abs() < 0.01 {
            feel.shadow = feel.shadow_target;
        }

        ui_transform.scale = Vec2::splat(feel.scale);
        if feel.shadow > 0.0 {
            commands.entity(entity).insert(shadow(feel.shadow));
        } else {
            commands.entity(entity).remove::<BoxShadow>();
        }

        // Back at rest: animation done, stop running it for this tile.
        if feel.scale == 1.0 && feel.shadow == 0.0 {
            commands.entity(entity).remove::<TileFeel>();
        }
    }
}

/// Shows the shared plan's landing position while a dragged tile hovers
/// the writing zone. Drop remains an independent authoritative computation.
pub fn zone_snap_highlight_system(
    mut commands: Commands,
    time: Res<Time>,
    zone: Single<Entity, With<WritingZone>>,
    preview: Res<PlacementPreview>,
    mut highlight: Query<
        (
            &mut SnapHighlight,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Without<WordTile>,
    >,
) {
    let zone_entity = *zone;
    let snap = preview.0.as_ref().map(|plan| (plan.cell, plan.size));

    // Ensure the highlight exists, as the zone's first child so dropped
    // tiles paint above it.
    let Some((mut highlight, mut node, mut background, mut border)) = highlight.single_mut().ok()
    else {
        let entity = commands
            .spawn((
                Name::new("snap_highlight"),
                LayoutConfig {
                    use_rounding: false,
                },
                SnapHighlight {
                    pos: Vec2::ZERO,
                    target: Vec2::ZERO,
                    size: Vec2::ZERO,
                    target_size: Vec2::ZERO,
                    alpha: 0.0,
                    target_alpha: 0.0,
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    // Match the paper tile's silhouette.
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(super::TILE_RADIUS)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::from(Color::NONE),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(zone_entity).insert_children(0, &[entity]);
        return;
    };

    match snap {
        Some((cell, tile_size)) => {
            highlight.target = cell;
            highlight.target_size = tile_size;
            highlight.target_alpha = 1.0;
        }
        None => {
            highlight.target_alpha = 0.0;
        }
    }

    if highlight.alpha == 0.0 && highlight.target_alpha == 0.0 {
        return;
    }

    // Ease position, size, and fade, then apply.
    let pos_ease = ease_factor(HIGHLIGHT_POS_SPEED, time.delta_secs());
    highlight.pos = highlight.pos.lerp(highlight.target, pos_ease);
    if (highlight.target - highlight.pos).length() < 0.5 {
        highlight.pos = highlight.target;
    }
    highlight.size = highlight.size.lerp(highlight.target_size, pos_ease);
    if (highlight.target_size - highlight.size).length() < 0.5 {
        highlight.size = highlight.target_size;
    }
    let fade_ease = ease_factor(HIGHLIGHT_FADE_SPEED, time.delta_secs());
    highlight.alpha += (highlight.target_alpha - highlight.alpha) * fade_ease;
    if (highlight.target_alpha - highlight.alpha).abs() < 0.01 {
        highlight.alpha = highlight.target_alpha;
    }

    let mut fill = HIGHLIGHT_FILL;
    fill.alpha *= highlight.alpha;
    let mut outline = HIGHLIGHT_BORDER;
    outline.alpha *= highlight.alpha;

    node.left = Val::Px(highlight.pos.x);
    node.top = Val::Px(highlight.pos.y);
    node.width = Val::Px(highlight.size.x);
    node.height = Val::Px(highlight.size.y);
    *background = BackgroundColor(Color::from(fill));
    *border = BorderColor::from(Color::from(outline));
}

/// Plays the snap-into-place animation for a tile dropped in the writing
/// zone: ease-out-back from the release point into the grid cell.
pub fn snap_anim_system(
    mut commands: Commands,
    time: Res<Time>,
    mut tiles: Query<(Entity, &mut SnapAnim, &mut Node), With<WordTile>>,
) {
    for (entity, mut anim, mut node) in &mut tiles {
        anim.t = (anim.t + time.delta_secs() / SNAP_DURATION).min(1.0);
        let eased = ease_out_back(anim.t);
        let pos = anim.from.lerp(anim.to, eased);
        if node.left != Val::Px(pos.x) || node.top != Val::Px(pos.y) {
            node.left = Val::Px(pos.x);
            node.top = Val::Px(pos.y);
        }
        if anim.t >= 1.0 {
            node.left = Val::Px(anim.to.x);
            node.top = Val::Px(anim.to.y);
            commands.entity(entity).remove::<SnapAnim>();
        }
    }
}

/// One writer for live push/rollback motion. Drop snaps are excluded.
pub fn push_preview_system(time: Res<Time>, active: Query<&DragFollow>, mut nodes: PlacedNodes) {
    for (entity, placed, mut node) in &mut nodes {
        let target = active
            .single()
            .ok()
            .and_then(|follow| follow.pushes.iter().find(|tile| tile.entity == entity))
            .map_or(placed.0, |tile| tile.pos);
        let current = Vec2::new(px_or_zero(node.left), px_or_zero(node.top));
        let pos = if current.distance(target) < 0.5 {
            target
        } else {
            current.lerp(target, ease_factor(PUSH_SPEED, time.delta_secs()))
        };
        if node.left != Val::Px(pos.x) || node.top != Val::Px(pos.y) {
            node.left = Val::Px(pos.x);
            node.top = Val::Px(pos.y);
        }
    }
}
