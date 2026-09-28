use super::*;
use bevy::input::{ButtonState, InputPlugin};
use bevy::input_focus::{FocusCause, InputDispatchPlugin, InputFocusPlugin};
use bevy::text::{FontCx, LayoutCx, TextEdit, apply_text_edits};
use bevy::ui_widgets::EditableTextInputPlugin;
use bevy::window::{Ime, PrimaryWindow};

fn editor_app() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        InputPlugin,
        InputFocusPlugin,
        InputDispatchPlugin,
        EditableTextInputPlugin,
        TitlePlugin,
    ))
    .init_resource::<Time>()
    .init_resource::<UiScale>()
    .init_resource::<FontCx>()
    .init_resource::<LayoutCx>()
    .init_resource::<bevy::clipboard::Clipboard>()
    .add_message::<Ime>()
    .add_message::<WindowFocused>()
    .add_message::<Pointer<Release>>()
    .add_systems(Startup, |mut commands: Commands| {
        commands
            .spawn(Node::default())
            .with_children(|root| spawn_title(root, default()));
    })
    .add_systems(PostUpdate, apply_text_edits.in_set(EditableTextSystems));
    crate::test_support::ui::enable_scenes(&mut app);
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    // Cursor movement needs real glyph clusters even without a renderer.
    // Use the shipped font rather than depending on the host's installed fonts.
    let font = Font::from_bytes(
        include_bytes!("../../../assets/fonts/Fraunces9ptSoft-Regular.ttf").to_vec(),
    );
    {
        let mut fonts = app.world_mut().resource_mut::<FontCx>();
        let families = fonts.collection.register_fonts(font.data, None);
        let family = fonts
            .collection
            .family_name(families[0].0)
            .unwrap()
            .to_string();
        fonts.set_sans_serif_family(&family).unwrap();
    }
    app.update();
    let title = app
        .world_mut()
        .query_filtered::<Entity, With<PoemTitle>>()
        .single(app.world())
        .unwrap();
    (app, title, window)
}

fn key(app: &mut App, window: Entity, logical_key: Key, text: Option<&str>) {
    app.world_mut().write_message(KeyboardInput {
        key_code: match logical_key {
            Key::Tab => KeyCode::Tab,
            _ => KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
        },
        logical_key,
        state: ButtonState::Pressed,
        text: text.map(Into::into),
        repeat: false,
        window,
    });
    app.update();
}

fn value(app: &App, title: Entity) -> String {
    app.world()
        .get::<EditableText>(title)
        .unwrap()
        .value()
        .to_string()
}

#[test]
fn title_keyboard_focus_unicode_selection_and_finish() {
    let (mut app, title, window) = editor_app();
    key(
        &mut app,
        window,
        Key::Character("ignored".into()),
        Some("ignored"),
    );
    assert_eq!(value(&app, title), "");
    // Tab navigation reaches the field; it must not capture typing at startup.
    key(&mut app, window, Key::Tab, None);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(title));
    key(
        &mut app,
        window,
        Key::Character("Café".into()),
        Some("Cafe\u{301}"),
    );
    assert_eq!(value(&app, title), "Cafe\u{301}");
    key(&mut app, window, Key::Backspace, None);
    // Parley removes a trailing combining accent before its base letter.
    assert_eq!(value(&app, title), "Cafe");
    key(&mut app, window, Key::Backspace, None);
    assert_eq!(value(&app, title), "Caf");
    key(&mut app, window, Key::ArrowLeft, None);
    key(&mut app, window, Key::Character("l".into()), Some("l"));
    assert_eq!(value(&app, title), "Calf");
    app.world_mut()
        .get_mut::<EditableText>(title)
        .unwrap()
        .queue_edit(TextEdit::SelectAll);
    app.update();
    key(
        &mut app,
        window,
        Key::Character("Moss".into()),
        Some("Moss"),
    );
    assert_eq!(value(&app, title), "Moss");
    key(&mut app, window, Key::Enter, None);
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    assert_eq!(value(&app, title), "Moss");
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(title, FocusCause::Navigated);
    key(&mut app, window, Key::Escape, None);
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    assert_eq!(value(&app, title), "Moss");
}

#[test]
fn title_is_bounded_single_line_and_blurs_on_window_loss() {
    let (mut app, title, window) = editor_app();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(title, FocusCause::Navigated);
    for text in ["one\ntwo", "one\rtwo", "a\tb", "a\u{2028}b", "a\u{2029}b"] {
        app.world_mut()
            .get_mut::<EditableText>(title)
            .unwrap()
            .queue_edit(TextEdit::Insert(text.into()));
        app.update();
        assert_eq!(value(&app, title), "");
    }
    let full = "é".repeat(120);
    key(
        &mut app,
        window,
        Key::Character(full.clone().into()),
        Some(&full),
    );
    assert_eq!(value(&app, title), full);
    key(&mut app, window, Key::Character("x".into()), Some("x"));
    assert_eq!(value(&app, title), full);
    app.world_mut().write_message(WindowFocused {
        window,
        focused: false,
    });
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    assert_eq!(value(&app, title), full);
}

#[test]
fn ime_composition_does_not_finish_the_title_on_enter() {
    let (mut app, title, window) = editor_app();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(title, FocusCause::Navigated);
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "詩".into(),
        cursor: Some((3, 3)),
    });
    app.update();
    assert!(
        app.world()
            .get::<EditableText>(title)
            .unwrap()
            .is_composing()
    );
    assert_eq!(value(&app, title), "");
    key(&mut app, window, Key::Enter, None);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(title));
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "詩".into(),
    });
    app.update();
    assert_eq!(value(&app, title), "詩");
    key(&mut app, window, Key::Enter, None);
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
}

#[test]
fn clicking_outside_blurs_and_placeholder_is_not_the_title_value() {
    use bevy::camera::{ManualTextureViewHandle, NormalizedRenderTarget};
    use bevy::picking::pointer::{Location, PointerId};
    let (mut app, title, _window) = editor_app();
    let viewport = app
        .world_mut()
        .spawn((
            super::super::writing::WritingViewport,
            ScrollPosition(Vec2::new(0.0, 56.0)),
        ))
        .id();
    let press = |app: &mut App, target| {
        app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::TextureView(ManualTextureViewHandle(5)),
                position: Vec2::ZERO,
            },
            Press {
                button: PointerButton::Primary,
                hit: bevy::picking::backend::HitData::new(target, 0.0, None, None),
                count: 1,
            },
            target,
        ));
        app.update();
    };
    let placeholder = app
        .world_mut()
        .query_filtered::<Entity, With<TitlePlaceholder>>()
        .single(app.world())
        .unwrap();
    assert_eq!(value(&app, title), "");
    assert_eq!(
        *app.world().get::<Visibility>(placeholder).unwrap(),
        Visibility::Inherited
    );
    press(&mut app, title);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(title));
    assert_eq!(
        app.world().get::<ScrollPosition>(viewport).unwrap().0.y,
        56.0
    );
    assert_eq!(
        *app.world().get::<Visibility>(placeholder).unwrap(),
        Visibility::Hidden
    );
    let outside = app.world_mut().spawn(Node::default()).id();
    press(&mut app, outside);
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    assert_eq!(
        *app.world().get::<Visibility>(placeholder).unwrap(),
        Visibility::Inherited
    );
    assert_eq!(value(&app, title), "");
}

#[test]
fn tab_reveals_an_offscreen_title_without_discarding_it() {
    let (mut app, title, window) = editor_app();
    let viewport = app
        .world_mut()
        .spawn((
            super::super::writing::WritingViewport,
            ScrollPosition(Vec2::new(0.0, 500.0)),
        ))
        .id();
    key(&mut app, window, Key::Tab, None);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(title));
    assert_eq!(
        app.world().get::<ScrollPosition>(viewport).unwrap().0.y,
        0.0
    );
}
