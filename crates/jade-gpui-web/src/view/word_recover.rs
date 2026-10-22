//! 自動回復 and バックアップ in the browser, as Word のオプション → 保存 sets
//! them (`jade-office-model`'s `word::recovery`):
//!
//! - while a Word document has unsaved edits, a copy of it goes into the
//!   browser's own store (IndexedDB, the page's `jadeStore`) every few
//!   minutes;
//! - a save (the download) or closing the document with nothing unsaved
//!   takes the copy away; closing the tab or the browser with edits unsaved
//!   leaves it, and the next visit offers it back (回復されたファイル, in
//!   the explorer);
//! - バックアップ ファイルを常に作成する: each save downloads, beside the
//!   file, the version it replaces as `<name> のバックアップ.wbk`.

use std::rc::Rc;
use std::time::Duration;

use gpui::{div, prelude::*, px, AnyElement, Context};
use jade_web::word::recovery::{backup_name, RecoverySettings, INTERVALS};

use super::word_kit::{check, chip, on};
use super::{word_views::settings, DocView, Document};
use crate::{i18n, palette as theme};

const SETTINGS: &str = "save.txt";

/// How often a document looks whether a copy is due.
const TICK: Duration = Duration::from_secs(20);

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// The save settings.
pub fn recovery_settings() -> RecoverySettings {
    settings::load(SETTINGS)
        .map(|text| RecoverySettings::parse(&text))
        .unwrap_or_default()
}

fn set_recovery_settings(changed: RecoverySettings) {
    settings::save(SETTINGS, &changed.format());
}

/// Milliseconds since the epoch.
pub fn now() -> f64 {
    js_sys::Date::now()
}

/// A document's recovery state.
#[derive(Default)]
pub struct Recovery {
    /// Its copy's key in the store: the name and when it opened, so two
    /// documents of one name keep two copies.
    pub key: String,
    /// When the last copy was made.
    pub last: Option<f64>,
    /// Whether the store holds a copy.
    pub stored: bool,
    /// Whether the clock is running.
    pub ticking: bool,
    /// The file as last saved (or opened): what a backup keeps.
    pub saved: Option<Rc<Vec<u8>>>,
}

/// The page's store, through `globalThis.jadeStore`.
pub(crate) mod store {
    use wasm_bindgen::{JsCast, JsValue};

    fn get(object: &JsValue, name: &str) -> Option<JsValue> {
        js_sys::Reflect::get(object, &JsValue::from_str(name))
            .ok()
            .filter(|value| !value.is_undefined() && !value.is_null())
    }

    fn call(name: &str, arguments: &[JsValue]) -> Option<js_sys::Promise> {
        let store = get(&js_sys::global(), "jadeStore")?;
        let method = get(&store, name)?.dyn_into::<js_sys::Function>().ok()?;
        let arguments = arguments.iter().cloned().collect::<js_sys::Array>();
        method
            .apply(&store, &arguments)
            .ok()?
            .dyn_into::<js_sys::Promise>()
            .ok()
    }

    async fn awaited(promise: Option<js_sys::Promise>) -> Option<JsValue> {
        wasm_bindgen_futures::JsFuture::from(promise?).await.ok()
    }

    /// A record: the document's name, when, and its bytes.
    fn record(name: &str, time: f64, bytes: &[u8]) -> JsValue {
        let record = js_sys::Object::new();
        js_sys::Reflect::set(&record, &"name".into(), &JsValue::from_str(name)).ok();
        js_sys::Reflect::set(&record, &"time".into(), &JsValue::from_f64(time)).ok();
        js_sys::Reflect::set(
            &record,
            &"bytes".into(),
            &js_sys::Uint8Array::from(bytes).into(),
        )
        .ok();
        record.into()
    }

    /// Stores `bytes` under `key`; the promise is left to finish.
    pub fn put(store: &str, key: &str, name: &str, time: f64, bytes: &[u8]) {
        let promise = call(
            "put",
            &[
                JsValue::from_str(store),
                JsValue::from_str(key),
                record(name, time, bytes),
            ],
        );
        wasm_bindgen_futures::spawn_local(async move {
            awaited(promise).await;
        });
    }

    pub fn put_template(key: &str, bytes: &[u8]) {
        put("templates", key, key, super::now(), bytes);
    }

    pub fn remove(store: &str, key: &str) {
        let promise = call(
            "remove",
            &[JsValue::from_str(store), JsValue::from_str(key)],
        );
        wasm_bindgen_futures::spawn_local(async move {
            awaited(promise).await;
        });
    }

    fn bytes_of(record: &JsValue) -> Option<Vec<u8>> {
        let bytes = get(record, "bytes")?
            .dyn_into::<js_sys::Uint8Array>()
            .ok()?;
        Some(bytes.to_vec())
    }

    /// One record's bytes.
    pub async fn bytes(store: &str, key: &str) -> Option<Vec<u8>> {
        let record = awaited(call(
            "get",
            &[JsValue::from_str(store), JsValue::from_str(key)],
        ))
        .await?;
        bytes_of(&record)
    }

    /// Every record of `store`: its key, name, time and bytes.
    pub async fn all(store: &str) -> Vec<(String, String, f64, Vec<u8>)> {
        let Some(records) = awaited(call("all", &[JsValue::from_str(store)])).await else {
            return Vec::new();
        };
        js_sys::Array::from(&records)
            .iter()
            .filter_map(|record| {
                Some((
                    get(&record, "key")?.as_string()?,
                    get(&record, "name")?.as_string()?,
                    get(&record, "time")?.as_f64()?,
                    bytes_of(&record)?,
                ))
            })
            .collect()
    }
}

impl DocView {
    /// Starts the document's clock, once: every few seconds it looks
    /// whether a copy is due.
    pub(crate) fn start_recovery(&mut self, cx: &mut Context<Self>) {
        if self.recovery.ticking || !matches!(self.document, Document::Pages { .. }) {
            return;
        }
        self.recovery.ticking = true;
        if self.recovery.key.is_empty() {
            self.recovery.key = format!("{}-{:.0}", self.name, now());
        }
        let executor = cx.background_executor().clone();
        cx.spawn(async move |view, cx| loop {
            executor.timer(TICK).await;
            if view.update(cx, |view, _cx| view.recovery_tick()).is_err() {
                break;
            }
        })
        .detach();
    }

    /// Makes a copy when one is due.
    fn recovery_tick(&mut self) {
        let settings = recovery_settings();
        if settings.due(self.is_dirty(), self.recovery.last, now()) {
            self.recovery_snapshot();
        }
    }

    /// Makes the copy now.
    pub(crate) fn recovery_snapshot(&mut self) {
        let Some(Ok(docx)) = self.word_ref().map(|document| document.word_docx()) else {
            return;
        };
        let time = now();
        store::put("recovery", &self.recovery.key, &self.name, time, &docx);
        self.recovery.last = Some(time);
        self.recovery.stored = true;
    }

    /// After a save of `bytes`: the copy goes, and with backups on, the
    /// version the save replaces is downloaded beside it.
    pub(crate) fn recovery_saved(&mut self, bytes: &[u8]) {
        if recovery_settings().always_backup {
            if let Some(previous) = self.recovery.saved.as_ref() {
                let name = backup_name(&self.name, !i18n::is_english());
                self.host.save(&name, previous);
            }
        }
        self.recovery.saved = Some(Rc::new(bytes.to_vec()));
        self.recovery.last = None;
        if std::mem::take(&mut self.recovery.stored) {
            store::remove("recovery", &self.recovery.key);
        }
    }

    /// Closing the document: with nothing unsaved its copy goes; with edits
    /// unsaved a fresh one stays, to be offered back.
    pub fn recovery_closing(&mut self) {
        if !matches!(self.document, Document::Pages { .. }) {
            return;
        }
        if self.is_dirty() {
            if recovery_settings().enabled {
                self.recovery_snapshot();
            }
        } else if self.recovery.stored {
            store::remove("recovery", &self.recovery.key);
        }
    }

    /// The key this document's copy is stored under, if it has one.
    pub fn recovery_key(&self) -> Option<String> {
        (!self.recovery.key.is_empty()).then(|| self.recovery.key.clone())
    }
}

/// Word のオプション → 保存: 自動回復 and its interval, バックアップ.
pub fn options_page(cx: &Context<DocView>) -> AnyElement {
    let current = recovery_settings();
    let intervals = INTERVALS.into_iter().map(|minutes| {
        chip(
            ("autorecover-minutes", minutes as usize),
            minutes.to_string(),
            current.minutes == minutes,
        )
        .on_click(on(cx, move |_view, cx| {
            set_recovery_settings(RecoverySettings {
                minutes,
                ..recovery_settings()
            });
            cx.notify();
        }))
        .into_any_element()
    });
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(theme::text_dim())
                .child(t("prefs.save_options")),
        )
        .child(
            check(
                "options-backup",
                t("prefs.always_backup"),
                current.always_backup,
            )
            .on_click(on(cx, |_view, cx| {
                let settings = recovery_settings();
                set_recovery_settings(RecoverySettings {
                    always_backup: !settings.always_backup,
                    ..settings
                });
                cx.notify();
            })),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_1()
                .child(
                    check(
                        "options-autorecover",
                        t("prefs.autorecover"),
                        current.enabled,
                    )
                    .on_click(on(cx, |_view, cx| {
                        let settings = recovery_settings();
                        set_recovery_settings(RecoverySettings {
                            enabled: !settings.enabled,
                            ..settings
                        });
                        cx.notify();
                    })),
                )
                .children(intervals)
                .child(
                    div()
                        .text_color(theme::text_dim())
                        .child(t("prefs.minutes")),
                ),
        )
        .child(
            div()
                .w(px(480.0))
                .text_color(theme::text_faint())
                .child(t("recovered.browser_note")),
        )
        .into_any_element()
}
