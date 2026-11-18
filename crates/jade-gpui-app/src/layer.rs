//! gpui's `deferred`, made safe to nest.
//!
//! gpui panics when a deferred element is drawn inside another
//! ("cannot call defer_draw during deferred drawing"). The ribbon needs
//! exactly that: a group folded away for lack of room opens in a popup, and
//! the lists inside the group (fonts, colours, borders …) drop from it.
//!
//! [`deferred`] here defers as gpui's does, unless it is drawn inside another
//! of these: then it is lifted out and drawn over the outer one once that is
//! drawn, in priority order, as gpui would have.

use std::cell::{Cell, RefCell};

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, Size, Style, Window,
};

thread_local! {
    /// Set while a layer is drawn: the layers inside it, to draw over it.
    static LIFTED: RefCell<Option<Vec<Lifted>>> = const { RefCell::new(None) };
    /// How many layers are being laid out round the element being laid out.
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// A layer inside another, not laid out yet: gpui checks that an element
/// is laid out, prepainted and painted under the same ids, so it is laid out
/// where it is drawn, over the outer layer, as a root of its own.
struct Lifted {
    element: AnyElement,
    origin: Point<Pixels>,
    priority: usize,
}

/// Like `gpui::deferred`: draws `child` after everything else, over it.
pub fn deferred(child: impl IntoElement) -> Layer {
    Layer {
        child: Some(child.into_any_element()),
        root: None,
        priority: 0,
    }
}

pub struct Layer {
    child: Option<AnyElement>,
    /// The child with the layers lifted out of it, laid out in its place.
    root: Option<AnyElement>,
    priority: usize,
}

impl Layer {
    /// Higher priorities are drawn over lower ones.
    pub fn with_priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

impl IntoElement for Layer {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Layer {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut root = Root {
            child: self.child.take().unwrap(),
            lifted: Vec::new(),
        }
        .into_any_element();
        if DEPTH.with(Cell::get) > 0 {
            // Inside another layer: laid out when that one is drawn.
            self.root = Some(root);
            return (window.request_layout(Style::default(), None, cx), ());
        }
        DEPTH.with(|depth| depth.set(depth.get() + 1));
        let layout = root.request_layout(window, cx);
        DEPTH.with(|depth| depth.set(depth.get() - 1));
        self.root = Some(root);
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let element = self.root.take().unwrap();
        let priority = self.priority;
        let unlifted = LIFTED.with(|lifted| match lifted.borrow_mut().as_mut() {
            Some(lifted) => {
                lifted.push(Lifted {
                    element,
                    origin: bounds.origin,
                    priority,
                });
                None
            }
            None => Some(element),
        });
        if let Some(element) = unlifted {
            window.defer_draw(element, window.element_offset(), priority);
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

/// What is drawn: the layer's child, then the layers lifted out of it (each
/// a `Root` again, with its own lifted inside it).
struct Root {
    child: AnyElement,
    lifted: Vec<Lifted>,
}

impl IntoElement for Root {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Root {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let outer = LIFTED.with(|lifted| lifted.borrow_mut().replace(Vec::new()));
        self.child.prepaint(window, cx);
        let mut lifted = LIFTED
            .with(|lifted| std::mem::replace(&mut *lifted.borrow_mut(), outer))
            .unwrap_or_default();
        // As gpui orders its deferred draws; a stable sort keeps tree order
        // among equals.
        lifted.sort_by_key(|layer| layer.priority);
        for layer in &mut lifted {
            DEPTH.with(|depth| depth.set(depth.get() + 1));
            layer.element.prepaint_as_root(
                layer.origin,
                Size::new(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
                window,
                cx,
            );
            DEPTH.with(|depth| depth.set(depth.get() - 1));
        }
        self.lifted = lifted;
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
        for layer in &mut self.lifted {
            layer.element.paint(window, cx);
        }
    }
}
