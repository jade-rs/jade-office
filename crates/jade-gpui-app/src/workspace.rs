//! The workspace window: every open file, one window, laid out like jade-ide.
//!
//! jlsi/jade's browser IDE (`packages/jade-ide`) frames its editors the way a
//! code editor does, and this is that frame in gpui:
//!
//! ```text
//! ┌──────────────────────── title bar: logo · active file · theme ────────┐
//! │act│ explorer │ tabs: contract.docx │ book.xlsx │ deck.pptx            │
//! │ivi│  folder  ├───────────────────────────────────────────────────────┤
//! │ty │   tree   │ the active view — document, workbook or deck          │
//! │bar│          │                                                       │
//! ├───┴──────────┴───────────────────────────────────────────────────────┤
//! │ ● Ready  contract.docx                              Word · 3 tabs     │
//! └───────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Each tab holds one of the three views as it is; the workspace only owns
//! the frame around them, which file is in front, and where the keyboard is.

use gpui::{
    div, linear_color_stop, linear_gradient, prelude::*, px, svg, AnyView, App, ClickEvent,
    Context, Entity, ExternalPaths, FocusHandle, Focusable, Global, PathPromptOptions, Rgba,
    SharedString, Window,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::editor::{self, Editor, EditorHandle};
use crate::shell::{self, recent, Kind};
use crate::tooltip::Tooltip;
use crate::{
    deck, i18n, theme, workbook, CloseWindow, OpenFolder, ToggleExplorer, ToggleInspector,
    ToggleTheme, ZoomActual, ZoomFit, ZoomIn, ZoomOut, UI_FONT_FAMILY,
};

pub const TITLE_BAR_HEIGHT: f32 = 40.0;
const ACTIVITY_BAR_WIDTH: f32 = 44.0;
const EXPLORER_WIDTH: f32 = 224.0;
const TAB_BAR_HEIGHT: f32 = 36.0;
const STATUS_BAR_HEIGHT: f32 = 24.0;
/// Room for the traffic lights, which the platform draws over the title bar.
const TRAFFIC_LIGHT_INSET: f32 = 86.0;

/// The size of the space the active view is given, set by the workspace each
/// frame before its views render.
///
/// The workbook and the deck paint to fit the space they have; reading it
/// from here rather than measuring the previous frame means a resize paints
/// at the right size on the frame it happens.
#[derive(Clone, Copy)]
pub struct EditorArea(pub f32, pub f32);

impl Global for EditorArea {}

/// The active view's space, or the whole window outside a workspace.
pub fn editor_area(window: &Window, cx: &App) -> (f32, f32) {
    match cx.try_global::<EditorArea>() {
        Some(area) => (area.0, area.1),
        None => {
            let size = window.viewport_size();
            (f32::from(size.width), f32::from(size.height))
        }
    }
}

/// A colour per format, so a tab or a tree row says what it is at a glance:
/// the 400 shades on the dark chrome, a step deeper on the light one, where
/// the 400s wash out against white.
fn kind_color(kind: Kind) -> Rgba {
    let (dark, light) = match kind {
        Kind::Document => (0x60a5fa, 0x3b82f6),
        Kind::Workbook => (0x34d399, 0x059669),
        Kind::Deck => (0xfb923c, 0xea580c),
    };
    gpui::rgb(if theme::is_dark() { dark } else { light })
}

fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Document => "icons/file-text.svg",
        Kind::Workbook => "icons/sheet.svg",
        Kind::Deck => "icons/presentation.svg",
    }
}

struct Tab {
    path: PathBuf,
    kind: Kind,
    editor: Box<dyn EditorHandle>,
    view: AnyView,
    focus: FocusHandle,
}

/// One directory entry the explorer shows: a folder, or a file it can open.
#[derive(Clone)]
struct Entry {
    path: PathBuf,
    name: SharedString,
    kind: Option<Kind>,
}

/// The folders and office files directly inside `directory`, folders first,
/// each group by name. Hidden entries are left out.
fn list_directory(directory: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut entries: Vec<(bool, Entry)> = read
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.starts_with("~$") {
                return None;
            }
            let is_dir = entry.file_type().ok()?.is_dir();
            let kind = Kind::of(&path);
            if !is_dir && kind.is_none() {
                return None;
            }
            Some((
                is_dir,
                Entry {
                    path,
                    name: SharedString::from(name),
                    kind,
                },
            ))
        })
        .collect();
    entries.sort_by(|(a_dir, a), (b_dir, b)| {
        b_dir
            .cmp(a_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries.into_iter().map(|(_, entry)| entry).collect()
}

pub struct Workspace {
    tabs: Vec<Tab>,
    active: Option<usize>,
    explorer_open: bool,
    /// The folder the explorer shows: the one picked with Open Folder, or the
    /// folder of the first file opened.
    root: Option<PathBuf>,
    expanded: HashSet<PathBuf>,
    /// Directory listings, read once per folder: the workspace renders on
    /// every frame anything in it changes, scrolling included, and the disk
    /// should not be read that often.
    listings: HashMap<PathBuf, Vec<Entry>>,
    recent: Vec<PathBuf>,
    error: Option<SharedString>,
    focus: FocusHandle,
}

impl Workspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            explorer_open: true,
            root: None,
            expanded: HashSet::new(),
            listings: HashMap::new(),
            recent: recent::load(),
            error: None,
            focus: cx.focus_handle(),
        }
    }

    /// Opens `path` in a new tab, or brings its tab forward when it is open.
    pub fn open(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Some(index) = self.tabs.iter().position(|tab| tab.path == path) {
            self.activate(index, window, cx);
            return Ok(());
        }
        let result = self.open_new(&path, cx);
        if let Err(error) = &result {
            self.error = Some(SharedString::from(error.clone()));
            cx.notify();
            return result;
        }
        recent::remember(&path);
        self.recent = recent::load();
        if self.root.is_none() {
            if let Some(parent) = path.parent() {
                self.set_root(parent.to_path_buf());
            }
        }
        self.error = None;
        self.activate(self.tabs.len() - 1, window, cx);
        Ok(())
    }

    fn open_new(&mut self, path: &Path, cx: &mut Context<Self>) -> Result<(), String> {
        let kind = Kind::of(path).ok_or_else(|| {
            i18n::format(
                "workspace.unsupported",
                &[("name", &shell::file_name(path))],
            )
        })?;
        if shell::is_compound_file(path) {
            return Err(i18n::format(
                "workspace.protected",
                &[("name", &shell::file_name(path))],
            ));
        }
        let editor: Box<dyn EditorHandle> = match kind {
            Kind::Document => Box::new(self.observed(crate::new_document_view(path, cx)?, cx)),
            Kind::Workbook => Box::new(self.observed(workbook::new_view(path, cx)?, cx)),
            Kind::Deck => Box::new(self.observed(deck::new_view(path, cx)?, cx)),
        };
        self.tabs.push(Tab {
            path: path.to_path_buf(),
            kind,
            view: editor.view(),
            focus: editor.focus_handle(cx),
            editor,
        });
        Ok(())
    }

    /// Repaints the workspace whenever `entity` changes, so the status bar
    /// follows the page, the slide and the zoom.
    fn observed<T: Editor>(&mut self, entity: Entity<T>, cx: &mut Context<Self>) -> Entity<T> {
        cx.observe(&entity, |_workspace, _entity, cx| cx.notify())
            .detach();
        entity
    }

    fn zoom_active(&mut self, zoom: impl FnOnce(f32) -> Option<f32>, cx: &mut Context<Self>) {
        let Some(tab) = self.active.and_then(|index| self.tabs.get(index)) else {
            return;
        };
        match zoom(tab.editor.zoom(cx)) {
            Some(zoom) => tab.editor.set_zoom(zoom, cx),
            None => tab.editor.zoom_to_fit(cx),
        }
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.listings.clear();
        self.expanded.clear();
        self.expanded.insert(root.clone());
        self.root = Some(root);
        self.explorer_open = true;
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        self.active = Some(index);
        window.focus(&tab.focus);
        window.set_window_title(&format!("{} — jade", shell::file_name(&tab.path)));
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.active = None;
            window.focus(&self.focus);
            window.set_window_title("jade");
            cx.notify();
            return;
        }
        let active = self.active.unwrap_or(0);
        let next = if active > index || active >= self.tabs.len() {
            active.saturating_sub(1)
        } else {
            active
        };
        self.activate(next.min(self.tabs.len() - 1), window, cx);
    }

    /// ⌘W: the active tab, and the window once there are none.
    fn close_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.active {
            Some(index) => self.close_tab(index, window, cx),
            None => window.remove_window(),
        }
    }

    fn open_from_ui(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.open(&path, window, cx) {
            eprintln!("{error}");
        }
    }

    fn prompt_for_folder(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from(i18n::text("workspace.prompt_folder"))),
        });
        cx.spawn(async move |workspace, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            workspace
                .update(cx, |workspace, cx| {
                    workspace.set_root(folder);
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn toggle_directory(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !self.expanded.remove(&path) {
            // Re-read on expand, so a folder closed and opened again shows
            // what is in it now.
            self.listings.remove(&path);
            self.expanded.insert(path);
        }
        cx.notify();
    }

    fn listing(&mut self, directory: &Path) -> Vec<Entry> {
        self.listings
            .entry(directory.to_path_buf())
            .or_insert_with(|| list_directory(directory))
            .clone()
    }

    /// The explorer's rows, depth first through the expanded folders.
    fn tree_rows(&mut self, directory: &Path, depth: usize, rows: &mut Vec<(Entry, usize)>) {
        // Deep enough for any real project; a symlink loop stops here.
        if depth > 12 {
            return;
        }
        for entry in self.listing(directory) {
            let expanded = entry.kind.is_none() && self.expanded.contains(&entry.path);
            let path = entry.path.clone();
            rows.push((entry, depth));
            if expanded {
                self.tree_rows(&path, depth + 1, rows);
            }
        }
    }

    fn active_tab(&self) -> Option<&Tab> {
        self.active.and_then(|index| self.tabs.get(index))
    }

    // ---- the frame ------------------------------------------------------

    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title: SharedString = match self.active_tab() {
            Some(tab) => SharedString::from(shell::file_name(&tab.path)),
            None => SharedString::from("jade office"),
        };
        let folder = self
            .active_tab()
            .and_then(|tab| tab.path.parent())
            .or(self.root.as_deref())
            .and_then(|folder| folder.file_name())
            .map(|name| SharedString::from(name.to_string_lossy().into_owned()));
        let dark = theme::is_dark();
        div()
            .id("title-bar")
            .h(px(TITLE_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .pl(px(TRAFFIC_LIGHT_INSET))
            .pr_3()
            .gap_3()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .on_click(|event: &ClickEvent, window, _cx| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
            // The logo: the IDE's gradient tile with its bolt.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size_6()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_lg()
                            .bg(linear_gradient(
                                135.0,
                                linear_color_stop(gpui::rgb(0x6366f1), 0.0),
                                linear_color_stop(gpui::rgb(0xec4899), 1.0),
                            ))
                            .child(
                                svg()
                                    .path("icons/zap.svg")
                                    .size_3()
                                    .text_color(gpui::white()),
                            ),
                    )
                    .child(div().text_xs().text_color(theme::text()).child("Jade")),
            )
            // Where the IDE has its search box: what is open, and where.
            .child(
                div().flex_1().flex().justify_center().child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .w(px(420.0))
                        .px_3()
                        .py(px(5.0))
                        .rounded_lg()
                        .bg(theme::hovered())
                        .text_xs()
                        .text_color(theme::text_dim())
                        .children(self.active_tab().map(|tab| {
                            svg()
                                .path(kind_icon(tab.kind))
                                .size_3p5()
                                .text_color(kind_color(tab.kind))
                        }))
                        .child(div().text_color(theme::text()).child(title))
                        .children(
                            folder
                                .map(|folder| div().text_color(theme::text_faint()).child(folder)),
                        ),
                ),
            )
            .child(
                icon_button(
                    "theme-toggle",
                    if dark {
                        "icons/sun.svg"
                    } else {
                        "icons/moon.svg"
                    },
                    false,
                    i18n::text("menu.view.theme"),
                )
                .on_click(cx.listener(|_workspace, _event, window, cx| {
                    window.dispatch_action(Box::new(ToggleTheme), cx);
                })),
            )
    }

    fn activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(ACTIVITY_BAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .py_2()
            .gap_1()
            .bg(theme::sidebar())
            .border_r_1()
            .border_color(theme::border())
            .child(
                activity_button(
                    "activity-explorer",
                    "icons/files.svg",
                    self.explorer_open,
                    i18n::text("workspace.explorer"),
                )
                .on_click(cx.listener(|workspace, _event, _window, cx| {
                    workspace.explorer_open = !workspace.explorer_open;
                    cx.notify();
                })),
            )
            .child(div().w_5().h(px(1.0)).my_1().bg(theme::border()))
            .child(
                activity_button(
                    "activity-inspector",
                    "icons/panel-right.svg",
                    false,
                    i18n::text("menu.view.details"),
                )
                .on_click(cx.listener(|workspace, _event, window, cx| {
                    // Sent to the active view, which owns its own panel.
                    if let Some(tab) = workspace.active_tab() {
                        window.focus(&tab.focus);
                    }
                    window.dispatch_action(Box::new(ToggleInspector), cx);
                })),
            )
            .child(div().flex_1())
            .child(
                activity_button(
                    "activity-open",
                    "icons/file-plus.svg",
                    false,
                    i18n::text("workspace.open_file"),
                )
                .on_click(
                    cx.listener(|_workspace, _event, _window, cx| shell::prompt_and_open(cx)),
                ),
            )
    }

    fn explorer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_path = self.active_tab().map(|tab| tab.path.clone());
        let mut rows = Vec::new();
        let root = self.root.clone();
        if let Some(root) = &root {
            self.tree_rows(root, 0, &mut rows);
        }
        let root_name = root
            .as_ref()
            .and_then(|root| root.file_name())
            .map(|name| name.to_string_lossy().to_uppercase())
            .unwrap_or_else(|| i18n::text("workspace.no_folder").to_string());

        let tree: Vec<_> = rows
            .into_iter()
            .enumerate()
            .map(|(index, (entry, depth))| {
                let selected = active_path.as_ref() == Some(&entry.path);
                let expanded = self.expanded.contains(&entry.path);
                let path = entry.path.clone();
                let kind = entry.kind;
                div()
                    .id(("explorer-row", index))
                    .relative()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .mx_1()
                    .py(px(4.0))
                    .pl(px(8.0 + depth as f32 * 12.0))
                    .pr_2()
                    .rounded_md()
                    .cursor_pointer()
                    .text_xs()
                    .text_color(if selected {
                        theme::text()
                    } else {
                        theme::text_dim()
                    })
                    .when(selected, |row| {
                        row.bg(theme::selected()).child(
                            div()
                                .absolute()
                                .left_0()
                                .top_1()
                                .bottom_1()
                                .w(px(2.0))
                                .rounded_r_sm()
                                .bg(theme::accent()),
                        )
                    })
                    .hover(|row| row.bg(theme::hovered()).text_color(theme::text()))
                    .map(|row| match kind {
                        None => row
                            .child(
                                svg()
                                    .path(if expanded {
                                        "icons/chevron-down.svg"
                                    } else {
                                        "icons/chevron-right.svg"
                                    })
                                    .size_3()
                                    .text_color(theme::text_faint()),
                            )
                            .child(
                                svg()
                                    .path(if expanded {
                                        "icons/folder-open.svg"
                                    } else {
                                        "icons/folder.svg"
                                    })
                                    .size_3p5()
                                    .text_color(theme::accent()),
                            ),
                        Some(kind) => row.child(div().w_3()).child(
                            svg()
                                .path(kind_icon(kind))
                                .size_3p5()
                                .text_color(kind_color(kind)),
                        ),
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(entry.name.clone()),
                    )
                    .on_click(cx.listener(move |workspace, _event, window, cx| {
                        if kind.is_some() {
                            workspace.open_from_ui(path.clone(), window, cx);
                        } else {
                            workspace.toggle_directory(path.clone(), cx);
                        }
                    }))
            })
            .collect();

        let recent: Vec<_> = self
            .recent
            .iter()
            .take(8)
            .enumerate()
            .map(|(index, path)| {
                let kind = Kind::of(path);
                let path = path.clone();
                div()
                    .id(("recent-row", index))
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .mx_1()
                    .px_2()
                    .py(px(4.0))
                    .rounded_md()
                    .cursor_pointer()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .hover(|row| row.bg(theme::hovered()).text_color(theme::text()))
                    .children(kind.map(|kind| {
                        svg()
                            .path(kind_icon(kind))
                            .size_3p5()
                            .text_color(kind_color(kind))
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(shell::file_name(&path)),
                    )
                    .on_click(cx.listener(move |workspace, _event, window, cx| {
                        workspace.open_from_ui(path.clone(), window, cx)
                    }))
            })
            .collect();

        div()
            .w(px(EXPLORER_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme::sidebar())
            .border_r_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_3()
                    .py_1p5()
                    .gap_0p5()
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(theme::text_faint())
                            .child(i18n::text("workspace.explorer")),
                    )
                    .child(
                        icon_button(
                            "explorer-open-file",
                            "icons/file-plus.svg",
                            false,
                            i18n::text("workspace.open_file"),
                        )
                        .on_click(
                            cx.listener(|_workspace, _event, _window, cx| {
                                shell::prompt_and_open(cx)
                            }),
                        ),
                    )
                    .child(
                        icon_button(
                            "explorer-open-folder",
                            "icons/folder-plus.svg",
                            false,
                            i18n::text("workspace.open_folder"),
                        )
                        .on_click(cx.listener(
                            |workspace, _event, _window, cx| workspace.prompt_for_folder(cx),
                        )),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .text_xs()
                    .text_color(theme::text_faint())
                    .child(root_name),
            )
            .child(
                div()
                    .id("explorer-tree")
                    .flex_1()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(tree)
                    .when(root.is_none(), |tree| {
                        tree.child(
                            div()
                                .px_3()
                                .py_2()
                                .text_xs()
                                .text_color(theme::text_faint())
                                .child(i18n::text("workspace.explorer_hint")),
                        )
                    }),
            )
            .when(!recent.is_empty(), |explorer| {
                explorer
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_3()
                            .pt_2()
                            .pb_1()
                            .border_t_1()
                            .border_color(theme::border())
                            .text_xs()
                            .text_color(theme::text_faint())
                            .child(
                                svg()
                                    .path("icons/clock.svg")
                                    .size_3()
                                    .text_color(theme::text_faint()),
                            )
                            .child(i18n::text("workspace.recent")),
                    )
                    .child(div().flex().flex_col().pb_2().children(recent))
            })
    }

    fn tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs: Vec<_> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let active = self.active == Some(index);
                let kind = tab.kind;
                let dirty = tab.editor.is_dirty(cx);
                div()
                    .id(("editor-tab", index))
                    .group("editor-tab")
                    .relative()
                    .h_full()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_1p5()
                    .pl_3p5()
                    .pr_2()
                    .cursor_pointer()
                    .border_r_1()
                    .border_color(theme::border())
                    .text_xs()
                    .text_color(if active {
                        theme::text()
                    } else {
                        theme::text_dim()
                    })
                    .when(active, |tab| tab.bg(theme::window()))
                    .hover(|tab| tab.text_color(theme::text()))
                    .child(svg().path(kind_icon(kind)).size_3().text_color(if active {
                        kind_color(kind)
                    } else {
                        theme::text_faint()
                    }))
                    .child(shell::file_name(&tab.path))
                    .child(
                        // A dot while there are unsaved edits, turning into
                        // the close button under the pointer, as in the IDE.
                        div()
                            .id(("editor-tab-close", index))
                            .relative()
                            .size_4()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .when(!active && !dirty, |close| {
                                close
                                    .invisible()
                                    .group_hover("editor-tab", |close| close.visible())
                            })
                            .hover(|close| close.bg(theme::hovered()))
                            .when(dirty, |close| {
                                close.child(
                                    div()
                                        .size_2()
                                        .rounded_full()
                                        .bg(theme::text_dim())
                                        .group_hover("editor-tab", |dot| dot.invisible()),
                                )
                            })
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size_2p5()
                                    .text_color(theme::text_dim())
                                    .when(dirty, |x| {
                                        x.absolute()
                                            .invisible()
                                            .group_hover("editor-tab", |x| x.visible())
                                    }),
                            )
                            .on_click(cx.listener(move |workspace, _event, window, cx| {
                                cx.stop_propagation();
                                workspace.close_tab(index, window, cx);
                            })),
                    )
                    .when(active, |tab| {
                        tab.child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_1()
                                .right_1()
                                .h(px(2.0))
                                .rounded_t_sm()
                                .bg(linear_gradient(
                                    90.0,
                                    linear_color_stop(theme::accent(), 0.0),
                                    linear_color_stop(theme::accent_alt(), 1.0),
                                )),
                        )
                    })
                    .on_click(cx.listener(move |workspace, _event, window, cx| {
                        workspace.activate(index, window, cx)
                    }))
            })
            .collect();
        div()
            .id("tab-bar")
            .h(px(TAB_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .overflow_x_scroll()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .children(tabs)
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab_count = self.tabs.len();
        let active = self.active_tab();
        let items = active
            .map(|tab| tab.editor.status_items(cx))
            .unwrap_or_default();
        let zoom = active.map(|tab| tab.editor.zoom(cx));
        let kind = active.map(|tab| match tab.kind {
            Kind::Document => i18n::text("kind.document"),
            Kind::Workbook => i18n::text("kind.workbook"),
            Kind::Deck => i18n::text("kind.deck"),
        });
        div()
            .h(px(STATUS_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .bg(theme::panel())
            .border_t_1()
            .border_color(theme::border())
            .text_xs()
            .text_color(theme::text_faint())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .child(div().size_1p5().rounded_full().bg(theme::ready()))
                            .child(i18n::text("workspace.ready")),
                    )
                    .children(items.into_iter().map(|item| div().child(item)))
                    .children(
                        self.error
                            .clone()
                            .map(|error| div().text_color(theme::accent()).child(error)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .children(kind)
                    .child(i18n::format("workspace.tabs", &[("n", &tab_count)]))
                    .children(zoom.map(|zoom| self.zoom_controls(zoom, cx))),
            )
    }

    /// − 100% + and fit, as Office puts them at the right of its status bar.
    fn zoom_controls(&self, zoom: f32, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_0p5()
            .child(status_button("zoom-out", "−").on_click(cx.listener(
                |workspace, _event, _w, cx| {
                    workspace.zoom_active(|zoom| Some(editor::zoom_step(zoom, false)), cx)
                },
            )))
            .child(
                // The percentage itself goes back to 100%.
                status_button("zoom-actual", "")
                    .w(px(44.0))
                    .justify_center()
                    .child(format!("{:.0}%", zoom * 100.0))
                    .on_click(cx.listener(|workspace, _event, _w, cx| {
                        workspace.zoom_active(|_| Some(1.0), cx)
                    })),
            )
            .child(status_button("zoom-in", "+").on_click(cx.listener(
                |workspace, _event, _w, cx| {
                    workspace.zoom_active(|zoom| Some(editor::zoom_step(zoom, true)), cx)
                },
            )))
            .child(
                status_button("zoom-fit", i18n::text("workspace.zoom_fit")).on_click(
                    cx.listener(|workspace, _event, _w, cx| workspace.zoom_active(|_| None, cx)),
                ),
            )
    }

    /// What the editor area shows with nothing open: the IDE's empty state,
    /// a way to open files, and the recent ones.
    fn welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(theme::window())
            .child(
                div()
                    .size_12()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_xl()
                    .bg(linear_gradient(
                        135.0,
                        linear_color_stop(gpui::rgb(0x6366f1), 0.0),
                        linear_color_stop(gpui::rgb(0xec4899), 1.0),
                    ))
                    .child(
                        svg()
                            .path("icons/zap.svg")
                            .size_6()
                            .text_color(gpui::white()),
                    ),
            )
            .child(
                div()
                    .text_xl()
                    .text_color(theme::text())
                    .child("jade office"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme::text_dim())
                    .child(i18n::text("workspace.tagline")),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        welcome_button(
                            "welcome-open",
                            "icons/file-plus.svg",
                            i18n::text("workspace.open_file"),
                            true,
                        )
                        .on_click(
                            cx.listener(|_workspace, _event, _window, cx| {
                                shell::prompt_and_open(cx)
                            }),
                        ),
                    )
                    .child(
                        welcome_button(
                            "welcome-folder",
                            "icons/folder-plus.svg",
                            i18n::text("workspace.open_folder"),
                            false,
                        )
                        .on_click(cx.listener(
                            |workspace, _event, _window, cx| workspace.prompt_for_folder(cx),
                        )),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_faint())
                    .child(i18n::text("workspace.drop_hint")),
            )
            .children(
                self.error
                    .clone()
                    .map(|error| div().text_xs().text_color(theme::accent()).child(error)),
            )
    }
}

fn status_button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .px_1p5()
        .rounded_sm()
        .cursor_pointer()
        .text_color(theme::text_dim())
        .hover(|button| button.bg(theme::hovered()).text_color(theme::text()))
        .child(label)
}

fn icon_button(
    id: &'static str,
    icon: &'static str,
    active: bool,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .tooltip(Tooltip::text(label))
        .p_1p5()
        .rounded_md()
        .cursor_pointer()
        .when(active, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .child(svg().path(icon).size_3p5().text_color(if active {
            theme::accent()
        } else {
            theme::text_dim()
        }))
}

fn activity_button(
    id: &'static str,
    icon: &'static str,
    active: bool,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .tooltip(Tooltip::text(label))
        .relative()
        .size_8()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .cursor_pointer()
        .when(active, |button| {
            button.bg(theme::selected()).child(
                div()
                    .absolute()
                    .left_0()
                    .top_2()
                    .bottom_2()
                    .w(px(2.0))
                    .rounded_r_sm()
                    .bg(theme::accent()),
            )
        })
        .hover(|button| button.bg(theme::hovered()))
        .child(svg().path(icon).size_4().text_color(if active {
            theme::accent()
        } else {
            theme::text_dim()
        }))
}

fn welcome_button(
    id: &'static str,
    icon: &'static str,
    label: &'static str,
    primary: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .px_4()
        .py_2()
        .rounded_lg()
        .cursor_pointer()
        .text_sm()
        .map(|button| {
            if primary {
                button
                    .text_color(gpui::white())
                    .bg(linear_gradient(
                        90.0,
                        linear_color_stop(gpui::rgb(0x6366f1), 0.0),
                        linear_color_stop(gpui::rgb(0xa855f7), 1.0),
                    ))
                    .hover(|button| button.opacity(0.9))
                    .child(svg().path(icon).size_4().text_color(gpui::white()))
            } else {
                button
                    .text_color(theme::text())
                    .border_1()
                    .border_color(theme::border())
                    .hover(|button| button.bg(theme::hovered()))
                    .child(svg().path(icon).size_4().text_color(theme::text_dim()))
            }
        })
        .child(label)
}

impl Focusable for Workspace {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The views below size themselves to this; set before they render.
        let viewport = window.viewport_size();
        let side = ACTIVITY_BAR_WIDTH
            + if self.explorer_open {
                EXPLORER_WIDTH
            } else {
                0.0
            };
        cx.set_global(EditorArea(
            (f32::from(viewport.width) - side).max(1.0),
            (f32::from(viewport.height) - TITLE_BAR_HEIGHT - TAB_BAR_HEIGHT - STATUS_BAR_HEIGHT)
                .max(1.0),
        ));

        let explorer_open = self.explorer_open;
        let explorer = explorer_open.then(|| self.explorer(cx));
        let active_view = self.active_tab().map(|tab| tab.view.clone());
        let has_tabs = !self.tabs.is_empty();

        div()
            .id("workspace")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::window())
            .text_color(theme::text())
            .font_family(UI_FONT_FAMILY)
            .track_focus(&self.focus)
            .key_context("Workspace")
            .on_action(cx.listener(|workspace, _: &CloseWindow, window, cx| {
                workspace.close_active(window, cx)
            }))
            .on_action(cx.listener(|workspace, _: &ToggleExplorer, _window, cx| {
                workspace.explorer_open = !workspace.explorer_open;
                cx.notify();
            }))
            .on_action(
                cx.listener(|workspace, _: &OpenFolder, _window, cx| {
                    workspace.prompt_for_folder(cx)
                }),
            )
            .on_action(cx.listener(|workspace, _: &ZoomIn, _window, cx| {
                workspace.zoom_active(|zoom| Some(editor::zoom_step(zoom, true)), cx)
            }))
            .on_action(cx.listener(|workspace, _: &ZoomOut, _window, cx| {
                workspace.zoom_active(|zoom| Some(editor::zoom_step(zoom, false)), cx)
            }))
            .on_action(cx.listener(|workspace, _: &ZoomActual, _window, cx| {
                workspace.zoom_active(|_| Some(1.0), cx)
            }))
            .on_action(cx.listener(|workspace, _: &ZoomFit, _window, cx| {
                workspace.zoom_active(|_| None, cx)
            }))
            .drag_over::<ExternalPaths>(|style, _, _, _| style.bg(theme::selected()))
            .on_drop(cx.listener(|workspace, paths: &ExternalPaths, window, cx| {
                for path in paths.paths() {
                    if path.is_dir() {
                        workspace.set_root(path.clone());
                        cx.notify();
                    } else {
                        workspace.open_from_ui(path.clone(), window, cx);
                    }
                }
            }))
            .child(self.title_bar(cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .overflow_hidden()
                    .child(self.activity_bar(cx))
                    .children(explorer)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .when(has_tabs, |editor| editor.child(self.tab_bar(cx)))
                            .child(div().flex_1().min_h_0().overflow_hidden().map(|content| {
                                match active_view {
                                    Some(view) => content.child(view),
                                    None => content.child(self.welcome(cx)),
                                }
                            })),
                    ),
            )
            .child(self.status_bar(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core")).join(name)
    }

    /// One file of each kind opens as a tab; opening one again brings its
    /// tab forward instead of adding a second; zoom and ⌘W act on the tab in
    /// front.
    #[gpui::test]
    fn every_format_opens_as_a_tab_and_zoom_follows_the_active_one(cx: &mut gpui::TestAppContext) {
        let (workspace, cx) = cx.add_window_view(|_window, cx| Workspace::new(cx));
        let files = [
            fixture("contract.docx"),
            fixture("public/fixtures/grid-operations-workbook.xlsx"),
            fixture("public/fixtures/sample-deck.pptx"),
        ];
        cx.update(|window, cx| {
            workspace.update(cx, |workspace, cx| {
                for file in &files {
                    workspace.open(file, window, cx).unwrap();
                }
                workspace.open(&files[1], window, cx).unwrap();
            })
        });
        cx.run_until_parked();
        workspace.read_with(cx, |workspace, cx| {
            assert_eq!(workspace.tabs.len(), 3);
            assert_eq!(workspace.active, Some(1));
            assert_eq!(workspace.tabs[1].kind, Kind::Workbook);
            // Opening is not editing: no tab asks to be saved.
            for tab in &workspace.tabs {
                assert!(
                    !tab.editor.is_dirty(cx),
                    "{} opens modified",
                    tab.path.display()
                );
            }
        });

        cx.dispatch_action(ZoomIn);
        let zoom = workspace.read_with(cx, |workspace, cx| workspace.tabs[1].editor.zoom(cx));
        assert_eq!(zoom, 1.1);
        cx.dispatch_action(ZoomActual);
        let zoom = workspace.read_with(cx, |workspace, cx| workspace.tabs[1].editor.zoom(cx));
        assert_eq!(zoom, 1.0);

        cx.dispatch_action(CloseWindow);
        workspace.read_with(cx, |workspace, _| {
            assert_eq!(workspace.tabs.len(), 2);
            assert!(workspace.tabs.iter().all(|tab| tab.kind != Kind::Workbook));
        });
    }

    /// A password-protected file is an OLE container, not a zip: the app
    /// says so instead of reporting a parse error.
    #[gpui::test]
    fn a_protected_file_says_it_is_protected(cx: &mut gpui::TestAppContext) {
        let (workspace, cx) = cx.add_window_view(|_window, cx| Workspace::new(cx));
        for name in ["locked.xlsx", "locked.docx", "locked.pptx"] {
            let path = std::env::temp_dir()
                .join(format!("jade-protected-{}", std::process::id()))
                .join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut bytes = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
            bytes.resize(4096, 0);
            std::fs::write(&path, bytes).unwrap();
            let result = cx.update(|window, cx| {
                workspace.update(cx, |workspace, cx| workspace.open(&path, window, cx))
            });
            let message = result.expect_err(name);
            assert_eq!(
                message,
                i18n::format("workspace.protected", &[("name", &name)])
            );
        }
    }

    #[gpui::test]
    fn an_unsupported_file_is_refused_with_a_message(cx: &mut gpui::TestAppContext) {
        let (workspace, cx) = cx.add_window_view(|_window, cx| Workspace::new(cx));
        let result = cx.update(|window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.open(&fixture("README.md"), window, cx)
            })
        });
        assert!(result.is_err());
        workspace.read_with(cx, |workspace, _| {
            assert!(workspace.tabs.is_empty());
            assert!(workspace.error.is_some());
        });
    }
}
