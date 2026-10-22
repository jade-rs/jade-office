//! The drawing-object tools the browser shell had not yet, from the native
//! app's `word_objects`, `word_objects_panels` and `word_objects_ribbon`:
//!
//! - トリミング: the selected picture's crop handles, a drag cutting into
//!   it (or back out) while the picture stays where it is;
//! - テキスト ボックスのリンクの作成: with a text box selected, the next
//!   empty one clicked takes its overflow;
//! - 折り返し点の編集: the wrap polygon's points shown and dragged;
//! - オブジェクトの選択と表示 (the Selection Pane): every object, front
//!   first, to select, show or hide, rename (double-click) and reorder.
//!
//! The commands are `jade-office-model`'s, which the native app runs too.

use gpui::{div, prelude::*, px, AnyElement, Context, MouseMoveEvent, SharedString};
use jade_web::text::TextEdit;
use jade_web::word::handles::{crop_drag, Handle};
use jade_web::word::objects::{stacking_index, wrap_polygon, Arrange, ObjectFormat, WordObjects};
use jade_web::word::picture::{ObjectKind, ObjectTarget, PlacedObject};
use jade_web::word::WordSession;

use super::word_kit::on;
use super::{DocView, Document};
use crate::i18n::text as t;
use crate::palette as theme;

/// The modes and panes the object tools turn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectUi {
    /// トリミング on and off.
    ToggleCrop,
    /// リンクの作成 from the selected text box.
    LinkTextBox,
    /// 折り返し点の編集 on and off.
    EditWrapPoints,
    /// The Selection Pane open and closed.
    SelectionPane,
}

/// What the ribbon shows as on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectModes {
    pub cropping: bool,
    pub linking: bool,
    pub wrap_points: bool,
    pub selection_pane: bool,
    /// Whether the selected object wraps tight or through, so has wrap
    /// points to edit.
    pub wrap_editable: bool,
}

/// A crop handle being dragged.
#[derive(Clone, Copy, Debug)]
struct CropDrag {
    object: PlacedObject,
    handle: Handle,
    from: (f64, f64),
    crop: jade_web::word::handles::ImageCrop,
    /// How far the handle has been dragged.
    delta: (f64, f64),
}

/// The tools' state, kept with the document view.
#[derive(Default)]
pub struct ObjectsUi {
    cropping: bool,
    crop_drag: Option<CropDrag>,
    linking: Option<ObjectTarget>,
    wrap_points: bool,
    /// A wrap point being dragged: its index and the polygon so far.
    wrap_drag: Option<(usize, Vec<(i64, i64)>)>,
    selection_pane: bool,
    /// An object being renamed in the Selection Pane.
    renaming: Option<(ObjectTarget, TextEdit)>,
}

impl ObjectsUi {
    pub fn modes(&self) -> ObjectModes {
        ObjectModes {
            cropping: self.cropping,
            linking: self.linking.is_some(),
            wrap_points: self.wrap_points,
            selection_pane: self.selection_pane,
            wrap_editable: false,
        }
    }

    /// The Selection Pane's rename field, while one is being typed.
    pub fn text_field(&self) -> Option<&TextEdit> {
        self.renaming.as_ref().map(|(_, edit)| edit)
    }

    pub fn text_field_mut(&mut self) -> Option<&mut TextEdit> {
        self.renaming.as_mut().map(|(_, edit)| edit)
    }
}

/// The wrap polygon's coordinate space: 21600 across and down the box.
const WRAP_SPACE: f64 = 21600.0;
const HANDLE: f64 = 6.0;

fn near(a: (f64, f64), b: (f64, f64), reach: f64) -> bool {
    (a.0 - b.0).abs() <= reach && (a.1 - b.1).abs() <= reach
}

impl DocView {
    /// What the ribbon shows of the object tools.
    pub(crate) fn object_modes(&self) -> ObjectModes {
        use jade_web::word::picture::Wrap;
        let wrap_editable = self
            .selected_placed()
            .and_then(|object| self.doc().object_format(object.target))
            .is_some_and(|format| matches!(format.wrap, Wrap::Tight | Wrap::Through));
        ObjectModes {
            wrap_editable,
            ..self.objects_ui.modes()
        }
    }

    /// The picture or shape selected on the pages.
    fn selected_placed(&self) -> Option<PlacedObject> {
        match &self.document {
            Document::Pages { object, .. } => object.map(|grab| grab.object),
            _ => None,
        }
    }

    /// Selects `target` as a click on it would; a hidden one is still
    /// selected for its tools, with no box on a page.
    fn select_target(&mut self, target: ObjectTarget) {
        let Document::Pages {
            document, object, ..
        } = &mut self.document
        else {
            return;
        };
        // A hidden object is laid out with no box: selected for its tools,
        // with nothing drawn on a page.
        let found = document
            .object_region(target)
            .filter(|found| found.width >= 1.0 && found.height >= 1.0)
            .or_else(|| {
                document.object_format(target).map(|format| PlacedObject {
                    target,
                    page: usize::MAX,
                    x: 0.0,
                    y: 0.0,
                    width: format.width,
                    height: format.height,
                })
            });
        *object = found.map(|found| super::ObjectGrab {
            object: found,
            from: (0.0, 0.0),
            rect: (found.x, found.y, found.width, found.height),
            resizing: false,
            dragging: false,
        });
    }

    /// After an object edit: the selection's box as it is now drawn.
    pub(super) fn refresh_selected_object(&mut self) {
        let Some(selected) = self.selected_placed() else {
            return;
        };
        self.select_target(selected.target);
    }

    fn object_ui_edit(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut jade_web::editing::WordDoc) -> Result<(), String>,
    ) {
        self.word_edit(cx, edit);
        self.refresh_selected_object();
    }

    /// Runs a mode or pane button.
    pub(crate) fn object_ui_action(&mut self, action: ObjectUi, cx: &mut Context<Self>) {
        let ui = &mut self.objects_ui;
        match action {
            ObjectUi::ToggleCrop => {
                ui.cropping = !ui.cropping;
            }
            ObjectUi::EditWrapPoints => ui.wrap_points = !ui.wrap_points,
            ObjectUi::SelectionPane => ui.selection_pane = !ui.selection_pane,
            ObjectUi::LinkTextBox => {
                let selected = self.selected_placed().map(|object| object.target);
                let ui = &mut self.objects_ui;
                ui.linking = match ui.linking {
                    Some(_) => None,
                    None => selected.filter(|target| target.kind == ObjectKind::TextBox),
                };
                self.status = match (ui.linking, selected) {
                    (Some(_), _) => t("objects.link_hint").into(),
                    (None, Some(_)) => SharedString::default(),
                    (None, None) => t("objects.nothing_selected").into(),
                };
            }
        }
        cx.notify();
    }

    /// A press on page `index` at `point` (page px) for the object tools:
    /// a crop handle or a wrap point is taken, or the
    /// text box clicked is linked. Returns whether it was taken.
    pub(super) fn object_ui_press(
        &mut self,
        index: usize,
        point: (f64, f64),
        document_y: f64,
        cx: &mut Context<Self>,
    ) -> bool {
        let selected = self.selected_placed().filter(|object| object.page == index);
        if let (true, Some(object)) = (self.objects_ui.wrap_points, selected) {
            let points = self.wrap_points_on_page(&object);
            if let Some(at) = points.iter().position(|p| near(*p, point, 5.0)) {
                let polygon = points
                    .iter()
                    .map(|(x, y)| to_wrap_space(&object, (*x, *y)))
                    .collect();
                self.objects_ui.wrap_drag = Some((at, polygon));
                return true;
            }
        }
        if let (true, Some(object)) = (self.objects_ui.cropping, selected) {
            let handle = Handle::ALL.into_iter().find(|handle| {
                let (fx, fy) = handle.anchor();
                near(
                    (object.x + object.width * fx, object.y + object.height * fy),
                    point,
                    HANDLE + 2.0,
                )
            });
            if let Some(handle) = handle {
                let crop =
                    jade_web::word::objects::object_run(self.doc().word_document(), object.target)
                        .and_then(|run| run.image.as_ref())
                        .and_then(|image| image.crop)
                        .unwrap_or_default();
                self.objects_ui.crop_drag = Some(CropDrag {
                    object,
                    handle,
                    from: point,
                    crop,
                    delta: (0.0, 0.0),
                });
                return true;
            }
        }
        if let Some(from) = self.objects_ui.linking {
            let Document::Pages { document, .. } = &self.document else {
                return false;
            };
            let clicked = document
                .object_at(point.0, document_y)
                .filter(|found| found.target.kind == ObjectKind::TextBox && found.target != from);
            if let Some(clicked) = clicked {
                self.objects_ui.linking = None;
                let to = clicked.target;
                self.object_ui_edit(cx, move |document| {
                    use jade_web::word::shapes::WordShapes;
                    document.link_text_boxes(from, to)
                });
                return true;
            }
        }
        false
    }

    /// A drag for the object tools, in page px of the page pressed on.
    /// Returns whether it was taken.
    pub(super) fn object_ui_drag(
        &mut self,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let page = match (&self.objects_ui.crop_drag, &self.objects_ui.wrap_drag) {
            (Some(drag), _) => drag.object.page,
            (_, Some(_)) => match self.selected_placed() {
                Some(object) => object.page,
                None => return false,
            },
            _ => return false,
        };
        let Some((x, document_y)) = self.page_point(page, event.position) else {
            return true;
        };
        let y = document_y - self.doc().document_y(page, 0.0);
        if let Some(drag) = self.objects_ui.crop_drag.as_mut() {
            drag.delta = (x - drag.from.0, y - drag.from.1);
        } else if let Some(object) = self.selected_placed() {
            if let Some((index, points)) = self.objects_ui.wrap_drag.as_mut() {
                if let Some(point) = points.get_mut(*index) {
                    *point = to_wrap_space(&object, (x, y));
                }
            }
        }
        cx.notify();
        true
    }

    /// The mouse coming up: the crop or the wrap polygon is written. Returns whether it was the tools'.
    pub(super) fn object_ui_release(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(drag) = self.objects_ui.crop_drag.take() {
            let start = (
                drag.object.x,
                drag.object.y,
                drag.object.width,
                drag.object.height,
            );
            if drag.delta == (0.0, 0.0) {
                return true;
            }
            let (crop, rect) = crop_drag(start, drag.crop, drag.handle, drag.delta);
            let target = drag.object.target;
            self.object_ui_edit(cx, move |document| {
                use jade_web::word::picture_format::WordPictureFormat;
                document.crop_picture_in_place(target, crop, rect)
            });
            return true;
        }
        if let Some((_, points)) = self.objects_ui.wrap_drag.take() {
            let Some(object) = self.selected_placed() else {
                return true;
            };
            let target = object.target;
            self.object_ui_edit(cx, move |document| {
                use jade_web::word::objects::WordObjectLayout;
                document.set_wrap_polygon(target, points)
            });
            return true;
        }
        false
    }

    /// Escape leaves トリミング, a link or a rename.
    pub(super) fn object_ui_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if let Some((target, edit)) = self.objects_ui.renaming.as_mut() {
            if edit.marked.is_some() {
                return false;
            }
            match key {
                "escape" => self.objects_ui.renaming = None,
                "enter" | "return" => {
                    let (target, name) = (*target, edit.text.clone());
                    self.objects_ui.renaming = None;
                    self.object_ui_edit(cx, move |document| document.rename_object(target, &name));
                    return true;
                }
                "backspace" => edit.backspace(),
                "delete" => edit.delete(),
                "left" => edit.step(false),
                "right" => edit.step(true),
                // Typing reaches the field through the input handler.
                _ => return false,
            }
            cx.notify();
            return true;
        }
        if key != "escape" {
            return false;
        }
        let ui = &mut self.objects_ui;
        if ui.linking.take().is_some() {
        } else if ui.cropping {
            ui.cropping = false;
        } else if ui.wrap_points {
            ui.wrap_points = false;
        } else {
            return false;
        }
        cx.notify();
        true
    }

    /// The wrap polygon's points on the page: the written polygon, or the
    /// object's box when there is none yet.
    fn wrap_points_on_page(&self, object: &PlacedObject) -> Vec<(f64, f64)> {
        let polygon = self
            .objects_ui
            .wrap_drag
            .as_ref()
            .map(|(_, points)| points.clone())
            .or_else(|| wrap_polygon(self.doc().word_document(), object.target))
            .unwrap_or_else(|| vec![(0, 0), (0, 21600), (21600, 21600), (21600, 0), (0, 0)]);
        let mut points: Vec<(f64, f64)> = polygon
            .iter()
            .map(|(x, y)| {
                (
                    object.x + object.width * (*x as f64 / WRAP_SPACE),
                    object.y + object.height * (*y as f64 / WRAP_SPACE),
                )
            })
            .collect();
        // The closing point repeats the start.
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        points
    }

    /// What the tools draw over page `index`: crop handles and the box a
    /// crop drag gives, the wrap points.
    pub(super) fn object_ui_overlay(&self, index: usize, zoom: f32) -> Vec<AnyElement> {
        let zoom = f64::from(zoom);
        let mut elements = Vec::new();
        let dot = |(x, y): (f64, f64), size: f64, color: gpui::Hsla| {
            div()
                .absolute()
                .left(px((x * zoom - size / 2.0) as f32))
                .top(px((y * zoom - size / 2.0) as f32))
                .size(px(size as f32))
                .bg(color)
                .into_any_element()
        };
        let selected = self.selected_placed().filter(|object| object.page == index);
        if let (true, Some(object)) = (self.objects_ui.cropping, selected) {
            let start = (object.x, object.y, object.width, object.height);
            let (x, y, width, height) = match self.objects_ui.crop_drag {
                Some(drag) => crop_drag(start, drag.crop, drag.handle, drag.delta).1,
                None => start,
            };
            elements.push(
                div()
                    .absolute()
                    .left(px((x * zoom) as f32))
                    .top(px((y * zoom) as f32))
                    .w(px((width * zoom) as f32))
                    .h(px((height * zoom) as f32))
                    .border_1()
                    .border_dashed()
                    .border_color(theme::accent())
                    .into_any_element(),
            );
            // Crop handles: dark bars at the corners and edges.
            for handle in Handle::ALL {
                let (fx, fy) = handle.anchor();
                elements.push(dot(
                    (object.x + object.width * fx, object.y + object.height * fy),
                    10.0,
                    gpui::black(),
                ));
            }
        }
        if let (true, Some(object)) = (self.objects_ui.wrap_points, selected) {
            elements.extend(
                self.wrap_points_on_page(&object)
                    .into_iter()
                    .map(|point| dot(point, 8.0, gpui::red())),
            );
        }
        elements
    }

    // ---- the Selection Pane ---------------------------------------------

    /// オブジェクトの選択と表示: every object, front first.
    pub(super) fn selection_pane(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.objects_ui.selection_pane {
            return None;
        }
        let Document::Pages { document, .. } = &self.document else {
            return None;
        };
        let word = document.word_document();
        let mut objects = document.body_objects();
        // Front first, as Word lists them; in-line ones after.
        objects.sort_by_key(|object| {
            std::cmp::Reverse(
                stacking_index(word, object.target)
                    .map(|index| index + 1)
                    .unwrap_or(0),
            )
        });
        let selected = self.selected_placed().map(|object| object.target);
        let renaming = self.objects_ui.renaming.as_ref().map(|(target, edit)| {
            (
                *target,
                TextEdit {
                    text: edit.text.clone(),
                    marked: edit.marked.clone(),
                    caret: edit.caret,
                    whole: edit.whole,
                },
            )
        });
        let rows: Vec<AnyElement> = objects
            .iter()
            .enumerate()
            .map(|(index, object)| selection_row(index, object, selected, renaming.as_ref(), cx))
            .collect();
        let button = |id: &'static str,
                      label: SharedString,
                      run: fn(&mut DocView, &mut Context<DocView>),
                      cx: &mut Context<DocView>| {
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .rounded_sm()
                .border_1()
                .border_color(theme::border())
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .child(label)
                .on_click(on(cx, run))
        };
        Some(
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
                .text_xs()
                .child(div().text_sm().child(t("objects.selection_pane")))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .child(button(
                            "show-all",
                            t("objects.show_all").into(),
                            |view, cx| view.set_all_hidden(false, cx),
                            cx,
                        ))
                        .child(button(
                            "hide-all",
                            t("objects.hide_all").into(),
                            |view, cx| view.set_all_hidden(true, cx),
                            cx,
                        ))
                        .child(button(
                            "pane-forward",
                            "▲".into(),
                            |view, cx| view.pane_arrange(Arrange::Forward, cx),
                            cx,
                        ))
                        .child(button(
                            "pane-backward",
                            "▼".into(),
                            |view, cx| view.pane_arrange(Arrange::Backward, cx),
                            cx,
                        )),
                )
                .children(rows)
                .when(objects.is_empty(), |pane| {
                    pane.child(
                        div()
                            .text_color(theme::text_faint())
                            .child(t("objects.no_objects")),
                    )
                })
                .into_any_element(),
        )
    }

    fn set_all_hidden(&mut self, hidden: bool, cx: &mut Context<Self>) {
        let targets: Vec<_> = self
            .doc()
            .body_objects()
            .into_iter()
            .filter(|object| object.hidden != hidden)
            .map(|object| object.target)
            .collect();
        self.object_ui_edit(cx, move |document| {
            for target in targets {
                document.set_object_hidden(target, hidden)?;
            }
            Ok(())
        });
    }

    fn pane_arrange(&mut self, arrange: Arrange, cx: &mut Context<Self>) {
        let Some(target) = self.selected_placed().map(|object| object.target) else {
            return;
        };
        self.object_ui_edit(cx, move |document| document.arrange_object(target, arrange));
    }
}

fn to_wrap_space(object: &PlacedObject, (x, y): (f64, f64)) -> (i64, i64) {
    (
        ((x - object.x) / object.width.max(1.0) * WRAP_SPACE).round() as i64,
        ((y - object.y) / object.height.max(1.0) * WRAP_SPACE).round() as i64,
    )
}

fn selection_row(
    index: usize,
    object: &ObjectFormat,
    selected: Option<ObjectTarget>,
    renaming: Option<&(ObjectTarget, TextEdit)>,
    cx: &mut Context<DocView>,
) -> AnyElement {
    let target = object.target;
    let hidden = object.hidden;
    let name: AnyElement = match renaming {
        Some((renamed, edit)) if *renamed == target => {
            super::word_kit::input(("selection-rename", index), edit, true).into_any_element()
        }
        _ => div()
            .when(hidden, |name| name.text_color(theme::text_faint()))
            .child(object.name.clone())
            .into_any_element(),
    };
    div()
        .id(("selection-row", index))
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .min_h(px(24.0))
        .rounded_sm()
        .when(selected == Some(target), |row| row.bg(theme::selected()))
        .hover(|row| row.bg(theme::hovered()))
        .cursor_pointer()
        .child(div().flex_1().child(name))
        .child(
            div()
                .id(("selection-eye", index))
                .px_1()
                .text_color(theme::text_dim())
                .child(if hidden { "—" } else { "👁" })
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    view.object_ui_edit(cx, move |document| {
                        document.set_object_hidden(target, !hidden)
                    });
                    cx.stop_propagation();
                })),
        )
        .on_click(
            cx.listener(move |view, event: &gpui::ClickEvent, _window, cx| {
                view.objects_ui.renaming = if event.click_count() >= 2 {
                    let name = view
                        .doc()
                        .object_format(target)
                        .map(|format| format.name)
                        .unwrap_or_default();
                    Some((target, TextEdit::selected(name)))
                } else {
                    None
                };
                view.select_target(target);
                cx.notify();
            }),
        )
        .into_any_element()
}
