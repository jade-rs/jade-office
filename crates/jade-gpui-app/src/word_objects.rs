//! Pictures and shapes on the page: selecting one (and, with ⇧, several),
//! moving it, sizing it by its eight handles, turning it by its rotation
//! handle, cropping a picture by its crop handles; the keys that act on a
//! selected object; pasting a picture from the clipboard. The tools that act
//! on the selection are `word_objects_ribbon` (図の形式, 図形の書式) and
//! `word_objects_panels` (レイアウトの詳細設定, the Selection Pane and the
//! 書式設定 pane). The commands are `jade-office-model`'s, which the browser
//! shell runs too.

use gpui::{div, prelude::*, px, AnyElement, Context, MouseMoveEvent, SharedString};
use jade_docx_editor::document::ImageCrop;
use jade_docx_editor::word_document::{FloatingObjectKind, FloatingObjectTarget};
use jade_docx_native_render::ObjectHit;
use jade_office_model::word::objects::{ObjectFormat, ObjectSort, WordObjects};
use jade_office_model::word::picture::PlacedObject;
use jade_office_model::word::picture_format::WordPictureFormat;

use crate::{theme, DocumentView};

/// Where a handle sits on an object's box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::TopLeft,
        Handle::Top,
        Handle::TopRight,
        Handle::Right,
        Handle::BottomRight,
        Handle::Bottom,
        Handle::BottomLeft,
        Handle::Left,
    ];

    /// Its place on a box, as fractions of the width and height.
    fn anchor(self) -> (f64, f64) {
        match self {
            Handle::TopLeft => (0.0, 0.0),
            Handle::Top => (0.5, 0.0),
            Handle::TopRight => (1.0, 0.0),
            Handle::Right => (1.0, 0.5),
            Handle::BottomRight => (1.0, 1.0),
            Handle::Bottom => (0.5, 1.0),
            Handle::BottomLeft => (0.0, 1.0),
            Handle::Left => (0.0, 0.5),
        }
    }

    fn is_corner(self) -> bool {
        matches!(
            self,
            Handle::TopLeft | Handle::TopRight | Handle::BottomRight | Handle::BottomLeft
        )
    }
}

/// What a drag on an object is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grip {
    Move,
    Resize(Handle),
    /// Turning by the handle above the box.
    Rotate,
    /// A crop handle, in トリミング.
    Crop(Handle),
}

/// A drag in progress.
#[derive(Clone, Copy, Debug)]
pub struct Drag {
    pub object: ObjectHit,
    pub grip: Grip,
    /// The box when the drag started, on its page.
    pub start: (f64, f64, f64, f64),
    pub from: (f64, f64),
    /// The picture's crop when the drag started.
    pub crop: ImageCrop,
    /// Whether this drag's edit is on the undo stack yet: the next move
    /// replaces it, so the whole drag is one undo.
    pub applied: bool,
    /// Keep the proportions while sizing (a picture's corner, or ⇧).
    pub keep_ratio: bool,
}

/// The object tools' own state, kept with the document view.
#[derive(Default)]
pub struct ObjectTools {
    /// Objects selected with ⇧ besides `DocumentView::selected_object`.
    pub extra: Vec<ObjectHit>,
    pub drag: Option<Drag>,
    /// トリミング: the selected picture shows crop handles.
    pub cropping: bool,
    /// The panels (`word_objects_panels`).
    pub panels: crate::word_objects_panels::Panels,
    /// A shape to draw with the next drag on a page (挿入 → 図形).
    pub drawing: Option<crate::word_objects_ribbon::DrawTool>,
    /// The rectangle being drawn, on its page: (page, x0, y0, x1, y1).
    pub sketch: Option<(usize, f64, f64, f64, f64)>,
    /// A freeform's points so far, on its page.
    pub freeform: Option<(usize, Vec<(f64, f64)>)>,
    /// リンクの作成: the text box whose overflow the next one clicked takes.
    pub linking: Option<FloatingObjectTarget>,
    /// 頂点の編集: the selected freeform's points are shown and dragged;
    /// the point being dragged.
    pub editing_points: bool,
    pub point_drag: Option<(usize, (f64, f64))>,
    /// What the last object command said when it did nothing.
    pub status: Option<SharedString>,
    /// SmartArt's text pane and a chart's data sheet.
    pub diagrams: crate::word_objects_diagrams::DiagramTools,
}

const HANDLE: f64 = 8.0;
/// How far above the box the rotation handle sits.
const ROTATE_REACH: f64 = 22.0;

fn hit(point: (f64, f64), center: (f64, f64), reach: f64) -> bool {
    (point.0 - center.0).abs() <= reach && (point.1 - center.1).abs() <= reach
}

/// The object a hit test or the renderer's record found, as an `ObjectHit`.
pub fn region_hit(
    document: &jade_docx_native_render::NativeDocument,
    target: FloatingObjectTarget,
) -> Option<ObjectHit> {
    let info = document.renderer().object_region(target)?;
    Some(ObjectHit {
        kind: info.kind,
        paragraph: info.paragraph_index,
        run: info.run_index,
        page: info.page,
        x: info.x,
        y: info.y,
        width: info.width,
        height: info.height,
    })
}

pub fn target_of(object: &ObjectHit) -> FloatingObjectTarget {
    FloatingObjectTarget {
        paragraph_index: object.paragraph,
        run_index: object.run,
        kind: object.kind,
    }
}

pub fn placed(object: &ObjectHit) -> PlacedObject {
    crate::placed_object(object)
}

impl DocumentView {
    /// The selected object's target, if one is selected.
    pub(crate) fn object_target(&self) -> Option<FloatingObjectTarget> {
        self.selected_object.as_ref().map(target_of)
    }

    /// What the selected object is and how it is laid out.
    pub(crate) fn selected_format(&self) -> Option<ObjectFormat> {
        self.document.object_format(self.object_target()?)
    }

    /// Every selected object, the primary first.
    pub(crate) fn selected_objects(&self) -> Vec<ObjectHit> {
        self.selected_object
            .into_iter()
            .chain(self.objects.extra.iter().copied())
            .collect()
    }

    /// Selects `target` alone, as a click on it would.
    pub(crate) fn select_object(&mut self, target: FloatingObjectTarget, cx: &mut Context<Self>) {
        self.selected_object = region_hit(&self.document, target);
        self.objects.extra.clear();
        self.objects.cropping = false;
        cx.notify();
    }

    /// After an edit: the selection's boxes as they are now drawn.
    pub(crate) fn refresh_selection(&mut self) {
        if let Some(object) = self.selected_object {
            self.selected_object = region_hit(&self.document, target_of(&object)).or(
                // Hidden or gone: keep what was selected so the tools still
                // act on it (the Selection Pane can show it again).
                self.document
                    .object_format(target_of(&object))
                    .map(|_| object),
            );
        }
        let extra = std::mem::take(&mut self.objects.extra);
        self.objects.extra = extra
            .into_iter()
            .filter_map(|object| region_hit(&self.document, target_of(&object)))
            .collect();
    }

    /// Runs an object command and shows what it did.
    pub(crate) fn object_edit(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut Self) -> Result<(), String>,
    ) {
        match edit(self) {
            Ok(()) => {
                self.objects.status = None;
                self.table_status = None;
                self.rebuild_after_edit();
                self.refresh_selection();
            }
            Err(error) => self.objects.status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    /// A press on a page. Takes it when it lands on an object, a handle of
    /// the selected one, or starts drawing a shape; otherwise leaves it for
    /// the caret.
    pub(crate) fn press_object(
        &mut self,
        page_index: usize,
        (x, y): (f64, f64),
        document_y: f64,
        shift: bool,
        click_count: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.objects.drawing == Some(crate::word_objects_ribbon::DrawTool::Freeform) {
            // A click per point; a double click, or a click on the first
            // point, finishes it.
            let points = self
                .objects
                .freeform
                .get_or_insert_with(|| (page_index, Vec::new()));
            let closes = points.1.len() >= 3
                && points
                    .1
                    .first()
                    .is_some_and(|first| (first.0 - x).abs() < 6.0 && (first.1 - y).abs() < 6.0);
            if !closes {
                points.1.push((x, y));
            }
            if closes || click_count >= 2 {
                self.finish_freeform(closes, cx);
            }
            cx.notify();
            return true;
        }
        if self.objects.editing_points {
            if let Some(index) = self.point_at((x, y), page_index) {
                self.objects.point_drag = Some((index, (x, y)));
                return true;
            }
        }
        if self.objects.drawing.is_some() {
            self.objects.sketch = Some((page_index, x, y, x, y));
            self.selected_object = None;
            self.objects.extra.clear();
            cx.notify();
            return true;
        }
        // The selected object's handles first: they reach outside its box.
        if let Some(object) = self
            .selected_object
            .filter(|o| o.page.saturating_sub(1) == page_index)
        {
            if let Some(grip) = self.grip_at(&object, (x, y)) {
                self.start_drag(object, grip, (x, y), shift);
                cx.notify();
                return true;
            }
        }
        let Some(object) = self.document.object_at(x, document_y) else {
            if self.selected_object.is_some() || !self.objects.extra.is_empty() {
                self.selected_object = None;
                self.objects.extra.clear();
                self.objects.cropping = false;
                cx.notify();
            }
            return false;
        };
        // リンクの作成: the box clicked continues the one picked before.
        if let Some(from) = self.objects.linking.take() {
            if object.kind == FloatingObjectKind::TextBox && target_of(&object) != from {
                use jade_office_model::word::shapes::WordShapes;
                let to = target_of(&object);
                self.object_edit(cx, |view| view.document.link_text_boxes(from, to));
                return true;
            }
        }
        // In a text box, a second click (or a double click) is in its text.
        if object.kind == FloatingObjectKind::TextBox
            && (click_count >= 2 || self.selected_object == Some(object))
        {
            if let Some(position) = self.document.position_at(x, document_y) {
                if self
                    .document
                    .word_document()
                    .text_box_host_for_surface(position.0)
                    .is_some()
                {
                    self.selected_object = Some(object);
                    self.objects.extra.clear();
                    self.insert_at = position;
                    self.drag_anchor = Some(position);
                    self.selection_anchor = None;
                    self.caret = self.place_caret_near(position, document_y);
                    self.refresh_caret_page_image();
                    cx.notify();
                    return true;
                }
            }
        }
        if shift && self.selected_object.is_some() {
            // ⇧-click adds to the selection, or takes one out of it.
            if self.selected_object == Some(object) {
                self.selected_object = self.objects.extra.pop();
            } else if let Some(at) = self.objects.extra.iter().position(|o| *o == object) {
                self.objects.extra.remove(at);
            } else {
                self.objects.extra.push(object);
            }
            cx.notify();
            return true;
        }
        if self.selected_object != Some(object) {
            self.objects.cropping = false;
        }
        self.selected_object = Some(object);
        self.objects.extra.clear();
        let grip = if self.objects.cropping {
            Grip::Crop(Handle::BottomRight)
        } else {
            Grip::Move
        };
        if grip == Grip::Move {
            self.start_drag(object, Grip::Move, (x, y), shift);
        }
        self.drag_anchor = None;
        cx.notify();
        true
    }

    /// The handle of `object` under `point`.
    fn grip_at(&self, object: &ObjectHit, point: (f64, f64)) -> Option<Grip> {
        let (left, top, width, height) = (object.x, object.y, object.width, object.height);
        if !self.objects.cropping && hit(point, (left + width / 2.0, top - ROTATE_REACH), HANDLE) {
            return Some(Grip::Rotate);
        }
        for handle in Handle::ALL {
            let (fx, fy) = handle.anchor();
            if hit(
                point,
                (left + width * fx, top + height * fy),
                HANDLE / 2.0 + 2.0,
            ) {
                return Some(if self.objects.cropping {
                    Grip::Crop(handle)
                } else {
                    Grip::Resize(handle)
                });
            }
        }
        None
    }

    fn start_drag(&mut self, object: ObjectHit, grip: Grip, from: (f64, f64), shift: bool) {
        let target = target_of(&object);
        let format = self.document.object_format(target);
        let crop =
            jade_office_model::word::objects::object_run(self.document.word_document(), target)
                .and_then(|run| run.image.as_ref())
                .and_then(|image| image.crop)
                .unwrap_or_default();
        let is_picture = format
            .as_ref()
            .is_some_and(|format| format.sort == ObjectSort::Picture);
        let keep_ratio =
            shift || matches!(grip, Grip::Resize(handle) if handle.is_corner() && is_picture);
        self.objects.drag = Some(Drag {
            object,
            grip,
            start: (object.x, object.y, object.width, object.height),
            from,
            crop,
            applied: false,
            keep_ratio,
        });
    }

    /// A drag on a page: moves, sizes, turns or crops the object held, or
    /// stretches the shape being drawn. Returns whether it was taken.
    pub(crate) fn drag_object(
        &mut self,
        page_index: usize,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(Some(bounds)) = self.page_bounds.borrow().get(page_index).copied() else {
            return self.objects.drag.is_some() || self.objects.sketch.is_some();
        };
        let zoom = f64::from(self.zoom);
        let x = f64::from(event.position.x - bounds.origin.x) / zoom;
        let y = f64::from(event.position.y - bounds.origin.y) / zoom;
        if let Some(sketch) = self.objects.sketch.as_mut() {
            sketch.3 = x;
            sketch.4 = y;
            cx.notify();
            return true;
        }
        if let Some(drag) = self.objects.point_drag.as_mut() {
            drag.1 = (x, y);
            cx.notify();
            return true;
        }
        let Some(drag) = self.objects.drag else {
            return false;
        };
        let (dx, dy) = (x - drag.from.0, y - drag.from.1);
        if !drag.applied && dx.abs() < 2.0 && dy.abs() < 2.0 {
            // A click, not a drag yet.
            return true;
        }
        let (sx, sy, sw, sh) = drag.start;
        let target = target_of(&drag.object);
        let result = match drag.grip {
            Grip::Move | Grip::Resize(_) => {
                let rect = match drag.grip {
                    Grip::Move => (sx + dx, sy + dy, sw, sh),
                    Grip::Resize(handle) => resize(drag.start, handle, (dx, dy), drag.keep_ratio),
                    _ => unreachable!(),
                };
                if drag.applied {
                    self.document
                        .retransform_object(drag.object, rect.0, rect.1, rect.2, rect.3)
                } else {
                    self.document
                        .transform_object(drag.object, rect.0, rect.1, rect.2, rect.3)
                }
            }
            Grip::Rotate => {
                let center = (sx + sw / 2.0, sy + sh / 2.0);
                let angle = (y - center.1).atan2(x - center.0).to_degrees() + 90.0;
                // ⇧ snaps to 15° as Word does.
                let angle = if event.modifiers.shift {
                    (angle / 15.0).round() * 15.0
                } else {
                    angle.round()
                };
                self.replace_drag_edit(drag.applied, |view| {
                    view.document.set_object_rotation(target, angle)
                })
            }
            Grip::Crop(handle) => {
                let (crop, rect) = crop_drag(drag.start, drag.crop, handle, (dx, dy));
                self.replace_drag_edit(drag.applied, |view| {
                    view.document.crop_picture_in_place(target, crop, rect)
                })
            }
        };
        match result {
            Ok(()) => {
                if let Some(drag) = self.objects.drag.as_mut() {
                    drag.applied = true;
                }
                self.dirty = true;
                self.rebuild_all_images();
                self.refresh_selection();
            }
            Err(error) => self.objects.status = Some(SharedString::from(error)),
        }
        cx.notify();
        true
    }

    /// Applies a drag's edit, taking the previous one of the same drag off
    /// the undo stack first.
    fn replace_drag_edit(
        &mut self,
        replace: bool,
        edit: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        if replace {
            self.document.undo()?;
        }
        edit(self)
    }

    /// The mouse coming up: ends a drag, or puts in the shape being drawn.
    pub(crate) fn release_object(&mut self, cx: &mut Context<Self>) {
        if let Some((index, point)) = self.objects.point_drag.take() {
            return self.move_point(index, point, cx);
        }
        if let Some((page, x0, y0, x1, y1)) = self.objects.sketch.take() {
            let tool = self.objects.drawing.take();
            if let Some(tool) = tool {
                self.draw_shape(tool, page, (x0, y0, x1, y1), cx);
            }
            return;
        }
        if let Some(drag) = self.objects.drag.take() {
            if drag.applied && matches!(drag.grip, Grip::Move | Grip::Resize(_)) {
                self.reroute_connectors();
            }
            self.refresh_selection();
            cx.notify();
        }
    }

    /// The freeform being drawn goes in.
    pub(crate) fn finish_freeform(&mut self, closed: bool, cx: &mut Context<Self>) {
        use jade_office_model::word::connectors::WordConnectors;
        let Some((page_index, points)) = self.objects.freeform.take() else {
            return;
        };
        self.objects.drawing = None;
        self.objects.status = None;
        if points.len() < 2 {
            return cx.notify();
        }
        let document_y = self.document.document_y(page_index, points[0].1);
        if let Some(position) = self.document.position_at(points[0].0, document_y) {
            self.insert_at = position;
        }
        self.place_document_caret();
        self.object_edit(cx, |view| view.document.insert_freeform(&points, closed));
    }

    /// The selected freeform's path and box, when it is one.
    fn freeform_path(&self) -> Option<(jade_docx_editor::document::GeometryPath, ObjectHit)> {
        let object = self.selected_object?;
        let run = jade_office_model::word::objects::object_run(
            self.document.word_document(),
            target_of(&object),
        )?;
        let path = run
            .text_box
            .as_ref()?
            .extras()
            .custom_geometry
            .as_ref()?
            .paths
            .first()?
            .clone();
        Some((path, object))
    }

    /// The freeform point near `point`.
    fn point_at(&self, point: (f64, f64), page_index: usize) -> Option<usize> {
        let (path, object) = self.freeform_path()?;
        if object.page.saturating_sub(1) != page_index {
            return None;
        }
        jade_office_model::word::connectors::path_points(&path, &placed(&object))
            .iter()
            .position(|(x, y)| (x - point.0).abs() <= 5.0 && (y - point.1).abs() <= 5.0)
    }

    /// A dragged point's new place goes into the path.
    fn move_point(&mut self, index: usize, point: (f64, f64), cx: &mut Context<Self>) {
        use jade_office_model::word::connectors::{move_path_point, WordConnectors};
        let Some((path, object)) = self.freeform_path() else {
            return;
        };
        let moved = move_path_point(&path, &placed(&object), index, point);
        let target = target_of(&object);
        self.object_edit(cx, |view| view.document.set_freeform_path(target, moved));
    }

    /// Keys for the selected object: Delete removes it, the arrows nudge it
    /// (⌥ by a pixel), Escape lets go. Returns whether the key was taken.
    pub(crate) fn object_key(&mut self, key: &str, alt: bool, cx: &mut Context<Self>) -> bool {
        if self.objects.drawing.is_some() && key == "escape" {
            self.objects.drawing = None;
            self.objects.sketch = None;
            self.objects.freeform = None;
            cx.notify();
            return true;
        }
        if self.objects.freeform.is_some() && matches!(key, "enter" | "return") {
            self.finish_freeform(false, cx);
            return true;
        }
        if self.objects.editing_points && key == "escape" {
            self.objects.editing_points = false;
            cx.notify();
            return true;
        }
        let Some(object) = self.selected_object else {
            return false;
        };
        // While the caret is in a text box, keys are the text's.
        if object.kind == FloatingObjectKind::TextBox
            && self
                .document
                .word_document()
                .text_box_host_for_surface(self.insert_at.0)
                .is_some()
            && key != "escape"
        {
            return false;
        }
        match key {
            "escape" => {
                if self.objects.cropping {
                    self.objects.cropping = false;
                } else {
                    self.selected_object = None;
                    self.objects.extra.clear();
                }
                cx.notify();
                true
            }
            "backspace" | "delete" => {
                let targets: Vec<_> = self.selected_objects().iter().map(target_of).collect();
                self.object_edit(cx, |view| delete_objects(view, targets));
                self.selected_object = None;
                self.objects.extra.clear();
                cx.notify();
                true
            }
            "left" | "right" | "up" | "down" => {
                let step = if alt { 1.0 } else { 7.5 };
                let (dx, dy) = match key {
                    "left" => (-step, 0.0),
                    "right" => (step, 0.0),
                    "up" => (0.0, -step),
                    _ => (0.0, step),
                };
                let objects = self.selected_objects();
                self.object_edit(cx, |view| {
                    for object in objects {
                        let placed = placed(&object);
                        view.document.move_object(
                            placed,
                            (object.x + dx, object.y + dy, object.width, object.height),
                        )?;
                    }
                    Ok(())
                });
                true
            }
            _ => false,
        }
    }

    /// ⌘V with a picture on the clipboard (a screenshot, an image copied in
    /// a browser): the picture goes in at the caret. Returns whether it did.
    pub(crate) fn paste_picture(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(item) = cx.read_from_clipboard() else {
            return false;
        };
        let picture = item.entries().iter().find_map(|entry| match entry {
            gpui::ClipboardEntry::Image(image) => Some(image.clone()),
            _ => None,
        });
        let Some(picture) = picture else {
            return false;
        };
        let bytes = match picture.format {
            gpui::ImageFormat::Png
            | gpui::ImageFormat::Jpeg
            | gpui::ImageFormat::Gif
            | gpui::ImageFormat::Svg => picture.bytes.clone(),
            // Word stores these as PNG.
            _ => match image::load_from_memory(&picture.bytes) {
                Ok(decoded) => {
                    let mut png = Vec::new();
                    if decoded
                        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                        .is_err()
                    {
                        return false;
                    }
                    png
                }
                Err(_) => return false,
            },
        };
        self.insert_picture(bytes, cx);
        true
    }

    /// 図の変更 → このデバイスから: another picture in the selected one's place.
    pub(crate) fn prompt_to_change_picture(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.object_target() else {
            return;
        };
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(crate::i18n::text(
                "objects.change_picture",
            ))),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.first() else {
                return;
            };
            match std::fs::read(path) {
                Ok(bytes) => {
                    view.update(cx, |view, cx| {
                        view.object_edit(cx, |view| view.document.replace_picture(target, bytes))
                    })
                    .ok();
                }
                Err(error) => eprintln!("{}: {error}", path.display()),
            }
        })
        .detach();
    }

    /// 図の変更 → クリップボードから.
    pub(crate) fn change_picture_from_clipboard(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.object_target() else {
            return;
        };
        let bytes = cx.read_from_clipboard().and_then(|item| {
            item.entries().iter().find_map(|entry| match entry {
                gpui::ClipboardEntry::Image(image) => Some(image.bytes.clone()),
                _ => None,
            })
        });
        match bytes {
            Some(bytes) => {
                self.object_edit(cx, |view| view.document.replace_picture(target, bytes))
            }
            None => {
                self.objects.status =
                    Some(crate::i18n::text("objects.no_clipboard_picture").into());
                cx.notify();
            }
        }
    }

    /// The selection, its handles and the shape being drawn, over a page.
    pub(crate) fn object_overlay(&self, page_index: usize) -> Vec<AnyElement> {
        let zoom = f64::from(self.zoom);
        let mut elements = Vec::new();
        let scaled = |value: f64| px((value * zoom) as f32);
        let outline = |object: &ObjectHit, primary: bool| {
            div()
                .absolute()
                .left(scaled(object.x))
                .top(scaled(object.y))
                .w(scaled(object.width))
                .h(scaled(object.height))
                .border_1()
                .border_color(if primary {
                    theme::accent()
                } else {
                    theme::text_dim()
                })
                .into_any_element()
        };
        for object in &self.objects.extra {
            if object.page.saturating_sub(1) == page_index {
                elements.push(outline(object, false));
            }
        }
        if let Some(object) = self
            .selected_object
            .filter(|o| o.page.saturating_sub(1) == page_index)
        {
            elements.push(outline(&object, true));
            let handle = |cx: f64, cy: f64, round: bool| {
                div()
                    .absolute()
                    .left(px((cx * zoom - HANDLE / 2.0) as f32))
                    .top(px((cy * zoom - HANDLE / 2.0) as f32))
                    .size(px(HANDLE as f32))
                    .bg(theme::panel())
                    .border_1()
                    .border_color(theme::accent())
                    .when(round, |handle| handle.rounded_full())
                    .into_any_element()
            };
            if self.objects.cropping {
                // Crop handles: dark bars at the corners and edges.
                for corner in Handle::ALL {
                    let (fx, fy) = corner.anchor();
                    elements.push(
                        div()
                            .absolute()
                            .left(px(((object.x + object.width * fx) * zoom - 5.0) as f32))
                            .top(px(((object.y + object.height * fy) * zoom - 5.0) as f32))
                            .size(px(10.0))
                            .bg(gpui::black())
                            .into_any_element(),
                    );
                }
            } else {
                for each in Handle::ALL {
                    let (fx, fy) = each.anchor();
                    elements.push(handle(
                        object.x + object.width * fx,
                        object.y + object.height * fy,
                        false,
                    ));
                }
                // The rotation handle, on a stem above the top edge.
                let center_x = object.x + object.width / 2.0;
                elements.push(
                    div()
                        .absolute()
                        .left(px((center_x * zoom - 0.5) as f32))
                        .top(px(((object.y - ROTATE_REACH) * zoom) as f32))
                        .w(px(1.0))
                        .h(px((ROTATE_REACH * zoom) as f32))
                        .bg(theme::accent())
                        .into_any_element(),
                );
                elements.push(handle(center_x, object.y - ROTATE_REACH, true));
            }
            if let Some(format) = self.selected_format() {
                if format.rotation.abs() > 0.01 {
                    elements.push(
                        div()
                            .absolute()
                            .left(scaled(object.x + object.width + 4.0))
                            .top(scaled(object.y - ROTATE_REACH - 6.0))
                            .px_1()
                            .rounded_sm()
                            .bg(theme::panel())
                            .text_xs()
                            .text_color(theme::text_dim())
                            .child(format!("{:.0}°", format.rotation))
                            .into_any_element(),
                    );
                }
            }
        }
        // A freeform's points so far, and the selected freeform's points
        // while 頂点の編集 is on.
        let dot = |(x, y): (f64, f64), color: gpui::Hsla| {
            div()
                .absolute()
                .left(px((x * zoom - 4.0) as f32))
                .top(px((y * zoom - 4.0) as f32))
                .size(px(8.0))
                .bg(color)
                .into_any_element()
        };
        if let Some((page, points)) = self.objects.freeform.as_ref() {
            if *page == page_index {
                elements.extend(points.iter().map(|point| dot(*point, gpui::black())));
            }
        }
        if self.objects.editing_points {
            if let Some((path, object)) = self.freeform_path() {
                if object.page.saturating_sub(1) == page_index {
                    let mut points =
                        jade_office_model::word::connectors::path_points(&path, &placed(&object));
                    if let Some((index, point)) = self.objects.point_drag {
                        if let Some(dragged) = points.get_mut(index) {
                            *dragged = point;
                        }
                    }
                    elements.extend(points.into_iter().map(|point| dot(point, gpui::red())));
                }
            }
        }
        if let Some((page, x0, y0, x1, y1)) = self.objects.sketch {
            if page == page_index {
                elements.push(
                    div()
                        .absolute()
                        .left(scaled(x0.min(x1)))
                        .top(scaled(y0.min(y1)))
                        .w(scaled((x1 - x0).abs()))
                        .h(scaled((y1 - y0).abs()))
                        .border_1()
                        .border_dashed()
                        .border_color(theme::accent())
                        .into_any_element(),
                );
            }
        }
        elements
    }
}

/// Removes objects, last first so the earlier targets stay valid.
fn delete_objects(
    view: &mut DocumentView,
    mut targets: Vec<FloatingObjectTarget>,
) -> Result<(), String> {
    use jade_docx_editor::commands::delete_object::DeleteObjectCommand;
    use jade_office_model::word::WordSession;
    targets.sort_by_key(|target| (target.paragraph_index, target.run_index));
    let commands: Vec<Box<dyn jade_docx_editor::commands::Command>> = targets
        .into_iter()
        .rev()
        .map(|target| Box::new(DeleteObjectCommand::new(target)) as Box<_>)
        .collect();
    WordSession::execute_command(
        &mut view.document,
        Box::new(jade_office_model::word::objects::Sequence::new(
            "delete_objects",
            commands,
        )),
        false,
    )
}

/// A box sized from one of its handles by `(dx, dy)`; `keep_ratio` keeps
/// its proportions.
pub fn resize(
    (x, y, width, height): (f64, f64, f64, f64),
    handle: Handle,
    (dx, dy): (f64, f64),
    keep_ratio: bool,
) -> (f64, f64, f64, f64) {
    const MIN: f64 = 4.0;
    let (fx, fy) = handle.anchor();
    let (mut left, mut top, mut right, mut bottom) = (x, y, x + width, y + height);
    if fx == 0.0 {
        left = (left + dx).min(right - MIN);
    } else if fx == 1.0 {
        right = (right + dx).max(left + MIN);
    }
    if fy == 0.0 {
        top = (top + dy).min(bottom - MIN);
    } else if fy == 1.0 {
        bottom = (bottom + dy).max(top + MIN);
    }
    if keep_ratio && handle.is_corner() && width > 0.0 && height > 0.0 {
        let ratio = width / height;
        let (new_width, new_height) = (right - left, bottom - top);
        // The larger change wins; the other follows.
        let (new_width, new_height) = if (new_width / width) > (new_height / height) {
            (new_width, new_width / ratio)
        } else {
            (new_height * ratio, new_height)
        };
        if fx == 0.0 {
            left = right - new_width;
        } else {
            right = left + new_width;
        }
        if fy == 0.0 {
            top = bottom - new_height;
        } else {
            bottom = top + new_height;
        }
    }
    (left, top, right - left, bottom - top)
}

/// A crop handle dragged by `(dx, dy)`: the new crop of the picture and the
/// visible part's new box. The picture itself does not move or scale: the
/// handle cuts into it (or, outward, back out of what was cut).
pub fn crop_drag(
    (x, y, width, height): (f64, f64, f64, f64),
    crop: ImageCrop,
    handle: Handle,
    (dx, dy): (f64, f64),
) -> (ImageCrop, (f64, f64, f64, f64)) {
    let visible_x = (1.0 - crop.left - crop.right).max(0.001);
    let visible_y = (1.0 - crop.top - crop.bottom).max(0.001);
    // The whole picture's size on the page.
    let (full_width, full_height) = (width / visible_x, height / visible_y);
    let (fx, fy) = handle.anchor();
    let mut crop = crop;
    let (mut left, mut top, mut right, mut bottom) = (x, y, x + width, y + height);
    if fx == 0.0 {
        let cut = (crop.left + dx / full_width).clamp(-1.0, 1.0 - crop.right - 0.02);
        left = x - (crop.left - cut) * full_width;
        crop.left = cut;
    } else if fx == 1.0 {
        let cut = (crop.right - dx / full_width).clamp(-1.0, 1.0 - crop.left - 0.02);
        right = x + width + (crop.right - cut) * full_width;
        crop.right = cut;
    }
    if fy == 0.0 {
        let cut = (crop.top + dy / full_height).clamp(-1.0, 1.0 - crop.bottom - 0.02);
        top = y - (crop.top - cut) * full_height;
        crop.top = cut;
    } else if fy == 1.0 {
        let cut = (crop.bottom - dy / full_height).clamp(-1.0, 1.0 - crop.top - 0.02);
        bottom = y + height + (crop.bottom - cut) * full_height;
        crop.bottom = cut;
    }
    (crop, (left, top, right - left, bottom - top))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_handles_keep_a_pictures_proportions() {
        let rect = resize(
            (10.0, 10.0, 100.0, 50.0),
            Handle::BottomRight,
            (100.0, 0.0),
            true,
        );
        assert_eq!(rect, (10.0, 10.0, 200.0, 100.0));
        let rect = resize((10.0, 10.0, 100.0, 50.0), Handle::Right, (50.0, 0.0), true);
        assert_eq!(rect, (10.0, 10.0, 150.0, 50.0));
        let rect = resize(
            (10.0, 10.0, 100.0, 50.0),
            Handle::TopLeft,
            (-100.0, 0.0),
            true,
        );
        assert_eq!(rect, (-90.0, -40.0, 200.0, 100.0));
    }

    #[test]
    fn a_crop_handle_cuts_into_the_picture_without_moving_it() {
        let (crop, rect) = crop_drag(
            (0.0, 0.0, 100.0, 50.0),
            ImageCrop::default(),
            Handle::Left,
            (25.0, 0.0),
        );
        assert!((crop.left - 0.25).abs() < 1e-9);
        assert_eq!(rect, (25.0, 0.0, 75.0, 50.0));
        // Back out again from there.
        let (crop, rect) = crop_drag(rect, crop, Handle::Left, (-25.0, 0.0));
        assert!(crop.left.abs() < 1e-9);
        assert_eq!(rect, (0.0, 0.0, 100.0, 50.0));
    }
}

/// The object tools as a user drives them: clicks and drags on the page,
/// the ribbon's controls, the dialog and the panes.
#[cfg(test)]
mod scenarios {
    use super::*;
    use crate::word_objects_ribbon::{DrawTool, ObjectAction};
    use gpui::{Modifiers, MouseButton, Pixels, Point, VisualTestContext};
    use jade_docx_editor::document::TextBoxShape;
    use jade_office_model::word::picture_format::{Effect, ShadowPreset};

    pub(crate) fn open_contract(
        cx: &mut gpui::TestAppContext,
    ) -> (gpui::Entity<DocumentView>, &mut VisualTestContext) {
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

    pub(crate) fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbaImage::from_fn(width, height, |x, _| {
            if x < width / 2 {
                image::Rgba([200, 30, 30, 255])
            } else {
                image::Rgba([30, 30, 200, 255])
            }
        })
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        bytes
    }

    /// A point on page `page` in window coordinates.
    pub(crate) fn on_page(
        view: &gpui::Entity<DocumentView>,
        cx: &mut VisualTestContext,
        page: usize,
        (x, y): (f64, f64),
    ) -> Point<Pixels> {
        view.read_with(cx, |view, _| {
            let bounds = view.page_bounds.borrow()[page].expect("the page was painted");
            let zoom = view.zoom;
            gpui::point(
                bounds.origin.x + gpui::px(x as f32 * zoom),
                bounds.origin.y + gpui::px(y as f32 * zoom),
            )
        })
    }

    /// A drag from `from` to `to` on page 0, in page coordinates.
    pub(crate) fn drag(
        view: &gpui::Entity<DocumentView>,
        cx: &mut VisualTestContext,
        from: (f64, f64),
        to: (f64, f64),
    ) {
        let start = on_page(view, cx, 0, from);
        let end = on_page(view, cx, 0, to);
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        let middle = gpui::point((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
        cx.simulate_mouse_move(middle, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }

    /// A picture put in at the start of the first body paragraph, floating
    /// in front of the text so it can be dragged anywhere.
    pub(crate) fn picture(
        view: &gpui::Entity<DocumentView>,
        cx: &mut VisualTestContext,
    ) -> ObjectHit {
        view.update(cx, |view, cx| {
            let paragraph = (0..view.document.paragraph_count())
                .find(|&index| {
                    view.document
                        .paragraph_text(index)
                        .is_some_and(|text| text.chars().count() > 10)
                })
                .unwrap();
            view.insert_at = (paragraph, 0);
            view.insert_picture(png(120, 60), cx);
            let run = view
                .document
                .word_document()
                .paragraph(paragraph)
                .unwrap()
                .runs
                .iter()
                .position(|run| run.image.is_some())
                .unwrap();
            let target = FloatingObjectTarget {
                paragraph_index: paragraph,
                run_index: run,
                kind: FloatingObjectKind::Image,
            };
            view.document
                .set_object_wrap(
                    target,
                    jade_office_model::word::picture::Wrap::InFrontOfText,
                )
                .unwrap();
            view.rebuild_after_edit();
            region_hit(&view.document, target).unwrap()
        })
    }

    fn image(
        view: &gpui::Entity<DocumentView>,
        cx: &mut VisualTestContext,
        object: ObjectHit,
    ) -> jade_docx_editor::document::RunImage {
        view.read_with(cx, |view, _| {
            view.document
                .word_document()
                .paragraph(object.paragraph)
                .unwrap()
                .runs[object.run]
                .image
                .clone()
                .unwrap()
        })
    }

    /// A click selects a picture and shows 図の形式; its effects apply from
    /// there; the rotation handle turns it and the crop handles cut it, each
    /// drag one undo step.
    #[gpui::test]
    fn a_picture_is_selected_formatted_turned_and_cropped(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        let object = picture(&view, cx);
        cx.run_until_parked();
        let center = (
            object.x + object.width / 2.0,
            object.y + object.height / 2.0,
        );
        let point = on_page(&view, cx, 0, center);
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            assert_eq!(view.object_target(), Some(target_of(&object)));
            let tabs = view.ribbon_tabs();
            assert!(tabs
                .iter()
                .any(|(tab, _)| *tab == crate::RibbonTab::ObjectFormat));
            view.ribbon_tab = crate::RibbonTab::ObjectFormat;
            view.run_object_action(
                ObjectAction::Effect(Effect::Shadow(Some(ShadowPreset::OUTER[0]))),
                cx,
            );
            assert!(view.objects.status.is_none(), "{:?}", view.objects.status);
        });
        cx.run_until_parked();
        assert!(image(&view, cx, object).format().shadow.is_some());

        // The rotation handle, dragged round to the right: a quarter turn.
        let object = view.read_with(cx, |view, _| view.selected_object.unwrap());
        let handle = (object.x + object.width / 2.0, object.y - ROTATE_REACH);
        let right = (
            object.x + object.width + 80.0,
            object.y + object.height / 2.0,
        );
        drag(&view, cx, handle, right);
        let rotation = image(&view, cx, object).layout.rotation_deg.unwrap_or(0.0);
        assert!((rotation - 90.0).abs() < 2.0, "{rotation}");
        view.update(cx, |view, cx| view.step_history(true, cx));
        assert!(
            image(&view, cx, object)
                .layout
                .rotation_deg
                .unwrap_or(0.0)
                .abs()
                < 0.5
        );

        // トリミング: the left handle moved in a quarter of the width.
        view.update(cx, |view, cx| {
            view.run_object_action(ObjectAction::ToggleCrop, cx)
        });
        let object = view.read_with(cx, |view, _| view.selected_object.unwrap());
        let left = (object.x, object.y + object.height / 2.0);
        drag(&view, cx, left, (object.x + object.width / 4.0, left.1));
        let crop = image(&view, cx, object).crop.expect("cropped");
        assert!((crop.left - 0.25).abs() < 0.03, "{crop:?}");
        let moved = view.read_with(cx, |view, _| view.selected_object.unwrap());
        assert!((moved.width - object.width * 0.75).abs() < 2.0, "{moved:?}");
        // Escape leaves トリミング, a second one lets go of the picture.
        view.update(cx, |view, cx| {
            assert!(view.object_key("escape", false, cx));
            assert!(!view.objects.cropping);
            assert!(view.object_key("escape", false, cx));
            assert!(view.selected_object.is_none());
        });
    }

    /// 挿入 → 図形: dragging on the page draws the shape there, styled as a
    /// new Word shape; a double click puts the caret in it to type.
    #[gpui::test]
    fn a_shape_is_drawn_and_typed_into(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            view.run_object_action(
                ObjectAction::Draw(DrawTool::Shape(TextBoxShape::Ellipse)),
                cx,
            )
        });
        drag(&view, cx, (150.0, 300.0), (310.0, 380.0));
        let (target, shape) = view.read_with(cx, |view, _| {
            let target = view.object_target().expect("the drawn shape is selected");
            let run =
                jade_office_model::word::objects::object_run(view.document.word_document(), target)
                    .unwrap();
            (target, run.text_box.clone().unwrap())
        });
        assert_eq!(shape.shape, TextBoxShape::Ellipse);
        assert_eq!(shape.fill_color.as_deref(), Some("#4472C4"), "accent 1");
        assert_eq!(shape.width_px.map(f64::round), Some(160.0));
        view.read_with(cx, |view, _| {
            assert!(view.ribbon_tab == crate::RibbonTab::ObjectFormat);
            let region = view.selected_object.unwrap();
            assert!(
                (region.x - 150.0).abs() < 1.0 && (region.y - 300.0).abs() < 1.0,
                "{region:?}"
            );
        });
        // Into its text, and type.
        let middle = on_page(&view, cx, 0, (230.0, 340.0));
        cx.simulate_click(middle, Modifiers::none());
        cx.simulate_click(middle, Modifiers::none());
        cx.run_until_parked();
        cx.simulate_input("ab");
        view.read_with(cx, |view, _| {
            let run =
                jade_office_model::word::objects::object_run(view.document.word_document(), target)
                    .unwrap();
            assert!(
                run.text_box.as_ref().unwrap().text.contains("ab"),
                "{:?}",
                run.text_box.as_ref().unwrap().text
            );
        });
    }

    /// ⌘V with a picture on the clipboard puts the picture in.
    #[gpui::test]
    fn a_picture_pastes_from_the_clipboard(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        let paragraph = view.read_with(cx, |view, _| view.insert_at.0);
        cx.write_to_clipboard(gpui::ClipboardItem::new_image(&gpui::Image::from_bytes(
            gpui::ImageFormat::Png,
            png(10, 10),
        )));
        cx.dispatch_action(crate::Paste);
        view.read_with(cx, |view, _| {
            let pictures = view
                .document
                .word_document()
                .paragraph(paragraph)
                .unwrap()
                .runs
                .iter()
                .filter(|run| run.image.is_some())
                .count();
            assert_eq!(pictures, 1);
        });
    }

    /// レイアウトの詳細設定: the position tab's alignment and the size tab's
    /// fields, typed, go in with OK as one step; the Selection Pane lists
    /// the picture by name, renames and hides it.
    #[gpui::test]
    fn the_layout_dialog_and_the_selection_pane(cx: &mut gpui::TestAppContext) {
        use crate::word_objects_panels::{DialogTab, Field};
        let (view, cx) = open_contract(cx);
        let object = picture(&view, cx);
        view.update(cx, |view, cx| {
            view.selected_object = Some(object);
            view.open_layout_dialog(cx);
            let dialog = view.objects.panels.dialog.as_mut().unwrap();
            dialog.h_align = Some("center".into());
            dialog.h_relative = jade_office_model::word::objects::RelativeTo::Margin;
            dialog.tab = DialogTab::Size;
            dialog.focus = Some(Field::Width);
            *dialog.field_mut(Field::Width).unwrap() =
                jade_office_model::text::TextEdit::new("50".into());
            view.dialog_typed();
            let height = view.objects.panels.dialog.as_ref().unwrap();
            assert_eq!(height_text(height), "25", "the proportions are kept");
            assert!(view.dialog_key("enter", false, cx));
        });
        cx.run_until_parked();
        let format = view.read_with(cx, |view, _| view.selected_format().unwrap());
        assert!((format.width / jade_office_model::word::objects::PX_PER_MM - 50.0).abs() < 0.2);
        assert_eq!(
            format.horizontal.unwrap().placement,
            jade_office_model::word::objects::Placement::Align("center".into())
        );
        view.update(cx, |view, cx| {
            view.run_object_action(ObjectAction::SelectionPane, cx);
            assert!(view.objects.panels.selection_pane);
            view.objects.panels.renaming = Some((
                target_of(&object),
                jade_office_model::text::TextEdit::new("ロゴ".into()),
            ));
            assert!(view.rename_key("enter", cx));
        });
        cx.run_until_parked();
        let names: Vec<String> = view.read_with(cx, |view, _| {
            view.document
                .body_objects()
                .into_iter()
                .map(|object| object.name)
                .collect()
        });
        assert!(names.contains(&"ロゴ".to_string()), "{names:?}");
        view.update(cx, |view, cx| {
            view.document
                .set_object_hidden(target_of(&object), true)
                .unwrap();
            view.rebuild_after_edit();
            assert!(region_hit(&view.document, target_of(&object)).is_none());
            cx.notify();
        });
    }

    fn height_text(dialog: &crate::word_objects_panels::LayoutDialog) -> String {
        dialog
            .fields
            .iter()
            .find(|(field, _)| *field == crate::word_objects_panels::Field::Height)
            .map(|(_, edit)| edit.text.clone())
            .unwrap()
    }
}

/// The diagram, connector and freeform tools as a user drives them.
#[cfg(test)]
mod more_scenarios {
    use super::scenarios::{drag, on_page, open_contract, picture};
    use super::*;
    use crate::word_objects_diagrams::DiagramAction;
    use crate::word_objects_ribbon::{DrawTool, ObjectAction};
    use gpui::{Modifiers, MouseButton};
    use jade_docx_editor::document::{ChartKind, TextBoxShape};
    use jade_office_model::word::connectors::ConnectorKind;
    use jade_office_model::word::diagrams::WordDiagrams;
    use jade_office_model::word::objects::ObjectSort;

    fn sorts(
        view: &gpui::Entity<DocumentView>,
        cx: &mut gpui::VisualTestContext,
    ) -> Vec<ObjectSort> {
        view.read_with(cx, |view, _| {
            view.document
                .body_objects()
                .into_iter()
                .map(|object| object.sort)
                .collect()
        })
    }

    /// 挿入 → SmartArt → 基本ステップ, then its text typed in the text pane:
    /// the graphic is drawn with it; a chart goes in and its data is edited
    /// in the sheet.
    #[gpui::test]
    fn smartart_and_a_chart_from_the_insert_tab(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            view.run_object_action(
                ObjectAction::Diagram(DiagramAction::InsertSmartArt(
                    jade_docx_editor::smartart::LAYOUTS[2].0,
                )),
                cx,
            );
            assert!(view.objects.status.is_none(), "{:?}", view.objects.status);
            assert!(
                view.objects.diagrams.text_pane.is_some(),
                "the text pane opens"
            );
        });
        cx.simulate_input("企画");
        view.update(cx, |view, cx| {
            assert!(view.object_tools_key("enter", false, false, cx));
        });
        cx.simulate_input("実装");
        view.update(cx, |view, cx| {
            assert!(view.object_tools_key("down", false, false, cx));
            assert!(
                view.object_tools_key("tab", false, false, cx),
                "demoted under 実装"
            );
        });
        let diagram = view.update(cx, |view, _| {
            let target = view.object_target().unwrap();
            view.document.smartart(target).unwrap()
        });
        assert_eq!(diagram.nodes[0].text, "企画");
        assert_eq!(diagram.nodes[1].text, "実装");
        assert_eq!(diagram.nodes[1].children.len(), 1, "{diagram:?}");
        assert!(sorts(&view, cx).contains(&ObjectSort::SmartArt));

        view.update(cx, |view, cx| {
            view.objects.diagrams.text_pane = None;
            view.selected_object = None;
            view.run_object_action(
                ObjectAction::Diagram(DiagramAction::InsertChart(ChartKind::Pie)),
                cx,
            );
            assert!(view.objects.status.is_none(), "{:?}", view.objects.status);
            assert!(view.object_target().is_some(), "the chart is selected");
            view.run_object_action(ObjectAction::Diagram(DiagramAction::EditData), cx);
            let sheet = view.objects.diagrams.chart_sheet.as_mut().unwrap();
            sheet.focus = Some((1, 1));
            sheet.cells[1][1] = jade_office_model::text::TextEdit::new("42".into());
            assert!(view.object_tools_key("enter", false, false, cx));
            let target = view.object_target().unwrap();
            let chart = view.document.chart_data(target).unwrap();
            assert_eq!(chart.kind, ChartKind::Pie);
            assert_eq!(chart.series[0].values[0], Some(42.0));
        });
    }

    /// A connector drawn from one shape to another is glued to both, and
    /// follows when one is dragged away.
    #[gpui::test]
    fn a_connector_follows_the_shape_it_is_glued_to(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        for (shape, from, to) in [
            (TextBoxShape::Rectangle, (120.0, 300.0), (220.0, 360.0)),
            (TextBoxShape::Ellipse, (420.0, 300.0), (520.0, 360.0)),
        ] {
            view.update(cx, |view, cx| {
                view.run_object_action(ObjectAction::Draw(DrawTool::Shape(shape)), cx)
            });
            drag(&view, cx, from, to);
        }
        view.update(cx, |view, cx| {
            view.run_object_action(
                ObjectAction::Draw(DrawTool::Connector(ConnectorKind::Straight, true)),
                cx,
            )
        });
        drag(&view, cx, (215.0, 330.0), (425.0, 330.0));
        let connector = view.read_with(cx, |view, _| {
            view.document
                .body_objects()
                .into_iter()
                .find(|object| {
                    jade_office_model::word::objects::object_run(
                        view.document.word_document(),
                        object.target,
                    )
                    .and_then(|run| run.text_box.as_ref())
                    .is_some_and(|text_box| text_box.extras().connector.is_some())
                })
                .expect("a glued connector")
                .target
        });
        let line = view.read_with(cx, |view, _| region_hit(&view.document, connector).unwrap());
        assert!(
            (line.x - 220.0).abs() < 1.5 && (line.x + line.width - 420.0).abs() < 1.5,
            "{line:?}"
        );
        // Drag the ellipse down; the connector's end goes with it.
        let ellipse_center = (470.0, 330.0);
        let at = on_page(&view, cx, 0, (600.0, 700.0));
        cx.simulate_click(at, Modifiers::none());
        drag(&view, cx, ellipse_center, (470.0, 430.0));
        let (line, ellipse) = view.read_with(cx, |view, _| {
            let ellipse = view
                .document
                .body_objects()
                .into_iter()
                .filter_map(|object| region_hit(&view.document, object.target))
                .find(|object| object.x > 400.0)
                .unwrap();
            (region_hit(&view.document, connector).unwrap(), ellipse)
        });
        // Glued to the ellipse's left point, wherever the drag left it.
        assert!(ellipse.y > 380.0, "the ellipse moved down: {ellipse:?}");
        assert!(
            (line.y + line.height - (ellipse.y + ellipse.height / 2.0)).abs() < 2.0,
            "{line:?} {ellipse:?}"
        );
    }

    /// フリーフォーム: a click per point, a double click to finish; 頂点の
    /// 編集 drags a point.
    #[gpui::test]
    fn a_freeform_is_drawn_and_its_points_dragged(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            view.run_object_action(ObjectAction::Draw(DrawTool::Freeform), cx)
        });
        for point in [(100.0, 400.0), (200.0, 300.0)] {
            let at = on_page(&view, cx, 0, point);
            cx.simulate_click(at, Modifiers::none());
        }
        let last = on_page(&view, cx, 0, (300.0, 400.0));
        cx.simulate_event(gpui::MouseDownEvent {
            position: last,
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count: 2,
            first_mouse: false,
        });
        cx.simulate_mouse_up(last, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let target = view.read_with(cx, |view, _| {
            view.document
                .body_objects()
                .into_iter()
                .find(|object| {
                    jade_office_model::word::objects::object_run(
                        view.document.word_document(),
                        object.target,
                    )
                    .and_then(|run| run.text_box.as_ref())
                    .is_some_and(|text_box| text_box.extras().custom_geometry.is_some())
                })
                .expect("the freeform")
                .target
        });
        view.update(cx, |view, cx| {
            view.select_object(target, cx);
            view.run_object_action(ObjectAction::EditPoints, cx);
        });
        drag(&view, cx, (200.0, 300.0), (200.0, 250.0));
        let points = view.read_with(cx, |view, _| {
            let (path, object) = view.freeform_path().unwrap();
            jade_office_model::word::connectors::path_points(&path, &placed(&object))
        });
        assert!(
            points
                .iter()
                .any(|(x, y)| (x - 200.0).abs() < 2.0 && (y - 250.0).abs() < 2.0),
            "{points:?}"
        );
    }

    #[gpui::test]
    fn text_boxes_are_linked_by_clicking(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        for (from, to) in [
            ((120.0, 300.0), (320.0, 360.0)),
            ((120.0, 420.0), (320.0, 480.0)),
        ] {
            view.update(cx, |view, cx| {
                view.run_object_action(ObjectAction::Draw(DrawTool::TextBox), cx)
            });
            drag(&view, cx, from, to);
        }
        let at = on_page(&view, cx, 0, (125.0, 302.0));
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            assert!(view.object_target().is_some(), "the first box is selected");
            view.run_object_action(ObjectAction::LinkTextBox, cx);
            assert!(view.objects.linking.is_some());
        });
        let at = on_page(&view, cx, 0, (125.0, 422.0));
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        view.update(cx, |view, _| {
            assert!(view.objects.status.is_none(), "{:?}", view.objects.status);
            assert!(view.objects.linking.is_none());
            let links: Vec<_> = view
                .document
                .body_objects()
                .into_iter()
                .filter_map(|object| {
                    jade_office_model::word::objects::object_run(
                        view.document.word_document(),
                        object.target,
                    )
                    .and_then(|run| run.text_box.as_ref())
                    .and_then(|text_box| text_box.extras().link)
                })
                .collect();
            assert_eq!(links.len(), 2, "{links:?}");
        });
    }

    /// 文字列の折り返し → 折り返し点の編集: a corner dragged is written as
    /// the picture's wrap polygon.
    #[gpui::test]
    fn wrap_points_are_dragged(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        let object = picture(&view, cx);
        let target = target_of(&object);
        view.update(cx, |view, cx| {
            view.document
                .set_object_wrap(target, jade_office_model::word::picture::Wrap::Tight)
                .unwrap();
            view.rebuild_after_edit();
            view.select_object(target, cx);
            view.run_object_action(ObjectAction::EditWrapPoints, cx);
        });
        cx.run_until_parked();
        let object = view.read_with(cx, |view, _| view.selected_object.expect("selected"));
        drag(
            &view,
            cx,
            (object.x, object.y),
            (
                object.x + object.width / 4.0,
                object.y + object.height / 4.0,
            ),
        );
        let polygon = view
            .read_with(cx, |view, _| {
                jade_office_model::word::objects::wrap_polygon(
                    view.document.word_document(),
                    target,
                )
            })
            .expect("a wrap polygon was written");
        assert!(
            polygon
                .iter()
                .any(|(x, y)| (x - 5400).abs() < 400 && (y - 5400).abs() < 400),
            "{polygon:?}"
        );
    }

    /// ⇧-click two pictures, 配置 → 左揃え lines them up; グループ化 makes
    /// them one; グループ解除 takes them apart.
    #[gpui::test]
    fn shift_selection_aligns_and_groups(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        for (shape, from, to) in [
            (TextBoxShape::Rectangle, (120.0, 300.0), (220.0, 360.0)),
            (TextBoxShape::Triangle, (300.0, 420.0), (380.0, 500.0)),
        ] {
            view.update(cx, |view, cx| {
                view.run_object_action(ObjectAction::Draw(DrawTool::Shape(shape)), cx)
            });
            drag(&view, cx, from, to);
        }
        let at = on_page(&view, cx, 0, (170.0, 330.0));
        cx.simulate_click(at, Modifiers::none());
        let at = on_page(&view, cx, 0, (340.0, 470.0));
        cx.simulate_click(at, Modifiers::shift());
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            assert_eq!(view.selected_objects().len(), 2);
            view.run_object_action(
                ObjectAction::AlignTo(jade_office_model::word::objects::AlignTo::Selection),
                cx,
            );
            view.run_object_action(
                ObjectAction::Align(jade_office_model::word::objects::Align::Left),
                cx,
            );
            assert!(view.objects.status.is_none(), "{:?}", view.objects.status);
            let lefts: Vec<f64> = view
                .selected_objects()
                .iter()
                .map(|object| object.x)
                .collect();
            assert!((lefts[0] - lefts[1]).abs() < 1.0, "{lefts:?}");
            view.run_object_action(ObjectAction::Group(true), cx);
            assert!(view.objects.status.is_none(), "{:?}", view.objects.status);
        });
        assert_eq!(sorts(&view, cx), vec![ObjectSort::Group]);
        view.update(cx, |view, cx| {
            view.refresh_selection();
            view.run_object_action(ObjectAction::Group(false), cx);
            assert!(view.objects.status.is_none(), "{:?}", view.objects.status);
        });
        assert_eq!(sorts(&view, cx).len(), 2);
        let _ = picture;
    }
}
