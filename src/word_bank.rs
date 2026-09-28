use crate::prelude::*;
use rand::rngs::SmallRng;
use rand::seq::{IndexedRandom, SliceRandom};
use rand::{Rng, SeedableRng};

pub(crate) const WORD_COUNT: usize = 40;

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

/// Word selection driven by a caller-supplied RNG, factored out so tests and
/// seeded startup options can drive it deterministically.
pub(crate) fn select_words_with_rng(word_bank: &WordBank, rng: &mut impl Rng) -> Vec<String> {
    let mut selected_words: Vec<String> = Vec::with_capacity(WORD_COUNT);

    //TODO: Get a more consistent/optimal ratio for words
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
        let picked: Vec<String> = category.sample(&mut *rng, *count).cloned().collect();
        selected_words.extend(picked);
    }

    selected_words.shuffle(&mut *rng);
    selected_words
}

pub(crate) fn select_words(word_bank: &WordBank, seed: Option<u64>) -> Vec<String> {
    match seed {
        Some(seed) => select_words_with_rng(word_bank, &mut SmallRng::seed_from_u64(seed)),
        None => select_words_with_rng(word_bank, &mut rand::rng()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_word_bank() -> WordBank {
        let words = |prefix: &str, count: usize| -> Vec<String> {
            (0..count)
                .map(|index| format!("{prefix}-{index}"))
                .collect()
        };
        WordBank {
            nouns: words("noun", 12),
            verbs: words("verb", 12),
            adjectives: words("adjective", 10),
            adverbs: words("adverb", 8),
            pronouns: words("pronoun", 10),
            prepositions: words("preposition", 14),
            conjunctions: words("conjunction", 6),
            articles: words("article", 5),
        }
    }

    #[test]
    fn same_seed_selects_identical_words() {
        let bank = test_word_bank();
        let first = select_words_with_rng(&bank, &mut SmallRng::seed_from_u64(42));
        let second = select_words_with_rng(&bank, &mut SmallRng::seed_from_u64(42));
        assert_eq!(first, second);
        assert_eq!(first.len(), 40);
    }

    #[test]
    fn different_seeds_can_differ() {
        let bank = test_word_bank();
        let a = select_words_with_rng(&bank, &mut SmallRng::seed_from_u64(1));
        let b = select_words_with_rng(&bank, &mut SmallRng::seed_from_u64(2));
        assert_ne!(a, b);
    }
}
