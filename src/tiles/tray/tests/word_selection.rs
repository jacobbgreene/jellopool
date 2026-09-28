//! Real font shaping and flex layout, without a window or GPU.
use crate::config::GameOptions;
use crate::prelude::*;
use crate::tiles::presentation::{
    fit_board_to_viewport, fit_page_lead, spawn_all_tiles, spawn_board_root,
};
use crate::word_bank::{WORD_COUNT, WordBank, select_words};
use bevy::app::{AppExit, PropagateSet};
use bevy::text::{
    FontAtlasSet, LayoutCx, RemSize, ScaleCx, TextIterScratch, TextLayoutInfo, TextPipeline,
    detect_text_needs_rerender, load_font_assets_into_font_collection,
};
use bevy::ui::{
    UiSystems,
    widget::{measure_text_system, text_system},
};

fn font_layout_app(size: Vec2, dpi: f32) -> App {
    let scale = (size.x / 1600.0).min(size.y / 900.0).min(1.0);
    let mut app = crate::test_support::ui::layout_app(size / scale, dpi, scale);
    app.init_resource::<Assets<Font>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<WordBank>>()
        .init_resource::<TextPipeline>()
        .init_resource::<LayoutCx>()
        .init_resource::<TextIterScratch>()
        .init_resource::<RemSize>()
        .init_resource::<FontAtlasSet>()
        .init_resource::<ScaleCx>()
        .add_message::<AppExit>()
        .insert_resource(GameOptions {
            seed: Some(0),
            show_signature: false,
            ..default()
        })
        .add_systems(Startup, (spawn_board_root, spawn_all_tiles).chain())
        .add_systems(PreUpdate, (fit_board_to_viewport, fit_page_lead).chain())
        .add_systems(
            PostUpdate,
            (
                load_font_assets_into_font_collection,
                detect_text_needs_rerender,
                measure_text_system,
            )
                .chain()
                .after(PropagateSet::<ComputedUiRenderTargetInfo>::default())
                .before(UiSystems::Layout),
        )
        .add_systems(PostUpdate, text_system.in_set(UiSystems::PostLayout));
    let word_font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .join(GameOptions::default().word_font),
            )
            .expect("the configured default word font must be shipped with the game"),
        ));
    let words = app
        .world_mut()
        .resource_mut::<Assets<WordBank>>()
        .add(ron::from_str::<WordBank>(include_str!("../../../../assets/word_bank.ron")).unwrap());
    app.insert_resource(GameAssets {
        words,
        word_font,
        signature_font: None,
    });
    for _ in 0..3 {
        app.update();
    }
    app
}

fn tray_tiles(app: &mut App) -> (Entity, Vec<(Entity, Entity)>) {
    let tray = app
        .world_mut()
        .query_filtered::<Entity, With<BoardTray>>()
        .single(app.world())
        .unwrap();
    // Preserve actual flex order so the reported seed reproduces the arrangement.
    let tiles = app
        .world()
        .get::<Children>(tray)
        .unwrap()
        .iter()
        .filter(|&entity| app.world().get::<WordTile>(entity).is_some())
        .map(|entity| (entity, app.world().get::<Children>(entity).unwrap()[0]))
        .collect::<Vec<_>>();
    assert_eq!(tiles.len(), WORD_COUNT);
    (tray, tiles)
}

fn layout_words(app: &mut App, tiles: &[(Entity, Entity)], words: &[String]) {
    assert!(words.len() <= tiles.len());
    for ((_, label), word) in tiles.iter().zip(words) {
        app.world_mut()
            .get_mut::<Text>(*label)
            .unwrap()
            .0
            .clone_from(word);
    }
    app.update();
    app.update();
}

fn assert_fits(app: &App, tray: Entity, tiles: &[(Entity, Entity)], words: &[String], case: &str) {
    assert_eq!(words.len(), WORD_COUNT);
    let node = app.world().get::<ComputedNode>(tray).unwrap();
    let transform = app.world().get::<UiGlobalTransform>(tray).unwrap();
    let bounds = Rect::from_center_size(transform.translation, node.size);
    let camera = app
        .world()
        .get::<ComputedUiTargetCamera>(tray)
        .unwrap()
        .get()
        .unwrap();
    let viewport = app
        .world()
        .get::<Camera>(camera)
        .unwrap()
        .physical_viewport_rect()
        .unwrap()
        .as_rect();
    assert!(
        bounds.min.cmpge(viewport.min - Vec2::ONE).all()
            && bounds.max.cmple(viewport.max + Vec2::ONE).all(),
        "{case}: tray exceeds viewport"
    );
    let mut rectangles = Vec::new();
    for ((tile, label), word) in tiles.iter().zip(words) {
        let node = app.world().get::<ComputedNode>(*tile).unwrap();
        let center = app
            .world()
            .get::<UiGlobalTransform>(*tile)
            .unwrap()
            .translation;
        let rect = Rect::from_center_size(center, node.size);
        assert!(
            rect.min.cmpge(bounds.min - Vec2::ONE).all()
                && rect.max.cmple(bounds.max + Vec2::ONE).all(),
            "{case}, word={word}, tile={rect:?}, tray={bounds:?}"
        );
        assert!(node.size.min_element() > 0.0);
        assert!(
            !app.world()
                .get::<TextLayoutInfo>(*label)
                .unwrap()
                .glyphs
                .is_empty(),
            "{case}: word {word} wasn't shaped"
        );
        for previous in &rectangles {
            let intersection = rect.intersect(*previous);
            assert!(
                intersection.width() <= 1.0 || intersection.height() <= 1.0,
                "{case}: overlapping tiles"
            );
        }
        rectangles.push(rect);
    }
}

#[test]
fn randomized_selections_fit_the_tray_with_real_font_metrics() {
    let bank: WordBank = ron::from_str(include_str!("../../../../assets/word_bank.ron")).unwrap();
    for (size, dpi) in [
        (Vec2::new(1600.0, 900.0), 1.0),
        (Vec2::new(1280.0, 720.0), 1.5),
        (Vec2::new(1920.0, 1200.0), 2.0),
    ] {
        let mut app = font_layout_app(size, dpi);
        let (tray, tiles) = tray_tiles(&mut app);
        for seed in 0..1024 {
            let words = select_words(&bank, Some(seed)).unwrap();
            layout_words(&mut app, &tiles, &words);
            assert_fits(
                &app,
                tray,
                &tiles,
                &words,
                &format!("seed={seed}, size={size}, dpi={dpi}"),
            );
        }
    }
}

#[test]
fn widest_valid_category_selection_fits_the_tray() {
    use rand::{SeedableRng, rngs::SmallRng, seq::SliceRandom};
    let bank: WordBank = ron::from_str(include_str!("../../../../assets/word_bank.ron")).unwrap();
    let mut app = font_layout_app(Vec2::new(1600.0, 900.0), 1.0);
    let (tray, tiles) = tray_tiles(&mut app);
    let mut widest = Vec::new();
    // Pick by measured glyph widths, not character count. Ordinary random seeds
    // are unlikely to draw all of the widest words in every category at once.
    for (category, quota) in [
        (&bank.nouns, 6),
        (&bank.verbs, 6),
        (&bank.adjectives, 4),
        (&bank.adverbs, 2),
        (&bank.pronouns, 6),
        (&bank.prepositions, 10),
        (&bank.conjunctions, 3),
        (&bank.articles, 3),
    ] {
        let mut measured = Vec::new();
        for chunk in category.chunks(WORD_COUNT) {
            layout_words(&mut app, &tiles, chunk);
            measured.extend(tiles.iter().zip(chunk).map(|((tile, _), word)| {
                (
                    app.world().get::<ComputedNode>(*tile).unwrap().size.x,
                    word.clone(),
                )
            }));
        }
        measured.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        widest.extend(measured.into_iter().take(quota).map(|(_, word)| word));
    }
    assert_eq!(widest.len(), WORD_COUNT);
    for seed in 0..32 {
        let mut words = widest.clone();
        words.shuffle(&mut SmallRng::seed_from_u64(seed));
        layout_words(&mut app, &tiles, &words);
        assert_fits(
            &app,
            tray,
            &tiles,
            &words,
            &format!("widest selection / permutation={seed}"),
        );
        assert!(app.world().get::<ComputedNode>(tray).unwrap().size.y > 207.0);
        let separator = app
            .world()
            .get::<Children>(tray)
            .unwrap()
            .iter()
            .find(|&child| {
                app.world()
                    .get::<Name>(child)
                    .is_some_and(|name| name.as_str() == "bank_separator")
            })
            .unwrap();
        let node = app.world().get::<ComputedNode>(separator).unwrap();
        let center = app
            .world()
            .get::<UiGlobalTransform>(separator)
            .unwrap()
            .translation;
        let tray_node = app.world().get::<ComputedNode>(tray).unwrap();
        let tray_center = app
            .world()
            .get::<UiGlobalTransform>(tray)
            .unwrap()
            .translation;
        assert!(
            (center.y - node.size.y * 0.5 - (tray_center.y - tray_node.size.y * 0.5)).abs() <= 1.0
        );
    }
}
