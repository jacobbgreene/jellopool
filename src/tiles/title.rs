//! One real text editor, with page-specific styling and focus lifecycle.
//! Bevy owns selection, Unicode, clipboard, IME and horizontal text scrolling.
use super::presentation::{INK, PAGE_LEFT, PAGE_RIGHT, RULE, SIGNATURE};
use crate::prelude::*;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{
    FocusCause, FocusGained, FocusedInput, InputFocus,
    tab_navigation::{TabGroup, TabIndex, TabNavigationPlugin},
};
use bevy::text::{
    EditableText, EditableTextFilter, EditableTextSystems, LineHeight, TextCursorStyle,
};
use bevy::window::WindowFocused;

#[derive(Component, Default, Clone)]
pub(crate) struct PoemTitle;
#[derive(Component, Default, Clone)]
struct TitlePlaceholder;

pub(super) struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TabNavigationPlugin)
            .add_observer(finish_editing)
            .add_observer(reveal_on_keyboard_focus)
            .add_systems(Update, blur_on_window_loss)
            .add_systems(
                PostUpdate,
                sync_title_chrome
                    .after(EditableTextSystems)
                    .before(bevy::ui::UiSystems::Layout),
            );
    }
}

pub(super) fn title_scene(font: Handle<Font>, title: String) -> impl Scene {
    use super::scenes::text_style;
    use bevy::scene::template_value;
    let editor = EditableText {
        max_characters: Some(crate::poems::TITLE_LIMIT),
        cursor_width: 0.045,
        ..EditableText::new(title)
    };
    bsn! {
        Name("poem_title_block")
        TabGroup::new(0)
        Node {
            position_type: PositionType::Absolute, left: px(PAGE_LEFT), right: px(PAGE_RIGHT), bottom: px(88),
            flex_direction: FlexDirection::Column,
        }
        Children [
            (
                Name("poem_title") PoemTitle TabIndex(0)
                template_value(editor)
                TextCursorStyle { color: Color::from(INK) }
                // Also filter pasted and IME text, not just the Enter key.
                template_value(EditableTextFilter::new(|c| !c.is_control() && c != '\u{2028}' && c != '\u{2029}'))
                text_style(font.clone(), 32.0, Color::from(INK))
                template_value(LineHeight::Px(44.0)) TextLayout::no_wrap()
                BorderColor::from(Color::from(RULE))
                LayoutConfig { use_rounding: false }
                Node { width: percent(100), height: px(50), border: UiRect::bottom(px(1)), overflow: Overflow::clip() }
                Children [(
                    Name("poem_title_placeholder") TitlePlaceholder Text("Give this poem a title.")
                    text_style(font, 32.0, Color::srgba(0.600, 0.616, 0.584, 1.0))
                    template_value(LineHeight::Px(44.0)) TextLayout::no_wrap() Pickable::IGNORE
                    Node { position_type: PositionType::Absolute, left: px(0), top: px(0) }
                )]
            ),
        ]
    }
}

#[cfg(test)]
fn spawn_title(parent: &mut ChildSpawnerCommands, font: Handle<Font>) {
    parent
        .spawn_empty()
        .apply_scene(title_scene(font, String::new()));
}

fn finish_editing(
    mut event: On<FocusedInput<KeyboardInput>>,
    titles: Query<&EditableText, With<PoemTitle>>,
    mut focus: ResMut<InputFocus>,
) {
    if let Ok(editor) = titles.get(event.focused_entity)
        && event.input.state.is_pressed()
        && matches!(event.input.logical_key, Key::Enter | Key::Escape)
        && !editor.is_composing()
    {
        focus.clear();
        event.propagate(false);
    }
}

fn reveal_on_keyboard_focus(
    event: On<FocusGained>,
    keys: Res<ButtonInput<KeyCode>>,
    titles: Query<(), With<PoemTitle>>,
    mut viewports: Query<&mut ScrollPosition, With<super::writing::WritingViewport>>,
) {
    // A Tab from a lower line must not leave the caret offscreen. A pointer
    // focus doesn't jump the page: the user is already looking at the field.
    // TabNavigation can also report Navigated for a pointer-acquired focus.
    if event.cause == FocusCause::Navigated
        && keys.just_pressed(KeyCode::Tab)
        && titles.contains(event.event_target())
    {
        for mut scroll in &mut viewports {
            if scroll.0.y != 0.0 {
                scroll.0.y = 0.0;
            }
        }
    }
}

fn blur_on_window_loss(
    mut events: MessageReader<WindowFocused>,
    titles: Query<(), With<PoemTitle>>,
    mut focus: ResMut<InputFocus>,
) {
    if events.read().any(|event| !event.focused)
        && focus.get().is_some_and(|entity| titles.contains(entity))
    {
        focus.clear();
    }
}

fn sync_title_chrome(
    time: Res<Time>,
    focus: Res<InputFocus>,
    mut title: Query<(Entity, &EditableText, &mut BorderColor), With<PoemTitle>>,
    mut placeholder: Query<(&mut Visibility, &mut Text), With<TitlePlaceholder>>,
) {
    let Ok((entity, editor, mut border)) = title.single_mut() else {
        return;
    };
    let focused = focus.get() == Some(entity);
    let color = BorderColor::from(Color::from(if focused { SIGNATURE } else { RULE }));
    if *border != color {
        *border = color;
    }
    let visibility = if editor.value() == "" && !focused {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let frames = [
        "Give this poem a title.",
        "Give this poem a title..",
        "Give this poem a title...",
    ];
    let frame = ((time.elapsed().as_millis() / 2) / 450 % frames.len() as u128) as usize;
    for (mut current, mut text) in &mut placeholder {
        if *current != visibility {
            *current = visibility;
        }
        if visibility != Visibility::Hidden && text.0 != frames[frame] {
            text.0 = frames[frame].into();
        }
    }
}

#[cfg(test)]
mod tests;
