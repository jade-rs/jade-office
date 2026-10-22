//! 表示 › イマーシブ リーダー: the document's text alone, over the window, in
//! a column for reading — with Word's 列幅, ページの色, 行フォーカス (↑ ↓
//! move it), テキストの間隔 and 音節 on its bar, and 読み上げ, the word being
//! read lit and the line focus following it. Esc, or 閉じる, goes back to
//! the pages. What each line shows is `jade-office-model`'s
//! `word::immersive`; the lines are where gpui's wrapper breaks them.

use gpui::{
    deferred, div, prelude::*, px, rgb, AnyElement, ClickEvent, Context, LineFragment,
    ScrollHandle, Window,
};
use jade_office_model::text::TextEdit;
use jade_office_model::word::immersive::{
    focus_top, lines_of, shown_paragraphs, shown_range, ColumnWidth, ImmersiveSettings, PageColor,
    ReaderLine, ShownParagraph,
};

use crate::{i18n, theme, DocumentView};

/// The reader, while it is open.
pub struct ImmersiveUi {
    pub settings: ImmersiveSettings,
    /// The line 行フォーカス is on.
    pub focus_line: usize,
    /// How many lines the reader had at its last paint.
    line_count: usize,
    scroll: ScrollHandle,
    /// Typing goes nowhere while the reader is open.
    pub sink: TextEdit,
}

/// What the reader's bar does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImmersiveAction {
    Column(ColumnWidth),
    Page(PageColor),
    LineFocus(Option<usize>),
    TextSpacing,
    Syllables,
    ReadAloud,
    Close,
}

/// イマーシブ リーダー: open or close it.
pub fn toggle(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.review.immersive = match view.review.immersive.take() {
        Some(_) => None,
        None => Some(ImmersiveUi {
            settings: ImmersiveSettings::default(),
            focus_line: 0,
            line_count: 0,
            scroll: ScrollHandle::new(),
            sink: TextEdit::new(String::new()),
        }),
    };
    if view.review.immersive.is_none() {
        crate::word_read_aloud::stop(view, cx);
    }
    cx.notify();
}

/// The 表示 tab's イマーシブ リーダー group.
pub fn view_group(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    DocumentView::ribbon_group(
        i18n::text("immersive.group"),
        vec![DocumentView::ribbon_row(vec![
            crate::word_review_tools::text_button(
                "view-immersive",
                "immersive.button",
                "immersive.title",
                view.review.immersive.is_some(),
                crate::word_review_tools::ReviewAction::Immersive,
                cx,
            ),
        ])],
    )
    .into_any_element()
}

/// The bar's controls.
pub fn act(view: &mut DocumentView, action: ImmersiveAction, cx: &mut Context<DocumentView>) {
    let Some(ui) = view.review.immersive.as_mut() else {
        return;
    };
    let settings = &mut ui.settings;
    match action {
        ImmersiveAction::Column(width) => settings.column = width,
        ImmersiveAction::Page(color) => settings.page = color,
        ImmersiveAction::LineFocus(lines) => settings.line_focus = lines,
        ImmersiveAction::TextSpacing => settings.text_spacing = !settings.text_spacing,
        ImmersiveAction::Syllables => settings.syllables = !settings.syllables,
        ImmersiveAction::ReadAloud => {
            if view.review.read_aloud.is_some() {
                return crate::word_read_aloud::play_pause(view, cx);
            }
            return crate::word_read_aloud::toggle(view, cx);
        }
        ImmersiveAction::Close => return toggle(view, cx),
    }
    cx.notify();
}

/// Moves 行フォーカス a line up or down.
pub fn move_focus(view: &mut DocumentView, down: bool, cx: &mut Context<DocumentView>) {
    let Some(ui) = view.review.immersive.as_mut() else {
        return;
    };
    let last = ui.line_count.saturating_sub(1);
    ui.focus_line = if down {
        (ui.focus_line + 1).min(last)
    } else {
        ui.focus_line.saturating_sub(1)
    };
    ui.scroll.scroll_to_item(ui.focus_line);
    cx.notify();
}

/// The reader's keys: ↑ ↓ move the line focus, space reads aloud, Esc
/// closes. Every other key is swallowed: the reader does not edit.
pub fn handle_key(view: &mut DocumentView, key: &str, cx: &mut Context<DocumentView>) -> bool {
    match key {
        "up" => move_focus(view, false, cx),
        "down" => move_focus(view, true, cx),
        "space" => act(view, ImmersiveAction::ReadAloud, cx),
        "escape" => act(view, ImmersiveAction::Close, cx),
        _ => {}
    }
    true
}

/// The paragraphs as shown, broken into the lines the column holds.
fn layout(
    paragraphs: &[ShownParagraph],
    settings: &ImmersiveSettings,
    window: &Window,
) -> Vec<ReaderLine> {
    let font = window.text_style().font();
    let mut wrapper = window
        .text_system()
        .line_wrapper(font, px(settings.font_size));
    paragraphs
        .iter()
        .flat_map(|paragraph| {
            let breaks: Vec<usize> = wrapper
                .wrap_line(
                    &[LineFragment::text(&paragraph.text)],
                    px(settings.column.pixels()),
                )
                .map(|boundary| boundary.ix)
                .collect();
            lines_of(paragraph, &breaks)
        })
        .collect()
}

fn bar_button(
    id: (&'static str, usize),
    label: String,
    on: bool,
    action: ImmersiveAction,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .px_2()
        .h(px(26.0))
        .flex()
        .items_center()
        .rounded_md()
        .cursor_pointer()
        .when(on, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .text_color(theme::text())
        .child(label)
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| act(view, action, cx)))
        .into_any_element()
}

fn caption(key: &'static str) -> AnyElement {
    div()
        .pl_2()
        .text_color(theme::text_dim())
        .child(i18n::text(key))
        .into_any_element()
}

/// The reader over the window, while it is open.
pub fn overlay(
    view: &mut DocumentView,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) -> Option<AnyElement> {
    let settings = view.review.immersive.as_ref()?.settings.clone();
    let paragraphs = shown_paragraphs(&view.document, &settings);
    let lines = layout(&paragraphs, &settings, window);
    // The word being read, and the line it is on, which the focus follows.
    let reading = view
        .review
        .read_aloud
        .as_ref()
        .and_then(|ui| ui.reading.current_word());
    let reading_line = reading.and_then(|((paragraph, start), (_, end))| {
        let shown = paragraphs
            .iter()
            .find(|shown| shown.paragraph == paragraph)?;
        lines.iter().enumerate().find_map(|(index, line)| {
            (line.paragraph == paragraph)
                .then(|| shown_range(shown, line, (start, end)))
                .flatten()
                .map(|range| (index, range))
        })
    });
    let ui = view.review.immersive.as_mut()?;
    ui.line_count = lines.len();
    if let Some((index, _)) = reading_line {
        if ui.focus_line != index {
            ui.focus_line = index;
            ui.scroll.scroll_to_item(index);
        }
    }
    ui.focus_line = ui.focus_line.min(lines.len().saturating_sub(1));
    let lit = settings.line_focus.map(|count| {
        let top = focus_top(ui.focus_line, count, lines.len());
        top..top + count
    });
    let scroll = ui.scroll.clone();
    let (page, ink) = settings.page.colors();
    let line_height = settings.line_height();
    let rows = lines.iter().enumerate().map(|(index, line)| {
        let shown = paragraphs
            .iter()
            .find(|shown| shown.paragraph == line.paragraph)
            .expect("lines come from the paragraphs");
        let text = &shown.text[line.start..line.end];
        let dimmed = lit.as_ref().is_some_and(|lit| !lit.contains(&index));
        let content: AnyElement = match reading_line {
            Some((at, (start, end))) if at == index => div()
                .flex()
                .child(shown.text[line.start..start].to_string())
                .child(
                    div()
                        .bg(rgb(0xC8DCF8))
                        .text_color(rgb(0x1F1F1F))
                        .child(shown.text[start..end].to_string()),
                )
                .child(shown.text[end..line.end].to_string())
                .into_any_element(),
            _ => div().child(text.to_string()).into_any_element(),
        };
        div()
            .id(("immersive-line", index))
            .h(px(line_height))
            .flex()
            .items_center()
            .whitespace_nowrap()
            .when(line.first && index > 0, |row| {
                row.mt(px(settings.paragraph_gap()))
            })
            .when(dimmed, |row| row.opacity(0.15))
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                if let Some(ui) = view.review.immersive.as_mut() {
                    ui.focus_line = index;
                }
                cx.notify();
            }))
            .child(content)
            .into_any_element()
    });
    let rows: Vec<AnyElement> = rows.collect();
    let mut bar: Vec<AnyElement> = vec![caption("immersive.column")];
    for (index, width) in ColumnWidth::ALL.into_iter().enumerate() {
        bar.push(bar_button(
            ("immersive-column", index),
            i18n::text(width.label_key()).to_string(),
            settings.column == width,
            ImmersiveAction::Column(width),
            cx,
        ));
    }
    bar.push(caption("immersive.page"));
    for (index, color) in PageColor::ALL.into_iter().enumerate() {
        bar.push(bar_button(
            ("immersive-page", index),
            i18n::text(color.label_key()).to_string(),
            settings.page == color,
            ImmersiveAction::Page(color),
            cx,
        ));
    }
    bar.push(caption("immersive.line_focus"));
    for (index, lines) in ImmersiveSettings::LINE_FOCUS.into_iter().enumerate() {
        let label = match lines {
            None => i18n::text("immersive.line_focus.off").to_string(),
            Some(n) => i18n::format("immersive.line_focus.lines", &[("n", &n)]),
        };
        bar.push(bar_button(
            ("immersive-focus", index),
            label,
            settings.line_focus == lines,
            ImmersiveAction::LineFocus(lines),
            cx,
        ));
    }
    bar.push(bar_button(
        ("immersive-spacing", 0),
        i18n::text("immersive.spacing").to_string(),
        settings.text_spacing,
        ImmersiveAction::TextSpacing,
        cx,
    ));
    bar.push(bar_button(
        ("immersive-syllables", 0),
        i18n::text("immersive.syllables").to_string(),
        settings.syllables,
        ImmersiveAction::Syllables,
        cx,
    ));
    bar.push(bar_button(
        ("immersive-read", 0),
        i18n::text("read.button").to_string(),
        view.review.read_aloud.is_some(),
        ImmersiveAction::ReadAloud,
        cx,
    ));
    bar.push(div().flex_1().into_any_element());
    bar.push(bar_button(
        ("immersive-close", 0),
        i18n::text("immersive.close").to_string(),
        false,
        ImmersiveAction::Close,
        cx,
    ));
    let reader = div()
        .id("immersive-reader")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .flex_col()
        .bg(rgb(page))
        .occlude()
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_1()
                .px_3()
                .py_1()
                .bg(theme::panel())
                .border_b_1()
                .border_color(theme::border())
                .text_sm()
                .children(bar),
        )
        .children(
            view.review
                .read_aloud
                .is_some()
                .then(|| crate::word_read_aloud::bar(view, cx))
                .flatten(),
        )
        .child(
            div()
                .id("immersive-lines")
                .flex_1()
                .overflow_y_scroll()
                .track_scroll(&scroll)
                .child(
                    div()
                        .w(px(settings.column.pixels() + 48.0))
                        .mx_auto()
                        .px_6()
                        .py_10()
                        .font_family(window.text_style().font_family)
                        .text_size(px(settings.font_size))
                        .text_color(rgb(ink))
                        .children(rows),
                ),
        );
    Some(deferred(reader).with_priority(2).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Focusable;

    /// The reader opens from the 表示 tab over the pages, takes its
    /// settings from its bar, moves the line focus with the arrow keys
    /// while typing changes nothing, and Esc closes it.
    #[gpui::test]
    fn the_immersive_reader_shows_the_text_in_lines(cx: &mut gpui::TestAppContext) {
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx.add_window_view(|_window, cx| {
            crate::untitled_view(&bytes, "reader.docx", cx).unwrap()
        });
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input(&"Reading is wonderful. ".repeat(8));
        view.update(cx, |view, cx| {
            toggle(view, cx);
            act(view, ImmersiveAction::LineFocus(Some(3)), cx);
            act(view, ImmersiveAction::Syllables, cx);
            act(view, ImmersiveAction::Page(PageColor::Sepia), cx);
        });
        cx.run_until_parked();
        let before = view.update(cx, |view, _| {
            let ui = view.review.immersive.as_ref().unwrap();
            assert!(ui.line_count > 2, "the paragraph wraps: {}", ui.line_count);
            assert_eq!(ui.settings.page, PageColor::Sepia);
            view.document.paragraph_text(0)
        });
        cx.simulate_keystrokes("down down");
        cx.simulate_input("x");
        view.update(cx, |view, _| {
            assert_eq!(view.review.immersive.as_ref().unwrap().focus_line, 2);
            assert_eq!(
                view.document.paragraph_text(0),
                before,
                "the reader does not edit"
            );
        });
        cx.simulate_keystrokes("escape");
        view.update(cx, |view, _| assert!(view.review.immersive.is_none()));
    }
}
