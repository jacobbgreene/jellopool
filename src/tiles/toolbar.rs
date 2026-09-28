//! Small, keyboard-accessible draft controls in the reserved menu strip.
use super::{
    presentation::{INK, MENU_HEIGHT, OUTLINE, PAPER, RULE, SIGNATURE},
    scenes::text_style,
    session::{DraftAction, DraftRequest},
};
use crate::{poems::DraftSession, prelude::*};
use bevy::input_focus::{
    InputFocus,
    tab_navigation::{TabGroup, TabIndex},
};
use bevy::scene::{on, template_value};
use bevy::text::LineHeight;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::{Activate, Button as WidgetButton};

#[derive(Component, Clone, Default)]
pub(super) enum Control {
    Previous,
    Next,
    #[default]
    New,
}
#[derive(Component, Clone, Default)]
pub(super) enum Label {
    #[default]
    Current,
    Status,
    Warning,
}

type Controls<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Control,
        &'static Interaction,
        &'static Children,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
        Has<InteractionDisabled>,
    ),
>;

pub(super) fn toolbar(font: Handle<Font>) -> impl Scene {
    bsn! {
        Name("menu_space") super::presentation::MenuSpace TabGroup::new(1)
        Node { width: percent(78), height: px(MENU_HEIGHT), flex_shrink: 0.0,
            flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, row_gap: px(4) }
        Children [
            (
                Node { align_items: AlignItems::Center, column_gap: px(10), height: px(32) }
                Children [
                    control("Previous draft", Control::Previous, font.clone()),
                    control("Next draft", Control::Next, font.clone()),
                    (
                        template_value(Label::Current) Text("Draft 1 of 1 · Untitled")
                        text_style(font.clone(), 16.0, Color::from(INK)) template_value(LineHeight::Px(24.0))
                        TextLayout::no_wrap() Pickable::IGNORE
                        Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0), overflow: Overflow::clip() }
                    ),
                    control("New draft +", Control::New, font.clone()),
                ]
            ),
            (
                template_value(Label::Status) Text("Loading drafts…") text_style(font.clone(), 12.0, Color::from(SIGNATURE))
                template_value(LineHeight::Px(16.0)) Pickable::IGNORE
            ),
            (
                Name("save_warning") template_value(Label::Warning) Text("")
                text_style(font, 14.0, Color::from(INK)) template_value(LineHeight::Px(20.0))
                BackgroundColor(Color::from(PAPER)) BorderColor::from(Color::from(OUTLINE)) GlobalZIndex(20)
                Node { display: Display::None, position_type: PositionType::Absolute,
                    top: px(MENU_HEIGHT), width: percent(100), padding: UiRect::all(px(12)), border: UiRect::all(px(1)) }
            ),
        ]
    }
}

fn control(label: &'static str, action: Control, font: Handle<Font>) -> impl Scene {
    bsn! {
        Name(label) WidgetButton TabIndex(0) Interaction
        bevy::scene::template_value(action)
        BorderColor::from(Color::from(RULE)) BackgroundColor(Color::NONE)
        Node { padding: UiRect::axes(px(10), px(4)), border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(2)), flex_shrink: 0.0 }
        on(activate)
        Children [(
            Text(label) text_style(font, 14.0, Color::from(INK))
            template_value(LineHeight::Px(20.0)) Pickable::IGNORE
        )]
    }
}

fn activate(
    event: On<Activate>,
    controls: Query<&Control>,
    session: Res<DraftSession>,
    mut request: ResMut<DraftRequest>,
) {
    let Some(book) = &session.book else {
        return;
    };
    let Ok(control) = controls.get(event.entity) else {
        return;
    };
    // At most one request per frame; double activation must not create two drafts.
    if request.action.is_some() {
        return;
    }
    let index = book
        .drafts
        .iter()
        .position(|draft| draft.id == book.active)
        .expect("validated active draft");
    request.action = match control {
        Control::New => Some(DraftAction::New),
        Control::Previous => index
            .checked_sub(1)
            .map(|index| DraftAction::Open(book.drafts[index].id.clone())),
        Control::Next => book
            .drafts
            .get(index + 1)
            .map(|draft| DraftAction::Open(draft.id.clone())),
    };
}

pub(super) fn sync_toolbar(
    session: Res<DraftSession>,
    problem: Res<super::session::CaptureError>,
    focus: Res<InputFocus>,
    mut labels: Query<(&Label, &mut Text, &mut Node)>,
    mut controls: Controls,
    mut button_text: Query<&mut TextColor, Without<Label>>,
    mut commands: Commands,
) {
    let Some(book) = &session.book else {
        return;
    };
    let index = book
        .drafts
        .iter()
        .position(|draft| draft.id == book.active)
        .expect("validated active draft");
    if session.is_changed()
        || problem.is_changed()
        || labels
            .iter()
            .any(|(_, text, _)| text.0 == "Loading drafts…")
    {
        for (label, mut text, mut node) in &mut labels {
            let value = match label {
                Label::Current => format!(
                    "Draft {} of {} · {}",
                    index + 1,
                    book.drafts.len(),
                    if book.active().title.is_empty() {
                        "Untitled"
                    } else {
                        &book.active().title
                    }
                ),
                Label::Status => if problem.0.is_some() {
                    "Draft not captured — see warning"
                } else {
                    session.status()
                }
                .to_owned(),
                Label::Warning => problem
                    .0
                    .as_ref()
                    .or(session.error.as_ref())
                    .or(session.notice.as_ref())
                    .cloned()
                    .unwrap_or_default(),
            };
            if matches!(label, Label::Warning) {
                let display = if value.is_empty() {
                    Display::None
                } else {
                    Display::Flex
                };
                if node.display != display {
                    node.display = display;
                }
            }
            if text.0 != value {
                text.0 = value;
            }
        }
    }
    for (entity, control, interaction, children, mut background, mut border, disabled) in
        &mut controls
    {
        let should_disable = match control {
            Control::Previous => index == 0,
            Control::Next => index + 1 == book.drafts.len(),
            Control::New => book.drafts.len() >= crate::poems::MAX_DRAFTS,
        };
        if should_disable != disabled {
            if should_disable {
                commands.entity(entity).insert(InteractionDisabled);
            } else {
                commands.entity(entity).remove::<InteractionDisabled>();
            }
        }
        let ink = Color::from(INK).with_alpha(if should_disable { 0.4 } else { 1.0 });
        for child in children {
            if let Ok(mut color) = button_text.get_mut(*child)
                && color.0 != ink
            {
                color.0 = ink;
            }
        }
        let color = if should_disable {
            Color::NONE
        } else if *interaction != Interaction::None || focus.get() == Some(entity) {
            Color::from(PAPER)
        } else {
            Color::NONE
        };
        if background.0 != color {
            background.0 = color;
        }
        let color = BorderColor::from(Color::from(
            if !should_disable && focus.get() == Some(entity) {
                INK
            } else {
                RULE
            },
        ));
        if *border != color {
            *border = color;
        }
    }
}
