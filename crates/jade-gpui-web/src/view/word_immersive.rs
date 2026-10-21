//! イマーシブ リーダー in the browser: the document's text alone, over the
//! page, in a column for reading — 列幅, ページの色, 行フォーカス (↑ ↓),
//! テキストの間隔, 音節 and 読み上げ with the word being read lit — as the
//! native app's `word_immersive`. What each line shows is
//! `jade-office-model`'s `word::immersive`.

use gpui::{deferred, rgb, IndentAdjustment, LineFragment};
use jade_web::word::immersive::{
    focus_top, lines_of, shown_paragraphs, shown_range, ColumnWidth, ImmersiveSettings, PageColor,
    ReaderLine, ShownParagraph,
};

use super::*;

/// The reader, while it is open.
pub(crate) struct WebImmersive {
    settings: ImmersiveSettings,
    focus_line: usize,
    line_count: usize,
    scroll: ScrollHandle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImmersiveAction {
    Column(ColumnWidth),
    Page(PageColor),
    LineFocus(Option<usize>),
    TextSpacing,
    Syllables,
    ReadAloud,
    Close,
}

impl DocView {
    fn immersive_mut(&mut self) -> Option<&mut WebImmersive> {
        match &mut self.document {
            Document::Pages { immersive, .. } => immersive.as_deref_mut(),
            _ => None,
        }
    }

    pub(super) fn immersive_open(&self) -> bool {
        matches!(
            &self.document,
            Document::Pages {
                immersive: Some(_),
                ..
            }
        )
    }

    /// イマーシブ リーダー: open or close it.
    pub(super) fn toggle_immersive(&mut self, cx: &mut Context<Self>) {
        let Document::Pages {
            immersive,
            read_aloud,
            ..
        } = &mut self.document
        else {
            return;
        };
        if immersive.take().is_none() {
            *immersive = Some(Box::new(WebImmersive {
                settings: ImmersiveSettings::default(),
                focus_line: 0,
                line_count: 0,
                scroll: ScrollHandle::new(),
            }));
        } else if read_aloud.is_some() {
            self.toggle_read_aloud(cx);
        }
        cx.notify();
    }

    fn immersive_act(&mut self, action: ImmersiveAction, cx: &mut Context<Self>) {
        let Some(ui) = self.immersive_mut() else {
            return;
        };
        let settings = &mut ui.settings;
        match action {
            ImmersiveAction::Column(width) => settings.column = width,
            ImmersiveAction::Page(color) => settings.page = color,
            ImmersiveAction::LineFocus(lines) => settings.line_focus = lines,
            ImmersiveAction::TextSpacing => settings.text_spacing = !settings.text_spacing,
            ImmersiveAction::Syllables => settings.syllables = !settings.syllables,
            ImmersiveAction::ReadAloud => return self.toggle_read_aloud(cx),
            ImmersiveAction::Close => return self.toggle_immersive(cx),
        }
        cx.notify();
    }

    /// The reader's keys while it is open: ↑ ↓ move the line focus, space
    /// reads aloud, Esc closes, and nothing else edits.
    pub(super) fn immersive_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(ui) = self.immersive_mut() else {
            return false;
        };
        match key {
            "up" | "down" => {
                let last = ui.line_count.saturating_sub(1);
                ui.focus_line = if key == "down" {
                    (ui.focus_line + 1).min(last)
                } else {
                    ui.focus_line.saturating_sub(1)
                };
                ui.scroll.scroll_to_item(ui.focus_line);
                cx.notify();
            }
            "space" => self.immersive_act(ImmersiveAction::ReadAloud, cx),
            "escape" => self.immersive_act(ImmersiveAction::Close, cx),
            _ => {}
        }
        true
    }

    /// The reader over the page, while it is open.
    pub(super) fn immersive_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let Document::Pages {
            document,
            immersive: Some(ui),
            read_aloud,
            ..
        } = &mut self.document
        else {
            return None;
        };
        let settings = ui.settings.clone();
        let paragraphs = shown_paragraphs(&**document, &settings);
        let lines = layout(&paragraphs, &settings, window);
        let reading = read_aloud
            .as_ref()
            .and_then(|reading| reading.current_word());
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
        let reading_now = read_aloud.is_some();
        let (page, ink) = settings.page.colors();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let shown = paragraphs
                .iter()
                .find(|shown| shown.paragraph == line.paragraph)
                .expect("lines come from the paragraphs");
            let dimmed = lit.as_ref().is_some_and(|lit| !lit.contains(&index));
            let content: gpui::AnyElement = match reading_line {
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
                _ => div()
                    .child(shown.text[line.start..line.end].to_string())
                    .into_any_element(),
            };
            rows.push(
                div()
                    .id(("immersive-line", index))
                    .h(px(settings.line_height()))
                    .flex()
                    .items_center()
                    .whitespace_nowrap()
                    .when(line.first && index > 0, |row| {
                        row.mt(px(settings.paragraph_gap()))
                    })
                    .when(dimmed, |row| row.opacity(0.15))
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        if let Some(ui) = view.immersive_mut() {
                            ui.focus_line = index;
                        }
                        cx.notify();
                    }))
                    .child(content)
                    .into_any_element(),
            );
        }
        let button = |id: (&'static str, usize),
                      label: String,
                      on: bool,
                      action: ImmersiveAction,
                      cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_2()
                .h(px(26.0))
                .flex()
                .items_center()
                .rounded_md()
                .cursor_pointer()
                .when(on, |button| button.bg(palette::selected()))
                .hover(|button| button.bg(palette::hovered()))
                .text_color(palette::text())
                .child(label)
                .on_click(
                    cx.listener(move |view, _event, _window, cx| view.immersive_act(action, cx)),
                )
                .into_any_element()
        };
        let caption = |key: &'static str| {
            div()
                .pl_2()
                .text_color(palette::dim())
                .child(i18n::text(key))
                .into_any_element()
        };
        let mut bar = vec![caption("immersive.column")];
        for (index, width) in ColumnWidth::ALL.into_iter().enumerate() {
            bar.push(button(
                ("immersive-column", index),
                i18n::text(width.label_key()).to_string(),
                settings.column == width,
                ImmersiveAction::Column(width),
                cx,
            ));
        }
        bar.push(caption("immersive.page"));
        for (index, color) in PageColor::ALL.into_iter().enumerate() {
            bar.push(button(
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
            bar.push(button(
                ("immersive-focus", index),
                label,
                settings.line_focus == lines,
                ImmersiveAction::LineFocus(lines),
                cx,
            ));
        }
        bar.push(button(
            ("immersive-spacing", 0),
            i18n::text("immersive.spacing").to_string(),
            settings.text_spacing,
            ImmersiveAction::TextSpacing,
            cx,
        ));
        bar.push(button(
            ("immersive-syllables", 0),
            i18n::text("immersive.syllables").to_string(),
            settings.syllables,
            ImmersiveAction::Syllables,
            cx,
        ));
        bar.push(button(
            ("immersive-read", 0),
            i18n::text("read.button").to_string(),
            reading_now,
            ImmersiveAction::ReadAloud,
            cx,
        ));
        bar.push(div().flex_1().into_any_element());
        bar.push(button(
            ("immersive-close", 0),
            i18n::text("immersive.close").to_string(),
            false,
            ImmersiveAction::Close,
            cx,
        ));
        let read_bar = self.read_aloud_bar(cx);
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
                    .bg(palette::panel())
                    .border_b_1()
                    .border_color(palette::border())
                    .text_sm()
                    .children(bar),
            )
            .children(read_bar)
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
                            .font_family(UI_FONT)
                            .text_size(px(settings.font_size))
                            .text_color(rgb(ink))
                            .children(rows),
                    ),
            );
        Some(deferred(reader).with_priority(2).into_any_element())
    }
}

/// The paragraphs as shown, broken into the lines the column holds.
fn layout(
    paragraphs: &[ShownParagraph],
    settings: &ImmersiveSettings,
    window: &Window,
) -> Vec<ReaderLine> {
    let font = gpui::font(UI_FONT);
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
                    IndentAdjustment::NoIndent,
                )
                .map(|boundary| boundary.ix)
                .collect();
            lines_of(paragraph, &breaks)
        })
        .collect()
}
