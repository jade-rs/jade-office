//! Word's 表示 tab in the native app — the views, ページ移動, グリッド線,
//! ズーム with its fits and dialog, 余白を表示しない — over the arithmetic
//! both shells share (`jade_office_model::word::view`).
//!
//! The pages are the engine's; what a view changes is how the shell lays
//! them out (one under another, several across, side by side), which part
//! of each it shows (下書き and 余白を表示しない keep the band between the
//! margins) and what it draws over them (the grid).

use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton, SharedString, Window};
use jade_office_model::text::TextEdit;
use jade_office_model::word::view::{
    self, fit_zoom, grid_lines, DrawingGrid, GridLines, PageBand, PageCrop, PageMovement, ViewKind,
    ZoomFit,
};

pub mod context;
pub mod customize;
pub mod find;
pub mod input;
pub mod marks;
pub mod mini;
pub mod modes;
pub mod options;
pub mod paste;
pub mod prefs;
pub mod select;
pub mod settings;
pub mod split;

use crate::ribbon::{group, listener, row, Action};
use crate::tooltip::Tooltip;
use crate::word_dialog::{button, chip, on};
use crate::{i18n, theme, workspace, DocumentView, Editor};

gpui::actions!(
    word_views,
    [
        /// ツール → オートコレクト….
        AutoCorrect,
        /// 編集 → 検索 → 高度な検索と置換….
        AdvancedFind,
        /// Word のオプション (環境設定, ⌘,).
        WordOptions,
        /// 上書き入力 (the Insert key).
        ToggleOvertype,
    ]
);

/// The keys of this module's commands, in the document's context.
pub fn key_bindings() -> Vec<gpui::KeyBinding> {
    const CONTEXT: Option<&str> = Some("Document");
    vec![
        gpui::KeyBinding::new("ctrl-alt-cmd-a", AutoCorrect, CONTEXT),
        gpui::KeyBinding::new("cmd-alt-shift-h", AdvancedFind, CONTEXT),
        gpui::KeyBinding::new("cmd-,", WordOptions, CONTEXT),
        gpui::KeyBinding::new("insert", ToggleOvertype, CONTEXT),
    ]
}

/// Word for Mac's ツール menu, as far as this module has it.
pub fn tools_menu() -> gpui::Menu {
    gpui::Menu {
        name: t("menu.tools").into(),
        items: vec![
            gpui::MenuItem::action(t("menu.tools.autocorrect"), AutoCorrect),
            gpui::MenuItem::action(t("options.menu"), WordOptions),
            gpui::MenuItem::action(t("options.overtype"), ToggleOvertype),
        ],
    }
}

/// Routes the module's actions on the document's root.
pub fn on_actions(
    root: gpui::Stateful<gpui::Div>,
    cx: &mut Context<DocumentView>,
) -> gpui::Stateful<gpui::Div> {
    root.on_action(
        cx.listener(|view, _: &AutoCorrect, _, cx| run(view, ViewAction::OpenAutoCorrect, cx)),
    )
    .on_action(cx.listener(|view, _: &AdvancedFind, _, cx| {
        find::open(view, false);
        cx.notify();
    }))
    .on_action(cx.listener(|view, _: &WordOptions, _, cx| {
        prefs::open(view, prefs::OptionsPage::View);
        cx.notify();
    }))
    .on_action(cx.listener(|view, _: &ToggleOvertype, _, cx| {
        if input::insert_key_toggles() {
            input::toggle_overtype(view, cx);
        }
    }))
}

/// What the 表示 tab's controls do.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewAction {
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
    /// ツール → オートコレクト….
    OpenAutoCorrect,
    /// ウィンドウ → 分割.
    Split,
    /// ウィンドウ → 新しいウィンドウを開く.
    NewWindow,
    /// 余白を表示しない / 余白を表示する.
    ToggleWhiteSpace,
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
        /// 倍率, in percent.
        percent: TextEdit,
        /// 複数ページ: how many across.
        pages: usize,
    },
    /// ツール → オートコレクト.
    AutoCorrect(Box<options::AutoCorrectDialog>),
    /// 高度な検索と置換.
    Find(Box<find::FindDialog>),
    /// Word のオプション, at one of its pages.
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
    /// ミニ ツールバー: shown over a selection the mouse made.
    pub mini: bool,
    pub mini_popup: Option<mini::MiniPopup>,
    /// The gallery entry previewed on the selection.
    pub preview: Option<mini::Preview>,
    /// Set while a preview runs its command, so the command does not
    /// settle the preview it is.
    pub previewing: bool,
    /// Ranges selected beside the document's own (⌘-drag, ⌥-drag).
    pub extra_ranges: Vec<(
        jade_office_model::word::Position,
        jade_office_model::word::Position,
    )>,
    /// A drag-and-drop or a block selection under way.
    pub gesture: Option<select::MouseGesture>,
    /// Set while a command runs on each of several ranges.
    pub on_ranges: bool,
    /// The right-click menu, while it is open.
    pub context: Option<context::OpenMenu>,
    /// 編集 / 校閲 / 閲覧, and its list while open.
    pub mode: modes::EditMode,
    pub mode_menu: bool,
    /// フォーカス.
    pub focus: bool,
    /// 分割: the second pane, while the window is split.
    pub split: Option<split::Pane>,
    /// How many 新しいウィンドウ have been opened on the document.
    pub windows: usize,
}

impl Views {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// The field typing goes into while the dialog is open.
    pub fn text_field(&self) -> Option<&TextEdit> {
        match self.dialog.as_ref()? {
            ViewDialog::Zoom { percent, .. } => Some(percent),
            ViewDialog::AutoCorrect(dialog) => dialog.fields.get(dialog.focus),
            ViewDialog::Find(dialog) => dialog.fields.get(dialog.focus),
            ViewDialog::Options(_) => None,
        }
    }

    pub fn text_field_mut(&mut self) -> Option<&mut TextEdit> {
        match self.dialog.as_mut() {
            Some(ViewDialog::Zoom { percent, .. }) => Some(percent),
            Some(ViewDialog::AutoCorrect(dialog)) => dialog.fields.get_mut(dialog.focus),
            Some(ViewDialog::Find(dialog)) => {
                let focus = dialog.focus;
                dialog.current = None;
                dialog.fields.get_mut(focus)
            }
            Some(ViewDialog::Options(_)) => None,
            None => None,
        }
    }

    /// Whether pages show only the band between their margins.
    pub fn crops_pages(&self) -> bool {
        self.kind == ViewKind::Draft || (self.kind == ViewKind::Print && self.hide_white_space)
    }

    /// How many pages one row of the layout holds.
    #[cfg(test)]
    pub fn per_row(&self, page_count: usize) -> usize {
        view::pages_per_row(self.fit, self.movement, page_count)
    }
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// Runs a 表示 action.
pub fn run(view: &mut DocumentView, action: ViewAction, cx: &mut Context<DocumentView>) {
    view.ribbon.popup = None;
    match action {
        ViewAction::SetView(kind) => set_view(view, kind),
        ViewAction::SetMovement(movement) => {
            view.views.movement = movement;
            // 横 turns the pages like a book's: a whole page in the window.
            view.views.fit = match movement {
                PageMovement::SideToSide => ZoomFit::OnePage,
                PageMovement::Vertical => ZoomFit::Custom,
            };
            apply_fit(view);
            view.scroll.scroll_to_item(view.current_page);
        }
        ViewAction::Fit(fit) => {
            view.views.fit = fit;
            if !matches!(fit, ZoomFit::OnePage | ZoomFit::MultiplePages(_)) {
                view.views.movement = PageMovement::Vertical;
            }
            apply_fit(view);
            view.scroll.scroll_to_item(view.current_page);
        }
        ViewAction::Percent(percent) => {
            view.views.fit = ZoomFit::Custom;
            view.views.movement = PageMovement::Vertical;
            view.set_zoom(percent as f32 / 100.0, cx);
        }
        ViewAction::OpenZoom => open_zoom(view),
        ViewAction::Split => return split::toggle_split(view, cx),
        ViewAction::NewWindow => return split::open_new_window(view, cx),
        ViewAction::OpenAutoCorrect => {
            view.views.dialog = Some(ViewDialog::AutoCorrect(Box::new(
                options::AutoCorrectDialog::open(),
            )))
        }
        ViewAction::ToggleGridlines => {
            view.views.gridlines = !view.views.gridlines;
            if view.views.gridlines && view.views.grid.is_none() {
                view.views.grid = Some(view::document_drawing_grid(view.document.main()));
            }
        }
        ViewAction::ToggleWhiteSpace => {
            view.views.hide_white_space = !view.views.hide_white_space;
            view.scroll.scroll_to_item(view.current_page);
        }
    }
    cx.notify();
}

/// Changes the view. 下書き and 印刷レイアウト show the same pages; the
/// others are drawn by their own modules.
fn set_view(view: &mut DocumentView, kind: ViewKind) {
    view.views.kind = kind;
    if kind == ViewKind::Draft {
        // 下書き scrolls one page under another, at the zoom it had.
        view.views.movement = PageMovement::Vertical;
        if matches!(view.views.fit, ZoomFit::OnePage | ZoomFit::MultiplePages(_)) {
            view.views.fit = ZoomFit::Custom;
        }
    }
    view.scroll.scroll_to_item(view.current_page);
}

/// The zoom a fit asks for in the space the view has now; called each
/// frame, so a fit follows the window as Word's does.
pub fn apply_fit(view: &mut DocumentView) {
    let fit = view.views.fit;
    if fit == ZoomFit::Custom {
        return;
    }
    let Some(area) = area(view) else {
        return;
    };
    let (page_width, page_height) = view.document.layout_page_size();
    let band = page_band(view, view.current_page);
    let text = band
        .map(|band| (band.left, band.right - band.left))
        .unwrap_or((0.0, page_width as f32));
    if let Some(zoom) = fit_zoom(fit, (page_width as f32, page_height as f32), text, area) {
        if (zoom - view.zoom).abs() > 0.001 {
            view.zoom = zoom;
        }
    }
}

/// The room the pages have: the editor area less the page list and the
/// panes beside the pages, and less the ribbon above them.
fn area(view: &DocumentView) -> Option<(f32, f32)> {
    const PAGE_LIST_WIDTH: f32 = 168.0;
    const RIBBON_HEIGHT: f32 = 132.0;
    // Read through a thread-local copy: a view has no `App` to hand when
    // the ribbon's button runs, only its own context, and the area is the
    // workspace's global.
    let (width, height) = AREA.with(|area| area.get())?;
    let beside = PAGE_LIST_WIDTH
        + if view.show_inspector { 260.0 } else { 0.0 }
        + if view.show_comments { 300.0 } else { 0.0 }
        + if view.references.navigation {
            260.0
        } else {
            0.0
        };
    Some((
        (width - beside).max(100.0),
        (height - RIBBON_HEIGHT).max(100.0),
    ))
}

thread_local! {
    /// The editor area of the last frame, for fits computed outside one.
    static AREA: std::cell::Cell<Option<(f32, f32)>> = const { std::cell::Cell::new(None) };
}

/// Called as the view renders: the area is known, and a fit follows it.
pub fn before_render(view: &mut DocumentView, window: &Window, cx: &mut Context<DocumentView>) {
    let area = workspace::editor_area(window, cx);
    AREA.with(|cell| cell.set(Some(area)));
    apply_fit(view);
    // The marks Word のオプション shows by kind, in every document.
    marks::apply(view);
}

/// What the status bar adds: 上書き while typing replaces.
pub fn status_items() -> Vec<gpui::SharedString> {
    if input::overtype() {
        vec![t("views.status.overtype").into()]
    } else {
        Vec::new()
    }
}

/// The band between the margins of page `index`, from its section.
pub fn page_band(view: &DocumentView, index: usize) -> Option<PageBand> {
    let main = view.document.main();
    if main.page_count() == 0 {
        return None;
    }
    let section = main.renderer().section_index_for_zero_based_page(index);
    let layout = main.word_document().section_layout(Some(section));
    if layout.width <= 0.0 || layout.height <= 0.0 {
        return None;
    }
    Some(PageBand::of(&layout))
}

/// The part of page `index` the view shows, when it shows only part.
pub fn page_crop(view: &DocumentView, index: usize) -> Option<PageCrop> {
    if !view.views.crops_pages() {
        return None;
    }
    let (width, height) = view.document.layout_page_size();
    Some(page_band(view, index)?.crop(view.views.kind, (width as f32, height as f32)))
}

/// Lays the pages container out for the view: one page under another,
/// several across (複数ページ), or a row turned sideways (横).
pub fn arrange_pages(pages: gpui::Stateful<gpui::Div>, views: &Views) -> gpui::Stateful<gpui::Div> {
    let pages = pages.flex().size_full().overflow_scroll();
    let pages = match (views.movement, views.fit) {
        (PageMovement::SideToSide, _) => pages.flex_row().items_center().gap_6().p_8(),
        (_, ZoomFit::MultiplePages(_)) => pages
            .flex_row()
            .flex_wrap()
            .justify_center()
            .content_start()
            .gap_6()
            .p_8(),
        _ => pages.flex_col().items_center(),
    };
    if views.focus {
        // フォーカス: the page alone on a dark ground.
        return pages.gap_6().p_8().bg(gpui::rgb(0x1b1b1f)).items_center();
    }
    match views.kind {
        // 下書き: the bands one after another on white, from the left.
        ViewKind::Draft => pages.items_start().gap_0().p_0().bg(theme::paper()),
        // 余白を表示しない: the pages close up, a rule between them.
        _ if views.crops_pages() => pages.gap(px(3.0)).py_8().bg(theme::canvas()),
        _ if views.movement == PageMovement::Vertical
            && !matches!(views.fit, ZoomFit::MultiplePages(_)) =>
        {
            pages.gap_6().p_8().bg(theme::canvas())
        }
        _ => pages.bg(theme::canvas()),
    }
}

/// The page surface sized to what the view shows of it.
pub fn frame_page(
    surface: gpui::Stateful<gpui::Div>,
    views: &Views,
    crop: Option<PageCrop>,
    zoom: f32,
) -> gpui::Stateful<gpui::Div> {
    let Some(crop) = crop else {
        return surface.shadow_lg();
    };
    let surface = surface
        .w(px(crop.width * zoom))
        .h(px(crop.height * zoom))
        .overflow_hidden();
    match views.kind {
        // Word's dotted page boundary.
        ViewKind::Draft => surface
            .border_b_1()
            .border_dashed()
            .border_color(theme::border()),
        _ => surface.shadow_lg(),
    }
}

/// The page's contents, moved so the cropped band is what shows.
pub fn page_contents(crop: Option<PageCrop>, zoom: f32) -> gpui::Div {
    let inner = div().relative();
    match crop {
        Some(crop) => inner.mt(px(-crop.top * zoom)).ml(px(-crop.left * zoom)),
        None => inner,
    }
}

/// グリッド線 over page `index`.
pub fn grid_overlay(view: &DocumentView, index: usize) -> Option<AnyElement> {
    if !view.views.gridlines || view.views.kind != ViewKind::Print {
        return None;
    }
    let grid = view.views.grid.unwrap_or_default();
    let main = view.document.main();
    let section = main.renderer().section_index_for_zero_based_page(index);
    let layout = main.word_document().section_layout(Some(section));
    let lines: GridLines = grid_lines(&layout, &grid);
    let zoom = view.zoom;
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

/// The page the viewport is mostly on, from where the pages were laid out;
/// `None` before the first layout.
pub fn page_in_view(scroll: &gpui::ScrollHandle, page_count: usize) -> Option<usize> {
    let viewport = scroll.bounds();
    if page_count == 0 || viewport.size.width <= px(0.0) {
        return None;
    }
    let offset = scroll.offset();
    // A point a third of the way into the viewport, in the content's
    // unscrolled coordinates, as the children's bounds are.
    let probe = gpui::point(
        viewport.origin.x - offset.x + viewport.size.width / 3.0,
        viewport.origin.y - offset.y + viewport.size.height / 3.0,
    );
    let mut best: Option<(usize, f32)> = None;
    for index in 0..page_count {
        let bounds = scroll.bounds_for_item(index)?;
        if bounds.contains(&probe) {
            return Some(index);
        }
        let dx = f32::from((bounds.center().x - probe.x).abs());
        let dy = f32::from((bounds.center().y - probe.y).abs());
        let distance = dx + dy;
        if best.is_none_or(|(_, nearest)| distance < nearest) {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
}

/// A double-click on the canvas between pages: 余白を表示しない, or
/// showing it again, as Word does. `true` when it was one.
pub fn white_space_click(
    view: &mut DocumentView,
    position: gpui::Point<gpui::Pixels>,
    click_count: usize,
    cx: &mut Context<DocumentView>,
) -> bool {
    if click_count < 2 || view.views.kind != ViewKind::Print {
        return false;
    }
    let on_a_page = view
        .page_bounds
        .borrow()
        .iter()
        .flatten()
        .any(|bounds| bounds.contains(&position));
    // With the white space hidden the pages touch: their shared edge is
    // what a double-click finds.
    let near_an_edge = view.views.hide_white_space && {
        let zoom = view.zoom;
        (0..view.images.len()).any(|index| {
            let Some(Some(bounds)) = view.page_bounds.borrow().get(index).copied() else {
                return false;
            };
            let Some(crop) = page_crop(view, index) else {
                return false;
            };
            let top = bounds.origin.y + px(crop.top * zoom);
            let bottom = top + px(crop.height * zoom);
            (position.y - top).abs() < px(4.0) || (position.y - bottom).abs() < px(4.0)
        })
    };
    if on_a_page && !near_an_edge {
        return false;
    }
    run(view, ViewAction::ToggleWhiteSpace, cx);
    true
}

// ---- the ズーム dialog ---------------------------------------------------------

fn open_zoom(view: &mut DocumentView) {
    let percent = (view.zoom * 100.0).round() as u32;
    let choice = match view.views.fit {
        ZoomFit::Custom if view::ZOOM_PERCENTS.contains(&percent) => ZoomChoice::Percent(percent),
        ZoomFit::Custom => ZoomChoice::Typed,
        fit => ZoomChoice::Fit(fit),
    };
    let pages = match view.views.fit {
        ZoomFit::MultiplePages(count) => count,
        _ => 2,
    };
    view.views.dialog = Some(ViewDialog::Zoom {
        choice,
        percent: TextEdit::new(percent.to_string()),
        pages,
    });
}

/// OK: the zoom the dialog chose.
fn confirm_zoom(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(ViewDialog::Zoom {
        choice,
        percent,
        pages,
    }) = view.views.dialog.take()
    else {
        return;
    };
    let action = match choice {
        ZoomChoice::Percent(value) => ViewAction::Percent(value),
        ZoomChoice::Fit(ZoomFit::MultiplePages(_)) => {
            ViewAction::Fit(ZoomFit::MultiplePages(pages))
        }
        ZoomChoice::Fit(fit) => ViewAction::Fit(fit),
        ZoomChoice::Typed => match percent.text.trim().trim_end_matches('%').parse::<u32>() {
            Ok(value) if (10..=500).contains(&value) => ViewAction::Percent(value),
            _ => {
                view.views.dialog = Some(ViewDialog::Zoom {
                    choice,
                    percent,
                    pages,
                });
                return cx.notify();
            }
        },
    };
    run(view, action, cx);
}

/// A key while the dialog is open; `true` when it was taken.
pub fn handle_key(view: &mut DocumentView, key: &str, cx: &mut Context<DocumentView>) -> bool {
    if view.views.dialog.is_none() {
        return false;
    }
    if view
        .views
        .text_field()
        .is_some_and(|field| field.marked.is_some())
    {
        return false;
    }
    if matches!(view.views.dialog, Some(ViewDialog::AutoCorrect(_))) {
        return options::handle_key(view, key, cx);
    }
    if matches!(view.views.dialog, Some(ViewDialog::Find(_))) {
        return find::handle_key(view, key, cx);
    }
    if matches!(view.views.dialog, Some(ViewDialog::Options(_))) {
        if matches!(key, "escape" | "enter" | "return") {
            view.views.dialog = None;
            cx.notify();
        }
        return true;
    }
    match key {
        "escape" => view.views.dialog = None,
        "enter" | "return" => {
            confirm_zoom(view, cx);
            return true;
        }
        key => {
            let Some(field) = view.views.text_field_mut() else {
                return false;
            };
            if !field.edit_key(key) {
                return false;
            }
            if let Some(ViewDialog::Zoom { choice, .. }) = view.views.dialog.as_mut() {
                *choice = ZoomChoice::Typed;
            }
        }
    }
    cx.notify();
    true
}

/// The open dialog, over the window.
pub fn overlay(view: &DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let (choice, percent, pages) = match view.views.dialog.as_ref()? {
        ViewDialog::Zoom {
            choice,
            percent,
            pages,
        } => (choice, percent, pages),
        ViewDialog::AutoCorrect(dialog) => return Some(options::overlay(dialog, cx)),
        ViewDialog::Find(dialog) => return Some(find::overlay(view, dialog, cx)),
        ViewDialog::Options(page) => return Some(prefs::overlay(view, *page, cx)),
    };
    let (choice, pages) = (*choice, *pages);
    fn set(
        cx: &Context<DocumentView>,
        chosen: ZoomChoice,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static {
        on(cx, move |view: &mut DocumentView, cx| {
            if let Some(ViewDialog::Zoom { choice, .. }) = view.views.dialog.as_mut() {
                *choice = chosen;
            }
            cx.notify();
        })
    }
    let percents = view::ZOOM_PERCENTS.iter().map(|value| {
        chip(
            SharedString::from(format!("zoom-percent-{value}")),
            format!("{value}%"),
            choice == ZoomChoice::Percent(*value),
        )
        .on_click(set(cx, ZoomChoice::Percent(*value)))
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
            .on_click(set(cx, ZoomChoice::Fit(fit)))
            .into_any_element()
    });
    let page_counts = (1..=6).map(|count| {
        chip(("zoom-pages", count), count.to_string(), pages == count)
            .on_click(on(cx, move |view: &mut DocumentView, cx| {
                if let Some(ViewDialog::Zoom { choice, pages, .. }) = view.views.dialog.as_mut() {
                    *pages = count;
                    *choice = ZoomChoice::Fit(ZoomFit::MultiplePages(count));
                }
                cx.notify();
            }))
            .into_any_element()
    });
    let (before, after) = percent.split();
    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(percents)
                .children(fits),
        )
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
                .children(page_counts),
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
                    div()
                        .id("zoom-percent-field")
                        .flex()
                        .items_center()
                        .w(px(80.0))
                        .h(px(24.0))
                        .px_2()
                        .rounded_sm()
                        .border_1()
                        .border_color(if choice == ZoomChoice::Typed {
                            theme::accent()
                        } else {
                            theme::border()
                        })
                        .bg(theme::canvas())
                        .on_click(set(cx, ZoomChoice::Typed))
                        .child(before.to_string())
                        .child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
                        .child(after.to_string()),
                )
                .child("%"),
        );
    Some(dialog_frame_with(
        "views-zoom",
        t("views.zoom.title"),
        360.0,
        body.into_any_element(),
        cx,
        confirm_zoom,
    ))
}

/// A dialog's frame: title, body, キャンセル and OK (which runs `ok`), over
/// a dimmed window.
pub fn dialog_frame_with(
    id: &'static str,
    title: &'static str,
    width: f32,
    body: AnyElement,
    cx: &Context<DocumentView>,
    ok: fn(&mut DocumentView, &mut Context<DocumentView>),
) -> AnyElement {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::rgba(0x00000055))
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .w(px(width))
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .bg(theme::panel())
                .shadow_lg()
                .text_xs()
                .text_color(theme::text())
                .child(div().text_sm().child(title))
                .child(body)
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            button("views-cancel", t("format.cancel"), false).on_click(on(
                                cx,
                                |view: &mut DocumentView, cx| {
                                    view.views.dialog = None;
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            button("views-ok", t("format.ok"), true)
                                .on_click(on(cx, move |view: &mut DocumentView, cx| ok(view, cx))),
                        ),
                ),
        )
        .into_any_element()
}

// ---- the 表示 tab ----------------------------------------------------------------

fn tab_button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    active: bool,
    action: ViewAction,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(active, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(Action::Views(action), cx))
        .child(label)
        .into_any_element()
}

/// The 表示 tab's groups: 表示, ページ移動, 表示 (the marks, rulers, grid
/// and panes), ズーム and the pages.
pub fn view_tab(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    let views = &view.views;
    let kinds = ViewKind::ALL.into_iter().enumerate().map(|(index, kind)| {
        tab_button(
            VIEW_IDS[index],
            t(kind.label_key()),
            t(kind.label_key()),
            views.kind == kind,
            ViewAction::SetView(kind),
            cx,
        )
    });
    let views_group = group(t("views.group.views"), vec![row(kinds.collect())]);
    let movement = group(
        t("views.group.movement"),
        vec![row(vec![
            tab_button(
                "views-vertical",
                t("views.vertical"),
                t("views.vertical"),
                views.movement == PageMovement::Vertical,
                ViewAction::SetMovement(PageMovement::Vertical),
                cx,
            ),
            tab_button(
                "views-side",
                t("views.side_to_side"),
                t("views.side_to_side"),
                views.movement == PageMovement::SideToSide,
                ViewAction::SetMovement(PageMovement::SideToSide),
                cx,
            ),
        ])],
    );
    let immersive = group(
        t("views.group.immersive"),
        vec![row(vec![modes::focus_button(view, cx)])],
    );
    let views = &view.views;
    let grid = tab_button(
        "views-gridlines",
        t("views.gridlines"),
        t("views.gridlines"),
        views.gridlines,
        ViewAction::ToggleGridlines,
        cx,
    );
    let white_space = tab_button(
        "views-white-space",
        t("views.white_space"),
        t("views.white_space_title"),
        views.hide_white_space,
        ViewAction::ToggleWhiteSpace,
        cx,
    );
    let fit = views.fit;
    let zoom = group(
        t("doc.group.zoom"),
        vec![
            row(vec![
                tab_button(
                    "views-zoom",
                    t("views.zoom.button"),
                    t("views.zoom.title"),
                    false,
                    ViewAction::OpenZoom,
                    cx,
                ),
                tab_button(
                    "views-100",
                    "100%",
                    t("doc.zoom_actual"),
                    fit == ZoomFit::Custom && (view.zoom - 1.0).abs() < 0.001,
                    ViewAction::Percent(100),
                    cx,
                ),
            ]),
            row(vec![
                tab_button(
                    "views-one-page",
                    t("views.one_page"),
                    t("views.one_page"),
                    fit == ZoomFit::OnePage,
                    ViewAction::Fit(ZoomFit::OnePage),
                    cx,
                ),
                tab_button(
                    "views-multiple",
                    t("views.multiple_pages"),
                    t("views.multiple_pages"),
                    matches!(fit, ZoomFit::MultiplePages(_)),
                    ViewAction::Fit(ZoomFit::MultiplePages(2)),
                    cx,
                ),
                tab_button(
                    "views-page-width",
                    t("views.page_width"),
                    t("views.page_width"),
                    fit == ZoomFit::PageWidth,
                    ViewAction::Fit(ZoomFit::PageWidth),
                    cx,
                ),
            ]),
        ],
    );
    let marks = view.document.formatting_marks();
    let show = group(
        t("doc.group.show"),
        vec![
            row(vec![
                view.ribbon_button(
                    "marks",
                    t("doc.marks"),
                    t("doc.marks_title"),
                    marks,
                    cx,
                    |view, cx| view.toggle_formatting_marks(cx),
                )
                .into_any_element(),
                view.field_codes_button(cx),
                grid,
                white_space,
            ]),
            row(crate::word_references::view_buttons(view, cx)),
        ],
    );
    let pages = group(
        t("doc.group.pages"),
        vec![row(vec![
            view.ribbon_button(
                "prev-page",
                t("doc.previous"),
                t("doc.previous_page"),
                false,
                cx,
                |view, cx| view.go_to_page(-1, cx),
            )
            .into_any_element(),
            view.ribbon_button(
                "next-page",
                t("doc.next"),
                t("doc.next_page"),
                false,
                cx,
                |view, cx| view.go_to_page(1, cx),
            )
            .into_any_element(),
        ])],
    );
    let window_group = group(
        t("views.group.window"),
        vec![row(vec![
            tab_button(
                "views-new-window",
                t("views.new_window"),
                t("views.new_window"),
                false,
                ViewAction::NewWindow,
                cx,
            ),
            tab_button(
                "views-split",
                t("views.split"),
                t("views.split"),
                view.views.split.is_some(),
                ViewAction::Split,
                cx,
            ),
        ])],
    );
    vec![
        views_group,
        immersive,
        movement,
        view.ruler_group(cx),
        show,
        zoom,
        window_group,
        pages,
    ]
}

const VIEW_IDS: [&str; 5] = [
    "views-read",
    "views-print",
    "views-web",
    "views-outline",
    "views-draft",
];

#[cfg(test)]
pub(crate) mod tests {
    use super::{run, ViewAction};
    use jade_office_model::word::view::{PageMovement, ViewKind, ZoomFit};

    pub(crate) fn open_contract(
        cx: &mut gpui::TestAppContext,
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &mut gpui::VisualTestContext,
    ) {
        use gpui::Focusable;
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    /// 1 ページ, 複数ページ and ページ幅 set the zoom from the window and
    /// keep it there; 100% and the ズーム dialog's percentages set it
    /// outright.
    #[gpui::test]
    fn the_zoom_fits_follow_the_window(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            super::AREA.with(|area| area.set(Some((1400.0, 900.0))));
            run(view, ViewAction::Fit(ZoomFit::OnePage), cx);
            let (width, height) = view.document.layout_page_size();
            let one = view.zoom;
            assert!(one * height as f32 <= 900.0, "{one}");
            run(view, ViewAction::Fit(ZoomFit::MultiplePages(2)), cx);
            assert_eq!(view.views.per_row(view.images.len()), 2);
            assert!(view.zoom * width as f32 * 2.0 <= 1400.0 - 168.0);
            run(view, ViewAction::Fit(ZoomFit::PageWidth), cx);
            assert!(view.zoom > one, "the page across is bigger than it whole");
            // The window grows: the fit follows.
            let before = view.zoom;
            super::AREA.with(|area| area.set(Some((1800.0, 900.0))));
            super::apply_fit(view);
            assert!(view.zoom > before);
            run(view, ViewAction::Percent(100), cx);
            assert_eq!(view.zoom, 1.0);
            assert_eq!(view.views.fit, ZoomFit::Custom);
        });
    }

    /// ズーム…: a percentage typed into 倍率 goes on with OK; a fit chosen
    /// there stays a fit.
    #[gpui::test]
    fn the_zoom_dialog_sets_a_typed_percentage(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| run(view, ViewAction::OpenZoom, cx));
        cx.simulate_keystrokes("backspace backspace backspace");
        cx.simulate_input("150");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.views.dialog.is_none());
            assert!((view.zoom - 1.5).abs() < 1e-4, "{}", view.zoom);
        });
    }

    /// 横 lays the pages side by side a page at a time; 下書き and
    /// 余白を表示しない show each page's band between its margins.
    #[gpui::test]
    fn movement_and_bands(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            super::AREA.with(|area| area.set(Some((1400.0, 900.0))));
            run(view, ViewAction::SetMovement(PageMovement::SideToSide), cx);
            assert_eq!(view.views.fit, ZoomFit::OnePage);
            assert_eq!(view.views.per_row(view.images.len()), view.images.len());
            run(view, ViewAction::SetMovement(PageMovement::Vertical), cx);

            assert!(super::page_crop(view, 0).is_none());
            run(view, ViewAction::ToggleWhiteSpace, cx);
            let crop = super::page_crop(view, 0).expect("the band");
            let (width, height) = view.document.layout_page_size();
            assert_eq!(crop.width, width as f32);
            assert!(crop.top > 0.0 && crop.height < height as f32);
            run(view, ViewAction::ToggleWhiteSpace, cx);

            run(view, ViewAction::SetView(ViewKind::Draft), cx);
            let draft = super::page_crop(view, 0).expect("the draft band");
            assert!(draft.left > 0.0 && draft.width < width as f32);
        });
    }

    /// グリッド線 reads the drawing grid and draws lines over the text area.
    #[gpui::test]
    fn gridlines_draw_over_the_page(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            assert!(super::grid_overlay(view, 0).is_none());
            run(view, ViewAction::ToggleGridlines, cx);
            assert!(view.views.grid.is_some());
            assert!(super::grid_overlay(view, 0).is_some());
        });
    }
}
