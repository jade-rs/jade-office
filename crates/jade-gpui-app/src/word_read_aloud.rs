//! 校閲 › 音声読み上げ › 読み上げ: the document read out from the caret in
//! the system voice, the word being read selected as it is spoken, with a
//! small bar over the pages — 前へ, 再生 / 一時停止, 次へ, 読み上げの速度 and
//! 閉じる — as Word's. What is read and where the voice is in the document
//! is `jade-office-model`'s `word::read_aloud`; the voice is
//! `NSSpeechSynthesizer`, whose word callbacks drive the selection.

use std::time::Duration;

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::word::read_aloud::{ReadAloud, WORDS_PER_MINUTE};

use crate::tooltip::Tooltip;
use crate::{i18n, theme, DocumentView};

/// What a voice reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpeechEvent {
    /// About to say the word at this UTF-16 range of what it was given.
    Word { start: usize, length: usize },
    /// Done with what it was given.
    Finished,
}

/// A voice.
pub trait Speaker {
    /// Says `text`, at `words_per_minute`, dropping whatever it was saying.
    fn speak(&mut self, text: &str, words_per_minute: f32);
    fn pause(&mut self);
    fn resume(&mut self);
    fn stop(&mut self);
    /// What it reported since the last call.
    fn events(&mut self) -> Vec<SpeechEvent>;
}

/// The reading, and the voice reading it.
pub struct ReadAloudUi {
    pub reading: ReadAloud,
    pub speaker: Box<dyn Speaker>,
    ticking: bool,
}

/// How often the voice's reports are looked at.
const TICK: Duration = Duration::from_millis(40);

fn system_speaker() -> Box<dyn Speaker> {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        Box::new(mac::MacSpeaker::new())
    }
    #[cfg(any(not(target_os = "macos"), test))]
    {
        Box::new(Silent::default())
    }
}

/// A voice that says nothing and reports each word at once: for tests, and
/// where there is no system voice.
#[derive(Default)]
#[cfg_attr(all(target_os = "macos", not(test)), allow(dead_code))]
pub struct Silent {
    pending: Vec<SpeechEvent>,
}

impl Speaker for Silent {
    fn speak(&mut self, text: &str, _words_per_minute: f32) {
        self.pending.clear();
        let mut units = 0;
        let mut word: Option<usize> = None;
        for ch in text.chars().chain(std::iter::once(' ')) {
            if ch.is_whitespace() {
                if let Some(start) = word.take() {
                    self.pending.push(SpeechEvent::Word {
                        start,
                        length: units - start,
                    });
                }
            } else if word.is_none() {
                word = Some(units);
            }
            units += ch.len_utf16();
        }
        self.pending.push(SpeechEvent::Finished);
    }
    fn pause(&mut self) {}
    fn resume(&mut self) {}
    fn stop(&mut self) {
        self.pending.clear();
    }
    fn events(&mut self) -> Vec<SpeechEvent> {
        // One at a time, as a voice would.
        if self.pending.is_empty() {
            Vec::new()
        } else {
            vec![self.pending.remove(0)]
        }
    }
}

/// 読み上げ: start reading at the caret, or stop.
pub fn toggle(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.review.read_aloud.is_some() {
        return stop(view, cx);
    }
    let from = view
        .selection_anchor
        .map(|anchor| anchor.min(view.insert_at))
        .unwrap_or(view.insert_at);
    let Some(reading) = ReadAloud::start(&view.document, from, 1.0) else {
        view.table_status = Some(i18n::text("read.nothing").into());
        return cx.notify();
    };
    view.review.read_aloud = Some(ReadAloudUi {
        reading,
        speaker: system_speaker(),
        ticking: false,
    });
    speak_current(view);
    start_ticker(view, cx);
    cx.notify();
}

/// 閉じる: the voice stops and the bar goes.
pub fn stop(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if let Some(mut ui) = view.review.read_aloud.take() {
        ui.speaker.stop();
    }
    cx.notify();
}

fn speak_current(view: &mut DocumentView) {
    let Some(ui) = view.review.read_aloud.as_mut() else {
        return;
    };
    let rate = WORDS_PER_MINUTE * ui.reading.speed;
    if let Some(text) = ui
        .reading
        .utterance()
        .map(|utterance| utterance.text.clone())
    {
        ui.speaker.speak(&text, rate);
        ui.reading.paused = false;
    }
}

/// 再生 / 一時停止.
pub fn play_pause(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if let Some(ui) = view.review.read_aloud.as_mut() {
        if ui.reading.paused {
            ui.speaker.resume();
        } else {
            ui.speaker.pause();
        }
        ui.reading.paused = !ui.reading.paused;
    }
    cx.notify();
}

/// 前へ (`false`) / 次へ: a paragraph back or on.
pub fn step(view: &mut DocumentView, forwards: bool, cx: &mut Context<DocumentView>) {
    let Some(ui) = view.review.read_aloud.as_mut() else {
        return;
    };
    if forwards {
        if !ui.reading.forward() {
            return stop(view, cx);
        }
    } else {
        ui.reading.back();
    }
    speak_current(view);
    start_ticker(view, cx);
    cx.notify();
}

/// 読み上げの速度: faster or slower, from the next paragraph… or at once,
/// by saying the rest of this one again at the new speed.
pub fn change_speed(view: &mut DocumentView, faster: bool, cx: &mut Context<DocumentView>) {
    if let Some(ui) = view.review.read_aloud.as_mut() {
        ui.reading.step_speed(faster);
    }
    speak_current(view);
    cx.notify();
}

/// What the voice reported: the word it is on selected, the next paragraph
/// when it is done with one. `false` once the reading is over.
pub fn tick(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> bool {
    let Some(ui) = view.review.read_aloud.as_mut() else {
        return false;
    };
    let mut word = None;
    let mut finished = false;
    for event in ui.speaker.events() {
        match event {
            SpeechEvent::Word { start, length } => word = ui.reading.word(start, length),
            SpeechEvent::Finished => finished = true,
        }
    }
    if let Some((start, end)) = word {
        view.select_range(start, end, cx);
    }
    if finished {
        let Some(ui) = view.review.read_aloud.as_mut() else {
            return false;
        };
        if ui.reading.forward() {
            speak_current(view);
        } else {
            stop(view, cx);
            return false;
        }
    }
    true
}

fn start_ticker(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(ui) = view.review.read_aloud.as_mut() else {
        return;
    };
    if ui.ticking || cfg!(test) {
        return;
    }
    ui.ticking = true;
    let executor = cx.background_executor().clone();
    cx.spawn(async move |view, cx| loop {
        executor.timer(TICK).await;
        let going = view.update(cx, tick).unwrap_or(false);
        if !going {
            break;
        }
    })
    .detach();
}

fn bar_button(
    id: &'static str,
    label: impl Into<gpui::SharedString>,
    tooltip: &'static str,
    cx: &mut Context<DocumentView>,
    run: fn(&mut DocumentView, &mut Context<DocumentView>),
) -> AnyElement {
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
        .hover(|button| button.bg(theme::hovered()))
        .text_color(theme::text())
        .child(label.into())
        .tooltip(Tooltip::text(i18n::text(tooltip)))
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| run(view, cx)))
        .into_any_element()
}

/// The bar above the pages while reading.
pub fn bar(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let ui = view.review.read_aloud.as_ref()?;
    let paused = ui.reading.paused;
    let speed = format!("{}×", ui.reading.speed);
    let controls = div()
        .id("read-aloud-bar")
        .flex()
        .items_center()
        .gap_1()
        .p_1()
        .rounded_md()
        .border_1()
        .border_color(theme::border())
        .bg(theme::panel())
        .shadow_md()
        .text_sm()
        .child(bar_button(
            "read-previous",
            "⏮",
            "read.previous",
            cx,
            |view, cx| step(view, false, cx),
        ))
        .child(bar_button(
            "read-play",
            if paused { "▶" } else { "⏸" },
            if paused { "read.play" } else { "read.pause" },
            cx,
            play_pause,
        ))
        .child(bar_button(
            "read-next",
            "⏭",
            "read.next",
            cx,
            |view, cx| step(view, true, cx),
        ))
        .child(bar_button(
            "read-slower",
            "−",
            "read.slower",
            cx,
            |view, cx| change_speed(view, false, cx),
        ))
        .child(div().text_xs().text_color(theme::text_dim()).child(speed))
        .child(bar_button(
            "read-faster",
            "+",
            "read.faster",
            cx,
            |view, cx| change_speed(view, true, cx),
        ))
        .child(bar_button("read-close", "×", "read.stop", cx, stop));
    Some(
        div()
            .flex()
            .flex_none()
            .justify_center()
            .py_1()
            .bg(theme::canvas())
            .child(controls)
            .into_any_element(),
    )
}

#[cfg(all(target_os = "macos", not(test)))]
#[allow(deprecated)]
mod mac {
    //! `NSSpeechSynthesizer`: its delegate's word and finish callbacks come
    //! on the main run loop, into a queue the ticker empties.

    use std::cell::RefCell;

    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{NSSpeechBoundary, NSSpeechSynthesizer, NSSpeechSynthesizerDelegate};
    use objc2_foundation::{NSRange, NSString};

    use super::{Speaker, SpeechEvent};

    thread_local! {
        static EVENTS: RefCell<Vec<SpeechEvent>> = const { RefCell::new(Vec::new()) };
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "JadeReadAloudDelegate"]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl NSSpeechSynthesizerDelegate for Delegate {
            #[unsafe(method(speechSynthesizer:willSpeakWord:ofString:))]
            fn will_speak_word(
                &self,
                _sender: &NSSpeechSynthesizer,
                range: NSRange,
                _string: &NSString,
            ) {
                EVENTS.with(|events| {
                    events.borrow_mut().push(SpeechEvent::Word {
                        start: range.location,
                        length: range.length,
                    })
                });
            }

            #[unsafe(method(speechSynthesizer:didFinishSpeaking:))]
            fn did_finish(&self, _sender: &NSSpeechSynthesizer, finished: bool) {
                // `false` is a stop: the reading decides what follows.
                if finished {
                    EVENTS.with(|events| events.borrow_mut().push(SpeechEvent::Finished));
                }
            }
        }
    );

    impl Delegate {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(());
            unsafe { msg_send![super(this), init] }
        }
    }

    pub struct MacSpeaker {
        synthesizer: Retained<NSSpeechSynthesizer>,
        _delegate: Retained<Delegate>,
    }

    impl MacSpeaker {
        pub fn new() -> Self {
            let mtm = MainThreadMarker::new().expect("the windows are on the main thread");
            let synthesizer = NSSpeechSynthesizer::new();
            let delegate = Delegate::new(mtm);
            synthesizer.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            Self {
                synthesizer,
                _delegate: delegate,
            }
        }
    }

    impl Speaker for MacSpeaker {
        fn speak(&mut self, text: &str, words_per_minute: f32) {
            self.synthesizer.stopSpeaking();
            EVENTS.with(|events| events.borrow_mut().clear());
            self.synthesizer.setRate(words_per_minute);
            self.synthesizer
                .startSpeakingString(&NSString::from_str(text));
        }
        fn pause(&mut self) {
            self.synthesizer
                .pauseSpeakingAtBoundary(NSSpeechBoundary::ImmediateBoundary);
        }
        fn resume(&mut self) {
            self.synthesizer.continueSpeaking();
        }
        fn stop(&mut self) {
            self.synthesizer.stopSpeaking();
            EVENTS.with(|events| events.borrow_mut().clear());
        }
        fn events(&mut self) -> Vec<SpeechEvent> {
            EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()))
        }
    }

    impl Drop for MacSpeaker {
        fn drop(&mut self) {
            self.synthesizer.setDelegate(None);
            self.synthesizer.stopSpeaking();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word_review_tools::ReviewAction;

    /// 読み上げ from the caret selects each word as the voice says it, goes
    /// on to the next paragraph, and stops at the end.
    #[gpui::test]
    fn reading_aloud_selects_each_word(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "read.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input("One two");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("three");
        view.update(cx, |view, cx| {
            view.insert_at = (0, 0);
            view.selection_anchor = None;
            crate::word_review_tools::run(view, ReviewAction::ReadAloud, cx);
            assert!(view.review.read_aloud.is_some());
            assert!(tick(view, cx));
            assert_eq!(view.selection_anchor, Some((0, 0)));
            assert_eq!(view.insert_at, (0, 3));
            tick(view, cx);
            assert_eq!(
                (view.selection_anchor, view.insert_at),
                (Some((0, 4)), (0, 7))
            );
            // Finished with the paragraph: on to the next.
            tick(view, cx);
            tick(view, cx);
            let at = jade_office_model::word::read_aloud::utterances(&view.document, (1, 0))[0]
                .text
                .find("three")
                .unwrap();
            assert_eq!(
                (view.selection_anchor, view.insert_at),
                (Some((1, at)), (1, at + 5))
            );
            change_speed(view, true, cx);
            assert_eq!(view.review.read_aloud.as_ref().unwrap().reading.speed, 1.25);
            play_pause(view, cx);
            assert!(view.review.read_aloud.as_ref().unwrap().reading.paused);
            step(view, false, cx);
            tick(view, cx);
            assert_eq!(view.insert_at, (0, 3));
            step(view, true, cx);
            step(view, true, cx);
            assert!(view.review.read_aloud.is_none(), "past the end it stops");
        });
    }
}
