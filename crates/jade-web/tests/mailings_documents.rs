//! Mail merge in the browser build's Word document, as both shells run it
//! from the 差し込み文書 tab: a list typed in 新しいアドレス帳 and saved as
//! CSV, merge fields and an address block put in, each recipient previewed,
//! recipients ticked off, filtered and sorted, the merge finished into a
//! new document; a CSV in Shift_JIS used as the list; labels laid out and
//! merged a sheet at a time; an envelope made on its own; エラーのチェック.

mod common;

use jade_web::editing::{EditableDocument, WordSession};
use jade_web::word::form_view::{FormEvent, FormId};
use jade_web::word::mailings::{MailDialog, MailMerge, MailOutcome};

fn t(key: &'static str) -> &'static str {
    key
}

fn click(merge: &mut MailMerge, document: &mut EditableDocument, id: &'static str) -> MailOutcome {
    merge
        .ribbon_event(document, FormEvent::Click(FormId::new(id)))
        .unwrap()
}

fn choose(
    merge: &mut MailMerge,
    document: &mut EditableDocument,
    id: &'static str,
    index: usize,
) -> MailOutcome {
    merge
        .ribbon_event(document, FormEvent::Choose(FormId::new(id), index))
        .unwrap()
}

fn dialog(merge: &mut MailMerge, document: &mut EditableDocument, event: FormEvent) -> MailOutcome {
    merge.dialog_event(document, event).unwrap()
}

fn texts(document: &EditableDocument) -> Vec<String> {
    (0..document.paragraph_count())
        .filter_map(|index| document.paragraph_text(index))
        .filter(|text| !text.trim().is_empty())
        .collect()
}

/// A list of two recipients typed into 新しいアドレス帳 and saved.
fn typed_list(merge: &mut MailMerge, document: &mut EditableDocument) {
    choose(merge, document, "mail-recipients", 0);
    assert!(matches!(merge.dialog, Some(MailDialog::List(_))));
    // 敬称, 姓, 名, 会社名, 郵便番号, 都道府県, 市区町村, 住所1 …
    let rows = [
        [
            "",
            "山田",
            "花子",
            "ジェイド株式会社",
            "100-0001",
            "東京都",
            "千代田区",
            "千代田1-1",
        ],
        [
            "",
            "鈴木",
            "一郎",
            "",
            "530-0001",
            "大阪府",
            "大阪市北区",
            "梅田2-2",
        ],
    ];
    for (row, values) in rows.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            dialog(
                merge,
                document,
                FormEvent::Cell(FormId::new("grid"), row, column),
            );
            if let Some(field) = merge.field_mut() {
                field.insert(value);
            }
        }
        if row == 0 {
            dialog(merge, document, FormEvent::Click(FormId::new("add-row")));
        }
    }
    let MailOutcome::SaveDataSource { name, bytes } =
        dialog(merge, document, FormEvent::Click(FormId::new("ok")))
    else {
        panic!("OK saves the list");
    };
    assert_eq!(name, "アドレス帳.csv");
    let csv = String::from_utf8(bytes).unwrap();
    assert!(csv.starts_with('\u{feff}'));
    assert!(csv.contains("\"山田\",\"花子\""), "{csv}");
    merge
        .saved_data_source(
            document,
            &name,
            Some("/Users/me/アドレス帳.csv".to_string()),
        )
        .unwrap();
    assert_eq!(merge.chosen(), vec![0, 1]);
}

#[test]
fn letters_are_merged_from_a_list_typed_here() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "宛名 ");
    let mut merge = MailMerge::read(&document, true);
    choose(&mut merge, &mut document, "mail-start", 0);
    typed_list(&mut merge, &mut document);
    let settings = document.word_document().mail_merge_settings();
    assert_eq!(
        settings.data_source.as_deref(),
        Some("/Users/me/アドレス帳.csv")
    );

    // 姓 and 名 at the caret.
    document.place_caret(paragraph, "宛名 ".len());
    let columns = merge.columns();
    let last = columns.iter().position(|column| column == "姓").unwrap();
    let first = columns.iter().position(|column| column == "名").unwrap();
    choose(&mut merge, &mut document, "mail-insert-field", last);
    let caret = document.caret().unwrap();
    document.place_caret(caret.0, caret.1);
    choose(&mut merge, &mut document, "mail-insert-field", first);
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("宛名 «姓»«名»"));
    // An address block at the start.
    document.place_caret(paragraph, 0);
    click(&mut merge, &mut document, "mail-address-block");
    let view = merge.dialog_view(t).expect("the address block dialog");
    assert!(view.rows.iter().any(
        |row| matches!(row, jade_web::word::form_view::FormRow::Preview(lines)
            if lines.first().map(String::as_str) == Some("〒100-0001"))
    ));
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Click(FormId::new("ok")),
    );

    // Preview, the first recipient then the next.
    click(&mut merge, &mut document, "mail-preview");
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .contains("宛名 山田花子"));
    click(&mut merge, &mut document, "mail-next");
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .contains("宛名 鈴木一郎"));
    let ribbon = merge.ribbon(&document, t);
    assert!(ribbon.iter().flat_map(|group| group.rows.iter().flatten()).any(
        |item| matches!(item, jade_web::word::form_view::ToolItem::Text(text) if text == "mail.record")
    ));

    // Finish: a new document with both letters.
    choose(&mut merge, &mut document, "mail-finish", 0);
    let MailOutcome::OpenDocument { bytes, .. } = dialog(
        &mut merge,
        &mut document,
        FormEvent::Click(FormId::new("ok")),
    ) else {
        panic!("the merge opens a new document");
    };
    let merged = common::open_bytes(&bytes);
    let lines = texts(&merged).join("\n");
    assert!(lines.contains("宛名 山田花子"), "{lines}");
    assert!(lines.contains("宛名 鈴木一郎"), "{lines}");
    assert!(lines.contains("ジェイド株式会社"));

    // Unticking a recipient leaves one letter.
    click(&mut merge, &mut document, "mail-edit-recipients");
    dialog(
        &mut merge,
        &mut document,
        FormEvent::RowCheck(FormId::new("grid"), 1),
    );
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Click(FormId::new("ok")),
    );
    assert_eq!(merge.chosen(), vec![0]);
    choose(&mut merge, &mut document, "mail-finish", 0);
    let MailOutcome::OpenDocument { bytes, .. } = dialog(
        &mut merge,
        &mut document,
        FormEvent::Click(FormId::new("ok")),
    ) else {
        panic!("the merge opens a new document");
    };
    let lines = texts(&common::open_bytes(&bytes)).join("\n");
    assert!(!lines.contains("鈴木"), "{lines}");

    // Saved and opened again, it is still a main document with its fields.
    let saved = document.export_docx().unwrap();
    let reopened = common::open_bytes(&saved);
    let again = MailMerge::read(&reopened, true);
    assert_eq!(
        again.wanted_source.as_deref(),
        Some("/Users/me/アドレス帳.csv")
    );
    assert!(again.preview);
}

#[test]
fn a_shift_jis_list_is_filtered_sorted_and_checked() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "様 ");
    let mut merge = MailMerge::read(&document, true);
    let csv = "姓,名,年齢\r\n佐藤,太郎,30\r\n伊藤,次郎,25\r\n加藤,三郎,40\r\n";
    let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(csv);
    assert_eq!(
        choose(&mut merge, &mut document, "mail-recipients", 1),
        MailOutcome::PickDataSource
    );
    merge
        .load_data_source(&mut document, "list.csv", None, &bytes)
        .unwrap();
    assert_eq!(merge.columns(), vec!["姓", "名", "年齢"]);

    // Sorted by 年齢, those over 26.
    click(&mut merge, &mut document, "mail-edit-recipients");
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Header(FormId::new("grid"), 2),
    );
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Choose(FormId::new("filter-column"), 2),
    );
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Choose(FormId::new("filter-op"), 7),
    );
    merge.field_mut().unwrap().insert("26");
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Click(FormId::new("filter-add")),
    );
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Click(FormId::new("ok")),
    );
    assert_eq!(merge.chosen(), vec![0, 2]);

    // A field the list does not have.
    document.place_caret(paragraph, 0);
    choose(&mut merge, &mut document, "mail-insert-field", 0);
    choose(&mut merge, &mut document, "mail-rules", 2);
    assert!(matches!(merge.dialog, Some(MailDialog::Rule(_))));
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Choose(FormId::new("rule-field"), 2),
    );
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Choose(FormId::new("rule-op"), 3),
    );
    merge.field_mut().unwrap().insert("35");
    dialog(&mut merge, &mut document, FormEvent::Tab(false));
    merge.field_mut().unwrap().insert("ベテラン");
    dialog(&mut merge, &mut document, FormEvent::Tab(false));
    merge.field_mut().unwrap().insert("若手");
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Click(FormId::new("ok")),
    );
    // The rule's result is computed text, not the paragraph's own: read
    // what the document says it shows.
    let shown = |document: &mut EditableDocument| {
        let saved = document.export_docx().unwrap();
        let package = jade_opc::OpcPackage::open(&saved).unwrap();
        package.part_str("word/document.xml").unwrap().unwrap()
    };
    click(&mut merge, &mut document, "mail-preview");
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("佐藤"));
    assert!(shown(&mut document).contains(">若手<"));
    click(&mut merge, &mut document, "mail-next");
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("加藤"));
    assert!(shown(&mut document).contains(">ベテラン<"));

    click(&mut merge, &mut document, "mail-check");
    let Some(MailDialog::Errors(check)) = &merge.dialog else {
        panic!("the error check");
    };
    assert!(check.problems.is_empty(), "{:?}", check.problems);
}

#[test]
fn labels_and_envelopes() {
    let mut document = common::contract();
    let mut merge = MailMerge::read(&document, true);
    choose(&mut merge, &mut document, "mail-start", 2);
    assert!(matches!(merge.dialog, Some(MailDialog::Labels(_))));
    dialog(
        &mut merge,
        &mut document,
        FormEvent::Click(FormId::new("ok")),
    );
    typed_list(&mut merge, &mut document);
    click(&mut merge, &mut document, "mail-update-labels");
    choose(&mut merge, &mut document, "mail-finish", 0);
    let MailOutcome::OpenDocument { bytes, .. } = dialog(
        &mut merge,
        &mut document,
        FormEvent::Click(FormId::new("ok")),
    ) else {
        panic!("the labels are merged into a new document");
    };
    let labels = common::open_bytes(&bytes);
    let lines = texts(&labels).join("\n");
    assert!(lines.contains("山田 花子 様"), "{lines}");
    assert!(lines.contains("鈴木 一郎 様"), "{lines}");
    assert_eq!(labels.page_count(), 1, "two recipients fit one sheet");

    // An envelope on its own: a new document the envelope's size.
    click(&mut merge, &mut document, "mail-envelopes");
    merge.field_mut().unwrap().insert("東京都千代田区千代田1-1");
    dialog(&mut merge, &mut document, FormEvent::Enter);
    merge.field_mut().unwrap().insert("山田 花子 様");
    let MailOutcome::OpenDocument { bytes, .. } = dialog(
        &mut merge,
        &mut document,
        FormEvent::Click(FormId::new("new-document")),
    ) else {
        panic!("the envelope opens as a new document");
    };
    let envelope = common::open_bytes(&bytes);
    let (width, height) = envelope.page_size();
    assert!(height > width, "長形 3 号 is tall: {width} × {height}");
    assert!(texts(&envelope).join("").contains("山田 花子 様"));
}
