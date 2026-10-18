//! The page's one window, framed as the native workspace frames its views
//! (`jade-gpui-app`'s `workspace.rs`):
//!
//! ```text
//! ┌──────────────────────── title bar: logo · active file · theme ────────┐
//! │act│ explorer │ tabs: contract.docx │ book.xlsx │ deck.pptx            │
//! │ivi│ samples  ├───────────────────────────────────────────────────────┤
//! │ty │ recent   │ the active view — document, workbook or deck          │
//! │bar│          │                                                       │
//! ├───┴──────────┴───────────────────────────────────────────────────────┤
//! │ ● Ready  Slide 1 / 3                       PowerPoint · 1 tab · 100%  │
//! └───────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! A browser has no paths to open, so the explorer lists the page's samples
//! instead of a folder, and every file — a sample, one picked or one dropped
//! on the page — arrives through `open_file` as a name and its bytes.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    div, linear_color_stop, linear_gradient, prelude::*, px, svg, AnyView, App, Context, Entity,
    FocusHandle, Focusable, KeyDownEvent, SharedString, Window,
};

use crate::view::{zoom_step, DocView, EditorArea, Kind};
use crate::{i18n, palette, Fonts, Host, UI_FONT};

const TITLE_BAR_HEIGHT: f32 = 40.0;
const ACTIVITY_BAR_WIDTH: f32 = 44.0;
const EXPLORER_WIDTH: f32 = 224.0;
const TAB_BAR_HEIGHT: f32 = 36.0;
const STATUS_BAR_HEIGHT: f32 = 24.0;
const RECENT_LIMIT: usize = 8;

pub struct Workspace {
    tabs: Vec<Entity<DocView>>,
    active: Option<usize>,
    explorer_open: bool,
    samples: Vec<SharedString>,
    /// The bytes of every file opened, so a closed one can come back from
    /// the recent list: the page has no path to read it from again.
    files: HashMap<SharedString, Rc<Vec<u8>>>,
    recent: Vec<SharedString>,
    error: Option<SharedString>,
    focus: FocusHandle,
    fonts: Rc<Fonts>,
    host: Host,
}

impl Workspace {
    pub fn new(samples: Vec<String>, fonts: Rc<Fonts>, host: Host, cx: &mut Context<Self>) -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            explorer_open: true,
            samples: samples.into_iter().map(SharedString::from).collect(),
            files: HashMap::new(),
            recent: Vec::new(),
            error: None,
            focus: cx.focus_handle(),
            fonts,
            host,
        }
    }

    /// Opens `bytes` as `name` in a new tab, or brings its tab forward when
    /// a file of that name is open.
    pub fn open(
        &mut self,
        name: SharedString,
        bytes: Rc<Vec<u8>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self.tabs.iter().position(|tab| tab.read(cx).name == name) {
            self.activate(index, window, cx);
            return;
        }
        let opened = DocView::open(
            name.clone(),
            &bytes,
            &self.fonts,
            self.host.clone(),
            cx.focus_handle(),
        );
        match opened {
            Ok(view) => {
                let entity = cx.new(|_| view);
                // Repaints the frame whenever the view changes, so the status
                // bar follows the page, the slide and the zoom.
                cx.observe(&entity, |_workspace, _entity, cx| cx.notify())
                    .detach();
                self.tabs.push(entity);
                self.files.insert(name.clone(), bytes);
                self.recent.retain(|recent| *recent != name);
                self.recent.insert(0, name);
                self.recent.truncate(RECENT_LIMIT);
                self.error = None;
                self.activate(self.tabs.len() - 1, window, cx);
            }
            Err(error) => {
                self.error = Some(
                    i18n::format("web.open_failed", &[("name", &name), ("error", &error)]).into(),
                );
                cx.notify();
            }
        }
    }

    /// A sample or a recent file, from the explorer.
    fn open_by_name(&mut self, name: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        match self.files.get(&name).cloned() {
            Some(bytes) => self.open(name, bytes, window, cx),
            // Fetched by the page, which hands it back through `open_file`.
            None => self.host.load(&name),
        }
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        self.active = Some(index);
        window.focus(&tab.focus_handle(cx), cx);
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.active = None;
            window.focus(&self.focus, cx);
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

    fn active_tab(&self) -> Option<&Entity<DocView>> {
        self.active.and_then(|index| self.tabs.get(index))
    }

    fn zoom_active(
        &mut self,
        zoom: impl FnOnce(f32) -> Option<f32>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.active_tab().cloned() else {
            return;
        };
        let wanted = zoom(tab.read(cx).zoom());
        tab.update(cx, |view, cx| match wanted {
            Some(zoom) => view.set_zoom(zoom, cx),
            None => view.zoom_to_fit(window, cx),
        });
    }

    // ---- the frame --------------------------------------------------------

    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active_tab().map(|tab| tab.read(cx));
        let title: SharedString = active
            .map(|view| view.name.clone())
            .unwrap_or_else(|| SharedString::from("jade office"));
        let kind = active.map(|view| view.kind);
        let editable = active.is_some_and(|view| view.is_editable());
        let dirty = active.is_some_and(|view| view.is_dirty());
        div()
            .id("title-bar")
            .h(px(TITLE_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .px_3()
            .gap_3()
            .bg(palette::panel())
            .border_b_1()
            .border_color(palette::border())
            // The logo: the IDE's gradient tile with its bolt.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(logo(24.0, 12.0, 8.0))
                    .child(div().text_xs().text_color(palette::text()).child("Jade")),
            )
            // Where the IDE has its search box: what is open.
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
                        .bg(palette::hovered())
                        .text_xs()
                        .text_color(palette::dim())
                        .children(kind.map(|kind| {
                            svg().path(kind.icon()).size_3p5().text_color(kind.color())
                        }))
                        .child(div().text_color(palette::text()).child(title))
                        .children(active.map(|_| {
                            div()
                                .text_color(palette::faint())
                                .child(i18n::text("web.samples"))
                        }))
                        .when(dirty, |pill| {
                            pill.child(div().size_1p5().rounded_full().bg(palette::dim()))
                        }),
                ),
            )
            // A browser has no menu bar: undo, redo and save live here.
            .when(editable, |bar| {
                bar.child(
                    icon_button("undo", "icons/undo.svg", false).on_click(cx.listener(
                        |workspace, _event, _window, cx| {
                            workspace.with_active(cx, |view, cx| view.step_history(true, cx))
                        },
                    )),
                )
                .child(
                    icon_button("redo", "icons/redo.svg", false).on_click(cx.listener(
                        |workspace, _event, _window, cx| {
                            workspace.with_active(cx, |view, cx| view.step_history(false, cx))
                        },
                    )),
                )
                .child(
                    icon_button("download", "icons/download.svg", false).on_click(cx.listener(
                        |workspace, _event, _window, cx| {
                            workspace.with_active(cx, |view, cx| view.save(cx))
                        },
                    )),
                )
            })
            .child(
                icon_button(
                    "theme-toggle",
                    if palette::is_dark() {
                        "icons/sun.svg"
                    } else {
                        "icons/moon.svg"
                    },
                    false,
                )
                .on_click(cx.listener(|_workspace, _event, window, cx| {
                    palette::toggle();
                    window.refresh();
                    cx.notify();
                })),
            )
    }

    fn with_active(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut DocView, &mut Context<DocView>),
    ) {
        if let Some(tab) = self.active_tab().cloned() {
            tab.update(cx, f);
        }
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
            .bg(palette::sidebar())
            .border_r_1()
            .border_color(palette::border())
            .child(
                activity_button("activity-explorer", "icons/files.svg", self.explorer_open)
                    .on_click(cx.listener(|workspace, _event, _window, cx| {
                        workspace.explorer_open = !workspace.explorer_open;
                        cx.notify();
                    })),
            )
            .child(div().w_5().h(px(1.0)).my_1().bg(palette::border()))
            .child(div().flex_1())
            .child(
                activity_button("activity-open", "icons/file-plus.svg", false)
                    .on_click(cx.listener(|workspace, _event, _window, _cx| workspace.host.pick())),
            )
    }

    fn explorer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_name = self.active_tab().map(|tab| tab.read(cx).name.clone());
        let row = |id: (&'static str, usize),
                   name: &SharedString,
                   depth_pad: f32,
                   selected: bool| {
            let kind = Kind::of(name);
            div()
                .id(id)
                .relative()
                .flex()
                .items_center()
                .gap_1p5()
                .mx_1()
                .py(px(4.0))
                .pl(px(depth_pad))
                .pr_2()
                .rounded_md()
                .cursor_pointer()
                .text_xs()
                .text_color(if selected {
                    palette::text()
                } else {
                    palette::dim()
                })
                .when(selected, |row| {
                    row.bg(palette::selected()).child(
                        div()
                            .absolute()
                            .left_0()
                            .top_1()
                            .bottom_1()
                            .w(px(2.0))
                            .rounded_r_sm()
                            .bg(palette::accent()),
                    )
                })
                .hover(|row| row.bg(palette::hovered()).text_color(palette::text()))
                .children(
                    kind.map(|kind| svg().path(kind.icon()).size_3p5().text_color(kind.color())),
                )
                .child(div().flex_1().min_w_0().truncate().child(name.clone()))
        };
        let tree: Vec<_> = self
            .samples
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let selected = active_name.as_ref() == Some(name);
                let name = name.clone();
                // The native tree indents files past the folder chevron.
                row(("explorer-row", index), &name, 20.0, selected).on_click(cx.listener(
                    move |workspace, _event, window, cx| {
                        workspace.open_by_name(name.clone(), window, cx)
                    },
                ))
            })
            .collect();
        let recent: Vec<_> = self
            .recent
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let name = name.clone();
                row(("recent-row", index), &name, 8.0, false).on_click(cx.listener(
                    move |workspace, _event, window, cx| {
                        workspace.open_by_name(name.clone(), window, cx)
                    },
                ))
            })
            .collect();

        div()
            .w(px(EXPLORER_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(palette::sidebar())
            .border_r_1()
            .border_color(palette::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_3()
                    .py_1p5()
                    .gap_0p5()
                    .border_b_1()
                    .border_color(palette::border())
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(palette::faint())
                            .child(i18n::text("workspace.explorer")),
                    )
                    .child(
                        icon_button("explorer-open-file", "icons/file-plus.svg", false).on_click(
                            cx.listener(|workspace, _event, _window, _cx| workspace.host.pick()),
                        ),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .text_xs()
                    .text_color(palette::faint())
                    .child(i18n::text("web.samples").to_uppercase()),
            )
            .child(
                div()
                    .id("explorer-tree")
                    .flex_1()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(tree),
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
                            .border_color(palette::border())
                            .text_xs()
                            .text_color(palette::faint())
                            .child(
                                svg()
                                    .path("icons/clock.svg")
                                    .size_3()
                                    .text_color(palette::faint()),
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
                let view = tab.read(cx);
                let active = self.active == Some(index);
                let kind = view.kind;
                let dirty = view.is_dirty();
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
                    .border_color(palette::border())
                    .text_xs()
                    .text_color(if active {
                        palette::text()
                    } else {
                        palette::dim()
                    })
                    .when(active, |tab| tab.bg(palette::window()))
                    .hover(|tab| tab.text_color(palette::text()))
                    .child(svg().path(kind.icon()).size_3().text_color(if active {
                        kind.color()
                    } else {
                        palette::faint()
                    }))
                    .child(view.name.clone())
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
                            .hover(|close| close.bg(palette::hovered()))
                            .when(dirty, |close| {
                                close.child(
                                    div()
                                        .size_2()
                                        .rounded_full()
                                        .bg(palette::dim())
                                        .group_hover("editor-tab", |dot| dot.invisible()),
                                )
                            })
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size_2p5()
                                    .text_color(palette::dim())
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
                                    linear_color_stop(palette::accent(), 0.0),
                                    linear_color_stop(palette::accent_alt(), 1.0),
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
            .bg(palette::panel())
            .border_b_1()
            .border_color(palette::border())
            .children(tabs)
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab_count = self.tabs.len();
        let active = self.active_tab().map(|tab| tab.read(cx));
        let items = active.map(|view| view.status_items()).unwrap_or_default();
        let zoom = active.map(|view| view.zoom());
        let kind = active.map(|view| view.kind.label());
        div()
            .h(px(STATUS_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .bg(palette::panel())
            .border_t_1()
            .border_color(palette::border())
            .text_xs()
            .text_color(palette::faint())
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
                            .child(div().size_1p5().rounded_full().bg(palette::ready()))
                            .child(i18n::text("workspace.ready")),
                    )
                    .children(items.into_iter().map(|item| div().child(item)))
                    .children(
                        self.error
                            .clone()
                            .map(|error| div().text_color(palette::accent()).child(error)),
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
                |workspace, _event, window, cx| {
                    workspace.zoom_active(|zoom| Some(zoom_step(zoom, false)), window, cx)
                },
            )))
            .child(
                // The percentage itself goes back to 100%.
                status_button("zoom-actual", "")
                    .w(px(44.0))
                    .justify_center()
                    .child(format!("{:.0}%", zoom * 100.0))
                    .on_click(cx.listener(|workspace, _event, window, cx| {
                        workspace.zoom_active(|_| Some(1.0), window, cx)
                    })),
            )
            .child(status_button("zoom-in", "+").on_click(cx.listener(
                |workspace, _event, window, cx| {
                    workspace.zoom_active(|zoom| Some(zoom_step(zoom, true)), window, cx)
                },
            )))
            .child(
                status_button("zoom-fit", i18n::text("workspace.zoom_fit")).on_click(cx.listener(
                    |workspace, _event, window, cx| workspace.zoom_active(|_| None, window, cx),
                )),
            )
    }

    /// What the editor area shows with nothing open: the IDE's empty state.
    fn welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(palette::window())
            .child(logo(48.0, 24.0, 12.0))
            .child(
                div()
                    .text_xl()
                    .text_color(palette::text())
                    .child("jade office"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(palette::dim())
                    .child(i18n::text("workspace.tagline")),
            )
            .child(
                div()
                    .id("welcome-open")
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .rounded_lg()
                    .cursor_pointer()
                    .text_sm()
                    .text_color(gpui::white())
                    .bg(linear_gradient(
                        90.0,
                        linear_color_stop(gpui::rgb(0x6366f1), 0.0),
                        linear_color_stop(gpui::rgb(0xa855f7), 1.0),
                    ))
                    .hover(|button| button.opacity(0.9))
                    .child(
                        svg()
                            .path("icons/file-plus.svg")
                            .size_4()
                            .text_color(gpui::white()),
                    )
                    .child(i18n::text("workspace.open_file"))
                    .on_click(cx.listener(|workspace, _event, _window, _cx| workspace.host.pick())),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(palette::faint())
                    .child(i18n::text("workspace.drop_hint")),
            )
            .children(
                self.error
                    .clone()
                    .map(|error| div().text_xs().text_color(palette::accent()).child(error)),
            )
    }
}

/// The IDE's gradient tile with its bolt.
fn logo(size: f32, icon: f32, radius: f32) -> impl IntoElement {
    div()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius))
        .bg(linear_gradient(
            135.0,
            linear_color_stop(gpui::rgb(0x6366f1), 0.0),
            linear_color_stop(gpui::rgb(0xec4899), 1.0),
        ))
        .child(
            svg()
                .path("icons/zap.svg")
                .size(px(icon))
                .text_color(gpui::white()),
        )
}

fn status_button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .px_1p5()
        .rounded_sm()
        .cursor_pointer()
        .text_color(palette::dim())
        .hover(|button| button.bg(palette::hovered()).text_color(palette::text()))
        .child(label)
}

fn icon_button(id: &'static str, icon: &'static str, active: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .p_1p5()
        .rounded_md()
        .cursor_pointer()
        .when(active, |button| button.bg(palette::selected()))
        .hover(|button| button.bg(palette::hovered()))
        .child(svg().path(icon).size_3p5().text_color(if active {
            palette::accent()
        } else {
            palette::dim()
        }))
}

fn activity_button(
    id: &'static str,
    icon: &'static str,
    active: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .relative()
        .size_8()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .cursor_pointer()
        .when(active, |button| {
            button.bg(palette::selected()).child(
                div()
                    .absolute()
                    .left_0()
                    .top_2()
                    .bottom_2()
                    .w(px(2.0))
                    .rounded_r_sm()
                    .bg(palette::accent()),
            )
        })
        .hover(|button| button.bg(palette::hovered()))
        .child(svg().path(icon).size_4().text_color(if active {
            palette::accent()
        } else {
            palette::dim()
        }))
}

impl Focusable for Workspace {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        palette::follow(window.appearance());
        let viewport = window.viewport_size();
        let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));

        // A slideshow has the whole window.
        let slideshow = self
            .active_tab()
            .filter(|tab| tab.read(cx).in_slideshow())
            .cloned();
        if let Some(tab) = slideshow {
            cx.set_global(EditorArea(width.max(1.0), height.max(1.0)));
            return div()
                .size_full()
                .child(AnyView::from(tab))
                .into_any_element();
        }

        // The views below size themselves to this; set before they render.
        let side = ACTIVITY_BAR_WIDTH
            + if self.explorer_open {
                EXPLORER_WIDTH
            } else {
                0.0
            };
        cx.set_global(EditorArea(
            (width - side).max(1.0),
            (height - TITLE_BAR_HEIGHT - TAB_BAR_HEIGHT - STATUS_BAR_HEIGHT).max(1.0),
        ));

        let explorer = self.explorer_open.then(|| self.explorer(cx));
        let active_view = self.active_tab().cloned().map(AnyView::from);
        let has_tabs = !self.tabs.is_empty();

        div()
            .id("workspace")
            .size_full()
            .flex()
            .flex_col()
            .bg(palette::window())
            .text_color(palette::text())
            .font_family(UI_FONT)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|workspace, event: &KeyDownEvent, _window, cx| {
                let modifiers = event.keystroke.modifiers;
                if (modifiers.platform || modifiers.control) && event.keystroke.key == "o" {
                    workspace.host.pick();
                    cx.stop_propagation();
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
            .into_any_element()
    }
}
