use super::*;
use crate::config::GameOptions;
use crate::word_bank::WordBank;

fn spawn_app(bank: Option<WordBank>) -> App {
    let mut app = App::new();
    crate::test_support::ui::enable_scenes(&mut app);
    app.init_resource::<Assets<WordBank>>()
        .init_resource::<GameOptions>()
        .add_message::<AppExit>()
        .add_systems(Startup, spawn_all_tiles);
    let words = bank
        .map(|bank| app.world_mut().resource_mut::<Assets<WordBank>>().add(bank))
        .unwrap_or_default();
    app.insert_resource(GameAssets {
        words,
        word_font: default(),
        signature_font: None,
    });
    app.world_mut().spawn((BoardTray, Node::default()));
    app
}

#[test]
fn missing_or_invalid_word_banks_exit_instead_of_spawning_a_partial_tray() {
    let invalid: WordBank = ron::from_str("(nouns: [], verbs: [], adjectives: [], adverbs: [], pronouns: [], prepositions: [], conjunctions: [], articles: [])").unwrap();
    for bank in [None, Some(invalid)] {
        let mut app = spawn_app(bank);
        app.update();
        let exits: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<AppExit>>()
            .drain()
            .collect();
        assert_eq!(exits, vec![AppExit::error()]);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<WordTile>>()
                .iter(app.world())
                .count(),
            0
        );
    }
}

#[test]
fn a_valid_bank_spawns_all_forty_tiles_without_an_exit() {
    let bank = ron::from_str(include_str!("../../../assets/word_bank.ron")).unwrap();
    let mut app = spawn_app(Some(bank));
    app.update();
    assert!(app.world().resource::<Messages<AppExit>>().is_empty());
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<WordTile>>()
            .iter(app.world())
            .count(),
        crate::word_bank::WORD_COUNT
    );
}
