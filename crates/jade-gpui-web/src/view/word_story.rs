//! Headers and footers edited where they are, as Word edits them — the
//! browser's copy of the native app's `word_story`: double-click into the
//! top or the bottom of a page, or 挿入 → ヘッダー / フッター, and the story
//! opens in place over its page, the body washed out behind it, with every
//! tool the body has — formatting, several paragraphs, tables, pictures,
//! fields — and the ヘッダーとフッター tab over the ribbon. Double-click the
//! body, Escape or 閉じる to go back.
//!
//! The document side is `jade-web`'s `WordDoc`, which edits the story as a
//! document of its own and writes it back as one undo step.

use jade_web::editing::StoryChange;
use jade_web::word::headless::HeadlessFieldKind;
use jade_web::word::page::Story;

use super::*;
use crate::ribbon::{group, listener, row};
use crate::tooltip::Tooltip;

/// The ヘッダーとフッター tab's commands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StoryAction {
    /// ヘッダーに移動 / フッターに移動.
    GoTo(Story),
    /// 前へ / 次へ: the previous or the next section's.
    Section(bool),
    /// 前と同じ, 先頭ページのみ別指定, 奇数/偶数ページ別指定 and the
    /// distance from the edge.
    Change(StoryChange),
    /// A field at the caret.
    Field(HeadlessFieldKind),
    Close,
}

/// What the ヘッダーとフッター tab shows, read while a story is open.
#[derive(Clone, Debug)]
pub struct StoryTabState {
    pub role: Story,
    pub linked: bool,
    pub section: usize,
    pub section_count: usize,
    pub different_first: bool,
    pub different_odd_even: bool,
    /// From the paper's edge, in points.
    pub distance: f32,
}

/// Today, as a DATE field shows it before it is laid out (`2026/10/21`).
fn today() -> String {
    let now = js_sys::Date::new_0();
    format!(
        "{}/{}/{}",
        now.get_full_year(),
        now.get_month() + 1,
        now.get_date()
    )
}

impl DocView {
    /// Whether a header or footer is open in place.
    pub(super) fn story_editing(&self) -> bool {
        matches!(&self.document, Document::Pages { document, .. } if document.story().is_some())
    }

    /// The tab's state, while a story is open.
    pub(super) fn story_tab_state(&self) -> Option<StoryTabState> {
        let Document::Pages { document, .. } = &self.document else {
            return None;
        };
        let story = document.story()?;
        let layout = document.story_layout()?;
        Some(StoryTabState {
            role: story.role,
            linked: story.linked,
            section: story.section,
            section_count: layout.section_count,
            different_first: layout.different_first_page,
            different_odd_even: layout.different_odd_even,
            distance: match story.role {
                Story::Header => layout.header_distance,
                Story::Footer => layout.footer_distance,
            },
        })
    }

    /// Opens `role` on page `page` in place.
    pub(super) fn open_story_on(&mut self, page: usize, role: Story, cx: &mut Context<Self>) {
        let Document::Pages {
            document,
            ribbon,
            find,
            dialog,
            object,
            ..
        } = &mut self.document
        else {
            return;
        };
        *find = None;
        *dialog = None;
        *object = None;
        let result = document.open_story(page, role);
        if result.is_ok() {
            ribbon.tab = ribbon::Tab::HeaderFooter;
            ribbon.popup = None;
        }
        self.refresh_repainted();
        self.reveal_caret();
        self.word_result(result, cx);
    }

    /// 挿入 → ヘッダー / フッター: the story of the page the caret is on.
    pub(super) fn open_story(&mut self, role: Story, cx: &mut Context<Self>) {
        let Document::Pages {
            document, current, ..
        } = &mut self.document
        else {
            return;
        };
        let page = document
            .caret()
            .and_then(|(paragraph, _)| document.paragraph_start_page(paragraph))
            .unwrap_or(*current);
        self.open_story_on(page, role, cx);
    }

    /// Closes the open story, writing it back; the caret goes back to where
    /// it was in the body.
    pub(super) fn close_story(&mut self, cx: &mut Context<Self>) {
        let Document::Pages {
            document,
            ribbon,
            object,
            ..
        } = &mut self.document
        else {
            return;
        };
        let result = document.close_story().map(|_| ());
        *object = None;
        if ribbon.tab == ribbon::Tab::HeaderFooter {
            ribbon.tab = ribbon::Tab::Insert;
        }
        self.refresh_repainted();
        self.word_result(result, cx);
    }

    /// A press on a page while deciding about stories: a double-click in
    /// the top or bottom margin opens the header or the footer there, one
    /// in the body closes an open story, and a press outside an open
    /// story's band does nothing else. Returns whether the press was taken.
    pub(super) fn story_click(
        &mut self,
        page: usize,
        y_on_page: f64,
        clicks: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        let Document::Pages { document, .. } = &self.document else {
            return false;
        };
        let margin = document.margin_story(page, y_on_page);
        if document.story().is_some() {
            if document.in_story(page, y_on_page) {
                return false;
            }
            if clicks >= 2 {
                match margin {
                    Some(role) => self.open_story_on(page, role, cx),
                    None => self.close_story(cx),
                }
            }
            return true;
        }
        match margin {
            Some(role) if clicks >= 2 => {
                self.open_story_on(page, role, cx);
                true
            }
            _ => false,
        }
    }

    /// Escape leaves the story, as in Word.
    pub(super) fn story_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if event.keystroke.key != "escape" || !self.story_editing() || self.finding() {
            return false;
        }
        self.close_story(cx);
        true
    }

    /// Runs a ヘッダーとフッター tab command.
    pub(super) fn story_action(&mut self, action: StoryAction, cx: &mut Context<Self>) {
        let name = self.name.to_string();
        let Document::Pages { document, .. } = &mut self.document else {
            return;
        };
        let Some(story) = document.story() else {
            return;
        };
        let (page, role) = (story.page, story.role);
        match action {
            StoryAction::Close => self.close_story(cx),
            StoryAction::GoTo(target) => self.open_story_on(page, target, cx),
            StoryAction::Section(next) => {
                if let Some(target) = document.story_section_page(next) {
                    self.open_story_on(target, role, cx);
                }
            }
            StoryAction::Field(kind) => {
                let cached = document.story_field_text(
                    kind,
                    &name,
                    &today(),
                    i18n::text("story.field.heading_missing"),
                );
                self.word_edit(cx, move |document| {
                    document.insert_story_field(kind, cached)
                });
            }
            StoryAction::Change(change) => {
                let result = document.change_story(change);
                self.refresh_repainted();
                self.word_result(result, cx);
            }
        }
    }

    /// The label on the open story's edge on page `index`: which story,
    /// which section, and whether it follows the previous section's.
    pub(super) fn story_label(&self, index: usize, zoom: f32) -> Option<gpui::AnyElement> {
        let Document::Pages { document, .. } = &self.document else {
            return None;
        };
        let story = document.story().filter(|story| story.page == index)?;
        let (_, height) = document.page_size();
        let (top, bottom) = story.band(height);
        let mut label = String::from(i18n::text(match (story.role, story.variant) {
            (Story::Header, "first") => "story.header.first",
            (Story::Header, "even") => "story.header.even",
            (Story::Header, _) => "story.header",
            (Story::Footer, "first") => "story.footer.first",
            (Story::Footer, "even") => "story.footer.even",
            (Story::Footer, _) => "story.footer",
        }));
        {
            use jade_web::editing::WordSession;
            if document.main().word_document().section_count() > 1 {
                label.push_str(&format!(
                    " - {}",
                    i18n::format("story.section", &[("n", &(story.section + 1))])
                ));
            }
        }
        let y = match story.role {
            Story::Header => bottom,
            Story::Footer => top - 16.0,
        };
        let tag = |text: SharedString| {
            div()
                .px_1()
                .rounded_sm()
                .bg(gpui::rgb(0x0F6CBD))
                .text_color(gpui::rgb(0xFFFFFF))
                .text_xs()
                .child(text)
        };
        Some(
            div()
                .absolute()
                .left(px(8.0))
                .top(px(y as f32 * zoom))
                .flex()
                .gap_1()
                .child(tag(label.into()))
                .when(story.linked, |labels| {
                    labels.child(tag(i18n::text("story.linked").into()))
                })
                .into_any_element(),
        )
    }
}

/// The ヘッダーとフッター tab, while a story is open.
pub fn story_groups(state: &StoryTabState, cx: &mut Context<DocView>) -> Vec<gpui::AnyElement> {
    let button = |id: &'static str,
                  label: &'static str,
                  tooltip: &'static str,
                  active: bool,
                  enabled: bool,
                  action: StoryAction,
                  cx: &mut Context<DocView>| {
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .h(px(26.0))
            .px_2()
            .rounded_md()
            .text_xs()
            .whitespace_nowrap()
            .text_color(if enabled {
                palette::text()
            } else {
                palette::faint()
            })
            .when(active, |button| button.bg(palette::selected()))
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|button| button.bg(palette::hovered()))
                    .on_click(listener(Action::Story(action), cx))
            })
            .tooltip(Tooltip::text(tooltip))
            .child(label)
            .into_any_element()
    };
    let t = i18n::text;
    let role = state.role;
    vec![
        group(
            t("story.group.insert"),
            vec![
                row(vec![
                    button(
                        "story-page",
                        t("story.field.page"),
                        t("story.field.page_title"),
                        false,
                        true,
                        StoryAction::Field(HeadlessFieldKind::Page),
                        cx,
                    ),
                    button(
                        "story-pages",
                        t("story.field.pages"),
                        t("story.field.pages_title"),
                        false,
                        true,
                        StoryAction::Field(HeadlessFieldKind::NumPages),
                        cx,
                    ),
                ]),
                row(vec![
                    button(
                        "story-date",
                        t("story.field.date"),
                        t("story.field.date_title"),
                        false,
                        true,
                        StoryAction::Field(HeadlessFieldKind::Date),
                        cx,
                    ),
                    button(
                        "story-heading",
                        t("story.field.heading"),
                        t("story.field.heading_title"),
                        false,
                        true,
                        StoryAction::Field(HeadlessFieldKind::StyleRef),
                        cx,
                    ),
                    button(
                        "story-file",
                        t("story.field.file"),
                        t("story.field.file_title"),
                        false,
                        true,
                        StoryAction::Field(HeadlessFieldKind::FileName),
                        cx,
                    ),
                ]),
            ],
        ),
        group(
            t("story.group.navigation"),
            vec![
                row(vec![
                    button(
                        "story-to-header",
                        t("story.goto.header"),
                        t("story.goto.header"),
                        role == Story::Header,
                        role != Story::Header,
                        StoryAction::GoTo(Story::Header),
                        cx,
                    ),
                    button(
                        "story-to-footer",
                        t("story.goto.footer"),
                        t("story.goto.footer"),
                        role == Story::Footer,
                        role != Story::Footer,
                        StoryAction::GoTo(Story::Footer),
                        cx,
                    ),
                ]),
                row(vec![
                    button(
                        "story-previous",
                        t("story.previous"),
                        t("story.previous_title"),
                        false,
                        state.section > 0,
                        StoryAction::Section(false),
                        cx,
                    ),
                    button(
                        "story-next",
                        t("story.next"),
                        t("story.next_title"),
                        false,
                        state.section + 1 < state.section_count,
                        StoryAction::Section(true),
                        cx,
                    ),
                    button(
                        "story-link",
                        t("story.link"),
                        t("story.link_title"),
                        state.linked,
                        state.section > 0,
                        StoryAction::Change(StoryChange::LinkToPrevious(!state.linked)),
                        cx,
                    ),
                ]),
            ],
        ),
        group(
            t("story.group.options"),
            vec![
                row(vec![button(
                    "story-first",
                    t("setup.header_footer.first_page"),
                    t("setup.header_footer.first_page"),
                    state.different_first,
                    true,
                    StoryAction::Change(StoryChange::DifferentFirst(!state.different_first)),
                    cx,
                )]),
                row(vec![button(
                    "story-odd-even",
                    t("setup.header_footer.odd_even"),
                    t("setup.header_footer.odd_even"),
                    state.different_odd_even,
                    true,
                    StoryAction::Change(StoryChange::DifferentOddEven(!state.different_odd_even)),
                    cx,
                )]),
            ],
        ),
        group(
            t("story.group.position"),
            vec![row(vec![
                button(
                    "story-nearer",
                    "−",
                    t("story.distance_title"),
                    false,
                    state.distance > 0.0,
                    StoryAction::Change(StoryChange::Distance(false)),
                    cx,
                ),
                div()
                    .text_xs()
                    .px_1()
                    .child(i18n::format(
                        match role {
                            Story::Header => "story.distance.header",
                            Story::Footer => "story.distance.footer",
                        },
                        &[("mm", &format!("{:.1}", state.distance * 25.4 / 72.0))],
                    ))
                    .into_any_element(),
                button(
                    "story-further",
                    "+",
                    t("story.distance_title"),
                    false,
                    true,
                    StoryAction::Change(StoryChange::Distance(true)),
                    cx,
                ),
            ])],
        ),
        group(
            t("story.group.close"),
            vec![row(vec![button(
                "story-close",
                t("story.close"),
                t("story.close_title"),
                false,
                true,
                StoryAction::Close,
                cx,
            )])],
        ),
    ]
}
