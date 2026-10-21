//! 互換モード: a document from an older Word says so in the title
//! ([互換モード]), 変換 upgrades it to the current format and lays it out
//! again, and the 互換性チェック lists what an older version would lose —
//! on its own, and before a document in 互換モード is saved, as Word asks.
//! What each does is `jade-office-model`'s (`word::compat`).

use std::path::{Path, PathBuf};

use gpui::{actions, div, prelude::*, px, AnyElement, ClickEvent, Context, SharedString};
use jade_office_model::word::compat::{self, Issue, WordVersion};
use jade_office_model::word::formats::{SaveOptions, WordFormat};

use crate::word_characters::{button, caption, check};
use crate::word_files::{FileDialog, Write};
use crate::{i18n, theme, DocumentView};

actions!(
    jade,
    [
        /// 互換性チェック.
        CheckCompatibility,
        /// 変換: the document upgraded to the current format.
        ConvertDocument,
    ]
);

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// The checker as it is open.
pub struct Checker {
    pub version: WordVersion,
    pub issues: Vec<(Issue, usize)>,
    /// The save that opened it, made when 続行 is pressed.
    pub then: Option<(Write, Option<SaveOptions>)>,
}

impl DocumentView {
    /// [互換モード], while the document is in one.
    pub(crate) fn compatibility_badge(&self) -> Option<SharedString> {
        compat::is_compatibility_mode(self.files.mode).then(|| t("compat.badge").into())
    }

    /// 互換性チェック, for the document's own version when it is in
    /// 互換モード (Word 97–2003 otherwise); `then` is the save waiting.
    pub(crate) fn open_compatibility_checker(
        &mut self,
        then: Option<(Write, Option<SaveOptions>)>,
        cx: &mut Context<Self>,
    ) {
        let version = if compat::is_compatibility_mode(self.files.mode) {
            WordVersion::of_mode(self.files.mode)
        } else {
            WordVersion::Word2003
        };
        let issues = self.compatibility_issues(version);
        self.files.dialog = Some(FileDialog::Compatibility(Checker {
            version,
            issues,
            then,
        }));
        cx.notify();
    }

    fn compatibility_issues(&self, version: WordVersion) -> Vec<(Issue, usize)> {
        self.document
            .export_docx()
            .map(|docx| compat::check(&docx, version))
            .unwrap_or_default()
    }

    /// Before saving a document in 互換モード: the checker, when there is
    /// something its version would lose. Whether it was shown.
    pub(crate) fn check_before_saving(&mut self, write: &Write, cx: &mut Context<Self>) -> bool {
        if !self.files.check_on_save || !compat::is_compatibility_mode(self.files.mode) {
            return false;
        }
        let version = WordVersion::of_mode(self.files.mode);
        let issues = self.compatibility_issues(version);
        if issues.is_empty() {
            return false;
        }
        self.files.dialog = Some(FileDialog::Compatibility(Checker {
            version,
            issues,
            then: Some((write.clone(), None)),
        }));
        cx.notify();
        true
    }

    /// 変換: the document in the current format, laid out again as one. A
    /// Word 97–2004 document becomes a `.docx` of the same name.
    pub(crate) fn convert_document(&mut self, cx: &mut Context<Self>) {
        if !compat::is_compatibility_mode(self.files.mode) {
            return self.show_message(t("compat.convert_title"), t("compat.already_current"), cx);
        }
        let converted = self
            .document
            .export_docx()
            .and_then(|docx| compat::upgrade(&docx))
            .and_then(|docx| crate::open_document_bytes(&docx));
        match converted {
            Ok(document) => {
                self.replace_document(document);
                self.files.mode = compat::CURRENT_MODE;
                if !self.files.format.is_package() {
                    let path = PathBuf::from(self.path.as_ref());
                    let renamed = path.with_extension("docx");
                    self.path = SharedString::from(renamed.to_string_lossy().into_owned());
                    self.files.format = WordFormat::Docx;
                    if !Path::new(&renamed).exists() {
                        // Not written yet: Save asks nothing more of it.
                        self.untitled = false;
                    }
                }
                self.dirty = true;
                cx.notify();
            }
            Err(error) => self.show_message(t("compat.convert_title"), &error, cx),
        }
    }
}

pub fn confirm(view: &mut DocumentView, checker: Checker, cx: &mut Context<DocumentView>) {
    if let Some((write, options)) = checker.then {
        view.files.checked = true;
        view.write_file(write, options, cx);
    }
}

fn with_checker(view: &mut DocumentView, change: impl FnOnce(&mut Checker, &DocumentView)) {
    if let Some(FileDialog::Compatibility(mut checker)) = view.files.dialog.take() {
        change(&mut checker, view);
        view.files.dialog = Some(FileDialog::Compatibility(checker));
    }
}

/// The checker's title, body and buttons.
pub fn dialog(
    checker: &Checker,
    check_on_save: bool,
    cx: &mut Context<DocumentView>,
) -> (SharedString, AnyElement, AnyElement) {
    let japanese = i18n::language() == i18n::Language::Japanese;
    let versions = WordVersion::ALL.into_iter().map(|version| {
        check(
            SharedString::from(format!("compat-version-{}", version.mode())),
            version.label(japanese),
            checker.version == version,
        )
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
            with_checker(view, |checker, view| {
                checker.version = version;
                checker.issues = view.compatibility_issues(version);
            });
            cx.notify();
        }))
    });
    let issues: Vec<AnyElement> = if checker.issues.is_empty() {
        vec![div()
            .text_color(theme::text_dim())
            .child(t("compat.no_issues"))
            .into_any_element()]
    } else {
        checker
            .issues
            .iter()
            .map(|(issue, count)| {
                div()
                    .flex()
                    .justify_between()
                    .gap_4()
                    .py_1()
                    .border_b_1()
                    .border_color(theme::border())
                    .child(div().flex_1().child(t(issue.key())))
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(i18n::format("compat.occurrences", &[("n", count)])),
                    )
                    .into_any_element()
            })
            .collect()
    };
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .w(px(480.0))
        .child(caption(t("compat.versions")))
        .child(div().flex().gap_3().children(versions))
        .child(caption(t("compat.summary")))
        .child(
            div()
                .id("compat-issues")
                .max_h(px(260.0))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .children(issues),
        )
        .child(
            check("compat-on-save", t("compat.check_on_save"), check_on_save).on_click(
                cx.listener(|view, _: &ClickEvent, _, cx| {
                    view.files.check_on_save = !view.files.check_on_save;
                    cx.notify();
                }),
            ),
        )
        .into_any_element();
    let saving = checker.then.is_some();
    let buttons = div()
        .flex()
        .justify_end()
        .gap_2()
        .when(saving, |row| {
            row.child(
                button("compat-cancel", t("format.cancel"), false).on_click(cx.listener(
                    |view, _: &ClickEvent, _, cx| {
                        view.files.dialog = None;
                        cx.notify();
                    },
                )),
            )
        })
        .child(
            button(
                "compat-ok",
                if saving {
                    t("compat.continue")
                } else {
                    t("format.ok")
                },
                true,
            )
            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                if let Some(FileDialog::Compatibility(checker)) = view.files.dialog.take() {
                    confirm(view, checker, cx);
                }
                cx.notify();
            })),
        )
        .into_any_element();
    (t("compat.title").into(), body, buttons)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open<'a>(
        path: &Path,
        cx: &'a mut gpui::TestAppContext,
    ) -> (gpui::Entity<DocumentView>, &'a mut gpui::VisualTestContext) {
        use gpui::Focusable;
        let path = path.to_path_buf();
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/public/fixtures"
        ))
        .join(name)
    }

    /// A Word 2010 document says [互換モード]; saving it shows what Word
    /// 2010 would lose, 続行 saves; 変換 upgrades it, and the badge goes.
    #[gpui::test]
    fn a_word_2010_document_is_in_compatibility_mode_until_converted(
        cx: &mut gpui::TestAppContext,
    ) {
        let path = crate::shell::scratch_copy(
            &fixture("word-compat-comprehensive.docx"),
            "compat-2010.docx",
        );
        let (view, cx) = open(&path, cx);
        view.read_with(cx, |view, _| {
            assert_eq!(view.files.mode, 14);
            assert!(view.compatibility_badge().is_some());
        });
        cx.dispatch_action(CheckCompatibility);
        view.read_with(cx, |view, _| {
            let Some(FileDialog::Compatibility(checker)) = &view.files.dialog else {
                panic!("the checker is open");
            };
            assert_eq!(checker.version, WordVersion::Word2010);
            assert!(checker.then.is_none());
        });
        cx.simulate_keystrokes("escape");
        cx.dispatch_action(ConvertDocument);
        view.read_with(cx, |view, _| {
            assert_eq!(view.files.mode, compat::CURRENT_MODE);
            assert!(view.compatibility_badge().is_none());
            assert!(view.dirty);
        });
        cx.dispatch_action(crate::Save);
        cx.run_until_parked();
        let saved = std::fs::read(&path).unwrap();
        assert_eq!(compat::compatibility_mode(&saved), 15);
    }

    /// A Word 97–2004 document opens in Word 2003's mode; converted, it is
    /// a .docx beside the .doc.
    #[gpui::test]
    fn a_doc_converts_to_a_docx(cx: &mut gpui::TestAppContext) {
        let source = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/crates/jade-doc-reader/tests/fixtures/hyperlink.doc"
        ));
        let path = crate::shell::scratch_copy(&source, "legacy.doc");
        let (view, cx) = open(&path, cx);
        view.read_with(cx, |view, _| {
            assert_eq!(view.files.mode, 11);
            assert!(view.compatibility_badge().is_some());
        });
        cx.dispatch_action(ConvertDocument);
        cx.dispatch_action(crate::Save);
        cx.run_until_parked();
        let converted = path.with_extension("docx");
        let bytes = std::fs::read(&converted).unwrap();
        assert_eq!(WordFormat::sniff(&bytes), Some(WordFormat::Docx));
        assert_eq!(compat::compatibility_mode(&bytes), 15);
    }

    /// Saving a Word 2007 document with text effects stops at the checker,
    /// which names them; 続行 saves it as it is.
    #[gpui::test]
    fn saving_a_compatibility_mode_document_checks_it_first(cx: &mut gpui::TestAppContext) {
        use jade_opc::OpcPackage;
        let blank = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let mut package = OpcPackage::open(&blank).unwrap();
        let settings = package.part_str("word/settings.xml").unwrap().unwrap();
        package.replace(
            "word/settings.xml",
            settings
                .replace(r#"w:val="15""#, r#"w:val="12""#)
                .into_bytes(),
        );
        let document = package.part_str("word/document.xml").unwrap().unwrap();
        package.replace(
            "word/document.xml",
            document
                .replace(
                    "<w:document ",
                    r#"<w:document xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" "#,
                )
                .replace(
                    "<w:body><w:p/>",
                    r#"<w:body><w:p><w:r><w:rPr><w14:glow w14:rad="63500"><w14:srgbClr w14:val="FF0000"/></w14:glow></w:rPr><w:t>glowing</w:t></w:r></w:p>"#,
                )
                .into_bytes(),
        );
        let directory = std::env::temp_dir().join(format!("jade-compat-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("word2007.docx");
        std::fs::write(&path, package.write().unwrap()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let (view, cx) = open(&path, cx);
        view.read_with(cx, |view, _| assert_eq!(view.files.mode, 12));
        cx.dispatch_action(crate::Save);
        view.read_with(cx, |view, _| {
            let Some(FileDialog::Compatibility(checker)) = &view.files.dialog else {
                panic!("the checker stops the save");
            };
            assert_eq!(checker.version, WordVersion::Word2007);
            assert!(checker
                .issues
                .iter()
                .any(|(issue, _)| *issue == Issue::TextEffects));
        });
        assert_eq!(std::fs::read(&path).unwrap(), before, "nothing written yet");
        cx.simulate_keystrokes("enter");
        let after = std::fs::read(&path).unwrap();
        assert_ne!(after, before);
        assert_eq!(
            compat::compatibility_mode(&after),
            12,
            "saved in its own mode"
        );
        std::fs::remove_dir_all(directory).ok();
    }
}
