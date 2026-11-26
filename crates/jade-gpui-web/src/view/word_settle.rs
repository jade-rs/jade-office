//! Headers and footers drawn again after an edit, off the keystroke: a
//! `Page X of N` total an added page changed, or a header's STYLEREF of a
//! heading typed into. Drawing them lays the whole document out — on a long
//! one, seconds — so the keystroke shows first and they catch up shortly
//! after, as jade-core's native host paints its stale pages after the key.

use std::time::Duration;

use gpui::Context;

use super::DocView;

/// How long after an edit the headers and footers are drawn again: long
/// enough that a run of keys is one settle.
const SETTLE_AFTER: Duration = Duration::from_millis(300);

impl DocView {
    /// Schedules [`Self::settle_page_chrome`] when an edit left a header or
    /// footer to draw again and none is scheduled yet. Checked per frame.
    pub(crate) fn schedule_page_chrome_settle(&mut self, cx: &mut Context<Self>) {
        if self.settle_scheduled
            || !self
                .word_ref()
                .is_some_and(|document| document.has_pending_settle())
        {
            return;
        }
        self.settle_scheduled = true;
        let executor = cx.background_executor().clone();
        cx.spawn(async move |view, cx| {
            executor.timer(SETTLE_AFTER).await;
            view.update(cx, |view, cx| view.settle_page_chrome(cx)).ok();
        })
        .detach();
    }

    /// Draws them, and shows the pages it drew.
    fn settle_page_chrome(&mut self, cx: &mut Context<Self>) {
        self.settle_scheduled = false;
        let Some(document) = self.word_mut() else {
            return;
        };
        match document.settle_pending() {
            Ok(true) => {
                self.refresh_repainted();
                cx.notify();
            }
            Ok(false) => {}
            Err(error) => {
                self.status = error.into();
                cx.notify();
            }
        }
    }
}
