//! Word's 表示 tab in the browser — the views (閲覧モード, 印刷レイアウト,
//! Web レイアウト, アウトライン, 下書き, フォーカス), ページ移動, グリッド線,
//! 余白を表示しない, ズーム with its fits and dialog, 分割 and 新しいウィンドウ
//! — with 編集 / 校閲 / 閲覧, the ミニ ツールバー, オートコレクト and Word の
//! オプション: the browser's copy of the native app's `word_views`, over the
//! arithmetic both shells share (`jade_office_model::word::view`,
//! `word::outline`, `word::customize`).
//!
//! The pages are the engine's; a view changes how the shell sets them out
//! (one under another, several across, side by side), which part of each it
//! shows (下書き and 余白を表示しない keep the band between the margins) and
//! what it draws over them. Web レイアウト, 閲覧モード and アウトライン hand
//! the engine a page box of their own.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Window};
use jade_web::text::TextEdit;
use jade_web::word::view::{
    self, fit_zoom, grid_lines, DrawingGrid, PageBand, PageCrop, PageGrid, PageMovement, ViewKind,
    ZoomFit,
};

pub mod customize;
pub mod mini;
pub mod modes;
pub mod options;
pub mod outline;
pub mod prefs;
pub mod settings;
pub mod split;

use super::word_kit::{button, chip, modal, on};
use super::{DocView as DocumentView, Document};
use crate::ribbon::{group, row, Action};
use crate::{i18n, palette as theme};

/// What the 表示 tab's controls (and the menus that reach them) do.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewsAction {
    /// 閲覧モード, 印刷レイアウト, Web レイアウト, アウトライン, 下書き.
    SetView(ViewKind),
    /// 縦 / 横.
    SetMovement(PageMovement),
    /// 1 ページ, 複数ページ, ページ幅を基準に表示.
    Fit(ZoomFit),
    /// 100%, or a percentage from the ズーム dialog.
    Percent(u32),
    /// ズーム….
    OpenZoom,
    /// グリッド線.
    ToggleGridlines,
    /// 余白を表示しない / 余白を表示する.
    ToggleWhiteSpace,
    /// フォーカス.
    ToggleFocus,
    /// ウィンドウ → 分割.
    Split,
    /// ウィンドウ → 新しいウィンドウを開く, and closing it.
    NewWindow,
    CloseWindow,
    /// 編集 / 校閲 / 閲覧, and its list.
    SetMode(modes::EditMode),
    ToggleModeMenu,
    /// ツール → オートコレクト….
    OpenAutoCorrect,
    /// Word のオプション, at one of its pages.
    OpenOptions(prefs::OptionsPage),
    /// The アウトライン tab.
    Outline(outline::OutlineCommand),
    /// A クイック アクセス ツール バー (or the user's group's) command.
    Quick(jade_web::word::customize::QuickCommand),
    /// The ミニ ツールバー's own lists.
    MiniPopup(Option<mini::MiniPopup>),
}

/// The ズーム dialog's choice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ZoomChoice {
    Percent(u32),
    Fit(ZoomFit),
    /// 倍率 typed into its box.
    Typed,
}

/// The open dialog.
pub enum ViewDialog {
    Zoom {
        choice: ZoomChoice,
        percent: TextEdit,
        pages: usize,
    },
    AutoCorrect(Box<options::AutoCorrectDialog>),
    Options(prefs::OptionsPage),
}

/// The view's state of the 表示 tab.
#[derive(Default)]
pub struct Views {
    pub kind: ViewKind,
    pub movement: PageMovement,
    pub fit: ZoomFit,
    pub gridlines: bool,
    /// The settings part's drawing grid, read when グリッド線 goes on.
    grid: Option<DrawingGrid>,
    pub hide_white_space: bool,
    pub dialog: Option<ViewDialog>,
    pub mode: modes::EditMode,
    pub mode_menu: bool,
    pub focus: bool,
    /// 分割's second pane, and 新しいウィンドウ's, while they show.
    pub split: Option<split::Pane>,
    pub window: Option<split::Pane>,
    /// How many 新しいウィンドウ have been opened.
    pub windows: usize,
    /// The bounds of the pane a press went down in, for the drag after it.
    pub drag_pane: Option<split::PageBounds>,
    /// アウトライン's レベルの表示: 1–9, or 0 / 10 for every level.
    pub outline_level: u8,
    /// ミニ ツールバー: shown over a selection the mouse made.
    pub mini: bool,
    pub mini_popup: Option<mini::MiniPopup>,
    /// Whether the mouse went down on a page and has not come up yet.
    pub pressed: bool,
}

impl Views {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// The field typing goes into while a dialog is open.
    pub fn text_field(&self) -> Option<&TextEdit> {
        match self.dialog.as_ref()? {
            ViewDialog::Zoom { percent, .. } => Some(percent),
            ViewDialog::AutoCorrect(dialog) => dialog.fields.get(dialog.focus),
            ViewDialog::Options(_) => None,
        }
    }

    pub fn text_field_mut(&mut self) -> Option<&mut TextEdit> {
        match self.dialog.as_mut()? {
            ViewDialog::Zoom {
                percent, choice, ..
            } => {
                *choice = ZoomChoice::Typed;
                Some(percent)
            }
            ViewDialog::AutoCorrect(dialog) => dialog.fields.get_mut(dialog.focus),
            ViewDialog::Options(_) => None,
        }
    }

    /// Whether pages show only the band between their margins.
    pub fn crops_pages(&self) -> bool {
        self.kind == ViewKind::Draft || (self.kind == ViewKind::Print && self.hide_white_space)
    }

    /// Whether the ribbon and the page list give way: フォーカス and 閲覧モード.
    pub fn hides_chrome(&self) -> bool {
        self.focus || self.kind == ViewKind::Read
    }

    /// Whether the pages sit in one row, scrolled sideways.
    pub fn horizontal(&self) -> bool {
        self.kind == ViewKind::Read
            || (self.movement == PageMovement::SideToSide && self.kind.keeps_pages())
    }

    /// Whether the view is one long sheet rather than pages.
    pub fn sheet(&self) -> bool {
        matches!(self.kind, ViewKind::Web | ViewKind::Outline)
    }
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// How the pages are set out this frame.
pub struct Arrangement {
    pub grid: PageGrid,
    /// The part of each page shown, when the view cuts them.
    pub crops: Vec<Option<PageCrop>>,
    pub background: gpui::Rgba,
    pub sheet: bool,
    /// 下書き: the bands from the left, as Word sets them.
    pub from_left: bool,
}

impl DocumentView {
    /// Runs a 表示 action.
    pub(crate) fn views_action(
        &mut self,
        action: ViewsAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Document::Pages { ribbon, .. } = &mut self.document {
            ribbon.popup = None;
        }
        // The keyboard back to the pages, as after any ribbon command; a
        // list of the mini toolbar's keeps it where it is.
        if !matches!(
            action,
            ViewsAction::MiniPopup(_) | ViewsAction::ToggleModeMenu
        ) {
            window.focus(&self.focus, cx);
        }
        match action {
            ViewsAction::SetView(kind) => self.set_view(kind),
            ViewsAction::SetMovement(movement) => {
                self.views.movement = movement;
                self.views.fit = match movement {
                    PageMovement::SideToSide => ZoomFit::OnePage,
                    PageMovement::Vertical => ZoomFit::Custom,
                };
                self.scroll_to_current();
            }
            ViewsAction::Fit(fit) => {
                self.views.fit = fit;
                if !matches!(fit, ZoomFit::OnePage | ZoomFit::MultiplePages(_)) {
                    self.views.movement = PageMovement::Vertical;
                }
                self.scroll_to_current();
            }
            ViewsAction::Percent(percent) => {
                self.views.movement = PageMovement::Vertical;
                self.set_zoom(percent as f32 / 100.0, cx);
            }
            ViewsAction::OpenZoom => self.open_zoom(),
            ViewsAction::ToggleGridlines => {
                self.views.gridlines = !self.views.gridlines;
                if self.views.gridlines && self.views.grid.is_none() {
                    self.views.grid = Some(view::document_drawing_grid(&**self.doc()));
                }
            }
            ViewsAction::ToggleWhiteSpace => {
                self.views.hide_white_space = !self.views.hide_white_space;
            }
            ViewsAction::ToggleFocus => modes::toggle_focus(self, window),
            ViewsAction::Split => split::toggle_split(self),
            ViewsAction::NewWindow => split::open_window(self),
            ViewsAction::CloseWindow => self.views.window = None,
            ViewsAction::SetMode(mode) => modes::set_mode(self, mode, cx),
            ViewsAction::ToggleModeMenu => self.views.mode_menu = !self.views.mode_menu,
            ViewsAction::OpenAutoCorrect => {
                self.views.dialog = Some(ViewDialog::AutoCorrect(Box::new(
                    options::AutoCorrectDialog::open(),
                )))
            }
            ViewsAction::OpenOptions(page) => self.views.dialog = Some(ViewDialog::Options(page)),
            ViewsAction::Outline(command) => return outline::run(self, command, window, cx),
            ViewsAction::Quick(command) => return customize::run(self, command, window, cx),
            ViewsAction::MiniPopup(popup) => self.views.mini_popup = popup,
        }
        cx.notify();
    }

    /// Changes the view: how its pages move and zoom, and the page box the
    /// document is laid out in.
    fn set_view(&mut self, kind: ViewKind) {
        let leaving_outline = self.views.kind == ViewKind::Outline && kind != ViewKind::Outline;
        if kind != ViewKind::Print && self.doc().story().is_some() {
            // A header open over the pages goes first: the views have none.
            if let Err(error) = self.doc_mut().close_story() {
                return self.set_status(error.into());
            }
        }
        let leaving_read = self.views.kind == ViewKind::Read && kind != ViewKind::Read;
        self.views.kind = kind;
        self.views.mini = false;
        if leaving_read {
            // 閲覧モード turned its columns sideways; the pages go back to
            // one under another.
            self.views.movement = PageMovement::Vertical;
            self.views.fit = ZoomFit::Custom;
        }
        match kind {
            ViewKind::Read => {
                self.views.movement = PageMovement::SideToSide;
                self.views.fit = ZoomFit::Custom;
            }
            ViewKind::Web | ViewKind::Outline => {
                self.views.movement = PageMovement::Vertical;
                self.views.fit = ZoomFit::Custom;
            }
            ViewKind::Draft => {
                self.views.movement = PageMovement::Vertical;
                if matches!(self.views.fit, ZoomFit::OnePage | ZoomFit::MultiplePages(_)) {
                    self.views.fit = ZoomFit::Custom;
                }
            }
            ViewKind::Print => {}
        }
        if kind == ViewKind::Outline {
            self.views.outline_level = jade_web::word::outline::BODY_LEVEL;
            if let Document::Pages { ribbon, .. } = &mut self.document {
                ribbon.outline = true;
            }
        } else if let Document::Pages { ribbon, .. } = &mut self.document {
            ribbon.outline = false;
            if ribbon.tab == crate::ribbon::Tab::Outlining {
                ribbon.tab = crate::ribbon::Tab::View;
            }
        }
        self.follow_view_layout(None);
        outline::follow(self);
        if leaving_outline {
            super::word_references::refresh_collapsed(self);
        }
        self.scroll_to_current();
    }

    /// Scrolls back to the page that was current, once the pages are set
    /// out anew.
    fn scroll_to_current(&mut self) {
        if let Document::Pages {
            scroll, current, ..
        } = &self.document
        {
            scroll.scroll_to_item(*current);
        }
    }

    /// Gives the document the page box its view wants for a viewport of
    /// `area` (the last frame's when `None`).
    pub(crate) fn follow_view_layout(&mut self, area: Option<(f32, f32)>) {
        let kind = self.views.kind;
        let zoom = self.zoom.unwrap_or(1.0);
        let wanted = match kind {
            ViewKind::Web | ViewKind::Read => {
                let Some(area) = area.or_else(|| self.pages_viewport()) else {
                    return;
                };
                view::view_geometry(kind, area, zoom)
            }
            ViewKind::Outline => view::outline_geometry(&self.doc().page_section_layout(0)),
            _ => None,
        };
        if view::geometry_close(self.doc().view_geometry(), wanted) {
            return;
        }
        let caret = self.doc().caret();
        let result = self.doc_mut().set_view_geometry(wanted);
        if let Some((paragraph, offset)) = caret {
            self.doc_mut().place_caret(paragraph, offset);
        }
        match result {
            Ok(_) => self.refresh_repainted(),
            Err(error) => self.set_status(error.into()),
        }
    }

    /// The pages' viewport as last laid out.
    fn pages_viewport(&self) -> Option<(f32, f32)> {
        let Document::Pages { scroll, .. } = &self.document else {
            return None;
        };
        let size = scroll.bounds().size;
        let (width, height) = (f32::from(size.width), f32::from(size.height));
        (width > 1.0 && height > 1.0).then_some((width, height))
    }

    /// Called as the pages render: a fit follows the viewport, a view that
    /// lays the document out anew follows the window, and Word のオプション's
    /// marks reach the document. Whether the zoom changed, which the status
    /// bar, drawn first, is to show.
    pub(crate) fn views_before_render(&mut self, area: (f32, f32)) -> bool {
        if !matches!(self.document, Document::Pages { .. }) {
            return false;
        }
        let mut changed = false;
        let viewport = self.pages_viewport().unwrap_or(area);
        if self.views.fit != ZoomFit::Custom {
            let (page_width, page_height) = self.doc().page_size();
            let band = self.page_band(0);
            let text = band
                .map(|band| (band.left, band.right - band.left))
                .unwrap_or((0.0, page_width as f32));
            if let Some(zoom) = fit_zoom(
                self.views.fit,
                (page_width as f32, page_height as f32),
                text,
                viewport,
            ) {
                // A hair under the fit, so the row of 複数ページ wraps
                // where it should.
                let zoom = if matches!(self.views.fit, ZoomFit::MultiplePages(_)) {
                    zoom * 0.99
                } else {
                    zoom
                };
                let zoom = zoom.clamp(super::ZOOM_MIN, super::ZOOM_MAX);
                if self.zoom.is_none_or(|now| (now - zoom).abs() > 0.001) {
                    self.zoom = Some(zoom);
                    changed = true;
                }
            }
        }
        if matches!(self.views.kind, ViewKind::Web | ViewKind::Read) {
            self.follow_view_layout(Some(viewport));
        }
        let kinds = prefs::mark_kinds();
        if self.doc().formatting_mark_kinds() != kinds {
            match self.doc_mut().set_formatting_mark_kinds(kinds) {
                Ok(_) => self.refresh_repainted(),
                Err(error) => self.set_status(error.into()),
            }
        }
        changed
    }

    /// The band between the margins of page `index`.
    pub(crate) fn page_band(&self, index: usize) -> Option<PageBand> {
        let layout = self.word_ref()?.page_section_layout(index);
        if layout.width <= 0.0 || layout.height <= 0.0 {
            return None;
        }
        Some(PageBand::of(&layout))
    }

    /// How the pages are set out: the grid they sit in and what of each
    /// shows.
    pub(crate) fn arrangement(&self, page_size: (f32, f32), zoom: f32) -> Arrangement {
        let views = &self.views;
        let count = match &self.document {
            Document::Pages { pages, .. } => pages.len(),
            _ => 0,
        };
        let crops: Vec<Option<PageCrop>> = (0..count)
            .map(|index| {
                if !views.crops_pages() {
                    return None;
                }
                Some(self.page_band(index)?.crop(views.kind, page_size))
            })
            .collect();
        let shown = crops
            .first()
            .copied()
            .flatten()
            .map_or(page_size, |crop| (crop.width, crop.height));
        let page = (shown.0 * zoom, shown.1 * zoom);
        let sheet = views.sheet() && !views.focus;
        let (gap, padding) = if sheet || views.kind == ViewKind::Draft {
            (0.0, 0.0)
        } else if views.kind == ViewKind::Read {
            (view::READ_GAP, super::PAGES_PADDING)
        } else if views.crops_pages() {
            (3.0, super::PAGES_PADDING)
        } else {
            (super::PAGE_GAP, super::PAGES_PADDING)
        };
        let per_row = match (views.kind.keeps_pages(), views.movement, views.fit) {
            (true, PageMovement::Vertical, ZoomFit::MultiplePages(count)) => count.max(1),
            _ => 1,
        };
        let background = if views.focus {
            gpui::rgb(0x1b1b1f)
        } else if sheet || views.kind == ViewKind::Draft {
            theme::paper()
        } else {
            theme::canvas()
        };
        Arrangement {
            grid: PageGrid {
                page,
                gap,
                padding,
                per_row,
                horizontal: views.horizontal(),
            },
            crops,
            background,
            sheet,
            from_left: views.kind == ViewKind::Draft && !views.focus,
        }
    }

    /// グリッド線 over page `index`.
    pub(crate) fn grid_overlay(&self, index: usize, zoom: f32) -> Option<AnyElement> {
        if !self.views.gridlines || self.views.kind != ViewKind::Print {
            return None;
        }
        let grid = self.views.grid.unwrap_or_default();
        let layout = self.word_ref()?.page_section_layout(index);
        let lines = grid_lines(&layout, &grid);
        let color = gpui::rgba(0x7f9fcf66);
        let (left, top) = (lines.left * zoom, lines.top * zoom);
        let (width, height) = (
            (lines.right - lines.left) * zoom,
            (lines.bottom - lines.top) * zoom,
        );
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .children(lines.rows.iter().map(|y| {
                    div()
                        .absolute()
                        .left(px(left))
                        .top(px(y * zoom))
                        .w(px(width))
                        .h(px(1.0))
                        .bg(color)
                }))
                .children(lines.columns.iter().map(|x| {
                    div()
                        .absolute()
                        .left(px(x * zoom))
                        .top(px(top))
                        .w(px(1.0))
                        .h(px(height))
                        .bg(color)
                }))
                .into_any_element(),
        )
    }

    // ---- the ズーム dialog -----------------------------------------------------

    fn open_zoom(&mut self) {
        let percent = (self.zoom() * 100.0).round() as u32;
        let choice = match self.views.fit {
            ZoomFit::Custom if view::ZOOM_PERCENTS.contains(&percent) => {
                ZoomChoice::Percent(percent)
            }
            ZoomFit::Custom => ZoomChoice::Typed,
            fit => ZoomChoice::Fit(fit),
        };
        let pages = match self.views.fit {
            ZoomFit::MultiplePages(count) => count,
            _ => 2,
        };
        self.views.dialog = Some(ViewDialog::Zoom {
            choice,
            percent: TextEdit::new(percent.to_string()),
            pages,
        });
    }

    fn confirm_zoom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ViewDialog::Zoom {
            choice,
            percent,
            pages,
        }) = self.views.dialog.take()
        else {
            return;
        };
        let action = match choice {
            ZoomChoice::Percent(value) => ViewsAction::Percent(value),
            ZoomChoice::Fit(ZoomFit::MultiplePages(_)) => {
                ViewsAction::Fit(ZoomFit::MultiplePages(pages))
            }
            ZoomChoice::Fit(fit) => ViewsAction::Fit(fit),
            ZoomChoice::Typed => match percent.text.trim().trim_end_matches('%').parse::<u32>() {
                Ok(value) if (10..=500).contains(&value) => ViewsAction::Percent(value),
                _ => {
                    self.views.dialog = Some(ViewDialog::Zoom {
                        choice,
                        percent,
                        pages,
                    });
                    return cx.notify();
                }
            },
        };
        self.views_action(action, window, cx);
    }

    /// A key while a 表示 dialog is open; `true` when it was taken.
    pub(crate) fn views_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.views.dialog.is_none() {
            return false;
        }
        if self
            .views
            .text_field()
            .is_some_and(|field| field.marked.is_some())
        {
            return false;
        }
        if matches!(self.views.dialog, Some(ViewDialog::AutoCorrect(_))) {
            return options::handle_key(self, key, cx);
        }
        if matches!(self.views.dialog, Some(ViewDialog::Options(_))) {
            if matches!(key, "escape" | "enter") {
                self.views.dialog = None;
                cx.notify();
            }
            return true;
        }
        match key {
            "escape" => self.views.dialog = None,
            "enter" => {
                self.confirm_zoom(window, cx);
                return true;
            }
            key => {
                let Some(field) = self.views.text_field_mut() else {
                    return false;
                };
                if !field.edit_key(key) {
                    return false;
                }
            }
        }
        cx.notify();
        true
    }

    /// The open 表示 dialog, over the window.
    pub(crate) fn views_dialog(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (choice, percent, pages) = match self.views.dialog.as_ref()? {
            ViewDialog::Zoom {
                choice,
                percent,
                pages,
            } => (*choice, percent, *pages),
            ViewDialog::AutoCorrect(dialog) => return Some(options::overlay(dialog, window, cx)),
            ViewDialog::Options(page) => return Some(prefs::overlay(*page, window, cx)),
        };
        fn set(
            view: &mut DocumentView,
            cx: &mut Context<DocumentView>,
            change: impl FnOnce(&mut ZoomChoice, &mut usize),
        ) {
            if let Some(ViewDialog::Zoom { choice, pages, .. }) = view.views.dialog.as_mut() {
                change(choice, pages);
            }
            cx.notify();
        }
        let percents = [200u32, 100, 75].into_iter().map(|value| {
            chip(
                ("zoom-percent", value as usize),
                format!("{value}%"),
                choice == ZoomChoice::Percent(value),
            )
            .on_click(on(cx, move |view, cx| {
                set(view, cx, |choice, _| *choice = ZoomChoice::Percent(value))
            }))
            .into_any_element()
        });
        let fits = [
            (ZoomFit::PageWidth, "views.zoom.page_width"),
            (ZoomFit::TextWidth, "views.zoom.text_width"),
            (ZoomFit::OnePage, "views.zoom.whole_page"),
            (ZoomFit::MultiplePages(pages), "views.zoom.many_pages"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (fit, key))| {
            let on_now = match (choice, fit) {
                (ZoomChoice::Fit(ZoomFit::MultiplePages(_)), ZoomFit::MultiplePages(_)) => true,
                (ZoomChoice::Fit(chosen), fit) => chosen == fit,
                _ => false,
            };
            chip(("zoom-fit", index), t(key), on_now)
                .on_click(on(cx, move |view, cx| {
                    set(view, cx, |choice, _| *choice = ZoomChoice::Fit(fit))
                }))
                .into_any_element()
        });
        let counts = (1..=4usize).map(|count| {
            chip(("zoom-pages", count), count.to_string(), pages == count)
                .on_click(on(cx, move |view, cx| {
                    set(view, cx, |choice, pages| {
                        *pages = count;
                        *choice = ZoomChoice::Fit(ZoomFit::MultiplePages(count));
                    })
                }))
                .into_any_element()
        });
        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().flex().flex_wrap().gap_1().children(percents))
            .child(div().flex().flex_wrap().gap_1().children(fits))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("views.zoom.pages")),
                    )
                    .children(counts),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("views.zoom.percent")),
                    )
                    .child(
                        super::word_kit::input("zoom-typed", percent, choice == ZoomChoice::Typed)
                            .w(px(80.0))
                            .on_click(on(cx, |view, cx| {
                                set(view, cx, |choice, _| *choice = ZoomChoice::Typed)
                            })),
                    ),
            )
            .into_any_element();
        let buttons = div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                button("zoom-cancel", t("format.cancel"), false).on_click(on(cx, |view, cx| {
                    view.views.dialog = None;
                    cx.notify();
                })),
            )
            .child(
                button("zoom-ok", t("format.ok"), true)
                    .on_click(cx.listener(|view, _, window, cx| view.confirm_zoom(window, cx))),
            )
            .into_any_element();
        Some(modal(
            "zoom-dialog",
            360.0,
            t("views.zoom.title"),
            body,
            None,
            buttons,
            window,
        ))
    }
}

/// A page's surface: sized to what the view shows of the page, with
/// `inner` — the whole page and what is drawn over it — moved so the band
/// the view keeps is what shows.
pub(crate) fn frame_page(
    surface: gpui::Stateful<gpui::Div>,
    inner: gpui::Div,
    crop: Option<PageCrop>,
    full: (f32, f32),
    zoom: f32,
    sheet: bool,
) -> gpui::Stateful<gpui::Div> {
    let surface = surface.relative().flex_none().bg(theme::paper());
    match crop {
        None => {
            let surface = surface.w(px(full.0)).h(px(full.1)).child(inner);
            // Web レイアウト and アウトライン are one sheet, not pages.
            if sheet {
                surface
            } else {
                surface.shadow_lg()
            }
        }
        Some(crop) => surface
            .w(px(crop.width * zoom))
            .h(px(crop.height * zoom))
            .overflow_hidden()
            .border_b_1()
            .border_color(theme::border())
            .child(inner.mt(px(-crop.top * zoom)).ml(px(-crop.left * zoom))),
    }
}

/// The pages' container set out for the view: one page under another,
/// several across (複数ページ), a row turned sideways (横, 閲覧モード), or
/// one long sheet (Web レイアウト, アウトライン).
pub(crate) fn lay_out_pages(
    container: gpui::Stateful<gpui::Div>,
    arrangement: &Arrangement,
    surfaces: Vec<gpui::Stateful<gpui::Div>>,
) -> gpui::Stateful<gpui::Div> {
    let grid = arrangement.grid;
    let container = container
        .overflow_scroll()
        .bg(arrangement.background)
        .flex()
        .gap(px(grid.gap))
        .p(px(grid.padding));
    let container = if grid.horizontal {
        container.flex_row().items_center()
    } else if grid.per_row > 1 {
        container
            .flex_row()
            .flex_wrap()
            .justify_center()
            .content_start()
    } else if arrangement.from_left {
        container.flex_col().items_start()
    } else {
        container.flex_col().items_center()
    };
    container.children(surfaces)
}

/// A 表示 tab button.
fn tab_button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    active: bool,
    action: ViewsAction,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    crate::word_page::text_button(id, label, tooltip, active, Action::Views(action), cx)
}

const VIEW_IDS: [&str; 5] = [
    "views-read",
    "views-print",
    "views-web",
    "views-outline",
    "views-draft",
];

/// The 表示 tab's groups: 表示 (the views), イマーシブ, ページ移動, ルーラー,
/// 表示 (the marks, the grid and the panes), ズーム, ウィンドウ.
pub fn view_tab(
    shown: &ViewsShown,
    before: Vec<AnyElement>,
    show: Vec<AnyElement>,
    after: Vec<AnyElement>,
    cx: &mut Context<DocumentView>,
) -> Vec<AnyElement> {
    let kinds = ViewKind::ALL
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            tab_button(
                VIEW_IDS[index],
                t(kind.label_key()),
                t(kind.label_key()),
                shown.kind == kind,
                ViewsAction::SetView(kind),
                cx,
            )
        })
        .collect();
    let views_group = group(t("views.group.views"), vec![row(kinds)]);
    let focus = group(
        t("views.group.immersive"),
        vec![row(vec![tab_button(
            "views-focus",
            t("views.focus"),
            t("views.focus_title"),
            shown.focus,
            ViewsAction::ToggleFocus,
            cx,
        )])],
    );
    let movement = group(
        t("views.group.movement"),
        vec![row(vec![
            tab_button(
                "views-vertical",
                t("views.vertical"),
                t("views.vertical"),
                shown.movement == PageMovement::Vertical,
                ViewsAction::SetMovement(PageMovement::Vertical),
                cx,
            ),
            tab_button(
                "views-side",
                t("views.side_to_side"),
                t("views.side_to_side"),
                shown.movement == PageMovement::SideToSide,
                ViewsAction::SetMovement(PageMovement::SideToSide),
                cx,
            ),
        ])],
    );
    let mut show_row = show;
    show_row.push(tab_button(
        "views-gridlines",
        t("views.gridlines"),
        t("views.gridlines"),
        shown.gridlines,
        ViewsAction::ToggleGridlines,
        cx,
    ));
    show_row.push(tab_button(
        "views-white-space",
        t("views.white_space"),
        t("views.white_space_title"),
        shown.hide_white_space,
        ViewsAction::ToggleWhiteSpace,
        cx,
    ));
    let show_group = group(t("doc.group.show"), vec![row(show_row)]);
    let fit = shown.fit;
    let zoom = group(
        t("doc.group.zoom"),
        vec![
            row(vec![
                tab_button(
                    "views-zoom",
                    t("views.zoom.button"),
                    t("views.zoom.title"),
                    false,
                    ViewsAction::OpenZoom,
                    cx,
                ),
                tab_button(
                    "views-100",
                    "100%",
                    t("doc.zoom_actual"),
                    fit == ZoomFit::Custom && (shown.zoom - 1.0).abs() < 0.001,
                    ViewsAction::Percent(100),
                    cx,
                ),
            ]),
            row(vec![
                tab_button(
                    "views-one-page",
                    t("views.one_page"),
                    t("views.one_page"),
                    fit == ZoomFit::OnePage,
                    ViewsAction::Fit(ZoomFit::OnePage),
                    cx,
                ),
                tab_button(
                    "views-multiple",
                    t("views.multiple_pages"),
                    t("views.multiple_pages"),
                    matches!(fit, ZoomFit::MultiplePages(_)),
                    ViewsAction::Fit(ZoomFit::MultiplePages(2)),
                    cx,
                ),
                tab_button(
                    "views-page-width",
                    t("views.page_width"),
                    t("views.page_width"),
                    fit == ZoomFit::PageWidth,
                    ViewsAction::Fit(ZoomFit::PageWidth),
                    cx,
                ),
            ]),
        ],
    );
    let window_group = group(
        t("views.group.window"),
        vec![row(vec![
            tab_button(
                "views-new-window",
                t("views.new_window"),
                t("views.new_window"),
                shown.window,
                ViewsAction::NewWindow,
                cx,
            ),
            tab_button(
                "views-split",
                t("views.split"),
                t("views.split"),
                shown.split,
                ViewsAction::Split,
                cx,
            ),
        ])],
    );
    // Word for Mac keeps these in its menus (環境設定, ツール); the browser
    // has no menu bar, so they sit at the tab's end.
    let options = group(
        t("views.group.options"),
        vec![row(vec![
            tab_button(
                "views-options",
                t("options.menu"),
                t("options.title"),
                false,
                ViewsAction::OpenOptions(prefs::OptionsPage::View),
                cx,
            ),
            tab_button(
                "views-autocorrect",
                t("menu.tools.autocorrect"),
                t("autocorrect.title"),
                false,
                ViewsAction::OpenAutoCorrect,
                cx,
            ),
        ])],
    );
    let mut groups = vec![views_group, focus];
    groups.extend(before);
    groups.push(movement);
    groups.push(show_group);
    groups.push(zoom);
    groups.push(window_group);
    groups.extend(after);
    groups.push(options);
    groups
}

/// What the 表示 tab shows as on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewsShown {
    pub kind: ViewKind,
    pub movement: PageMovement,
    pub fit: ZoomFit,
    pub zoom: f32,
    pub gridlines: bool,
    pub hide_white_space: bool,
    pub focus: bool,
    pub split: bool,
    pub window: bool,
    pub mode: modes::EditMode,
    pub mode_menu: bool,
    pub outline_level: u8,
}

impl DocumentView {
    pub(crate) fn views_shown(&self) -> ViewsShown {
        let views = &self.views;
        ViewsShown {
            kind: views.kind,
            movement: views.movement,
            fit: views.fit,
            zoom: self.zoom(),
            gridlines: views.gridlines,
            hide_white_space: views.hide_white_space,
            focus: views.focus,
            split: views.split.is_some(),
            window: views.window.is_some(),
            mode: views.mode,
            mode_menu: views.mode_menu,
            outline_level: views.outline_level,
        }
    }

    /// The status bar's word on the view, while it is not 印刷レイアウト.
    pub(crate) fn views_status(&self) -> Option<SharedString> {
        let mut parts = Vec::new();
        if self.views.kind != ViewKind::Print {
            parts.push(t(self.views.kind.label_key()));
        }
        if self.views.mode != modes::EditMode::Editing {
            parts.push(t(self.views.mode.label_key()));
        }
        if prefs::overtype() {
            parts.push(t("views.status.overtype"));
        }
        (!parts.is_empty()).then(|| parts.join(" · ").into())
    }
}
