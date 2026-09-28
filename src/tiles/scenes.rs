//! Actual Bevy scene blueprints, composed with BSN. These create entities;
//! interaction systems and durable poem documents remain independent.
use super::drag::{on_tile_drag, tile_drag_end, tile_drag_start};
use super::placement::{LINE_COUNT, LINE_PITCH};
use super::presentation::{
    self, BANK, INK, OUTLINE, PAGE_INSET, PAGE_LEFT, PAGE_RIGHT, PAPER, PageLead, SIGNATURE,
    TILE_FACE, TILE_RADIUS,
};
use super::writing::{
    LineGuide, ScrollThumb, ScrollTrack, WritingViewport, scroll_thumb_drag, scroll_track_press,
};
use crate::prelude::*;
use bevy::scene::{on, template_value};
use bevy::text::{FontSourceTemplate, LineHeight};

/// Visual customization only. Logical line geometry belongs to the document
/// format and is shared with placement, not independently overridden here.
#[derive(Clone)]
pub(crate) struct WorkspaceStyle {
    pub paper: Color,
    pub stock: Color,
}

impl Default for WorkspaceStyle {
    fn default() -> Self {
        Self {
            paper: Color::from(PAPER),
            stock: Color::from(BANK),
        }
    }
}

#[derive(Clone)]
pub(crate) struct TileStyle {
    pub face: Color,
    pub ink: Color,
    pub outline: Color,
}

impl Default for TileStyle {
    fn default() -> Self {
        Self {
            face: Color::from(TILE_FACE),
            ink: Color::from(INK),
            outline: Color::from(OUTLINE),
        }
    }
}

#[derive(Component, Default, Clone)]
pub(crate) struct WorkspaceRoot;

pub(crate) fn writing_workspace(
    font: Handle<Font>,
    signature: Option<Handle<Font>>,
    title: String,
    style: WorkspaceStyle,
) -> impl Scene {
    let signature: Vec<_> = signature.into_iter().map(wordmark).collect();
    bsn! {
        Name("board_root")
        WorkspaceRoot
        BackgroundColor({style.stock})
        Node {
            width: percent(100), height: percent(100),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::End,
            align_items: AlignItems::Center,
        }
        Children [
            super::toolbar::toolbar(font.clone()),
            writing_page(font, title, style.paper),
            (
                tray(style.stock)
                Children [separator(), {signature}]
            ),
            drag_layer(),
        ]
    }
}

pub(crate) fn writing_page(font: Handle<Font>, title: String, paper: Color) -> impl Scene {
    let lines: Vec<Box<dyn SceneList>> = (0..LINE_COUNT)
        .map(|line| Box::new(numbered_line(line, font.clone())) as Box<dyn SceneList>)
        .collect();
    bsn! {
        Name("writing_section")
        Node {
            width: percent(78), flex_grow: 1.0, flex_basis: px(0), min_height: px(0),
            justify_content: JustifyContent::Center,
            padding: UiRect::vertical(px(PAGE_INSET)),
        }
        Children [(
            Name("writing_frame")
            Node { width: {Val::Px(crate::poems::PAPER_WIDTH + PAGE_LEFT + PAGE_RIGHT)}, max_width: percent(100), height: percent(100), min_height: px(0) }
            Children [
                (
                    Name("writing_viewport") WritingViewport ScrollPosition
                    BackgroundColor(paper)
                    Node { width: percent(100), height: percent(100), min_height: px(0), overflow: Overflow::scroll_y() }
                    Children [(
                        Name("writing_page")
                        Node { width: percent(100), align_self: AlignSelf::Start, flex_direction: FlexDirection::Column,
                            padding: UiRect::bottom(px(PAGE_INSET)), flex_shrink: 0.0 }
                        Children [
                            (
                                Name("page_lead") PageLead
                                Node { height: {Val::Px(presentation::page_lead_height(900.0))}, flex_shrink: 0.0 }
                                Children [super::title::title_scene(font, title)]
                            ),
                            (
                                Name("writing_zone") WritingZone
                                Node { margin: {UiRect { left: px(PAGE_LEFT), right: px(PAGE_RIGHT), ..default() }},
                                    height: {Val::Px(LINE_COUNT as f32 * LINE_PITCH)}, flex_shrink: 0.0 }
                                Children [{lines}]
                            ),
                        ]
                    )]
                ),
                scrollbar(),
            ]
        )]
    }
}

fn numbered_line(line: usize, font: Handle<Font>) -> impl SceneList {
    bsn_list![
        (
            Name({format!("line_{}_guide", line + 1)}) LineGuide
            LayoutConfig { use_rounding: false }
            Pickable::IGNORE
            BorderColor::from(Color::srgba(0.60, 0.62, 0.58, 0.20))
            Node { position_type: PositionType::Absolute, left: px(0), top: {Val::Px(line as f32 * LINE_PITCH)},
                width: percent(100), height: px(40), border: UiRect::bottom(px(1)) }
        ),
        (
            Name({format!("line_{}_number", line + 1)}) Pickable::IGNORE
            Text({(line + 1).to_string()})
            text_style(font, 14.0, Color::srgba(0.392, 0.416, 0.384, 0.70))
            template_value(LineHeight::Px(40.0)) TextLayout::no_wrap()
            Node { position_type: PositionType::Absolute, left: px(-40), top: {Val::Px(line as f32 * LINE_PITCH)},
                width: px(28), height: px(40) }
        ),
    ]
}

fn scrollbar() -> impl Scene {
    bsn! {
        Name("writing_scroll_track") ScrollTrack
        Node { position_type: PositionType::Absolute, right: px(4), top: px(12), bottom: px(12), width: px(16) }
        on(scroll_track_press)
        Children [(
            Name("writing_scroll_thumb") ScrollThumb BackgroundColor(Color::from(OUTLINE))
            Node { position_type: PositionType::Absolute, left: px(5), top: px(0), width: px(6), height: px(32),
                border_radius: BorderRadius::all(px(3)) }
            on(scroll_thumb_drag)
        )]
    }
}

pub(crate) fn word_tile(word: String, font: Handle<Font>, style: TileStyle) -> impl Scene {
    bsn! {
        Name({word.clone()}) WordTile LayoutConfig { use_rounding: false }
        Node { padding: UiRect::axes(px(14), px(7)), border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(TILE_RADIUS)) }
        BorderColor::from(style.outline) BackgroundColor({style.face})
        Children [(
            Text(word) LayoutConfig
            text_style(font, 20.0, style.ink)
            template_value(LineHeight::Px(24.0)) TextLayout::no_wrap() Pickable::IGNORE
        )]
        on(tile_drag_start) on(on_tile_drag) on(tile_drag_end)
    }
}

pub(super) fn text_style(font: Handle<Font>, size: f32, color: Color) -> impl Scene {
    bsn! {
        TextFont { font: {FontSourceTemplate::Handle(font.into())}, font_size: {FontSize::Px(size)} }
        TextColor(color)
    }
}

fn tray(stock: Color) -> impl Scene {
    let (name, marker, _, node) = presentation::get_board_tray();
    bsn! { template_value(name) template_value(marker) BackgroundColor(stock) template_value(node) }
}
fn separator() -> impl Scene {
    let (name, color, picking, node) = presentation::get_separator();
    bsn! { template_value(name) template_value(color) template_value(picking) template_value(node) }
}
fn drag_layer() -> impl Scene {
    let (name, marker, picking, node) = presentation::get_drag_layer();
    bsn! { template_value(name) template_value(marker) template_value(picking) template_value(node) }
}
fn wordmark(font: Handle<Font>) -> impl Scene {
    bsn! {
        Name("wordmark") Text("jellopool")
        TextFont { font: {FontSourceTemplate::Handle(font.into())}, font_size: {FontSize::Vw(1.4)}, style: FontStyle::Italic }
        TextColor(Color::from(SIGNATURE)) TextLayout::no_wrap() Pickable::IGNORE
        Node { position_type: PositionType::Absolute, left: {Val::Vw(-8.5)}, bottom: percent(100), padding: UiRect::bottom(px(18)) }
    }
}
