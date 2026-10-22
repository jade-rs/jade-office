//! Content controls and legacy form fields in the browser build's Word
//! document, as both shells have them through the 開発 tab: a control put in
//! with its placeholder selected and typed over, a check box ticked by a
//! click, a drop-down's entries set in プロパティ and one picked, a date
//! picked from the calendar, the box drawn round the caret's control, locks
//! that refuse typing, design mode's tags, and form fields filled in a
//! document protected for forms — each written into the .docx.

mod common;

use jade_web::editing::WordSession;

use jade_web::word::controls::{
    ContentControlAction, ContentControlKind, ControlChrome, ControlClick, ControlPropertiesForm,
    FormFieldKind, WordControls,
};
use jade_web::word::form_view::{FormEvent, FormId, FormOutcome};

const TODAY: (i32, u32, u32) = (2026, 10, 21);

fn reopen(
    document: &mut jade_web::editing::EditableDocument,
) -> jade_web::editing::EditableDocument {
    let saved = document.export_docx().unwrap();
    common::open_bytes(&saved)
}

#[test]
fn a_rich_text_control_is_put_in_with_its_placeholder_and_typed_over() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "Name: ");
    document.place_caret(paragraph, 6);
    document
        .insert_content_control(ContentControlKind::RichText, true)
        .unwrap();
    let control = document.caret_control().expect("the caret is in it");
    assert!(control.showing_placeholder);
    assert_eq!(
        document.selection(),
        Some((
            (control.start.paragraph, control.start.offset),
            (control.end.paragraph, control.end.offset)
        ))
    );
    // Its box is drawn round it.
    let chrome = document.control_chrome();
    assert!(chrome.iter().any(|chrome| matches!(chrome,
        ControlChrome::Box { regions, .. } if !regions.is_empty())));

    // Typing replaces the placeholder.
    document.type_over_placeholder("Taro").unwrap().unwrap();
    document.insert_text(" Yamada").unwrap();
    let control = document.content_controls().remove(0);
    assert!(!control.showing_placeholder);
    assert_eq!(control.text, "Taro Yamada");
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("Name: Taro Yamada"));

    let reopened = reopen(&mut document);
    let control = reopened.content_controls().remove(0);
    assert_eq!(control.kind, ContentControlKind::RichText);
    assert_eq!(control.text, "Taro Yamada");
}

#[test]
fn a_click_ticks_a_check_box_and_drops_a_list_and_a_calendar() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "OK ");
    document.place_caret(paragraph, 0);
    document
        .insert_content_control(ContentControlKind::CheckBox, true)
        .unwrap();
    let check = document.content_controls().remove(0);
    let at = (check.start.paragraph, check.start.offset + 1);
    document.place_caret(at.0, at.1);
    assert_eq!(document.click_control(at, TODAY), ControlClick::Toggled);
    assert!(document.paragraph_text(paragraph).unwrap().starts_with('☒'));
    assert!(document.content_controls()[0].checked);

    // A drop-down list: two entries set in its properties, one picked.
    let end = document.content_controls()[0].end;
    document.place_caret(end.paragraph, end.offset);
    document
        .insert_content_control(ContentControlKind::DropDownList, true)
        .unwrap();
    let list = document.caret_control().unwrap();
    assert_eq!(list.kind, ContentControlKind::DropDownList);
    let mut form = ControlPropertiesForm::new(&list, Vec::new());
    for entry in ["赤", "青"] {
        form.event(FormEvent::Click(FormId::new("entry-display")));
        form.field_mut().insert(entry);
        form.event(FormEvent::Click(FormId::new("entry-add")));
    }
    form.event(FormEvent::Toggle(FormId::new("no-delete")));
    let FormOutcome::Do(action) = form.event(FormEvent::Click(FormId::new("ok"))) else {
        panic!("OK applies the properties");
    };
    document.run_control_action(action).unwrap();
    let list = document.content_controls()[1].clone();
    assert_eq!(list.entries.len(), 3);
    assert!(list.cannot_delete);
    let inside = (list.start.paragraph, list.start.offset + 1);
    let ControlClick::List {
        control, entries, ..
    } = document.click_control(inside, TODAY)
    else {
        panic!("a click drops the list");
    };
    assert_eq!(entries[2], "青");
    document
        .run_control_action(ContentControlAction::ChooseEntry { control, entry: 2 })
        .unwrap();
    assert_eq!(document.content_controls()[1].text, "青");

    // A date picker.
    let end = document.content_controls()[1].end;
    document.place_caret(end.paragraph, end.offset);
    document
        .insert_content_control(ContentControlKind::Date, true)
        .unwrap();
    let date = document.content_controls()[2].clone();
    let ControlClick::Date(mut picker) =
        document.click_control((date.start.paragraph, date.start.offset + 1), TODAY)
    else {
        panic!("a click drops the calendar");
    };
    assert_eq!((picker.year, picker.month), (2026, 10));
    picker.step(true);
    assert_eq!(picker.month, 11);
    assert_eq!(picker.weeks()[0][0], Some(1), "1 November 2026 is a Sunday");
    document.run_control_action(picker.choose(3)).unwrap();
    assert_eq!(document.content_controls()[2].text, "2026/11/03");

    let reopened = reopen(&mut document);
    let kinds: Vec<_> = reopened
        .content_controls()
        .into_iter()
        .map(|control| control.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            ContentControlKind::CheckBox,
            ContentControlKind::DropDownList,
            ContentControlKind::Date
        ]
    );
}

#[test]
fn locked_contents_refuse_typing_and_design_mode_shows_tags() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "fixed text ");
    document.select((paragraph, 0), (paragraph, 5)).unwrap();
    document
        .insert_content_control(ContentControlKind::PlainText, false)
        .unwrap();
    let control = document.content_controls().remove(0);
    assert_eq!(control.text, "fixed");
    let mut form = ControlPropertiesForm::new(&control, Vec::new());
    form.field_mut().insert("Locked");
    form.event(FormEvent::Toggle(FormId::new("no-edit")));
    let FormOutcome::Do(action) = form.event(FormEvent::Enter) else {
        panic!("Enter applies the properties");
    };
    document.run_control_action(action).unwrap();
    let control = document.content_controls().remove(0);
    assert_eq!(control.title.as_deref(), Some("Locked"));
    document.place_caret(paragraph, 2);
    assert!(document.insert_text("x").is_err());
    assert_eq!(
        document.paragraph_text(paragraph).unwrap()[..5].to_string(),
        "fixed"
    );

    document
        .run_control_action(ContentControlAction::DesignMode(true))
        .unwrap();
    assert!(document.design_mode());
    assert!(document
        .control_chrome()
        .iter()
        .any(|chrome| matches!(chrome,
        ControlChrome::Tags { title, start: Some(_), end: Some(_), .. } if title == "Locked")));
}

#[test]
fn form_fields_are_filled_in_a_document_protected_for_forms() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "Name: ");
    document.place_caret(paragraph, 6);
    document
        .run_control_action(ContentControlAction::InsertFormField {
            kind: FormFieldKind::Text,
        })
        .unwrap();
    let after = document.form_fields()[0].result_end;
    document.place_caret(after.paragraph, after.offset);
    document
        .run_control_action(ContentControlAction::InsertFormField {
            kind: FormFieldKind::CheckBox,
        })
        .unwrap();
    document
        .run_control_action(ContentControlAction::ProtectForms(true))
        .unwrap();
    assert!(document.protected_for_forms());

    // Outside the fields nothing can be typed.
    document.place_caret(paragraph, 1);
    assert!(document.insert_text("x").is_err());
    // In the text field it can.
    let field = document.form_fields()[0].clone();
    document
        .select(
            (field.result_start.paragraph, field.result_start.offset),
            (field.result_end.paragraph, field.result_end.offset),
        )
        .unwrap();
    document.type_over_placeholder("Hanako").unwrap().unwrap();
    document.insert_text("さん").unwrap();
    assert_eq!(document.form_fields()[0].result, "Hanakoさん");
    // A click ticks the check box.
    let check = document.form_fields()[1].clone();
    let at = (check.result_start.paragraph, check.result_start.offset);
    assert_eq!(document.click_control(at, TODAY), ControlClick::Toggled);
    assert!(document.form_fields()[1].checked);
    assert!(document
        .control_chrome()
        .iter()
        .any(|chrome| matches!(chrome, ControlChrome::Shading { .. })));

    let reopened = reopen(&mut document);
    let fields = reopened.form_fields();
    assert_eq!(fields[0].result, "Hanakoさん");
    assert!(fields[1].checked);
    assert!(reopened.protected_for_forms());
}

#[test]
fn a_selection_saved_to_the_gallery_fills_a_gallery_control() {
    use jade_web::word::controls::SaveBlockForm;
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "敬具 ");
    document.select((paragraph, 0), (paragraph, 6)).unwrap();
    let mut form = SaveBlockForm::default();
    form.name.insert("結び");
    let FormOutcome::Do(action) = form.event(FormEvent::Enter) else {
        panic!("Enter saves the block");
    };
    document.run_control_action(action).unwrap();
    assert_eq!(
        document
            .word_document()
            .building_blocks(Some("Quick Parts")),
        vec!["結び"]
    );
    document.place_caret(paragraph, 0);
    document
        .insert_content_control(ContentControlKind::BuildingBlockGallery, true)
        .unwrap();
    let gallery = document.content_controls().remove(0);
    let ControlClick::Gallery { control, blocks } =
        document.click_control((gallery.start.paragraph, gallery.start.offset + 3), TODAY)
    else {
        panic!("a click drops the gallery");
    };
    assert_eq!(blocks, vec!["結び"]);
    document
        .run_control_action(ContentControlAction::InsertBuildingBlock {
            control,
            name: blocks[0].clone(),
        })
        .unwrap();
    assert!(document.content_controls()[0].text.starts_with("敬具"));
}

#[test]
fn tab_goes_from_field_to_field_in_a_protected_form() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "A  B ");
    document.place_caret(paragraph, 1);
    document
        .run_control_action(ContentControlAction::InsertFormField {
            kind: FormFieldKind::Text,
        })
        .unwrap();
    document.place_caret(paragraph, 0);
    document
        .insert_content_control(ContentControlKind::PlainText, false)
        .unwrap();
    document
        .run_control_action(ContentControlAction::ProtectForms(true))
        .unwrap();
    document.place_caret(paragraph, 0);
    assert!(document.tab_to_form_field(false));
    let control = document.content_controls().remove(0);
    let field = document.form_fields().remove(0);
    let selected = document.selection().unwrap();
    // Two stops, visited in order and round again.
    let stops = [
        (
            (control.start.paragraph, control.start.offset),
            (control.end.paragraph, control.end.offset),
        ),
        (
            (field.result_start.paragraph, field.result_start.offset),
            (field.result_end.paragraph, field.result_end.offset),
        ),
    ];
    assert!(stops.contains(&selected));
    assert!(document.tab_to_form_field(false));
    let second = document.selection().unwrap();
    assert!(stops.contains(&second));
    assert_ne!(selected, second);
    assert!(document.tab_to_form_field(false));
    assert_eq!(document.selection().unwrap(), selected);
}
