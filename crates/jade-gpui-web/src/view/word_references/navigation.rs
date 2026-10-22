//! The ナビゲーション pane — 見出し (click to go, collapse in the pane,
//! drag to move a section, レベル上げ / レベル下げ / 上へ / 下へ), ページ
//! and 結果 for what is typed in its search box — the headings collapsed in
//! the body by the triangles beside them, ジャンプ, 次/前のコメント, and
//! ブックマーク表示's brackets.

use std::collections::BTreeSet;

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context, Render, SharedString, Window};
use jade_web::word::references::bookmarks::WordBookmarks;
use jade_web::word::references::navigation::{GoTo, GoToTarget, WordNavigation};

use super::{line, select, t, text_field, Dialog, NavTab, RefList};
use crate::palette as theme;
use crate::view::DocView as DocumentView;
#[allow(unused_imports)]
use jade_web::editing::WordSession;

pub fn toggle(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.references.navigation = !view.references.navigation;
    if !view.references.navigation {
        view.references.searching = false;
    }
    cx.notify();
}

// ---- collapsing in the body ---------------------------------------------

/// Tells the renderer what the collapsed headings hide.
fn apply_collapsed(view: &mut DocumentView) {
    let mut blocks = view.doc().hidden_by_collapse(&view.references.collapsed);
    // アウトライン's レベルの表示 hides the levels below the one shown.
    blocks.extend(crate::view::word_views::outline::extra_hidden(view));
    blocks.sort_unstable();
    blocks.dedup();
    if blocks.as_slice() != view.doc().collapsed_blocks() {
        if let Err(error) = view.doc_mut().set_collapsed_blocks(blocks) {
            view.set_status(SharedString::from(error));
        }
        view.refresh_repainted();
    }
}

/// After an edit: headings that are headings no more stay expanded.
pub fn refresh_collapsed(view: &mut DocumentView) {
    if view.word_ref().is_none() {
        return;
    }
    if view.references.collapsed.is_empty()
        && crate::view::word_views::outline::extra_hidden(view).is_empty()
    {
        if !view.doc().collapsed_blocks().is_empty() {
            apply_collapsed(view);
        }
        return;
    }
    let headings: BTreeSet<usize> = view
        .doc()
        .outline()
        .into_iter()
        .map(|heading| heading.paragraph)
        .collect();
    view.references
        .collapsed
        .retain(|paragraph| headings.contains(paragraph));
    apply_collapsed(view);
}

/// Expands whatever hides a paragraph, as Word does when the caret goes
/// there.
pub fn expand_to(view: &mut DocumentView, paragraph: usize) {
    if view.references.collapsed.is_empty() {
        return;
    }
    let hiding: Vec<usize> = view
        .references
        .collapsed
        .iter()
        .copied()
        .filter(|&heading| {
            heading != paragraph
                && view
                    .doc()
                    .heading_section(heading)
                    .zip(view.doc().word_document().body_element_of(paragraph))
                    .is_some_and(|((start, end), element)| start < element && element <= end)
        })
        .collect();
    if hiding.is_empty() {
        return;
    }
    for heading in hiding {
        view.references.collapsed.remove(&heading);
    }
    apply_collapsed(view);
}

/// The triangle beside a heading: collapses or expands it in the body.
pub fn toggle_heading(view: &mut DocumentView, heading: usize, cx: &mut Context<DocumentView>) {
    if !view.references.collapsed.remove(&heading) {
        view.references.collapsed.insert(heading);
        // The caret cannot stay in what is hidden.
        if view.doc().is_collapsed_away(
            view.caret_at().0,
            &view.doc().hidden_by_collapse(&view.references.collapsed),
        ) {
            view.doc_mut().place_caret(heading, 0);
        }
    }
    apply_collapsed(view);
    cx.notify();
}

/// すべての見出しを展開 / 折りたたみ.
pub fn set_all_collapsed(view: &mut DocumentView, collapsed: bool, cx: &mut Context<DocumentView>) {
    view.references.collapsed = if collapsed {
        view.doc()
            .outline()
            .into_iter()
            .filter(|heading| heading.level == 1)
            .map(|heading| heading.paragraph)
            .collect()
    } else {
        BTreeSet::new()
    };
    if collapsed {
        if let Some(first) = view.references.collapsed.iter().next().copied() {
            view.doc_mut().place_caret(first, 0);
        }
    }
    apply_collapsed(view);
    cx.notify();
}

// ---- the pane -------------------------------------------------------------

/// A heading dragged in the pane.
#[derive(Clone)]
pub struct DraggedHeading {
    paragraph: usize,
    text: SharedString,
}

impl Render for DraggedHeading {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(theme::panel())
            .border_1()
            .border_color(theme::accent())
            .font_family(crate::UI_FONT)
            .text_xs()
            .text_color(theme::text())
            .child(self.text.clone())
    }
}

/// The pane, left of the pages, while it is open.
pub fn navigation_pane(
    view: &mut DocumentView,
    cx: &mut Context<DocumentView>,
) -> Option<AnyElement> {
    if !view.references.navigation {
        return None;
    }
    let tab = view.references.nav_tab;
    let searching = view.references.searching;
    let query_text = view.references.query.text.clone();
    let (before, after) = view.references.query.split();
    let (before, after) = (before.to_string(), after.to_string());
    let search = div()
        .id("nav-search")
        .flex()
        .items_center()
        .h(px(24.0))
        .px_2()
        .rounded_md()
        .bg(theme::canvas())
        .border_1()
        .border_color(if searching {
            theme::accent()
        } else {
            theme::border()
        })
        .cursor_text()
        .overflow_hidden()
        .whitespace_nowrap()
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
            view.references.searching = true;
            view.references.nav_tab = NavTab::Results;
            cx.notify();
        }))
        .when(query_text.is_empty() && !searching, |field| {
            field.child(div().text_color(theme::faint()).child(t("nav.search")))
        })
        .when(!query_text.is_empty() || searching, |field| {
            field
                .child(before)
                .when(searching, |field| {
                    field.child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
                })
                .child(after)
        });
    let tabs = div().flex().gap_1().children(
        [
            (NavTab::Headings, "nav.headings"),
            (NavTab::Pages, "nav.pages"),
            (NavTab::Results, "nav.results"),
        ]
        .into_iter()
        .map(|(entry, label)| {
            div()
                .id(SharedString::from(format!("nav-tab-{label}")))
                .px_2()
                .py_0p5()
                .rounded_sm()
                .cursor_pointer()
                .when(entry == tab, |tab| tab.bg(theme::selected()))
                .hover(|tab| tab.bg(theme::hovered()))
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    view.references.nav_tab = entry;
                    cx.notify();
                }))
                .child(t(label))
                .into_any_element()
        }),
    );
    let content = match tab {
        NavTab::Headings => headings(view, cx),
        NavTab::Pages => {
            let current = view.page_on_show();
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .children((0..view.doc().page_count()).map(|index| {
                    div()
                        .id(("nav-page", index))
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .cursor_pointer()
                        .when(index == current, |row| row.bg(theme::selected()))
                        .hover(|row| row.bg(theme::hovered()))
                        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                            view.show_page(index);
                            cx.notify();
                        }))
                        .child(crate::i18n::format("nav.page", &[("n", &(index + 1))]))
                        .into_any_element()
                }))
                .into_any_element()
        }
        NavTab::Results => results(view, cx),
    };
    Some(
        div()
            .id("navigation-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(240.0))
            .h_full()
            .gap_2()
            .px_2()
            .pt_3()
            .bg(theme::sidebar())
            .border_r_1()
            .border_color(theme::border())
            .text_xs()
            .text_color(theme::text())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(div().text_sm().child(t("nav.title")))
                    .child(
                        div()
                            .id("nav-close")
                            .px_1()
                            .cursor_pointer()
                            .hover(|button| button.bg(theme::hovered()))
                            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| toggle(view, cx)))
                            .child("×"),
                    ),
            )
            .child(search)
            .child(tabs)
            .child(
                div()
                    .id("nav-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(content),
            )
            .into_any_element(),
    )
}

/// The 見出し tab: the outline, the caret's heading highlighted, with the
/// pane's own triangles and the buttons that act on the heading picked.
fn headings(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let outline = view.doc().outline();
    let current = view.doc().current_heading(view.caret_at().0);
    let mut rows = Vec::new();
    let mut hidden_below: Option<u8> = None;
    for (index, heading) in outline.iter().enumerate() {
        if let Some(level) = hidden_below {
            if heading.level > level {
                continue;
            }
            hidden_below = None;
        }
        let folded = view.references.pane_folded.contains(&heading.paragraph);
        if folded {
            hidden_below = Some(heading.level);
        }
        let paragraph = heading.paragraph;
        let text: SharedString = heading.text.clone().into();
        let dragged = DraggedHeading {
            paragraph,
            text: text.clone(),
        };
        rows.push(
            div()
                .id(("nav-heading", index))
                .flex()
                .items_center()
                .gap_1()
                .h(px(22.0))
                .pl(px(f32::from(heading.level.saturating_sub(1)) * 12.0))
                .pr_1()
                .rounded_sm()
                .cursor_pointer()
                .when(current == Some(index), |row| row.bg(theme::selected()))
                .hover(|row| row.bg(theme::hovered()))
                .on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
                .drag_over::<DraggedHeading>(|row, _, _, _| {
                    row.border_t_2().border_color(theme::accent())
                })
                .on_drop(cx.listener(move |view, dragged: &DraggedHeading, _, cx| {
                    move_before(view, dragged.paragraph, paragraph, cx)
                }))
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    view.reveal((paragraph, 0), (paragraph, 0), cx)
                }))
                .child(div().id(("nav-fold", index)).w(px(12.0)).flex_none().when(
                    heading.has_children,
                    |toggle| {
                        toggle
                            .child(
                                gpui::svg()
                                    .path(if folded {
                                        "icons/chevron-right.svg"
                                    } else {
                                        "icons/chevron-down.svg"
                                    })
                                    .size_3()
                                    .text_color(theme::dim()),
                            )
                            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                if !view.references.pane_folded.remove(&paragraph) {
                                    view.references.pane_folded.insert(paragraph);
                                }
                                cx.notify();
                            }))
                    },
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(text),
                )
                .children(heading.page.map(|page| {
                    div()
                        .flex_none()
                        .text_color(theme::faint())
                        .child(page.to_string())
                }))
                .into_any_element(),
        );
    }
    if rows.is_empty() {
        rows.push(
            div()
                .text_color(theme::faint())
                .whitespace_normal()
                .child(t("nav.no_headings"))
                .into_any_element(),
        );
    }
    let button = |id: &'static str,
                  label: &'static str,
                  title: &'static str,
                  cx: &mut Context<DocumentView>,
                  act: fn(&mut DocumentView, &mut Context<DocumentView>)| {
        div()
            .id(id)
            .px_1p5()
            .py_0p5()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .cursor_pointer()
            .hover(|button| button.bg(theme::hovered()))
            .tooltip(crate::tooltip::Tooltip::text(t(title)))
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| act(view, cx)))
            .child(label)
            .into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(button(
                    "nav-promote",
                    "←",
                    "nav.promote",
                    cx,
                    |view, cx| shift(view, true, cx),
                ))
                .child(button("nav-demote", "→", "nav.demote", cx, |view, cx| {
                    shift(view, false, cx)
                }))
                .child(button("nav-up", "↑", "nav.move_up", cx, |view, cx| {
                    move_section(view, true, cx)
                }))
                .child(button(
                    "nav-down",
                    "↓",
                    "nav.move_down",
                    cx,
                    |view, cx| move_section(view, false, cx),
                ))
                .child(button(
                    "nav-collapse-all",
                    t("nav.collapse"),
                    "nav.collapse_all",
                    cx,
                    |view, cx| set_all_collapsed(view, true, cx),
                ))
                .child(button(
                    "nav-expand-all",
                    t("nav.expand"),
                    "nav.expand_all",
                    cx,
                    |view, cx| set_all_collapsed(view, false, cx),
                )),
        )
        .children(rows)
        .into_any_element()
}

/// The heading the caret's section belongs to.
fn current_heading(view: &DocumentView) -> Option<usize> {
    let outline = view.doc().outline();
    view.doc()
        .current_heading(view.caret_at().0)
        .map(|index| outline[index].paragraph)
}

/// レベル上げ / レベル下げ on the caret's heading and those under it.
fn shift(view: &mut DocumentView, promote: bool, cx: &mut Context<DocumentView>) {
    let Some(heading) = current_heading(view) else {
        return;
    };
    let done = view.doc_mut().shift_heading_level(heading, promote);
    view.after_reference_edit(done.map(|()| None), cx);
}

/// 上へ / 下へ: the caret's heading's section past its neighbour.
fn move_section(view: &mut DocumentView, up: bool, cx: &mut Context<DocumentView>) {
    let Some(heading) = current_heading(view) else {
        return;
    };
    let done = view.doc_mut().move_heading_section(heading, up).map(Some);
    view.after_reference_edit(done, cx);
}

/// A heading dropped on another: its section goes before the other one.
fn move_before(
    view: &mut DocumentView,
    dragged: usize,
    target: usize,
    cx: &mut Context<DocumentView>,
) {
    if dragged == target {
        return;
    }
    let document = view.doc();
    let Some((start, end)) = document.heading_section(dragged) else {
        return;
    };
    let Some(before) = document.word_document().body_element_of(target) else {
        return;
    };
    if (start..=end).contains(&before) {
        return;
    }
    let done = view.doc_mut().move_section_to(start, end, before).map(Some);
    view.after_reference_edit(done, cx);
}

/// The 結果 tab: every match with its context, a click selecting it.
fn results(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let results = view.doc().search_results(&view.references.query.text);
    let count = results.len();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .when(!view.references.query.text.is_empty(), |list| {
            list.child(
                div()
                    .text_color(theme::faint())
                    .child(crate::i18n::format("nav.result_count", &[("n", &count)])),
            )
        })
        .children(results.into_iter().enumerate().map(|(index, result)| {
            let start = (result.paragraph, result.start);
            let end = (result.paragraph, result.end);
            div()
                .id(("nav-result", index))
                .p_1()
                .rounded_sm()
                .cursor_pointer()
                .hover(|row| row.bg(theme::hovered()))
                .on_click(
                    cx.listener(move |view, _: &ClickEvent, _, cx| view.reveal(start, end, cx)),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .child(result.before)
                        .child(
                            div()
                                .bg(gpui::rgba(0xFFE08A99))
                                .text_color(theme::text())
                                .child(result.matched),
                        )
                        .child(result.after),
                )
                .children(result.page.map(|page| {
                    div()
                        .text_color(theme::faint())
                        .child(crate::i18n::format("nav.page", &[("n", &page)]))
                }))
                .into_any_element()
        }))
        .into_any_element()
}

/// A key while the search box has the keyboard.
pub fn search_key(view: &mut DocumentView, key: &str, cx: &mut Context<DocumentView>) -> bool {
    match key {
        "escape" => view.references.searching = false,
        "enter" | "return" => {
            // The next match after the caret, as Word's search box does.
            let results = view.doc().search_results(&view.references.query.text);
            let next = results
                .iter()
                .find(|result| (result.paragraph, result.start) > view.caret_at())
                .or_else(|| results.first())
                .map(|result| {
                    (
                        (result.paragraph, result.start),
                        (result.paragraph, result.end),
                    )
                });
            if let Some((start, end)) = next {
                view.reveal(start, end, cx);
            }
        }
        _ => {
            if !view.references.query.edit_key(key) {
                return key.chars().count() > 1 && key != "space";
            }
        }
    }
    cx.notify();
    true
}

// ---- ジャンプ ---------------------------------------------------------------

pub fn open_go_to(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.references
        .open(Dialog::GoTo(GoToTarget::Page), vec![String::new()]);
    cx.notify();
}

/// ジャンプ: the caret to the target; the dialog stays open for the next.
pub fn confirm_go_to(view: &mut DocumentView, target: GoToTarget, cx: &mut Context<DocumentView>) {
    view.references.dialog = Some(Dialog::GoTo(target));
    let spec = view.references.field(0).to_string();
    match view.doc().go_to(target, &spec, view.caret_at()) {
        Ok(GoTo::Position(at)) => {
            view.references.problem = None;
            view.reveal(at, at, cx);
        }
        Ok(GoTo::Page(page)) => {
            view.references.problem = None;
            go_to_page(view, page, cx);
        }
        Ok(GoTo::Line(number)) => {
            view.references.problem = None;
            go_to_line(view, number, cx);
        }
        Err(problem) => {
            view.references.problem = Some(problem);
            cx.notify();
        }
    }
}

/// The first place on a page: its first line's start.
fn go_to_page(view: &mut DocumentView, page: usize, cx: &mut Context<DocumentView>) {
    let pages = view.doc().page_count().max(1);
    let index = page.clamp(1, pages) - 1;
    let (width, height) = view.doc().page_size();
    let mut found = None;
    let mut y = 0.0;
    while y < height && found.is_none() {
        found = view
            .doc()
            .position_at(width / 2.0, view.doc().document_y(index, y));
        y += 12.0;
    }
    match found {
        Some(at) => view.reveal(at, at, cx),
        None => {
            view.show_page(index);
            cx.notify();
        }
    }
}

/// The document's `number`-th line from its start.
fn go_to_line(view: &mut DocumentView, number: usize, cx: &mut Context<DocumentView>) {
    let first = (0..view.doc().paragraph_count())
        .find(|&paragraph| {
            view.doc()
                .word_document()
                .is_body_paragraph_surface(paragraph)
        })
        .unwrap_or(0);
    view.doc_mut().place_caret(first, 0);
    for _ in 1..number {
        let before = view.caret_at();
        if view
            .doc_mut()
            .move_caret(jade_web::editing::Motion::Down, false)
            .is_err()
            || view.caret_at() == before
        {
            break;
        }
    }
    let at = view.caret_at();
    view.reveal(at, at, cx);
}

pub fn go_to_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::GoTo(target)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let target = *target;
    let entries: Vec<(SharedString, bool)> = GoToTarget::ALL
        .iter()
        .map(|entry| (t(entry.label_key()).into(), *entry == target))
        .collect();
    let bookmarks: Vec<String> = if target == GoToTarget::Bookmark {
        view.doc()
            .bookmarks()
            .into_iter()
            .filter(|bookmark| !bookmark.hidden())
            .map(|bookmark| bookmark.name)
            .collect()
    } else {
        Vec::new()
    };
    let bookmark_entries: Vec<(SharedString, bool)> = bookmarks
        .iter()
        .map(|name| (name.clone().into(), view.references.field(0) == name))
        .collect();
    div()
        .w(px(380.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(line("goto.target").child(select(
            view,
            RefList::ReferenceType,
            t(target.label_key()),
            160.0,
            entries,
            cx,
            |references, index| {
                references.dialog = Some(Dialog::GoTo(GoToTarget::ALL[index]));
            },
        )))
        .child(if target == GoToTarget::Bookmark {
            line("goto.bookmark_name")
                .child(select(
                    view,
                    RefList::Bookmark,
                    view.references.field(0).to_string(),
                    200.0,
                    bookmark_entries,
                    cx,
                    move |references, index| {
                        references.fields[0] =
                            jade_web::text::TextEdit::new(bookmarks[index].clone());
                    },
                ))
                .into_any_element()
        } else {
            text_field(view, 0, "goto.number", 160.0, cx)
        })
        .child(
            div()
                .text_color(theme::faint())
                .whitespace_normal()
                .child(t("goto.hint")),
        )
        .into_any_element()
}

// ---- comments -------------------------------------------------------------

/// 次のコメント / 前のコメント: the comment's text selected, the pane open.
pub fn step_comment(view: &mut DocumentView, forwards: bool, cx: &mut Context<DocumentView>) {
    let from = match view.doc().selection() {
        Some((anchor, focus)) if forwards => anchor.max(focus),
        Some((anchor, focus)) => anchor.min(focus),
        None => view.caret_at(),
    };
    match view
        .doc()
        .step_comment(from, forwards)
        .and_then(|comment| comment.anchor)
    {
        Some((start, end)) => {
            view.show_comments_pane();
            view.reveal(start, end, cx);
        }
        None => {
            view.set_status(SharedString::from(t("comments.none")));
            cx.notify();
        }
    }
}

// ---- marks on the pages -----------------------------------------------------

/// Something drawn over a page in the page's coordinates: a bookmark's
/// bracket, or a heading's collapse triangle.
#[derive(Clone, Debug)]
pub enum BookmarkMark {
    /// `[`, `]`, or an I-beam for a point bookmark.
    Bracket {
        page: usize,
        x: f32,
        y: f32,
        height: f32,
        kind: char,
    },
    /// ▸ or ▾ left of a heading.
    Toggle {
        page: usize,
        x: f32,
        y: f32,
        height: f32,
        heading: usize,
        collapsed: bool,
    },
}

/// Where ブックマーク表示's brackets and the headings' triangles go, read
/// from the last layout.
pub fn bookmark_marks(view: &mut DocumentView) -> Vec<BookmarkMark> {
    let show_bookmarks = view.references.show_bookmarks;
    let toggles = view.references.navigation || !view.references.collapsed.is_empty();
    if !show_bookmarks && !toggles {
        return Vec::new();
    }
    let mut marks = Vec::new();
    let rect = |view: &mut DocumentView, at: (usize, usize)| {
        jade_web::word::WordSession::position_metrics(view.doc(), at).map(|(x, top, height)| {
            let (page, y) = view.doc().page_and_offset(top);
            (page, x as f32, y as f32, height as f32)
        })
    };
    if show_bookmarks {
        for (start, end) in view.doc().bookmark_marks() {
            if start == end {
                if let Some((page, x, y, height)) = rect(view, start) {
                    marks.push(BookmarkMark::Bracket {
                        page,
                        x,
                        y,
                        height,
                        kind: 'I',
                    });
                }
                continue;
            }
            if let Some((page, x, y, height)) = rect(view, start) {
                marks.push(BookmarkMark::Bracket {
                    page,
                    x,
                    y,
                    height,
                    kind: '[',
                });
            }
            if let Some((page, x, y, height)) = rect(view, end) {
                marks.push(BookmarkMark::Bracket {
                    page,
                    x,
                    y,
                    height,
                    kind: ']',
                });
            }
        }
    }
    if toggles {
        for heading in view.doc().outline() {
            if let Some((page, x, y, height)) = rect(view, (heading.paragraph, 0)) {
                marks.push(BookmarkMark::Toggle {
                    page,
                    x,
                    y,
                    height,
                    heading: heading.paragraph,
                    collapsed: view.references.collapsed.contains(&heading.paragraph),
                });
            }
        }
    }
    marks
}

/// The marks of one page, over it.
pub fn page_marks(
    marks: &[BookmarkMark],
    page: usize,
    zoom: f32,
    cx: &mut Context<DocumentView>,
) -> Vec<AnyElement> {
    marks
        .iter()
        .enumerate()
        .filter_map(|(index, mark)| match *mark {
            BookmarkMark::Bracket {
                page: on,
                x,
                y,
                height,
                kind,
            } if on == page => {
                let gray = gpui::rgb(0x808080);
                let stem = div().absolute().w(px(1.0)).h(px(height * zoom)).bg(gray);
                let bar = || div().absolute().w(px(3.0)).h(px(1.0)).bg(gray);
                Some(
                    div()
                        .absolute()
                        .left(px(x * zoom - 1.0))
                        .top(px(y * zoom))
                        .w(px(4.0))
                        .h(px(height * zoom))
                        .child(stem.left(px(if kind == ']' { 3.0 } else { 0.0 })))
                        .when(kind != 'I', |mark| {
                            // A bar is 3px, so from the box's left it meets either stem.
                            let left = 0.0;
                            mark.child(bar().left(px(left)).top_0())
                                .child(bar().left(px(left)).bottom_0())
                        })
                        .into_any_element(),
                )
            }
            BookmarkMark::Toggle {
                page: on,
                x,
                y,
                height,
                heading,
                collapsed,
            } if on == page => Some(
                div()
                    .id(("heading-toggle", index))
                    .absolute()
                    .left(px((x - 16.0).max(0.0) * zoom))
                    .top(px(y * zoom))
                    .h(px(height * zoom))
                    .flex()
                    .items_center()
                    .text_size(px(10.0 * zoom.max(0.8)))
                    .text_color(theme::dim())
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                        toggle_heading(view, heading, cx)
                    }))
                    .child(
                        gpui::svg()
                            .path(if collapsed {
                                "icons/chevron-right.svg"
                            } else {
                                "icons/chevron-down.svg"
                            })
                            .size(px(10.0 * zoom.max(0.8)))
                            .text_color(theme::dim()),
                    )
                    .into_any_element(),
            ),
            _ => None,
        })
        .collect()
}
