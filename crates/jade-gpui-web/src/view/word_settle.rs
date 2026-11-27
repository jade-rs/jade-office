//! Headers and footers drawn again after an edit, off the keystroke: a
//! `Page X of N` total an added page changed, or a header's STYLEREF of a
//! heading typed into. Drawing them lays the whole document out — on a long
//! one, seconds — so the keystroke shows first and they catch up shortly
//! after, as jade-core's native host paints its stale pages after the key.

use std::time::Duration;

use gpui::Context;
use wasm_bindgen::{JsCast, JsValue};

use super::DocView;

/// How long the edits must stop for before the headers and footers are
/// drawn again: each key starts the wait afresh, so a run of keys in a
/// heading is not held up halfway by the whole-document pass, and is one
/// settle at its end.
const SETTLE_QUIET: Duration = Duration::from_millis(300);

/// The longest a header or footer stays wrong, from the first frame that
/// found it so: a stream of edits that never pauses for [`SETTLE_QUIET`]
/// cannot put the settle off for good.
const SETTLE_MAX_WAIT: Duration = Duration::from_millis(1500);

/// How long is left to wait at `now` (ms) for a settle pending since
/// `first_pending` whose edits last moved at `last_edit`, or `None` while
/// the last try failed and no edit has come since: see
/// [`jade_web::editing::settle_wait`], which is tested natively (this crate
/// builds for wasm32 only).
fn settle_wait(first_pending: f64, last_edit: f64, now: f64, failed: bool) -> Option<Duration> {
    jade_web::editing::settle_wait(
        first_pending,
        last_edit,
        now,
        failed,
        SETTLE_QUIET,
        SETTLE_MAX_WAIT,
    )
}

/// Milliseconds on the page's monotonic clock (`performance.now()`): the
/// waits above are measured on it, which the wall clock going back (a
/// sync, a time zone change) cannot stretch. `Date.now()` where there is no
/// `performance`.
fn now() -> f64 {
    let global = js_sys::global();
    js_sys::Reflect::get(&global, &JsValue::from_str("performance"))
        .ok()
        .filter(|performance| performance.is_object())
        .and_then(|performance| {
            let now = js_sys::Reflect::get(&performance, &JsValue::from_str("now")).ok()?;
            now.dyn_into::<js_sys::Function>()
                .ok()?
                .call0(&performance)
                .ok()?
                .as_f64()
        })
        .unwrap_or_else(js_sys::Date::now)
}

/// When the headers and footers an edit left wrong are to be drawn again.
#[derive(Default)]
pub(crate) struct SettleClock {
    /// The first frame (ms) that found a settle pending, while one is.
    first_pending: Option<f64>,
    /// When the document's edit count last moved (ms).
    last_edit: f64,
    /// [`jade_web::editing::WordDoc::deferred_settles`] as last seen.
    edits_seen: u64,
    /// Whether a timer is waiting to settle.
    waiting: bool,
    /// The edit count a settle failed at (a render error): no other is
    /// tried, each a whole-document pass, until the count moves.
    failed_at: Option<u64>,
}

impl SettleClock {
    /// Whether the last settle failed and no edit has come since.
    fn latched(&self) -> bool {
        self.failed_at == Some(self.edits_seen)
    }
}

impl DocView {
    /// Notes, per frame, whether an edit left a header or footer to draw
    /// again and when the edits last moved, and starts the timer that
    /// draws them once they are due ([`settle_wait`]).
    pub(crate) fn schedule_page_chrome_settle(&mut self, cx: &mut Context<Self>) {
        let Some((pending, edits)) = self
            .word_ref()
            .map(|document| (document.has_pending_settle(), document.deferred_settles()))
        else {
            return;
        };
        let clock = &mut self.settle;
        if clock.failed_at.is_some_and(|failed| failed != edits) {
            // An edit since the failure: a settle may be tried again.
            clock.failed_at = None;
        }
        if !pending || clock.failed_at == Some(edits) {
            clock.first_pending = None;
            clock.edits_seen = edits;
            return;
        }
        let at = now();
        if clock.first_pending.is_none() {
            clock.first_pending = Some(at);
            clock.last_edit = at;
        } else if clock.edits_seen != edits {
            clock.last_edit = at;
        }
        clock.edits_seen = edits;
        if clock.waiting {
            return;
        }
        clock.waiting = true;
        let executor = cx.background_executor().clone();
        cx.spawn(async move |view, cx| {
            let mut wait = SETTLE_QUIET;
            loop {
                executor.timer(wait).await;
                match view.update(cx, |view, cx| view.settle_when_due(cx)) {
                    Ok(Some(more)) => wait = more,
                    Ok(None) | Err(_) => break,
                }
            }
        })
        .detach();
    }

    /// Draws them when due; otherwise how much longer to wait.
    fn settle_when_due(&mut self, cx: &mut Context<Self>) -> Option<Duration> {
        let clock = &mut self.settle;
        let Some(first_pending) = clock.first_pending else {
            // Settled meanwhile, by a whole-document repaint.
            clock.waiting = false;
            return None;
        };
        let Some(wait) = settle_wait(first_pending, clock.last_edit, now(), clock.latched()) else {
            clock.waiting = false;
            clock.first_pending = None;
            return None;
        };
        if !wait.is_zero() {
            return Some(wait);
        }
        clock.waiting = false;
        clock.first_pending = None;
        let edits = clock.edits_seen;
        let document = self.word_mut()?;
        match document.settle_pending() {
            Ok(true) => {
                self.refresh_repainted();
                cx.notify();
            }
            Ok(false) => {}
            Err(error) => {
                // Not tried again until the next edit: see `failed_at`.
                self.settle.failed_at = Some(edits);
                // What the pass drew before it failed is shown all the same.
                self.refresh_repainted();
                self.status = error.into();
                cx.notify();
            }
        }
        None
    }
}
