//! 校閲 › 音声読み上げ › 読み上げ in the browser: the document read out from
//! the caret by the page's `speechSynthesis`, the word being read selected
//! as its `boundary` events arrive, with the bar of 前へ, 再生 / 一時停止,
//! 次へ, 読み上げの速度 and 閉じる — as the native app's `word_read_aloud`.
//! Where and what to read is `jade-office-model`'s `word::read_aloud`.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use jade_web::word::read_aloud::ReadAloud;

use super::*;

/// What the voice reported.
#[derive(Clone, Debug, PartialEq, Eq)]
enum SpeechEvent {
    Word { start: usize, length: usize },
    Finished,
}

type Events = Rc<RefCell<Vec<SpeechEvent>>>;

/// The page's voice. Outside a browser it says nothing and reports each
/// word in turn, so the reading still walks the document.
struct Voice {
    events: Events,
    #[cfg(target_arch = "wasm32")]
    _handlers: Vec<wasm_bindgen::closure::Closure<dyn FnMut(wasm_bindgen::JsValue)>>,
}

impl Voice {
    fn new() -> Self {
        Self {
            events: Rc::default(),
            #[cfg(target_arch = "wasm32")]
            _handlers: Vec::new(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn synthesis() -> Option<wasm_bindgen::JsValue> {
        js_sys::Reflect::get(&js_sys::global(), &"speechSynthesis".into())
            .ok()
            .filter(|value| !value.is_undefined())
    }

    #[cfg(target_arch = "wasm32")]
    fn call(method: &str) {
        use wasm_bindgen::JsCast;
        let Some(synthesis) = Self::synthesis() else {
            return;
        };
        if let Some(function) = js_sys::Reflect::get(&synthesis, &method.into())
            .ok()
            .and_then(|function| function.dyn_into::<js_sys::Function>().ok())
        {
            function.call0(&synthesis).ok();
        }
    }

    fn speak(&mut self, text: &str, speed: f32) {
        self.events.borrow_mut().clear();
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::closure::Closure;
            use wasm_bindgen::{JsCast, JsValue};
            Self::call("cancel");
            let Some(synthesis) = Self::synthesis() else {
                return self.pretend(text);
            };
            let Some(constructor) =
                js_sys::Reflect::get(&js_sys::global(), &"SpeechSynthesisUtterance".into())
                    .ok()
                    .and_then(|value| value.dyn_into::<js_sys::Function>().ok())
            else {
                return self.pretend(text);
            };
            let Ok(utterance) =
                js_sys::Reflect::construct(&constructor, &js_sys::Array::of1(&text.into()))
            else {
                return self.pretend(text);
            };
            js_sys::Reflect::set(&utterance, &"rate".into(), &JsValue::from_f64(speed.into())).ok();
            let lang = if text.chars().any(|ch| ch as u32 >= 0x3000) {
                "ja-JP"
            } else {
                "en-US"
            };
            js_sys::Reflect::set(&utterance, &"lang".into(), &lang.into()).ok();
            let events = Rc::clone(&self.events);
            let utf16: Vec<u16> = text.encode_utf16().collect();
            let boundary = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                let get = |name: &str| {
                    js_sys::Reflect::get(&event, &name.into())
                        .ok()
                        .and_then(|value| value.as_f64())
                };
                let is_word = js_sys::Reflect::get(&event, &"name".into())
                    .ok()
                    .and_then(|value| value.as_string())
                    .is_none_or(|name| name == "word");
                let Some(start) = get("charIndex").filter(|_| is_word) else {
                    return;
                };
                let start = start as usize;
                // Not every browser gives the length: to the next space.
                let length = get("charLength")
                    .filter(|length| *length > 0.0)
                    .map(|length| length as usize)
                    .unwrap_or_else(|| {
                        utf16[start.min(utf16.len())..]
                            .iter()
                            .position(|unit| {
                                char::from_u32(u32::from(*unit)).is_some_and(char::is_whitespace)
                            })
                            .unwrap_or(utf16.len().saturating_sub(start))
                    });
                events
                    .borrow_mut()
                    .push(SpeechEvent::Word { start, length });
            });
            let events = Rc::clone(&self.events);
            let end = Closure::<dyn FnMut(JsValue)>::new(move |_event: JsValue| {
                events.borrow_mut().push(SpeechEvent::Finished);
            });
            js_sys::Reflect::set(
                &utterance,
                &"onboundary".into(),
                boundary.as_ref().unchecked_ref(),
            )
            .ok();
            js_sys::Reflect::set(&utterance, &"onend".into(), end.as_ref().unchecked_ref()).ok();
            self._handlers = vec![boundary, end];
            if let Some(speak) = js_sys::Reflect::get(&synthesis, &"speak".into())
                .ok()
                .and_then(|function| function.dyn_into::<js_sys::Function>().ok())
            {
                speak.call1(&synthesis, &utterance).ok();
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = speed;
            self.pretend(text);
        }
    }

    /// Each word reported in turn, then the end.
    fn pretend(&mut self, text: &str) {
        let mut events = self.events.borrow_mut();
        let mut units = 0;
        let mut word: Option<usize> = None;
        for ch in text.chars().chain(std::iter::once(' ')) {
            if ch.is_whitespace() {
                if let Some(start) = word.take() {
                    events.push(SpeechEvent::Word {
                        start,
                        length: units - start,
                    });
                }
            } else if word.is_none() {
                word = Some(units);
            }
            units += ch.len_utf16();
        }
        events.push(SpeechEvent::Finished);
    }

    fn pause(&mut self) {
        #[cfg(target_arch = "wasm32")]
        Self::call("pause");
    }

    fn resume(&mut self) {
        #[cfg(target_arch = "wasm32")]
        Self::call("resume");
    }

    fn stop(&mut self) {
        self.events.borrow_mut().clear();
        #[cfg(target_arch = "wasm32")]
        Self::call("cancel");
    }

    fn events(&mut self) -> Vec<SpeechEvent> {
        let mut events = self.events.borrow_mut();
        if cfg!(target_arch = "wasm32") {
            std::mem::take(&mut *events)
        } else if events.is_empty() {
            Vec::new()
        } else {
            vec![events.remove(0)]
        }
    }
}

/// The reading, and the voice reading it.
pub(crate) struct WebReading {
    reading: ReadAloud,
    voice: Voice,
    ticking: bool,
}

const TICK: Duration = Duration::from_millis(40);

impl DocView {
    fn reading_mut(&mut self) -> Option<&mut WebReading> {
        match &mut self.document {
            Document::Pages { read_aloud, .. } => read_aloud.as_deref_mut(),
            _ => None,
        }
    }

    /// 読み上げ: start at the caret, or stop.
    pub(super) fn toggle_read_aloud(&mut self, cx: &mut Context<Self>) {
        let Document::Pages {
            document,
            read_aloud,
            ..
        } = &mut self.document
        else {
            return;
        };
        if let Some(mut reading) = read_aloud.take() {
            reading.voice.stop();
            return cx.notify();
        }
        let from = document
            .selection()
            .map(|(start, end)| start.min(end))
            .or_else(|| document.caret())
            .unwrap_or((0, 0));
        let Some(reading) = ReadAloud::start(&**document, from, 1.0) else {
            self.status = i18n::text("read.nothing").into();
            return cx.notify();
        };
        *read_aloud = Some(Box::new(WebReading {
            reading,
            voice: Voice::new(),
            ticking: false,
        }));
        self.speak_current();
        self.start_reading_ticker(cx);
        cx.notify();
    }

    fn speak_current(&mut self) {
        let Some(ui) = self.reading_mut() else {
            return;
        };
        if let Some(text) = ui
            .reading
            .utterance()
            .map(|utterance| utterance.text.clone())
        {
            let speed = ui.reading.speed;
            ui.voice.speak(&text, speed);
            ui.reading.paused = false;
        }
    }

    fn stop_reading(&mut self, cx: &mut Context<Self>) {
        if let Document::Pages { read_aloud, .. } = &mut self.document {
            if let Some(mut reading) = read_aloud.take() {
                reading.voice.stop();
            }
        }
        cx.notify();
    }

    fn reading_step(&mut self, forwards: bool, cx: &mut Context<Self>) {
        let Some(ui) = self.reading_mut() else {
            return;
        };
        if forwards {
            if !ui.reading.forward() {
                return self.stop_reading(cx);
            }
        } else {
            ui.reading.back();
        }
        self.speak_current();
        self.start_reading_ticker(cx);
        cx.notify();
    }

    fn reading_play_pause(&mut self, cx: &mut Context<Self>) {
        if let Some(ui) = self.reading_mut() {
            if ui.reading.paused {
                ui.voice.resume();
            } else {
                ui.voice.pause();
            }
            ui.reading.paused = !ui.reading.paused;
        }
        cx.notify();
    }

    fn reading_speed(&mut self, faster: bool, cx: &mut Context<Self>) {
        if let Some(ui) = self.reading_mut() {
            ui.reading.step_speed(faster);
        }
        self.speak_current();
        cx.notify();
    }

    /// The voice's reports: the word selected, the next paragraph after
    /// each. `false` once the reading is over.
    pub(super) fn reading_tick(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(ui) = self.reading_mut() else {
            return false;
        };
        let mut word = None;
        let mut finished = false;
        for event in ui.voice.events() {
            match event {
                SpeechEvent::Word { start, length } => word = ui.reading.word(start, length),
                SpeechEvent::Finished => finished = true,
            }
        }
        if let Some((start, end)) = word {
            self.word_move(cx, |document| document.select(start, end));
            self.reveal_caret();
        }
        if finished {
            let Some(ui) = self.reading_mut() else {
                return false;
            };
            if ui.reading.forward() {
                self.speak_current();
            } else {
                self.stop_reading(cx);
                return false;
            }
        }
        true
    }

    fn start_reading_ticker(&mut self, cx: &mut Context<Self>) {
        let Some(ui) = self.reading_mut() else {
            return;
        };
        if ui.ticking {
            return;
        }
        ui.ticking = true;
        let executor = cx.background_executor().clone();
        cx.spawn(async move |view, cx| loop {
            executor.timer(TICK).await;
            let going = view
                .update(cx, |view, cx| view.reading_tick(cx))
                .unwrap_or(false);
            if !going {
                break;
            }
        })
        .detach();
    }

    /// The bar above the pages while reading.
    pub(super) fn read_aloud_bar(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let Document::Pages {
            read_aloud: Some(ui),
            ..
        } = &self.document
        else {
            return None;
        };
        let paused = ui.reading.paused;
        let speed = format!("{}×", ui.reading.speed);
        let button = |id: &'static str,
                      label: &'static str,
                      run: fn(&mut DocView, &mut Context<DocView>),
                      cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .min_w(px(28.0))
                .h(px(26.0))
                .px_1()
                .rounded_md()
                .cursor_pointer()
                .hover(|button| button.bg(palette::hovered()))
                .text_color(palette::text())
                .child(label)
                .on_click(cx.listener(move |view, _event, _window, cx| run(view, cx)))
        };
        let controls = div()
            .flex()
            .items_center()
            .gap_1()
            .p_1()
            .rounded_md()
            .border_1()
            .border_color(palette::border())
            .bg(palette::panel())
            .text_sm()
            .child(button(
                "read-previous",
                "⏮",
                |view, cx| view.reading_step(false, cx),
                cx,
            ))
            .child(button(
                "read-play",
                if paused { "▶" } else { "⏸" },
                |view, cx| view.reading_play_pause(cx),
                cx,
            ))
            .child(button(
                "read-next",
                "⏭",
                |view, cx| view.reading_step(true, cx),
                cx,
            ))
            .child(button(
                "read-slower",
                "−",
                |view, cx| view.reading_speed(false, cx),
                cx,
            ))
            .child(div().text_xs().text_color(palette::dim()).child(speed))
            .child(button(
                "read-faster",
                "+",
                |view, cx| view.reading_speed(true, cx),
                cx,
            ))
            .child(button(
                "read-close",
                "×",
                |view, cx| view.stop_reading(cx),
                cx,
            ));
        Some(
            div()
                .flex()
                .flex_none()
                .justify_center()
                .py_1()
                .bg(palette::canvas())
                .child(controls)
                .into_any_element(),
        )
    }
}
