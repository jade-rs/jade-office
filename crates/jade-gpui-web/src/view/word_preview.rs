//! リアルタイムのプレビュー in the browser: resting the pointer on a font, a
//! size, a colour, a highlight, shading or a style in a ribbon list shows it
//! on the selection; leaving takes it back and a click keeps it — the
//! command applied and undone, as the native app does it.

use super::*;
use crate::ribbon::Action;

/// The entry being previewed.
pub(crate) struct Preview {
    action: Action,
}

/// Whether hovering `action` previews it.
pub(crate) fn previewable(action: &Action) -> bool {
    matches!(
        action,
        Action::FontFamilyName(_)
            | Action::FontSize(_)
            | Action::FontColor(_)
            | Action::Highlight(_)
            | Action::Shading(_)
            | Action::Style(_)
    )
}

/// The pointer entering (`true`) or leaving a list entry.
pub(crate) fn hover(
    action: Action,
    cx: &mut Context<DocView>,
) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    let view = cx.weak_entity();
    move |entered, window, cx| {
        let action = action.clone();
        let entered = *entered;
        view.update(cx, move |view, cx| {
            if entered {
                start(view, action, window, cx);
            } else if view
                .preview
                .as_ref()
                .is_some_and(|preview| preview.action == action)
            {
                end(view, cx);
            }
        })
        .ok();
    }
}

fn start(view: &mut DocView, action: Action, window: &mut Window, cx: &mut Context<DocView>) {
    if view
        .preview
        .as_ref()
        .is_some_and(|preview| preview.action == action)
    {
        return;
    }
    end(view, cx);
    let Document::Pages {
        document, ribbon, ..
    } = &view.document
    else {
        return;
    };
    if document.selection().is_none() {
        return;
    }
    let popup = ribbon.popup;
    view.previewing = true;
    view.ribbon_action(action.clone(), window, cx);
    view.previewing = false;
    if let Document::Pages { ribbon, .. } = &mut view.document {
        // The list stays open under the pointer.
        ribbon.popup = popup;
    }
    view.preview = Some(Preview { action });
    cx.notify();
}

/// Takes the preview back off.
pub(crate) fn end(view: &mut DocView, cx: &mut Context<DocView>) {
    if view.preview.take().is_none() {
        return;
    }
    view.word_edit(cx, |document| document.undo().map(|_| ()));
}

/// Before a ribbon action: a click on the entry being previewed keeps it;
/// anything else takes the preview off first. `true` when the action is
/// already done.
pub(crate) fn settle(view: &mut DocView, action: &Action, cx: &mut Context<DocView>) -> bool {
    if view.previewing {
        return false;
    }
    if view
        .preview
        .as_ref()
        .is_some_and(|preview| &preview.action == action)
    {
        view.preview = None;
        if let Document::Pages { ribbon, .. } = &mut view.document {
            ribbon.popup = None;
        }
        cx.notify();
        return true;
    }
    if view.preview.is_some() {
        end(view, cx);
    }
    false
}
