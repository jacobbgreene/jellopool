use bevy::prelude::*;
use rand::seq::{IndexedRandom, SliceRandom};

#[derive(Asset, TypePath, serde::Deserialize)]
pub struct WordBank {
    pub nouns: Vec<String>,
    pub verbs: Vec<String>,
    pub adjectives: Vec<String>,
    pub adverbs: Vec<String>,
    pub pronouns: Vec<String>,
    pub prepositions: Vec<String>,
    pub conjunctions: Vec<String>,
    pub articles: Vec<String>,
}

#[derive(Resource)]
pub(crate) struct WordBankHandle(pub(crate) Handle<WordBank>);

pub fn load_word_bank(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset: Handle<WordBank> = asset_server.load("word_bank.ron");
    commands.insert_resource(WordBankHandle(asset));
}

pub(crate) fn select_words(word_bank: &WordBank) -> Vec<String> {
    let mut selected_words: Vec<String> = Vec::new();
    let mut rng = rand::rng();

    //TODO: Get a more consistent/optimal ration for words
    // There should be a slider or something for users to pick how many words they want
    let plan = [
        (&word_bank.nouns, 6),
        (&word_bank.verbs, 6),
        (&word_bank.adjectives, 4),
        (&word_bank.adverbs, 2),
        (&word_bank.pronouns, 6),
        (&word_bank.prepositions, 10),
        (&word_bank.conjunctions, 3),
        (&word_bank.articles, 3),
    ];

    for (category, count) in &plan {
        let picked: Vec<String> = category.sample(&mut rng, *count).cloned().collect();
        selected_words.extend(picked);
    }

    selected_words.shuffle(&mut rng);
    return selected_words;
}
