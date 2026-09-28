use super::*;

#[test]
fn duplicate_words_have_independent_stable_ids_and_ron_roundtrips() {
    let mut book = book();
    let draft = &mut book.drafts[0];
    draft.title = "Café — 詩".into();
    draft.tiles[0].position = Some(PaperPosition {
        line: 23,
        x: 123.25,
    });
    draft.tray = vec![TileId(2), TileId(1)];
    assert_ne!(draft.tiles[0].id, draft.tiles[1].id);
    book.validate().unwrap();
    let encoded = ron::to_string(&book).unwrap();
    let decoded: DraftBook = ron::from_str(&encoded).unwrap();
    assert_eq!(book, decoded);
}

#[test]
fn switching_does_not_modify_either_document() {
    let mut book = book();
    let original = book.active().clone();
    book.add(PoemDocument::new(vec!["new".into()]).unwrap())
        .unwrap();
    assert_eq!(book.drafts.len(), 2);
    book.switch(&original.id).unwrap();
    assert_eq!(book.active(), &original);
    let before = book.clone();
    assert!(book.switch(&PoemId("missing".into())).is_err());
    assert_eq!(book, before);
    assert!(!book.replace_active(original).unwrap());
}

#[test]
fn malformed_documents_are_rejected() {
    type InvalidEdit = Box<dyn Fn(&mut DraftBook)>;
    let mut cases: Vec<InvalidEdit> = vec![
        Box::new(|b| b.version = 999),
        Box::new(|b| b.active = PoemId("missing".into())),
        Box::new(|b| b.drafts[0].id = PoemId("bad-id".into())),
        Box::new(|b| b.drafts[0].title = "é".repeat(TITLE_LIMIT + 1)),
        Box::new(|b| b.drafts[0].title = "bad\nline".into()),
        Box::new(|b| b.drafts[0].tiles[0].word.clear()),
        Box::new(|b| b.drafts[0].tiles[1].id = TileId(0)),
        Box::new(|b| b.drafts[0].tray = vec![TileId(0), TileId(0), TileId(2)]),
        Box::new(|b| b.drafts[0].tray.clear()),
        Box::new(|b| b.drafts.push(b.drafts[0].clone())),
    ];
    for (line, x) in [
        (24, 0.0),
        (0, -1.0),
        (0, f32::NAN),
        (0, f32::INFINITY),
        (0, 804.0),
    ] {
        cases.push(Box::new(move |b| {
            b.drafts[0].tiles[0].position = Some(PaperPosition { line, x });
            b.drafts[0].tray.remove(0);
        }));
    }
    for mutate in cases {
        let mut b = book();
        mutate(&mut b);
        assert!(b.validate().is_err(), "{b:?}");
    }
}
