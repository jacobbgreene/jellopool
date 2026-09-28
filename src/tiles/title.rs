//! One real text editor, with page-specific styling and focus lifecycle.
//! Bevy owns selection, Unicode, clipboard, IME and horizontal text scrolling.
use super::presentation::{INK, RULE, SIGNATURE};
use crate::prelude::*;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{
    FocusCause, FocusGained, FocusedInput, InputFocus,
    tab_navigation::{TabGroup, TabIndex, TabNavigationPlugin},
};
use bevy::text::{EditableText, EditableTextFilter, EditableTextSystems, LineHeight};
use bevy::window::WindowFocused;

#[derive(Component)]
pub(crate) struct PoemTitle;
#[derive(Component)]
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

pub(super) fn spawn_title(parent: &mut ChildSpawnerCommands, font: Handle<Font>) {
    let title_font = TextFont {
        font: FontSource::Handle(font.clone()),
        font_size: FontSize::Px(32.0),
        ..default()
    };
    parent
        .spawn((
            Name::new("poem_title_block"),
            TabGroup::new(0),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(64.0),
                right: Val::Px(32.0),
                bottom: Val::Px(88.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .with_children(|block| {
            block.spawn((
                Text::new("POEM TITLE"),
                TextFont {
                    font: FontSource::Handle(font),
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::from(SIGNATURE)),
                LineHeight::Px(16.0),
                Pickable::IGNORE,
            ));
            block
                .spawn((
                    Name::new("poem_title"),
                    PoemTitle,
                    TabIndex(0),
                    EditableText {
                        max_characters: Some(120),
                        cursor_width: 0.045,
                        ..default()
                    },
                    // `allow_newlines` disables Enter, but pasted/IME text also needs filtering.
                    EditableTextFilter::new(|c| {
                        !c.is_control() && c != '\u{2028}' && c != '\u{2029}'
                    }),
                    title_font.clone(),
                    TextColor(Color::from(INK)),
                    LineHeight::Px(44.0),
                    TextLayout::no_wrap(),
                    BorderColor::from(Color::from(RULE)),
                    LayoutConfig {
                        use_rounding: false,
                    },
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(50.0),
                        border: UiRect::bottom(Val::Px(1.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ))
                .with_child((
                    Name::new("poem_title_placeholder"),
                    TitlePlaceholder,
                    Text::new("Give this poem a title"),
                    title_font,
                    TextColor(Color::srgba(0.600, 0.616, 0.584, 1.0)),
                    LineHeight::Px(44.0),
                    TextLayout::no_wrap(),
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        ..default()
                    },
                ));
        });
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
    focus: Res<InputFocus>,
    mut title: Query<(Entity, &EditableText, &mut BorderColor), With<PoemTitle>>,
    mut placeholder: Query<&mut Visibility, With<TitlePlaceholder>>,
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
    for mut current in &mut placeholder {
        if *current != visibility {
            *current = visibility;
        }
    }
}

#[cfg(test)]
mod tests;
