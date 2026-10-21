//! The object tools that need more room than the ribbon: Word's
//! レイアウトの詳細設定 dialog (位置, 文字列の折り返し, サイズ), the
//! オブジェクトの選択と表示 pane (the Selection Pane: every object, front
//! first, to pick, rename, hide or reorder), the 書式設定 pane (fill, line,
//! effects, text box), and 折り返し点の編集 (dragging a tight wrap's points).

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_docx_editor::word_document::FloatingObjectTarget;
use jade_office_model::text::TextEdit;
use jade_office_model::word::group::WordGroups;
use jade_office_model::word::objects::{
    stacking_index, wrap_polygon, AlignTo, Arrange, LayoutSettings, ObjectFormat, ObjectSort,
    Placement, Position, RelativeTo, WordObjectLayout, WordObjects, PX_PER_MM,
};
use jade_office_model::word::picture::{PlacedObject, Wrap, WRAPS};
use jade_office_model::word::picture_format::{
    Dash, Effect, OutlineEdit, ReflectionPreset, ShadowPreset, WordPictureFormat,
};
use jade_office_model::word::shapes::{Fill, Paint, WordShapes};

use crate::i18n::{self, text as t};
use crate::word_objects::target_of;
use crate::{theme, DocumentView};

/// The panels' state.
pub struct Panels {
    pub selection_pane: bool,
    pub format_pane: bool,
    /// 折り返し点の編集: the selected object's wrap points are shown and can
    /// be dragged.
    pub wrap_points: bool,
    /// What 配置 aligns to.
    pub align_to: AlignTo,
    pub dialog: Option<LayoutDialog>,
    /// An object being renamed in the Selection Pane.
    pub renaming: Option<(FloatingObjectTarget, TextEdit)>,
    /// A wrap point being dragged: its index and the polygon so far.
    pub wrap_drag: Option<(usize, Vec<(i64, i64)>)>,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            selection_pane: false,
            format_pane: false,
            wrap_points: false,
            align_to: AlignTo::Margin,
            dialog: None,
            renaming: None,
            wrap_drag: None,
        }
    }
}

/// The dialog's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogTab {
    Position,
    Wrapping,
    Size,
}

/// The dialog's text fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    HorizontalOffset,
    VerticalOffset,
    DistanceTop,
    DistanceBottom,
    DistanceLeft,
    DistanceRight,
    Height,
    Width,
    Rotation,
}

impl Field {
    const ORDER: [Field; 9] = [
        Field::HorizontalOffset,
        Field::VerticalOffset,
        Field::DistanceTop,
        Field::DistanceBottom,
        Field::DistanceLeft,
        Field::DistanceRight,
        Field::Height,
        Field::Width,
        Field::Rotation,
    ];

    fn tab(self) -> DialogTab {
        match self {
            Field::HorizontalOffset | Field::VerticalOffset => DialogTab::Position,
            Field::Height | Field::Width | Field::Rotation => DialogTab::Size,
            _ => DialogTab::Wrapping,
        }
    }
}

/// レイアウトの詳細設定, open on an object.
pub struct LayoutDialog {
    pub target: FloatingObjectTarget,
    pub tab: DialogTab,
    pub wrap: Wrap,
    pub wrap_side: &'static str,
    /// `Some(align)` for 配置, `None` for 右方向の距離 (the offset field).
    pub h_align: Option<String>,
    pub h_relative: RelativeTo,
    pub v_align: Option<String>,
    pub v_relative: RelativeTo,
    pub moves_with_text: bool,
    pub locked: bool,
    pub allow_overlap: bool,
    pub layout_in_cell: bool,
    pub keep_ratio: bool,
    /// The original size, for 縦横比を固定する.
    pub ratio: f64,
    pub fields: Vec<(Field, TextEdit)>,
    pub focus: Option<Field>,
}

impl LayoutDialog {
    fn field(&self, field: Field) -> &str {
        self.fields
            .iter()
            .find(|(candidate, _)| *candidate == field)
            .map(|(_, edit)| edit.text.as_str())
            .unwrap_or("")
    }

    pub fn field_mut(&mut self, field: Field) -> Option<&mut TextEdit> {
        self.fields
            .iter_mut()
            .find(|(candidate, _)| *candidate == field)
            .map(|(_, edit)| edit)
    }

    /// The field typing goes into.
    pub fn focused_mut(&mut self) -> Option<&mut TextEdit> {
        let field = self.focus?;
        self.field_mut(field)
    }

    pub fn focused(&self) -> Option<&TextEdit> {
        let field = self.focus?;
        self.fields
            .iter()
            .find(|(candidate, _)| *candidate == field)
            .map(|(_, edit)| edit)
    }

    fn number(&self, field: Field) -> Option<f64> {
        self.field(field)
            .trim()
            .trim_end_matches("mm")
            .trim_end_matches('°')
            .trim()
            .replace(',', ".")
            .parse()
            .ok()
    }

    /// What OK puts in.
    fn settings(&self) -> LayoutSettings {
        let millimetres = |field| self.number(field).map(|mm| mm * PX_PER_MM);
        let position = |align: &Option<String>, relative: RelativeTo, field| {
            let placement = match align {
                Some(align) => Placement::Align(align.clone()),
                None => Placement::Offset(millimetres(field).unwrap_or(0.0)),
            };
            Position {
                relative_to: relative,
                placement,
            }
        };
        let v_relative = if self.moves_with_text {
            match self.v_relative {
                RelativeTo::Page | RelativeTo::Margin if self.v_align.is_none() => {
                    RelativeTo::Paragraph
                }
                other => other,
            }
        } else {
            match self.v_relative {
                RelativeTo::Paragraph | RelativeTo::Line => RelativeTo::Page,
                other => other,
            }
        };
        LayoutSettings {
            wrap: Some(self.wrap),
            wrap_side: Some(self.wrap_side),
            horizontal: Some(position(
                &self.h_align,
                self.h_relative,
                Field::HorizontalOffset,
            )),
            vertical: Some(position(&self.v_align, v_relative, Field::VerticalOffset)),
            locked: Some(self.locked),
            allow_overlap: Some(self.allow_overlap),
            layout_in_cell: Some(self.layout_in_cell),
            distances: Some([
                millimetres(Field::DistanceTop).unwrap_or(0.0),
                millimetres(Field::DistanceBottom).unwrap_or(0.0),
                millimetres(Field::DistanceLeft).unwrap_or(0.0),
                millimetres(Field::DistanceRight).unwrap_or(0.0),
            ]),
            size: millimetres(Field::Width).zip(millimetres(Field::Height)),
            rotation: self.number(Field::Rotation),
        }
    }
}

fn mm(px: f64) -> String {
    let value = px / PX_PER_MM;
    let text = format!("{value:.1}");
    text.trim_end_matches(".0").to_string()
}

fn relative_label(relative: RelativeTo) -> &'static str {
    t(match relative {
        RelativeTo::Margin => "objects.relative.margin",
        RelativeTo::Page => "objects.relative.page",
        RelativeTo::Column => "objects.relative.column",
        RelativeTo::Character => "objects.relative.character",
        RelativeTo::LeftMargin => "objects.relative.left_margin",
        RelativeTo::RightMargin => "objects.relative.right_margin",
        RelativeTo::InsideMargin => "objects.relative.inside_margin",
        RelativeTo::OutsideMargin => "objects.relative.outside_margin",
        RelativeTo::Paragraph => "objects.relative.paragraph",
        RelativeTo::Line => "objects.relative.line",
        RelativeTo::TopMargin => "objects.relative.top_margin",
        RelativeTo::BottomMargin => "objects.relative.bottom_margin",
    })
}

fn align_label(align: &str) -> &'static str {
    t(match align {
        "left" => "objects.align.left",
        "center" => "objects.align.center",
        "right" => "objects.align.right",
        "top" => "objects.align.top",
        "bottom" => "objects.align.bottom",
        "inside" => "objects.relative.inside",
        _ => "objects.relative.outside",
    })
}

/// A small pill that is either picked or not.
fn choice(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    picked: bool,
    cx: &mut Context<DocumentView>,
    on: impl Fn(&mut DocumentView) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_md()
        .text_xs()
        .cursor_pointer()
        .border_1()
        .border_color(if picked {
            theme::accent()
        } else {
            theme::border()
        })
        .when(picked, |pill| pill.bg(theme::selected()))
        .hover(|pill| pill.bg(theme::hovered()))
        .child(label.into())
        .on_click(cx.listener(move |view, _event, _window, cx| {
            on(view);
            cx.notify();
        }))
        .into_any_element()
}

fn check(
    id: &'static str,
    label: &'static str,
    on: bool,
    cx: &mut Context<DocumentView>,
    toggle: impl Fn(&mut LayoutDialog) + 'static,
) -> AnyElement {
    choice(
        id,
        format!("{} {label}", if on { "☑" } else { "☐" }),
        false,
        cx,
        move |view| {
            if let Some(dialog) = view.objects.panels.dialog.as_mut() {
                toggle(dialog);
            }
        },
    )
}

impl DocumentView {
    /// レイアウトの詳細設定 for the selected object.
    pub(crate) fn open_layout_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(format) = self.selected_format() else {
            return;
        };
        let position = |position: &Option<Position>, default| match position {
            Some(Position {
                relative_to,
                placement: Placement::Align(align),
            }) => (Some(align.clone()), *relative_to, 0.0),
            Some(Position {
                relative_to,
                placement: Placement::Offset(offset),
            }) => (None, *relative_to, *offset),
            None => (None, default, 0.0),
        };
        let (h_align, h_relative, h_offset) = position(&format.horizontal, RelativeTo::Column);
        let (v_align, v_relative, v_offset) = position(&format.vertical, RelativeTo::Paragraph);
        let side = jade_office_model::word::objects::object_run(
            self.document.word_document(),
            format.target,
        )
        .and_then(|run| {
            run.image
                .as_ref()
                .map(|image| image.layout.wrap_text)
                .or_else(|| {
                    run.text_box
                        .as_ref()
                        .map(|text_box| text_box.layout.wrap_text)
                })
        })
        .map(|side| side.as_ooxml())
        .unwrap_or("bothSides");
        let field = |field, value: String| (field, TextEdit::new(value));
        self.objects.panels.dialog = Some(LayoutDialog {
            target: format.target,
            tab: DialogTab::Position,
            wrap: format.wrap,
            wrap_side: side,
            h_align,
            h_relative,
            v_align,
            v_relative,
            moves_with_text: format.moves_with_text,
            locked: format.locked,
            allow_overlap: format.allow_overlap,
            layout_in_cell: format.layout_in_cell,
            keep_ratio: format.sort == ObjectSort::Picture,
            ratio: format.width / format.height.max(1.0),
            fields: vec![
                field(Field::HorizontalOffset, mm(h_offset)),
                field(Field::VerticalOffset, mm(v_offset)),
                field(Field::DistanceTop, mm(format.distances[0])),
                field(Field::DistanceBottom, mm(format.distances[1])),
                field(Field::DistanceLeft, mm(format.distances[2])),
                field(Field::DistanceRight, mm(format.distances[3])),
                field(Field::Height, mm(format.height)),
                field(Field::Width, mm(format.width)),
                field(Field::Rotation, format!("{:.0}", format.rotation)),
            ],
            focus: None,
        });
        cx.notify();
    }

    /// The dialog's OK.
    pub(crate) fn apply_layout_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.objects.panels.dialog.take() else {
            return;
        };
        let target = dialog.target;
        let settings = dialog.settings();
        self.object_edit(cx, |view| view.document.apply_layout(target, settings));
    }

    /// Keys while the dialog is open: Tab to the next field, Enter for OK,
    /// Escape to cancel, and the field's own editing keys.
    pub(crate) fn dialog_key(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        let Some(dialog) = self.objects.panels.dialog.as_mut() else {
            return false;
        };
        match key {
            "escape" => self.objects.panels.dialog = None,
            "enter" | "return" => {
                return {
                    self.apply_layout_dialog(cx);
                    true
                }
            }
            "tab" => {
                let fields: Vec<Field> = Field::ORDER
                    .into_iter()
                    .filter(|field| field.tab() == dialog.tab)
                    .collect();
                let at = dialog
                    .focus
                    .and_then(|focus| fields.iter().position(|field| *field == focus));
                let next = match (at, shift) {
                    (Some(at), false) => (at + 1) % fields.len().max(1),
                    (Some(at), true) => (at + fields.len() - 1) % fields.len().max(1),
                    (None, _) => 0,
                };
                dialog.focus = fields.get(next).copied();
            }
            _ => {
                let Some(field) = dialog.focused_mut() else {
                    return true;
                };
                match key {
                    "backspace" => field.backspace(),
                    "delete" => field.delete(),
                    "left" => field.step(false),
                    "right" => field.step(true),
                    "home" | "up" => field.to_end(false),
                    "end" | "down" => field.to_end(true),
                    _ => return true,
                }
            }
        }
        cx.notify();
        true
    }

    /// The dialog, over the pages while it is open.
    pub(crate) fn layout_dialog_element(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.objects.panels.dialog.as_ref()?;
        let tab = dialog.tab;
        let tabs = [
            (DialogTab::Position, "objects.dialog.position"),
            (DialogTab::Wrapping, "objects.dialog.wrapping"),
            (DialogTab::Size, "objects.dialog.size"),
        ];
        let tab_strip = div()
            .flex()
            .gap_1()
            .children(tabs.into_iter().map(|(which, label)| {
                choice(label, t(label), which == tab, cx, move |view| {
                    if let Some(dialog) = view.objects.panels.dialog.as_mut() {
                        dialog.tab = which;
                        dialog.focus = None;
                    }
                })
            }));
        let body = match tab {
            DialogTab::Position => self.position_tab(cx),
            DialogTab::Wrapping => self.wrapping_tab(cx),
            DialogTab::Size => self.size_tab(cx),
        };
        let buttons = div()
            .flex()
            .justify_end()
            .gap_2()
            .child(choice(
                "dialog-cancel",
                t("format.cancel"),
                false,
                cx,
                |view| {
                    view.objects.panels.dialog = None;
                },
            ))
            .child(
                div()
                    .id("dialog-ok")
                    .px_3()
                    .py_0p5()
                    .rounded_md()
                    .text_xs()
                    .bg(theme::accent())
                    .text_color(gpui::white())
                    .cursor_pointer()
                    .child(t("format.ok"))
                    .on_click(
                        cx.listener(|view, _event, _window, cx| view.apply_layout_dialog(cx)),
                    ),
            );
        Some(
            div()
                .id("layout-dialog")
                .occlude()
                .w(px(520.0))
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .rounded_lg()
                .bg(theme::panel())
                .border_1()
                .border_color(theme::border())
                .shadow_lg()
                .text_sm()
                .text_color(theme::text())
                .child(t("objects.dialog.title"))
                .child(tab_strip)
                .child(body)
                .child(buttons)
                .into_any_element(),
        )
    }

    /// A text field of the dialog, which takes typing when clicked.
    fn dialog_field(
        &self,
        field: Field,
        label: &'static str,
        unit: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dialog = self.objects.panels.dialog.as_ref();
        let focused = dialog.is_some_and(|dialog| dialog.focus == Some(field));
        let text = dialog
            .map(|dialog| dialog.field(field).to_string())
            .unwrap_or_default();
        div()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .child(
                div()
                    .w(px(110.0))
                    .text_color(theme::text_dim())
                    .child(label),
            )
            .child(
                div()
                    .id(SharedString::from(format!("field-{field:?}")))
                    .w(px(90.0))
                    .h(px(22.0))
                    .px_1()
                    .flex()
                    .items_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(if focused {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(theme::window())
                    .cursor_text()
                    .child(if focused { format!("{text}▏") } else { text })
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        if let Some(dialog) = view.objects.panels.dialog.as_mut() {
                            dialog.focus = Some(field);
                            if let Some(edit) = dialog.field_mut(field) {
                                edit.to_end(true);
                            }
                        }
                        cx.notify();
                    })),
            )
            .child(div().text_color(theme::text_faint()).child(unit))
            .into_any_element()
    }

    fn position_tab(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let dialog = self.objects.panels.dialog.as_ref().expect("open");
        let (h_align, h_relative, v_align, v_relative) = (
            dialog.h_align.clone(),
            dialog.h_relative,
            dialog.v_align.clone(),
            dialog.v_relative,
        );
        let (moves, locked, overlap, in_cell) = (
            dialog.moves_with_text,
            dialog.locked,
            dialog.allow_overlap,
            dialog.layout_in_cell,
        );
        let inline = dialog.wrap == Wrap::Inline;
        let axis = |horizontal: bool, cx: &mut Context<Self>| {
            let (align, relative, choices, relatives): (_, _, &[&str], &[RelativeTo]) =
                if horizontal {
                    (
                        &h_align,
                        h_relative,
                        &["left", "center", "right", "inside", "outside"],
                        &RelativeTo::HORIZONTAL,
                    )
                } else {
                    (
                        &v_align,
                        v_relative,
                        &["top", "center", "bottom", "inside", "outside"],
                        &RelativeTo::VERTICAL,
                    )
                };
            let mut column = div().flex().flex_col().gap_1();
            column = column.child(div().text_xs().text_color(theme::text_dim()).child(t(
                if horizontal {
                    "objects.dialog.horizontal"
                } else {
                    "objects.dialog.vertical"
                },
            )));
            let mut modes = div().flex().flex_wrap().gap_1();
            for (index, value) in choices.iter().enumerate() {
                let value = value.to_string();
                let picked = align.as_deref() == Some(value.as_str());
                modes = modes.child(choice(
                    SharedString::from(format!("align-{horizontal}-{index}")),
                    align_label(&value),
                    picked,
                    cx,
                    move |view| {
                        if let Some(dialog) = view.objects.panels.dialog.as_mut() {
                            if horizontal {
                                dialog.h_align = Some(value.clone());
                            } else {
                                dialog.v_align = Some(value.clone());
                            }
                        }
                    },
                ));
            }
            modes = modes.child(choice(
                SharedString::from(format!("offset-{horizontal}")),
                t("objects.dialog.offset"),
                align.is_none(),
                cx,
                move |view| {
                    if let Some(dialog) = view.objects.panels.dialog.as_mut() {
                        if horizontal {
                            dialog.h_align = None;
                        } else {
                            dialog.v_align = None;
                        }
                    }
                },
            ));
            column = column.child(modes);
            if align.is_none() {
                column = column.child(self.dialog_field(
                    if horizontal {
                        Field::HorizontalOffset
                    } else {
                        Field::VerticalOffset
                    },
                    t(if horizontal {
                        "objects.dialog.right_of"
                    } else {
                        "objects.dialog.below"
                    }),
                    "mm",
                    cx,
                ));
            }
            let mut relative_row = div().flex().flex_wrap().gap_1().items_center().child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(t("objects.dialog.relative_to")),
            );
            for (index, candidate) in relatives.iter().enumerate() {
                let candidate = *candidate;
                relative_row = relative_row.child(choice(
                    SharedString::from(format!("relative-{horizontal}-{index}")),
                    relative_label(candidate),
                    candidate == relative,
                    cx,
                    move |view| {
                        if let Some(dialog) = view.objects.panels.dialog.as_mut() {
                            if horizontal {
                                dialog.h_relative = candidate;
                            } else {
                                dialog.v_relative = candidate;
                                dialog.moves_with_text =
                                    matches!(candidate, RelativeTo::Paragraph | RelativeTo::Line);
                            }
                        }
                    },
                ));
            }
            column.child(relative_row).into_any_element()
        };
        let horizontal = axis(true, cx);
        let vertical = axis(false, cx);
        let options = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(check(
                "opt-move",
                t("objects.dialog.move_with_text"),
                moves,
                cx,
                |dialog| {
                    dialog.moves_with_text = !dialog.moves_with_text;
                    dialog.v_relative = if dialog.moves_with_text {
                        RelativeTo::Paragraph
                    } else {
                        RelativeTo::Page
                    };
                },
            ))
            .child(check(
                "opt-lock",
                t("objects.dialog.lock_anchor"),
                locked,
                cx,
                |dialog| dialog.locked = !dialog.locked,
            ))
            .child(check(
                "opt-overlap",
                t("objects.dialog.allow_overlap"),
                overlap,
                cx,
                |dialog| dialog.allow_overlap = !dialog.allow_overlap,
            ))
            .child(check(
                "opt-cell",
                t("objects.dialog.layout_in_cell"),
                in_cell,
                cx,
                |dialog| dialog.layout_in_cell = !dialog.layout_in_cell,
            ));
        div()
            .flex()
            .flex_col()
            .gap_3()
            .when(inline, |body| {
                body.child(
                    div()
                        .text_xs()
                        .text_color(theme::text_faint())
                        .child(t("objects.dialog.inline_note")),
                )
            })
            .child(horizontal)
            .child(vertical)
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(t("objects.dialog.options")),
            )
            .child(options)
            .into_any_element()
    }

    fn wrapping_tab(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let dialog = self.objects.panels.dialog.as_ref().expect("open");
        let (wrap, side) = (dialog.wrap, dialog.wrap_side);
        let styles = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(WRAPS.iter().enumerate().map(|(index, candidate)| {
                let candidate = *candidate;
                choice(
                    ("wrap-style", index),
                    crate::wrap_label(candidate),
                    candidate == wrap,
                    cx,
                    move |view| {
                        if let Some(dialog) = view.objects.panels.dialog.as_mut() {
                            dialog.wrap = candidate;
                        }
                    },
                )
            }));
        let sides = [
            ("bothSides", "objects.dialog.both_sides"),
            ("left", "objects.dialog.left_only"),
            ("right", "objects.dialog.right_only"),
            ("largest", "objects.dialog.largest_only"),
        ];
        let side_row =
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(
                    sides
                        .into_iter()
                        .enumerate()
                        .map(|(index, (value, label))| {
                            choice(
                                ("wrap-side", index),
                                t(label),
                                value == side,
                                cx,
                                move |view| {
                                    if let Some(dialog) = view.objects.panels.dialog.as_mut() {
                                        dialog.wrap_side = value;
                                    }
                                },
                            )
                        }),
                );
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(t("objects.dialog.wrap_style")),
            )
            .child(styles)
            .when(
                matches!(wrap, Wrap::Square | Wrap::Tight | Wrap::Through),
                |body| {
                    body.child(
                        div()
                            .text_xs()
                            .text_color(theme::text_dim())
                            .child(t("objects.dialog.wrap_text")),
                    )
                    .child(side_row)
                },
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(t("objects.dialog.distance")),
            )
            .child(self.dialog_field(Field::DistanceTop, t("objects.dialog.top"), "mm", cx))
            .child(self.dialog_field(Field::DistanceBottom, t("objects.dialog.bottom"), "mm", cx))
            .child(self.dialog_field(Field::DistanceLeft, t("objects.dialog.left"), "mm", cx))
            .child(self.dialog_field(Field::DistanceRight, t("objects.dialog.right"), "mm", cx))
            .into_any_element()
    }

    fn size_tab(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let keep = self
            .objects
            .panels
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.keep_ratio);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.dialog_field(Field::Height, t("objects.size.height"), "mm", cx))
            .child(self.dialog_field(Field::Width, t("objects.size.width"), "mm", cx))
            .child(self.dialog_field(Field::Rotation, t("objects.dialog.rotation"), "°", cx))
            .child(check(
                "keep-ratio",
                t("objects.dialog.lock_ratio"),
                keep,
                cx,
                |dialog| dialog.keep_ratio = !dialog.keep_ratio,
            ))
            .into_any_element()
    }

    /// Typing into the dialog: keeps the proportions while 縦横比を固定する
    /// is on, the way Word's fields follow each other.
    pub(crate) fn dialog_typed(&mut self) {
        let Some(dialog) = self.objects.panels.dialog.as_mut() else {
            return;
        };
        if !dialog.keep_ratio {
            return;
        }
        let ratio = dialog.ratio;
        match dialog.focus {
            Some(Field::Height) => {
                if let Some(height) = dialog.number(Field::Height) {
                    let width = format!("{:.1}", height * ratio);
                    if let Some(edit) = dialog.field_mut(Field::Width) {
                        *edit = TextEdit::new(width.trim_end_matches(".0").to_string());
                    }
                }
            }
            Some(Field::Width) => {
                if let Some(width) = dialog.number(Field::Width) {
                    let height = format!("{:.1}", width / ratio);
                    if let Some(edit) = dialog.field_mut(Field::Height) {
                        *edit = TextEdit::new(height.trim_end_matches(".0").to_string());
                    }
                }
            }
            _ => {}
        }
    }

    // ---- the Selection Pane -------------------------------------------------

    /// オブジェクトの選択と表示: every object, front first.
    pub(crate) fn selection_pane(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let document = self.document.word_document();
        let mut objects = self.document.body_objects();
        // Front first, as Word lists them; in-line ones after.
        objects.sort_by_key(|object| {
            std::cmp::Reverse(
                stacking_index(document, object.target)
                    .map(|index| index + 1)
                    .unwrap_or(0),
            )
        });
        let selected = self.object_target();
        let renaming = self
            .objects
            .panels
            .renaming
            .as_ref()
            .map(|(target, edit)| (*target, edit.text.clone()));
        let rows: Vec<AnyElement> = objects
            .iter()
            .enumerate()
            .map(|(index, object)| {
                self.selection_row(index, object, selected, renaming.as_ref(), cx)
            })
            .collect();
        div()
            .id("selection-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(240.0))
            .h_full()
            .gap_1()
            .px_3()
            .pt_3()
            .overflow_y_scroll()
            .bg(theme::panel())
            .border_l_1()
            .border_color(theme::border())
            .child(div().text_sm().child(t("objects.selection_pane")))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(choice(
                        "show-all",
                        t("objects.show_all"),
                        false,
                        cx,
                        |view| view.set_all_hidden(false),
                    ))
                    .child(choice(
                        "hide-all",
                        t("objects.hide_all"),
                        false,
                        cx,
                        |view| view.set_all_hidden(true),
                    ))
                    .child(choice("pane-forward", "▲", false, cx, |view| {
                        view.pane_arrange(Arrange::Forward)
                    }))
                    .child(choice("pane-backward", "▼", false, cx, |view| {
                        view.pane_arrange(Arrange::Backward)
                    })),
            )
            .children(rows)
            .when(objects.is_empty(), |pane| {
                pane.child(
                    div()
                        .text_xs()
                        .text_color(theme::text_faint())
                        .child(t("objects.no_objects")),
                )
            })
            .into_any_element()
    }

    fn selection_row(
        &mut self,
        index: usize,
        object: &ObjectFormat,
        selected: Option<FloatingObjectTarget>,
        renaming: Option<&(FloatingObjectTarget, String)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let target = object.target;
        let hidden = object.hidden;
        let name = match renaming {
            Some((renamed, text)) if *renamed == target => format!("{text}▏"),
            _ => object.name.clone(),
        };
        div()
            .id(("selection-row", index))
            .flex()
            .items_center()
            .gap_2()
            .px_1()
            .h(px(24.0))
            .rounded_sm()
            .text_xs()
            .when(selected == Some(target), |row| row.bg(theme::selected()))
            .hover(|row| row.bg(theme::hovered()))
            .cursor_pointer()
            .child(
                div()
                    .flex_1()
                    .when(hidden, |name| name.text_color(theme::text_faint()))
                    .child(name),
            )
            .child(
                div()
                    .id(("selection-eye", index))
                    .px_1()
                    .text_color(theme::text_dim())
                    .child(if hidden { "—" } else { "👁" })
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        view.object_edit(cx, |view| {
                            view.document.set_object_hidden(target, !hidden)
                        });
                        cx.stop_propagation();
                    })),
            )
            .on_click(
                cx.listener(move |view, event: &gpui::ClickEvent, _window, cx| {
                    if event.click_count() >= 2 {
                        let name = view
                            .document
                            .object_format(target)
                            .map(|format| format.name)
                            .unwrap_or_default();
                        view.objects.panels.renaming = Some((target, TextEdit::selected(name)));
                    } else {
                        view.objects.panels.renaming = None;
                    }
                    view.select_object(target, cx);
                    if view.selected_object.is_none() {
                        // Hidden: still selectable from the pane, for its tools.
                        view.selected_object = view.document.object_format(target).map(|format| {
                            jade_docx_native_render::ObjectHit {
                                kind: target.kind,
                                paragraph: target.paragraph_index,
                                run: target.run_index,
                                page: 0,
                                x: 0.0,
                                y: 0.0,
                                width: format.width,
                                height: format.height,
                            }
                        });
                    }
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    fn set_all_hidden(&mut self, hidden: bool) {
        let targets: Vec<_> = self
            .document
            .body_objects()
            .into_iter()
            .filter(|object| object.hidden != hidden)
            .map(|object| object.target)
            .collect();
        let mut done = Ok(());
        for target in targets {
            done = done.and_then(|()| self.document.set_object_hidden(target, hidden));
        }
        match done {
            Ok(()) => {
                self.rebuild_after_edit();
                self.refresh_selection();
            }
            Err(error) => self.objects.status = Some(error.into()),
        }
    }

    fn pane_arrange(&mut self, arrange: Arrange) {
        let Some(target) = self.object_target() else {
            return;
        };
        match self.document.arrange_object(target, arrange) {
            Ok(()) => {
                self.rebuild_after_edit();
                self.refresh_selection();
            }
            Err(error) => self.objects.status = Some(error.into()),
        }
    }

    /// Keys while an object is being renamed in the pane.
    pub(crate) fn rename_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some((target, edit)) = self.objects.panels.renaming.as_mut() else {
            return false;
        };
        match key {
            "escape" => self.objects.panels.renaming = None,
            "enter" | "return" => {
                let (target, name) = (*target, edit.text.clone());
                self.objects.panels.renaming = None;
                self.object_edit(cx, |view| view.document.rename_object(target, &name));
            }
            "backspace" => edit.backspace(),
            "delete" => edit.delete(),
            "left" => edit.step(false),
            "right" => edit.step(true),
            _ => return true,
        }
        cx.notify();
        true
    }

    // ---- the format pane ------------------------------------------------

    /// 図形の書式設定 / 図の書式設定: fill, line and effects in one place.
    pub(crate) fn format_pane(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let format = self.selected_format();
        let mut pane = div()
            .id("format-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(260.0))
            .h_full()
            .gap_2()
            .px_3()
            .pt_3()
            .overflow_y_scroll()
            .bg(theme::panel())
            .border_l_1()
            .border_color(theme::border())
            .child(div().text_sm().child(t("objects.format_pane")));
        let Some(format) = format else {
            return pane
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_faint())
                        .child(t("objects.nothing_selected")),
                )
                .into_any_element();
        };
        let target = format.target;
        let shape = !matches!(
            format.sort,
            ObjectSort::Picture | ObjectSort::Chart | ObjectSort::SmartArt | ObjectSort::Group
        );
        let section = |label: &'static str| {
            div()
                .pt_2()
                .text_xs()
                .text_color(theme::text_dim())
                .child(t(label))
                .into_any_element()
        };
        let colors = |id: &'static str,
                      cx: &mut Context<Self>,
                      apply: fn(FloatingObjectTarget, &str) -> PaneEdit| {
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(crate::word_objects_ribbon::COLORS.iter().enumerate().map(
                    |(index, (color, name))| {
                        let value = u32::from_str_radix(color, 16).unwrap_or(0);
                        let edit = apply(target, color);
                        div()
                            .id((id, index))
                            .size(px(18.0))
                            .rounded_sm()
                            .bg(gpui::rgb(value))
                            .border_1()
                            .border_color(theme::border())
                            .cursor_pointer()
                            .tooltip(crate::tooltip::Tooltip::text(t(name)))
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.pane_edit(edit.clone(), cx)
                            }))
                    },
                ))
                .into_any_element()
        };
        if shape {
            pane = pane
                .child(section("objects.fill"))
                .child(colors("pane-fill", cx, |target, color| {
                    PaneEdit::Fill(target, Fill::Paint(Paint::Rgb(color.into())))
                }))
                .child(div().flex().gap_1().children([
                    pane_button(
                        "pane-fill-none",
                        t("objects.fill.none"),
                        PaneEdit::Fill(target, Fill::None),
                        cx,
                    ),
                    pane_button(
                        "pane-fill-50",
                        "50%",
                        PaneEdit::Fill(target, Fill::Transparency(0.5)),
                        cx,
                    ),
                    pane_button(
                        "pane-fill-0",
                        "0%",
                        PaneEdit::Fill(target, Fill::Transparency(0.0)),
                        cx,
                    ),
                ]));
        }
        pane = pane
            .child(section("objects.outline"))
            .child(colors("pane-line", cx, |target, color| {
                PaneEdit::Outline(target, OutlineEdit::Color(color.into()))
            }))
            .child(div().flex().flex_wrap().gap_1().children([
                pane_button(
                    "pane-line-none",
                    t("objects.outline.none"),
                    PaneEdit::Outline(target, OutlineEdit::None),
                    cx,
                ),
                pane_button(
                    "pane-line-05",
                    "0.5 pt",
                    PaneEdit::Outline(target, OutlineEdit::Width(0.5)),
                    cx,
                ),
                pane_button(
                    "pane-line-1",
                    "1 pt",
                    PaneEdit::Outline(target, OutlineEdit::Width(1.0)),
                    cx,
                ),
                pane_button(
                    "pane-line-3",
                    "3 pt",
                    PaneEdit::Outline(target, OutlineEdit::Width(3.0)),
                    cx,
                ),
                pane_button(
                    "pane-line-solid",
                    t("objects.dash.solid"),
                    PaneEdit::Outline(target, OutlineEdit::Dash(Dash::Solid)),
                    cx,
                ),
                pane_button(
                    "pane-line-dash",
                    t("objects.dash.dash"),
                    PaneEdit::Outline(target, OutlineEdit::Dash(Dash::Dash)),
                    cx,
                ),
            ]))
            .child(section("objects.effects"))
            .child(div().flex().flex_wrap().gap_1().children([
                pane_button(
                    "pane-shadow",
                    t("objects.effect.shadow"),
                    PaneEdit::Effect(target, Effect::Shadow(Some(ShadowPreset::OUTER[0]))),
                    cx,
                ),
                pane_button(
                    "pane-shadow-none",
                    t("objects.effect.no_shadow"),
                    PaneEdit::Effect(target, Effect::Shadow(None)),
                    cx,
                ),
                pane_button(
                    "pane-reflection",
                    t("objects.effect.reflection"),
                    PaneEdit::Effect(target, Effect::Reflection(Some(ReflectionPreset::Half))),
                    cx,
                ),
                pane_button(
                    "pane-reflection-none",
                    t("objects.effect.no_reflection"),
                    PaneEdit::Effect(target, Effect::Reflection(None)),
                    cx,
                ),
                pane_button(
                    "pane-glow",
                    t("objects.effect.glow"),
                    PaneEdit::Effect(target, Effect::Glow(Some((8.0, "4472C4".into())))),
                    cx,
                ),
                pane_button(
                    "pane-glow-none",
                    t("objects.effect.no_glow"),
                    PaneEdit::Effect(target, Effect::Glow(None)),
                    cx,
                ),
                pane_button(
                    "pane-soft",
                    t("objects.effect.soft_edges"),
                    PaneEdit::Effect(target, Effect::SoftEdge(Some(5.0))),
                    cx,
                ),
                pane_button(
                    "pane-soft-none",
                    t("objects.effect.no_soft_edges"),
                    PaneEdit::Effect(target, Effect::SoftEdge(None)),
                    cx,
                ),
                pane_button(
                    "pane-bevel",
                    t("objects.effect.bevel"),
                    PaneEdit::Effect(target, Effect::Bevel(Some("circle".into()))),
                    cx,
                ),
                pane_button(
                    "pane-bevel-none",
                    t("objects.effect.no_bevel"),
                    PaneEdit::Effect(target, Effect::Bevel(None)),
                    cx,
                ),
            ]))
            .child(section("objects.dialog.size"))
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(i18n::format(
                        "objects.size.summary",
                        &[
                            ("w", &format!("{:.1}", format.width / PX_PER_MM)),
                            ("h", &format!("{:.1}", format.height / PX_PER_MM)),
                            ("r", &format!("{:.0}", format.rotation)),
                        ],
                    )),
            )
            .child(pane_button(
                "pane-layout",
                t("objects.layout_more"),
                PaneEdit::Dialog,
                cx,
            ));
        pane.into_any_element()
    }

    fn pane_edit(&mut self, edit: PaneEdit, cx: &mut Context<Self>) {
        match edit {
            PaneEdit::Fill(target, fill) => {
                self.object_edit(cx, |view| view.document.set_shape_fill(target, fill))
            }
            PaneEdit::Outline(target, outline) => self.object_edit(cx, |view| {
                view.document.set_picture_outline(target, outline)
            }),
            PaneEdit::Effect(target, effect) => {
                self.object_edit(cx, |view| view.document.set_object_effect(target, effect))
            }
            PaneEdit::Dialog => self.open_layout_dialog(cx),
        }
    }

    // ---- grouping -------------------------------------------------------

    pub(crate) fn group_selected(&mut self, objects: Vec<PlacedObject>) -> Result<(), String> {
        self.document.group_objects(&objects)?;
        self.objects.extra.clear();
        // The group is where the first object was.
        let first = objects
            .iter()
            .min_by_key(|object| (object.target.paragraph_index, object.target.run_index))
            .copied();
        self.selected_object = first.and_then(|object| {
            let target = FloatingObjectTarget {
                kind: jade_docx_editor::word_document::FloatingObjectKind::Image,
                ..object.target
            };
            self.document
                .object_format(target)
                .map(|_| jade_docx_native_render::ObjectHit {
                    kind: target.kind,
                    paragraph: target.paragraph_index,
                    run: target.run_index,
                    page: object.page + 1,
                    x: object.x,
                    y: object.y,
                    width: object.width,
                    height: object.height,
                })
        });
        Ok(())
    }

    pub(crate) fn ungroup_selected(&mut self) -> Result<(), String> {
        let object = self.selected_object.ok_or("nothing is selected")?;
        let placed = crate::word_objects::placed(&object);
        self.document.ungroup_object(placed)?;
        self.selected_object = None;
        Ok(())
    }

    // ---- wrap points ----------------------------------------------------

    /// The selected object's wrap points, on its page, while 折り返し点の
    /// 編集 is on.
    pub(crate) fn wrap_point_overlay(&self, page_index: usize) -> Vec<AnyElement> {
        if !self.objects.panels.wrap_points {
            return Vec::new();
        }
        let Some(object) = self
            .selected_object
            .filter(|o| o.page.saturating_sub(1) == page_index)
        else {
            return Vec::new();
        };
        let points = self.wrap_points_on_page(&object);
        let zoom = f64::from(self.zoom);
        points
            .into_iter()
            .map(|(x, y)| {
                div()
                    .absolute()
                    .left(px((x * zoom - 4.0) as f32))
                    .top(px((y * zoom - 4.0) as f32))
                    .size(px(8.0))
                    .bg(gpui::black())
                    .border_1()
                    .border_color(gpui::red())
                    .into_any_element()
            })
            .collect()
    }

    /// The wrap polygon's points on the page: the written polygon, or the
    /// object's box when there is none yet.
    fn wrap_points_on_page(&self, object: &jade_docx_native_render::ObjectHit) -> Vec<(f64, f64)> {
        let polygon = self
            .objects
            .panels
            .wrap_drag
            .as_ref()
            .map(|(_, points)| points.clone())
            .or_else(|| wrap_polygon(self.document.word_document(), target_of(object)))
            .unwrap_or_else(|| vec![(0, 0), (0, 21600), (21600, 21600), (21600, 0), (0, 0)]);
        let mut points: Vec<(f64, f64)> = polygon
            .iter()
            .map(|(x, y)| {
                (
                    object.x + object.width * (*x as f64 / 21600.0),
                    object.y + object.height * (*y as f64 / 21600.0),
                )
            })
            .collect();
        // The closing point repeats the start.
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        points
    }

    /// A press on a wrap point starts dragging it.
    pub(crate) fn press_wrap_point(&mut self, page_index: usize, (x, y): (f64, f64)) -> bool {
        if !self.objects.panels.wrap_points {
            return false;
        }
        let Some(object) = self
            .selected_object
            .filter(|o| o.page.saturating_sub(1) == page_index)
        else {
            return false;
        };
        let points = self.wrap_points_on_page(&object);
        let Some(index) = points
            .iter()
            .position(|(px_, py)| (px_ - x).abs() <= 5.0 && (py - y).abs() <= 5.0)
        else {
            return false;
        };
        let polygon = points
            .iter()
            .map(|(px_, py)| {
                (
                    ((px_ - object.x) / object.width.max(1.0) * 21600.0).round() as i64,
                    ((py - object.y) / object.height.max(1.0) * 21600.0).round() as i64,
                )
            })
            .collect();
        self.objects.panels.wrap_drag = Some((index, polygon));
        true
    }

    /// A wrap point being dragged follows the pointer.
    pub(crate) fn drag_wrap_point(&mut self, (x, y): (f64, f64)) -> bool {
        let Some(object) = self.selected_object else {
            return false;
        };
        let Some((index, points)) = self.objects.panels.wrap_drag.as_mut() else {
            return false;
        };
        if let Some(point) = points.get_mut(*index) {
            *point = (
                ((x - object.x) / object.width.max(1.0) * 21600.0).round() as i64,
                ((y - object.y) / object.height.max(1.0) * 21600.0).round() as i64,
            );
        }
        true
    }

    /// The drag's end writes the polygon.
    pub(crate) fn release_wrap_point(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((_, points)) = self.objects.panels.wrap_drag.take() else {
            return false;
        };
        let Some(target) = self.object_target() else {
            return true;
        };
        self.object_edit(cx, |view| view.document.set_wrap_polygon(target, points));
        true
    }
}

/// What a click in the format pane does.
#[derive(Clone, Debug)]
enum PaneEdit {
    Fill(FloatingObjectTarget, Fill),
    Outline(FloatingObjectTarget, OutlineEdit),
    Effect(FloatingObjectTarget, Effect),
    Dialog,
}

fn pane_button(
    id: &'static str,
    label: &'static str,
    edit: PaneEdit,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_md()
        .text_xs()
        .border_1()
        .border_color(theme::border())
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .child(label)
        .on_click(cx.listener(move |view, _event, _window, cx| view.pane_edit(edit.clone(), cx)))
        .into_any_element()
}

/// The object tools' entry points into the view: the field typing goes
/// into, the keys they take, their panes and overlays.
impl DocumentView {
    /// The text field one of the object tools has focused, if any.
    pub(crate) fn object_field(&self) -> Option<&TextEdit> {
        if let Some(field) = self
            .objects
            .panels
            .dialog
            .as_ref()
            .and_then(|dialog| dialog.focused())
        {
            return Some(field);
        }
        if let Some((_, field)) = self.objects.panels.renaming.as_ref() {
            return Some(field);
        }
        self.objects.diagrams.focused()
    }

    pub(crate) fn object_field_mut(&mut self) -> Option<&mut TextEdit> {
        if self
            .objects
            .panels
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.focus.is_some())
        {
            return self
                .objects
                .panels
                .dialog
                .as_mut()
                .and_then(|dialog| dialog.focused_mut());
        }
        if self.objects.panels.renaming.is_some() {
            return self
                .objects
                .panels
                .renaming
                .as_mut()
                .map(|(_, field)| field);
        }
        self.objects.diagrams.focused_mut()
    }

    /// A key for the object tools: the dialog's, a rename's, the text pane's
    /// or the chart sheet's field, then the selected object.
    pub(crate) fn object_tools_key(
        &mut self,
        key: &str,
        shift: bool,
        alt: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        (self.objects.panels.dialog.is_some() && self.dialog_key(key, shift, cx))
            || self.rename_key(key, cx)
            || self.chart_sheet_key(key, shift, cx)
            || self.text_pane_key(key, shift, cx)
            || self.object_key(key, alt, cx)
    }

    /// The object tools' panes beside the pages.
    pub(crate) fn object_panes(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut panes = Vec::new();
        if self.objects.panels.selection_pane {
            panes.push(self.selection_pane(cx));
        }
        if self.objects.panels.format_pane {
            panes.push(self.format_pane(cx));
        }
        panes.extend(self.text_pane_element(cx));
        panes
    }

    /// The object tools' dialogs, over everything.
    pub(crate) fn object_dialogs(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut dialogs = Vec::new();
        dialogs.extend(self.layout_dialog_element(cx));
        dialogs.extend(self.chart_sheet_element(cx));
        dialogs
            .into_iter()
            .map(|dialog| {
                gpui::deferred(
                    gpui::anchored()
                        .position(gpui::point(px(260.0), px(140.0)))
                        .child(dialog),
                )
                .with_priority(3)
                .into_any_element()
            })
            .collect()
    }
}
