mod model;
mod storage;

use super::*;

fn book() -> DraftBook {
    DraftBook::new(PoemDocument::new(vec!["the".into(), "the".into(), "café".into()]).unwrap())
}

/// Each disk test owns an isolated directory and never reads personal saves.
pub(super) struct TestDirectory(pub std::path::PathBuf);
impl TestDirectory {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "jellopool-save-test-{:032x}",
            rand::random::<u128>()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
