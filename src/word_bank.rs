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

impl WordBank {
    /// One source of truth for both validation and category quotas.
    fn selection_plan(&self) -> [(&str, &[String], usize); 8] {
        [
            ("nouns", &self.nouns, 6),
            ("verbs", &self.verbs, 6),
            ("adjectives", &self.adjectives, 4),
            ("adverbs", &self.adverbs, 2),
            ("pronouns", &self.pronouns, 6),
            ("prepositions", &self.prepositions, 10),
            ("conjunctions", &self.conjunctions, 3),
            ("articles", &self.articles, 3),
        ]
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        for (name, words, required) in self.selection_plan() {
            if words.len() < required {
                return Err(format!(
                    "word bank category {name} needs at least {required} entries, found {}",
                    words.len()
                ));
            }
        }
        Ok(())
    }
}

/// Validate before sampling: slice::sample otherwise silently truncates a quota.
pub(crate) fn select_words_with_rng(
    word_bank: &WordBank,
    rng: &mut impl Rng,
) -> Result<Vec<String>, String> {
    word_bank.validate()?;
    let mut selected_words = Vec::with_capacity(WORD_COUNT);

    for (_, category, count) in word_bank.selection_plan() {
        selected_words.extend(category.sample(&mut *rng, count).cloned());
    }

    selected_words.shuffle(&mut *rng);
    Ok(selected_words)
}

pub(crate) fn select_words(word_bank: &WordBank, seed: Option<u64>) -> Result<Vec<String>, String> {
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
        let first = select_words_with_rng(&bank, &mut SmallRng::seed_from_u64(42)).unwrap();
        let second = select_words_with_rng(&bank, &mut SmallRng::seed_from_u64(42)).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.len(), 40);
    }

    #[test]
    fn different_seeds_can_differ() {
        let bank = test_word_bank();
        let a = select_words_with_rng(&bank, &mut SmallRng::seed_from_u64(1)).unwrap();
        let b = select_words_with_rng(&bank, &mut SmallRng::seed_from_u64(2)).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn every_category_must_meet_its_quota_before_sampling() {
        let names = [
            "nouns",
            "verbs",
            "adjectives",
            "adverbs",
            "pronouns",
            "prepositions",
            "conjunctions",
            "articles",
        ];
        let quotas = [6, 6, 4, 2, 6, 10, 3, 3];
        assert_eq!(quotas.iter().sum::<usize>(), WORD_COUNT);
        for index in 0..names.len() {
            let mut bank = test_word_bank();
            let categories = [
                &mut bank.nouns,
                &mut bank.verbs,
                &mut bank.adjectives,
                &mut bank.adverbs,
                &mut bank.pronouns,
                &mut bank.prepositions,
                &mut bank.conjunctions,
                &mut bank.articles,
            ];
            categories[index].truncate(quotas[index] - 1);
            for seed in [None, Some(42)] {
                let error = select_words(&bank, seed).unwrap_err();
                assert!(error.contains(names[index]), "{error}");
                assert!(
                    error.contains(&format!(
                        "at least {} entries, found {}",
                        quotas[index],
                        quotas[index] - 1
                    )),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn syntactically_valid_ron_can_still_be_an_invalid_word_bank() {
        let bank: WordBank = ron::from_str("(nouns: [], verbs: [], adjectives: [], adverbs: [], pronouns: [], prepositions: [], conjunctions: [], articles: [])").unwrap();
        assert!(bank.validate().is_err());
        assert!(select_words(&bank, Some(42)).is_err());
    }

    #[test]
    fn shipped_bank_always_selects_forty_words() {
        let bank: WordBank = ron::from_str(include_str!("../assets/word_bank.ron")).unwrap();
        for seed in 0..1024 {
            assert_eq!(select_words(&bank, Some(seed)).unwrap().len(), WORD_COUNT);
        }
    }
}
