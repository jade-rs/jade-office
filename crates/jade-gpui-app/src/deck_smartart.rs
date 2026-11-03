//! SmartArt in the presentation view, as PowerPoint for Mac's 挿入 ›
//! SmartArt gallery and its SmartArt のデザイン and 書式 tabs: the
//! gallery by category, the テキスト ウィンドウ, 図形の追加, レベル上げ /
//! レベル下げ, 上へ移動 / 下へ移動, レイアウト, 色の変更, SmartArt の
//! スタイル, グラフィックのリセット, 図形に変換, and for one shape of a
//! diagram its fill, outline, shape and picture. Also テキストを SmartArt
//! に変換 for a text shape.
//!
//! A child module of the view (`deck.rs` names it), so it reaches the
//! view's state; the editing itself is jade-office-model's.

use gpui::{div, prelude::*, px, svg, AnyElement, Context, Div, SharedString, Stateful};
use jade_office_model::deck::{
    SmartArtCategory, SmartArtEdit, SmartArtFormat, SmartArtPlace, SMARTART_COLORS,
    SMARTART_LAYOUTS, SMARTART_STYLES,
};
use jade_office_model::text::TextEdit as LineEdit;

use super::{menu_panel, menu_row, toolbar_button, with_menu, DeckView};
use crate::{i18n, theme};

/// A list open from the SmartArt tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmartArtPopup {
    /// 挿入 › SmartArt: the gallery, one category shown.
    Gallery(SmartArtCategory),
    /// テキストを SmartArt に変換.
    Convert,
    AddShape,
    Layouts,
    Colors,
    Styles,
    Fill,
    Outline,
    ChangeShape,
}

/// The テキスト ウィンドウ: its lines and the one typed into.
pub struct TextPane {
    pub id: u32,
    pub lines: Vec<(usize, LineEdit)>,
    pub focus: Option<usize>,
}

/// The SmartArt tools' state.
#[derive(Default)]
pub struct SmartArtTools {
    pub popup: Option<SmartArtPopup>,
    pub pane: Option<TextPane>,
    /// The bullet whose shape is selected inside the selected diagram.
    pub node: Option<String>,
}

fn japanese() -> bool {
    i18n::language() == i18n::Language::Japanese
}

/// Colours the fill and outline lists offer: the theme's accents as
/// PowerPoint's palette shows them, as RGB.
const SWATCHES: [(&str, &str); 10] = [
    ("4472C4", "deck.smartart.color.blue"),
    ("ED7D31", "deck.smartart.color.orange"),
    ("A5A5A5", "deck.smartart.color.gray"),
    ("FFC000", "deck.smartart.color.gold"),
    ("5B9BD5", "deck.smartart.color.light_blue"),
    ("70AD47", "deck.smartart.color.green"),
    ("FF0000", "deck.smartart.color.red"),
    ("000000", "deck.smartart.color.black"),
    ("FFFFFF", "deck.smartart.color.white"),
    ("7030A0", "deck.smartart.color.purple"),
];

/// Shapes 図形の変更 offers.
const SHAPES: [(&str, &str); 8] = [
    ("rect", "deck.smartart.shape.rect"),
    ("roundRect", "deck.smartart.shape.round_rect"),
    ("ellipse", "deck.smartart.shape.ellipse"),
    ("triangle", "deck.smartart.shape.triangle"),
    ("diamond", "deck.smartart.shape.diamond"),
    ("hexagon", "deck.smartart.shape.hexagon"),
    ("chevron", "deck.smartart.shape.chevron"),
    ("rightArrow", "deck.smartart.shape.right_arrow"),
];

fn bar_button(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .px_2()
        .py_0p5()
        .rounded_md()
        .text_xs()
        .whitespace_nowrap()
        .text_color(theme::text_dim())
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()).text_color(theme::text()))
        .child(label)
}

fn separator() -> Div {
    div().w(px(1.0)).h(px(18.0)).mx_1().bg(theme::border())
}

impl DeckView {
    /// The diagram selected on its own, if the selection is one.
    pub(super) fn selected_smartart(&self) -> Option<u32> {
        let id = self.selected.filter(|_| self.more.is_empty())?;
        (self.shape(id)?.kind == "diagram").then_some(id)
    }

    fn smartart_popup(&mut self, popup: SmartArtPopup, cx: &mut Context<Self>) {
        self.smartart.popup = if self.smartart.popup == Some(popup) {
            None
        } else {
            Some(popup)
        };
        self.popup = None;
        cx.notify();
    }

    fn smartart_done(&mut self, result: Result<bool, String>, cx: &mut Context<Self>) {
        self.smartart.popup = None;
        match result {
            Ok(_) => {
                self.reload(true, cx);
                self.refresh_pane();
            }
            Err(error) => self.status = SharedString::from(error),
        }
        cx.notify();
    }

    /// 挿入 › SmartArt › a layout.
    pub(super) fn insert_smartart(&mut self, layout: &str, cx: &mut Context<Self>) {
        self.smartart.popup = None;
        let result = self.deck.insert_smartart(self.current, layout);
        if let Some(id) = self.inserted(result, cx) {
            self.open_pane(id);
        }
    }

    /// テキストを SmartArt に変換 on the selected text shape.
    fn convert_to_smartart(&mut self, layout: &str, cx: &mut Context<Self>) {
        self.smartart.popup = None;
        let Some(id) = self.selected else {
            return;
        };
        let result = self.deck.convert_to_smartart(self.current, id, layout);
        self.inserted(result, cx);
    }

    fn open_pane(&mut self, id: u32) {
        let lines = self
            .deck
            .smartart_text_pane(self.current, id)
            .unwrap_or_default()
            .into_iter()
            .map(|(level, text)| (level, LineEdit::new(text)))
            .collect::<Vec<_>>();
        let focus = (!lines.is_empty()).then_some(0);
        self.smartart.pane = Some(TextPane { id, lines, focus });
    }

    /// The pane again from the diagram, after an edit elsewhere.
    fn refresh_pane(&mut self) {
        let Some(pane) = &self.smartart.pane else {
            return;
        };
        let (id, focus) = (pane.id, pane.focus);
        match self.deck.smartart_text_pane(self.current, id) {
            Some(lines) => {
                let count = lines.len();
                self.smartart.pane = Some(TextPane {
                    id,
                    lines: lines
                        .into_iter()
                        .map(|(level, text)| (level, LineEdit::new(text)))
                        .collect(),
                    focus: focus
                        .map(|f| f.min(count.saturating_sub(1)))
                        .filter(|_| count > 0),
                });
            }
            None => self.smartart.pane = None,
        }
    }

    fn toggle_pane(&mut self, cx: &mut Context<Self>) {
        if self.smartart.pane.is_some() {
            self.commit_pane(cx);
            self.smartart.pane = None;
        } else if let Some(id) = self.selected_smartart() {
            self.open_pane(id);
        }
        cx.notify();
    }

    /// Writes the pane's lines to the diagram.
    fn commit_pane(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = &self.smartart.pane else {
            return;
        };
        let id = pane.id;
        let lines: Vec<(usize, String)> = pane
            .lines
            .iter()
            .map(|(level, edit)| (*level, edit.text.clone()))
            .collect();
        if self.deck.smartart_text_pane(self.current, id).as_ref() == Some(&lines) {
            return;
        }
        let result = self.deck.set_smartart_text_pane(self.current, id, &lines);
        self.smartart_done(result, cx);
    }

    /// Keys while the テキスト ウィンドウ has a line: typing, Return for a
    /// new bullet, Tab and ⇧Tab to demote and promote, Backspace on an
    /// empty line to delete it, ↑ ↓ between lines, Esc to leave. Whether
    /// the key was the pane's.
    pub(super) fn smartart_key(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        if !self.pane_has_keys() {
            return false;
        }
        let Some(pane) = self.smartart.pane.as_mut() else {
            return false;
        };
        let Some(index) = pane.focus else {
            return false;
        };
        let mut commit = false;
        match key {
            "enter" => {
                let level = pane.lines[index].0;
                pane.lines
                    .insert(index + 1, (level, LineEdit::new(String::new())));
                pane.focus = Some(index + 1);
                commit = true;
            }
            "tab" => {
                let level = pane.lines[index].0;
                if shift {
                    pane.lines[index].0 = level.saturating_sub(1);
                } else if index > 0 && pane.lines[index - 1].0 >= level {
                    pane.lines[index].0 = level + 1;
                }
                commit = true;
            }
            "backspace" if pane.lines[index].1.text.is_empty() && pane.lines.len() > 1 => {
                pane.lines.remove(index);
                pane.focus = Some(index.saturating_sub(1));
                commit = true;
            }
            "up" => pane.focus = Some(index.saturating_sub(1)),
            "down" => pane.focus = Some((index + 1).min(pane.lines.len() - 1)),
            "escape" => {
                pane.focus = None;
                commit = true;
            }
            "backspace" => pane.lines[index].1.backspace(),
            "delete" => pane.lines[index].1.delete(),
            "left" => pane.lines[index].1.step(false),
            "right" => pane.lines[index].1.step(true),
            _ => return false,
        }
        if commit {
            self.commit_pane(cx);
        }
        cx.notify();
        true
    }

    /// Whether the pane's line has the keys: it is shown (its diagram
    /// selected) and a line has the caret.
    fn pane_has_keys(&self) -> bool {
        self.smartart
            .pane
            .as_ref()
            .is_some_and(|pane| pane.focus.is_some() && self.selected_smartart() == Some(pane.id))
    }

    /// Text typed (or composed) while a line of the テキスト ウィンドウ has
    /// the caret: whether it went there.
    pub(super) fn smartart_text(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        if !self.pane_has_keys() {
            return false;
        }
        let Some(pane) = self.smartart.pane.as_mut() else {
            return false;
        };
        let Some(index) = pane.focus else {
            return false;
        };
        pane.lines[index].1.insert(text);
        cx.notify();
        true
    }

    /// The selected shape of a diagram, outlined on the slide.
    pub(super) fn smartart_overlay(&mut self, scale: f32) -> Option<AnyElement> {
        let id = self.selected_smartart()?;
        let node = self.smartart.node.clone()?;
        let rect = self.deck.smartart_node_rect(self.current, id, &node)?;
        Some(
            div()
                .absolute()
                .left(px(rect.x as f32 * scale - 2.0))
                .top(px(rect.y as f32 * scale - 2.0))
                .w(px(rect.width as f32 * scale + 4.0))
                .h(px(rect.height as f32 * scale + 4.0))
                .border_2()
                .border_color(theme::accent())
                .into_any_element(),
        )
    }

    /// A second click in the selected diagram selects the shape there.
    pub(super) fn smartart_click(&mut self, x: f64, y: f64, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.selected_smartart() else {
            self.smartart.node = None;
            return false;
        };
        let Some(node) = self.deck.smartart_node_at(self.current, id, x, y) else {
            return false;
        };
        self.smartart.node = Some(node.clone());
        if let Some(pane) = self.smartart.pane.as_mut() {
            let order: Vec<String> = self
                .deck
                .smartart(self.current, id)
                .map(|info| jade_smartart_outline_ids(&info.nodes))
                .unwrap_or_default();
            pane.focus = order.iter().position(|n| *n == node);
        }
        cx.notify();
        true
    }

    fn selected_node(&self) -> Option<String> {
        if let Some(node) = &self.smartart.node {
            return Some(node.clone());
        }
        let id = self.selected_smartart()?;
        let pane = self.smartart.pane.as_ref()?;
        let index = pane.focus?;
        let info = self.deck.smartart(self.current, id)?;
        jade_smartart_outline_ids(&info.nodes).get(index).cloned()
    }

    fn add_shape(&mut self, place: SmartArtPlace, cx: &mut Context<Self>) {
        let Some(id) = self.selected_smartart() else {
            return;
        };
        self.commit_pane(cx);
        let node = self.selected_node();
        match self
            .deck
            .smartart_add_shape(self.current, id, node.as_deref(), place)
        {
            Ok(new) => {
                self.smartart.node = new;
                self.smartart_done(Ok(true), cx);
            }
            Err(error) => self.smartart_done(Err(error), cx),
        }
    }

    fn node_command(
        &mut self,
        cx: &mut Context<Self>,
        command: impl FnOnce(
            &mut jade_office_model::deck::Deck<jade_office_model::skia::SkiaBackend>,
            usize,
            u32,
            &str,
        ) -> Result<bool, String>,
    ) {
        let Some(id) = self.selected_smartart() else {
            return;
        };
        self.commit_pane(cx);
        let Some(node) = self.selected_node() else {
            self.status = SharedString::from(i18n::text("deck.smartart.pick_shape"));
            cx.notify();
            return;
        };
        let result = command(&mut self.deck, self.current, id, &node);
        self.smartart_done(result, cx);
    }

    fn smartart_edit(&mut self, edit: SmartArtEdit, cx: &mut Context<Self>) {
        let Some(id) = self.selected_smartart() else {
            return;
        };
        self.commit_pane(cx);
        let result = self.deck.edit_smartart(self.current, id, &edit);
        self.smartart_done(result, cx);
    }

    fn format_node(&mut self, change: impl FnOnce(&mut SmartArtFormat), cx: &mut Context<Self>) {
        let Some(id) = self.selected_smartart() else {
            return;
        };
        let Some(node) = self.selected_node() else {
            self.status = SharedString::from(i18n::text("deck.smartart.pick_shape"));
            cx.notify();
            return;
        };
        let Some(info) = self.deck.smartart(self.current, id) else {
            return;
        };
        let mut format = jade_smartart_find(&info.nodes, &node)
            .map(|n| n.format.clone())
            .unwrap_or_default();
        change(&mut format);
        let result =
            self.deck
                .edit_smartart(self.current, id, &SmartArtEdit::Format { node, format });
        self.smartart_done(result, cx);
    }

    fn convert_to_shapes(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_smartart() else {
            return;
        };
        self.smartart.pane = None;
        self.smartart.node = None;
        let result = self.deck.convert_smartart_to_shapes(self.current, id);
        self.inserted(result, cx);
    }

    fn pick_node_picture(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(node)) = (self.selected_smartart(), self.selected_node()) else {
            return;
        };
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let Ok(bytes) = std::fs::read(&path) else {
                return;
            };
            view.update(cx, |view, cx| {
                let result = view
                    .deck
                    .set_smartart_picture(view.current, id, &node, &bytes);
                view.smartart_done(result, cx);
            })
            .ok();
        })
        .detach();
    }

    /// 挿入 › SmartArt ▾: the gallery by category.
    pub(super) fn smartart_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let category = match self.smartart.popup {
            Some(SmartArtPopup::Gallery(category)) => Some(category),
            _ => None,
        };
        let menu =
            category.map(|current| {
                let tabs = div().flex().flex_wrap().gap_1().p_1().children(
                    SmartArtCategory::ALL.iter().map(|&category| {
                        let label = if japanese() {
                            category.name_ja()
                        } else {
                            category.name_en()
                        };
                        div()
                            .id(SharedString::from(format!(
                                "smartart-category-{}",
                                category.id()
                            )))
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .cursor_pointer()
                            .when(category == current, |tab| tab.bg(theme::selected()))
                            .hover(|tab| tab.bg(theme::hovered()))
                            .child(label)
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.smartart.popup = Some(SmartArtPopup::Gallery(category));
                                cx.notify();
                            }))
                    }),
                );
                menu_panel("smartart-gallery")
                    .w(px(300.0))
                    .child(tabs)
                    .children(
                        SMARTART_LAYOUTS
                            .iter()
                            .filter(|layout| layout.category == current)
                            .map(|layout| {
                                let id = layout.id();
                                let label = if japanese() {
                                    layout.name_ja
                                } else {
                                    layout.name_en
                                };
                                menu_row(
                                    SharedString::from(format!("smartart-layout-{}", layout.short)),
                                    label,
                                    false,
                                )
                                .on_click(cx.listener(
                                    move |view, _event, _window, cx| view.insert_smartart(&id, cx),
                                ))
                            }),
                    )
                    .into_any_element()
            });
        with_menu(
            toolbar_button(
                "insert-smartart",
                "icons/list-tree.svg",
                i18n::text("deck.smartart"),
            )
            .when(category.is_some(), |button| button.bg(theme::selected()))
            .on_click(cx.listener(|view, _event, _window, cx| {
                view.smartart_popup(SmartArtPopup::Gallery(SmartArtCategory::List), cx)
            }))
            .into_any_element(),
            menu,
        )
    }

    fn list_button(
        &self,
        id: &'static str,
        label: &'static str,
        popup: SmartArtPopup,
        rows: impl FnOnce(&mut Context<Self>) -> Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.smartart.popup == Some(popup);
        let menu = open.then(|| menu_panel(id).children(rows(cx)).into_any_element());
        with_menu(
            bar_button(id, label)
                .child(
                    svg()
                        .path("icons/chevron-down.svg")
                        .size_3()
                        .ml_0p5()
                        .text_color(theme::text_dim()),
                )
                .when(open, |button| button.bg(theme::selected()))
                .on_click(
                    cx.listener(move |view, _event, _window, cx| view.smartart_popup(popup, cx)),
                )
                .into_any_element(),
            menu,
        )
    }

    /// テキストを SmartArt に変換 ▾, on the format bar's row while a text
    /// shape is selected.
    pub(super) fn convert_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.selected.filter(|_| self.more.is_empty())?;
        let shape = self.shape(id)?;
        if shape.kind != "shape" && shape.kind != "text" {
            return None;
        }
        Some(self.list_button(
            "smartart-convert",
            i18n::text("deck.smartart.convert"),
            SmartArtPopup::Convert,
            |cx| {
                SMARTART_LAYOUTS
                    .iter()
                    .filter(|layout| !layout.pictures)
                    .map(|layout| {
                        let id = layout.id();
                        let label = if japanese() {
                            layout.name_ja
                        } else {
                            layout.name_en
                        };
                        menu_row(
                            SharedString::from(format!("smartart-convert-{}", layout.short)),
                            label,
                            false,
                        )
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            view.convert_to_smartart(&id, cx)
                        }))
                        .into_any_element()
                    })
                    .collect()
            },
            cx,
        ))
    }

    /// SmartArt のデザイン and 書式, while a diagram is selected.
    pub(super) fn smartart_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.selected_smartart()?;
        let info = self.deck.smartart(self.current, id)?;
        let node = self.selected_node();
        let layout_short = info
            .layout
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_string();
        let colors_short = info
            .colors
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_string();
        let style_short = info
            .style
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_string();
        let pictures = SMARTART_LAYOUTS
            .iter()
            .any(|layout| layout.short == layout_short && layout.pictures);
        let pane_open = self.smartart.pane.is_some();
        let add = self.list_button(
            "smartart-add-shape",
            i18n::text("deck.smartart.add_shape"),
            SmartArtPopup::AddShape,
            |cx| {
                [
                    (SmartArtPlace::After, "deck.smartart.add_after"),
                    (SmartArtPlace::Before, "deck.smartart.add_before"),
                    (SmartArtPlace::Above, "deck.smartart.add_above"),
                    (SmartArtPlace::Below, "deck.smartart.add_below"),
                    (SmartArtPlace::Assistant, "deck.smartart.add_assistant"),
                ]
                .into_iter()
                .map(|(place, key)| {
                    menu_row(
                        SharedString::from(format!("smartart-add-{place:?}")),
                        i18n::text(key),
                        false,
                    )
                    .on_click(
                        cx.listener(move |view, _event, _window, cx| view.add_shape(place, cx)),
                    )
                    .into_any_element()
                })
                .collect()
            },
            cx,
        );
        let layouts = self.list_button(
            "smartart-layouts",
            i18n::text("deck.smartart.layouts"),
            SmartArtPopup::Layouts,
            |cx| {
                let mut rows = Vec::new();
                for category in SmartArtCategory::ALL {
                    rows.push(
                        div()
                            .px_2()
                            .pt_1()
                            .text_color(theme::text_faint())
                            .child(if japanese() {
                                category.name_ja()
                            } else {
                                category.name_en()
                            })
                            .into_any_element(),
                    );
                    for layout in SMARTART_LAYOUTS.iter().filter(|l| l.category == category) {
                        let id = layout.id();
                        let label = if japanese() {
                            layout.name_ja
                        } else {
                            layout.name_en
                        };
                        rows.push(
                            menu_row(
                                SharedString::from(format!("smartart-relayout-{}", layout.short)),
                                label,
                                layout.short == layout_short,
                            )
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.smartart_edit(SmartArtEdit::Layout(id.clone()), cx)
                            }))
                            .into_any_element(),
                        );
                    }
                }
                rows
            },
            cx,
        );
        let colors = self.list_button(
            "smartart-colors",
            i18n::text("deck.smartart.colors"),
            SmartArtPopup::Colors,
            |cx| {
                SMARTART_COLORS
                    .iter()
                    .map(|scheme| {
                        let id = scheme.id();
                        let label = if japanese() {
                            scheme.name_ja
                        } else {
                            scheme.name_en
                        };
                        menu_row(
                            SharedString::from(format!("smartart-colors-{}", scheme.short)),
                            label,
                            scheme.short == colors_short,
                        )
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            view.smartart_edit(SmartArtEdit::Colors(id.clone()), cx)
                        }))
                        .into_any_element()
                    })
                    .collect()
            },
            cx,
        );
        let styles = self.list_button(
            "smartart-styles",
            i18n::text("deck.smartart.styles"),
            SmartArtPopup::Styles,
            |cx| {
                SMARTART_STYLES
                    .iter()
                    .map(|style| {
                        let id = style.id();
                        let label = if japanese() {
                            style.name_ja
                        } else {
                            style.name_en
                        };
                        menu_row(
                            SharedString::from(format!("smartart-style-{}", style.short)),
                            label,
                            style.short == style_short,
                        )
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            view.smartart_edit(SmartArtEdit::Style(id.clone()), cx)
                        }))
                        .into_any_element()
                    })
                    .collect()
            },
            cx,
        );
        let swatch_rows =
            |prefix: &'static str, fill: bool, cx: &mut Context<Self>| -> Vec<AnyElement> {
                SWATCHES
                    .iter()
                    .map(|(rgb, key)| {
                        let rgb = rgb.to_string();
                        menu_row(
                            SharedString::from(format!("{prefix}-{rgb}")),
                            i18n::text(key),
                            false,
                        )
                        .child(
                            div()
                                .size_3()
                                .border_1()
                                .border_color(theme::border())
                                .bg(gpui::rgb(u32::from_str_radix(&rgb, 16).unwrap_or(0))),
                        )
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            let rgb = rgb.clone();
                            view.format_node(
                                move |format| {
                                    if fill {
                                        format.fill = Some(rgb);
                                    } else {
                                        format.line = Some(rgb);
                                    }
                                },
                                cx,
                            )
                        }))
                        .into_any_element()
                    })
                    .collect()
            };
        let fill = self.list_button(
            "smartart-fill",
            i18n::text("deck.smartart.fill"),
            SmartArtPopup::Fill,
            |cx| swatch_rows("smartart-fill", true, cx),
            cx,
        );
        let outline = self.list_button(
            "smartart-outline",
            i18n::text("deck.smartart.outline"),
            SmartArtPopup::Outline,
            |cx| swatch_rows("smartart-outline", false, cx),
            cx,
        );
        let change_shape = self.list_button(
            "smartart-change-shape",
            i18n::text("deck.smartart.change_shape"),
            SmartArtPopup::ChangeShape,
            |cx| {
                SHAPES
                    .iter()
                    .map(|(preset, key)| {
                        let preset = preset.to_string();
                        menu_row(
                            SharedString::from(format!("smartart-shape-{preset}")),
                            i18n::text(key),
                            false,
                        )
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            let preset = preset.clone();
                            view.format_node(move |format| format.preset = Some(preset), cx)
                        }))
                        .into_any_element()
                    })
                    .collect()
            },
            cx,
        );
        Some(
            div()
                .id("smartart-bar")
                .flex_none()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_0p5()
                .px_3()
                .py_1()
                .bg(theme::panel())
                .border_b_1()
                .border_color(theme::border())
                .child(
                    div()
                        .px_1()
                        .text_xs()
                        .text_color(theme::accent())
                        .child(i18n::text("deck.smartart.design")),
                )
                .child(
                    bar_button("smartart-text-pane", i18n::text("deck.smartart.text_pane"))
                        .when(pane_open, |button| button.bg(theme::selected()))
                        .on_click(cx.listener(|view, _event, _window, cx| view.toggle_pane(cx))),
                )
                .child(add)
                .child(
                    bar_button("smartart-promote", i18n::text("deck.smartart.promote")).on_click(
                        cx.listener(|view, _event, _window, cx| {
                            view.node_command(cx, |deck, slide, id, node| {
                                deck.smartart_promote(slide, id, node)
                            })
                        }),
                    ),
                )
                .child(
                    bar_button("smartart-demote", i18n::text("deck.smartart.demote")).on_click(
                        cx.listener(|view, _event, _window, cx| {
                            view.node_command(cx, |deck, slide, id, node| {
                                deck.smartart_demote(slide, id, node)
                            })
                        }),
                    ),
                )
                .child(
                    bar_button("smartart-move-up", i18n::text("deck.smartart.move_up")).on_click(
                        cx.listener(|view, _event, _window, cx| {
                            view.node_command(cx, |deck, slide, id, node| {
                                deck.smartart_move(slide, id, node, true)
                            })
                        }),
                    ),
                )
                .child(
                    bar_button("smartart-move-down", i18n::text("deck.smartart.move_down"))
                        .on_click(cx.listener(|view, _event, _window, cx| {
                            view.node_command(cx, |deck, slide, id, node| {
                                deck.smartart_move(slide, id, node, false)
                            })
                        })),
                )
                .child(
                    bar_button(
                        "smartart-delete-shape",
                        i18n::text("deck.smartart.delete_shape"),
                    )
                    .on_click(cx.listener(|view, _event, _window, cx| {
                        view.node_command(cx, |deck, slide, id, node| {
                            deck.smartart_delete_shape(slide, id, node)
                        });
                        view.smartart.node = None;
                    })),
                )
                .child(separator())
                .child(layouts)
                .child(colors)
                .child(styles)
                .child(
                    bar_button("smartart-reset", i18n::text("deck.smartart.reset")).on_click(
                        cx.listener(|view, _event, _window, cx| {
                            view.smartart_edit(SmartArtEdit::ResetGraphic, cx)
                        }),
                    ),
                )
                .child(
                    bar_button("smartart-to-shapes", i18n::text("deck.smartart.to_shapes"))
                        .on_click(
                            cx.listener(|view, _event, _window, cx| view.convert_to_shapes(cx)),
                        ),
                )
                .when(node.is_some(), |bar| {
                    bar.child(separator())
                        .child(
                            div()
                                .px_1()
                                .text_xs()
                                .text_color(theme::accent())
                                .child(i18n::text("deck.smartart.format")),
                        )
                        .child(fill)
                        .child(outline)
                        .child(change_shape)
                        .when(pictures, |bar| {
                            bar.child(
                                bar_button("smartart-picture", i18n::text("deck.smartart.picture"))
                                    .on_click(cx.listener(|view, _event, _window, cx| {
                                        view.pick_node_picture(cx)
                                    })),
                            )
                        })
                })
                .into_any_element(),
        )
    }

    /// The テキスト ウィンドウ, beside the slide.
    pub(super) fn smartart_pane(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pane = self.smartart.pane.as_ref()?;
        if self.selected_smartart() != Some(pane.id) {
            return None;
        }
        let rows = pane.lines.iter().enumerate().map(|(index, (level, edit))| {
            let focused = pane.focus == Some(index);
            let text = edit.text.to_string();
            let shown = if text.is_empty() && !focused {
                i18n::text("deck.smartart.placeholder").to_string()
            } else if focused {
                let (before, after) = text.split_at(edit.caret().min(text.len()));
                format!("{before}|{after}")
            } else {
                text
            };
            div()
                .id(SharedString::from(format!("smartart-line-{index}")))
                .flex()
                .items_center()
                .gap_1()
                .pl(px(8.0 + *level as f32 * 16.0))
                .pr_2()
                .py_0p5()
                .rounded_md()
                .text_xs()
                .cursor(gpui::CursorStyle::IBeam)
                .when(focused, |row| row.bg(theme::selected()))
                .when(edit.text.is_empty() && !focused, |row| {
                    row.text_color(theme::text_faint())
                })
                .child("•")
                .child(shown)
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |view, _event, window, cx| {
                        cx.stop_propagation();
                        window.focus(&view.focus);
                        if let Some(pane) = view.smartart.pane.as_mut() {
                            pane.focus = Some(index);
                        }
                        view.smartart.node = None;
                        cx.notify();
                    }),
                )
        });
        Some(
            div()
                .id("smartart-text-pane")
                .absolute()
                .top_2()
                .left_2()
                .w(px(220.0))
                .max_h(px(360.0))
                .occlude()
                .rounded_md()
                .shadow_lg()
                .flex()
                .flex_col()
                .gap_0p5()
                .p_2()
                .bg(theme::panel())
                .border_r_1()
                .border_color(theme::border())
                .overflow_y_scroll()
                .child(
                    div()
                        .pb_1()
                        .text_xs()
                        .text_color(theme::text_dim())
                        .child(i18n::text("deck.smartart.text_pane_title")),
                )
                .children(rows)
                .into_any_element(),
        )
    }
}

fn jade_smartart_outline_ids(nodes: &[jade_office_model::deck::SmartArtNode]) -> Vec<String> {
    fn walk(nodes: &[jade_office_model::deck::SmartArtNode], out: &mut Vec<String>) {
        for node in nodes {
            out.push(node.id.clone());
            walk(&node.children, out);
        }
    }
    let mut out = Vec::new();
    walk(nodes, &mut out);
    out
}

fn jade_smartart_find<'a>(
    nodes: &'a [jade_office_model::deck::SmartArtNode],
    id: &str,
) -> Option<&'a jade_office_model::deck::SmartArtNode> {
    nodes.iter().find_map(|node| {
        if node.id == id {
            Some(node)
        } else {
            jade_smartart_find(&node.children, id)
        }
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use gpui::{Entity, Focusable};
    use jade_office_model::deck::SmartArtEdit;

    use super::super::DeckView;

    fn open(cx: &mut gpui::TestAppContext) -> (Entity<DeckView>, &mut gpui::VisualTestContext) {
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/public/fixtures/sample-deck.pptx"
        ));
        let (view, cx) = cx.add_window_view(|_window, cx| DeckView::open(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    /// 挿入 › SmartArt › 基本プロセス: the diagram selected with its テキスト
    /// ウィンドウ open; typing fills the first bullet, Enter makes the next,
    /// Tab demotes it; the SmartArt のデザイン commands then edit it.
    #[gpui::test]
    fn a_diagram_is_inserted_typed_into_and_restyled(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        view.update(cx, |view, cx| view.insert_smartart("process1", cx));
        cx.run_until_parked();
        let id = view.read_with(cx, |view, _| {
            assert!(view.smartart.pane.is_some(), "the text pane opens");
            view.selected_smartart().expect("the diagram is selected")
        });
        cx.simulate_input("企画");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("範囲");
        cx.simulate_keystrokes("tab");
        view.update(cx, |view, cx| view.toggle_pane(cx));
        cx.run_until_parked();
        let pane = view.read_with(cx, |view, _| {
            view.deck.smartart_text_pane(view.current, id).unwrap()
        });
        assert_eq!(pane[0], (0, "企画".to_string()));
        assert_eq!(pane[1], (1, "範囲".to_string()));
        view.update(cx, |view, cx| {
            view.smartart_edit(SmartArtEdit::Colors("colorful1".into()), cx)
        });
        cx.run_until_parked();
        let colors = view.read_with(cx, |view, _| {
            view.deck.smartart(view.current, id).unwrap().colors
        });
        assert!(colors.ends_with("colorful1"));
        view.update(cx, |view, cx| view.convert_to_shapes(cx));
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            let group = view.selected.expect("the group is selected");
            assert_eq!(view.shape(group).unwrap().kind, "group");
        });
    }
}
