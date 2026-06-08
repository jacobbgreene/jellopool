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
