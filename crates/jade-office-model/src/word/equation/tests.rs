use super::*;

fn editor() -> EquationEditor {
    EquationEditor::new(
        EquationTarget {
            paragraph: 0,
            run_index: None,
            display: true,
        },
        MathElement::new(K::Math, Vec::new()),
    )
}

fn count(e: &MathElement, kind: K) -> usize {
    usize::from(e.kind == kind)
        + e.children
            .iter()
            .map(|c| count(c, kind.clone()))
            .sum::<usize>()
}

fn type_all(editor: &mut EquationEditor, text: &str) {
    for ch in text.chars() {
        editor.type_text(&ch.to_string());
    }
}

#[test]
fn typing_linear_format_builds_up_on_a_space() {
    let mut e = editor();
    assert!(e.is_empty());
    type_all(&mut e, "a/b");
    assert_eq!(
        count(e.root(), K::Fraction),
        0,
        "nothing builds before the space"
    );
    e.type_text(" ");
    assert_eq!(count(e.root(), K::Fraction), 1);
    assert!(e.omml().contains("<m:num>"));
    // The caret is after the fraction: typing continues the equation.
    type_all(&mut e, "+1");
    assert!(e.linear_text().ends_with("+1"), "{}", e.linear_text());
}

#[test]
fn math_autocorrect_turns_control_words_into_symbols() {
    let mut e = editor();
    type_all(&mut e, r"\alpha");
    e.type_text(" ");
    type_all(&mut e, "+1");
    assert_eq!(e.linear_text(), "α+1");
}

#[test]
fn structures_go_in_at_the_caret_and_the_caret_walks_their_arguments() {
    let mut e = editor();
    e.insert(&EquationInsert::Structure(StructureGroup::Fraction, 0));
    // In the empty numerator.
    assert_eq!(e.caret.index, 0);
    type_all(&mut e, "1");
    e.move_right();
    // Off the numerator's end into the denominator.
    type_all(&mut e, "2");
    e.move_right();
    type_all(&mut e, "=x");
    assert_eq!(e.linear_text(), "1/2=x");
    // Back left: into the denominator from after the fraction.
    e.move_home();
    e.move_right();
    assert!(e.caret.slot.len() > 1, "inside the fraction");
}

#[test]
fn backspace_selects_a_structure_before_deleting_it() {
    let mut e = editor();
    type_all(&mut e, "x");
    e.insert(&EquationInsert::Structure(StructureGroup::Radical, 0));
    type_all(&mut e, "2");
    e.move_right();
    assert_eq!(count(e.root(), K::Radical), 1);
    e.backspace();
    assert!(e.selected.is_some());
    assert_eq!(count(e.root(), K::Radical), 1);
    e.backspace();
    assert_eq!(count(e.root(), K::Radical), 0);
    assert_eq!(e.linear_text(), "x");
}

#[test]
fn an_empty_structure_goes_with_a_backspace_from_inside() {
    let mut e = editor();
    e.insert(&EquationInsert::Structure(StructureGroup::Script, 0));
    e.backspace();
    assert!(e.is_empty());
}

#[test]
fn the_equation_switches_between_two_dimensional_and_linear_forms() {
    let mut e = editor();
    type_all(&mut e, "(a+b)/c ");
    assert_eq!(count(e.root(), K::Fraction), 1);
    e.to_linear_form();
    assert_eq!(count(e.root(), K::Fraction), 0);
    assert_eq!(e.linear_text(), "(a+b)/c");
    e.to_professional_form();
    assert_eq!(count(e.root(), K::Fraction), 1);
    e.format = LinearFormat::LaTeX;
    e.to_linear_form();
    assert_eq!(e.linear_text().replace(' ', ""), r"\frac{a+b}{c}");
    e.to_professional_form();
    assert_eq!(count(e.root(), K::Fraction), 1);
}

#[test]
fn normal_text_goes_in_as_normal_text() {
    let mut e = editor();
    type_all(&mut e, "x");
    e.normal_text = true;
    type_all(&mut e, " if ");
    e.normal_text = false;
    type_all(&mut e, "y");
    assert!(e.omml().contains("<m:nor/>"), "{}", e.omml());
    assert_eq!(e.linear_text(), "x\" if \"y");
}

#[test]
fn every_built_in_equation_and_structure_item_builds() {
    for (index, entry) in BUILT_IN_EQUATIONS.iter().enumerate() {
        let equation = built_in_equation(index).unwrap();
        assert!(has_content(&equation), "{}", entry.0);
    }
    let quadratic = built_in_equation(0).unwrap();
    assert_eq!(count(&quadratic, K::Radical), 1);
    assert_eq!(count(&quadratic, K::Fraction), 1);
    let binomial = built_in_equation(1).unwrap();
    assert_eq!(count(&binomial, K::Nary), 1);
    let fourier = built_in_equation(3).unwrap();
    assert_eq!(count(&fourier, K::Function), 2);
    for group in StructureGroup::ALL {
        for (index, item) in group.items().iter().enumerate() {
            let template =
                structure_template(group, index).unwrap_or_else(|| panic!("{group:?} {item}"));
            if item.contains('⬚') {
                let mut e = editor();
                e.insert_structure(template.clone());
                // A template with arguments puts the caret in an empty one.
                assert!(
                    e.caret.slot.len() > 1 || item.starts_with('⬚') && item.chars().count() <= 2,
                    "{group:?} {item}: caret {:?}",
                    e.caret
                );
            }
        }
    }
}

#[test]
fn tab_visits_every_argument_in_order() {
    let mut e = editor();
    e.insert(&EquationInsert::Structure(StructureGroup::LargeOperator, 1));
    let first = e.caret.slot.clone();
    e.next_slot();
    assert_ne!(e.caret.slot, first);
    e.next_slot();
    e.next_slot();
    let last = e.caret.slot.clone();
    e.next_slot();
    assert_eq!(e.caret.slot, last, "no slot after the last");
}

#[cfg(feature = "skia")]
mod in_a_document {
    use super::*;
    use crate::test_support::core;
    use jade_docx_native_render::{FaceSpec, NativeDocument};

    fn open(body: &str) -> NativeDocument {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
        );
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let options = zip::write::SimpleFileOptions::default();
            for (path, content) in [
                (
                    "[Content_Types].xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
                ),
                (
                    "_rels/.rels",
                    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
                ),
                ("word/document.xml", xml.as_str()),
            ] {
                zip.start_file(path, options).unwrap();
                zip.write_all(content.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        let font = core().join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf");
        let faces =
            ["Noto Sans JP", "Times New Roman", "Cambria Math", "serif"].map(|family| FaceSpec {
                family,
                weight: 400,
                path: &font,
            });
        NativeDocument::open(&cursor.into_inner(), 816.0, 1056.0, &faces).unwrap()
    }

    #[test]
    fn a_new_equation_is_edited_in_place_and_saved() {
        let mut document = open("<w:p/><w:p><w:r><w:t>text</w:t></w:r></w:p>");
        // An empty paragraph takes a display equation.
        let target = insert_new_equation(&mut document, (0, 0), None).unwrap();
        assert!(target.display);
        let zones = WordSession::math_zones(&document);
        let zone = zone_for(&zones, &target).expect("the new equation was drawn");
        let mut editor = EquationEditor::open(&document, zone).unwrap();
        assert!(editor.is_empty());
        for ch in "x^2 ".chars() {
            editor.type_text(&ch.to_string());
        }
        editor.commit(&mut document).unwrap();
        let zones = WordSession::math_zones(&document);
        let zone = zone_for(&zones, &target).unwrap();
        assert_eq!(zone.plan.text, "𝑥2");
        // The caret sits after the superscript, on the page.
        let caret = editor.caret_geometry(zone).unwrap();
        assert!(caret.x > zone.x);
        assert_eq!(caret.page, 1);
        // A click on the base puts the caret before or after the x.
        let base = zone.plan.glyphs[0].x + zone.x;
        editor.click(zone, base + 0.5, zone.baseline - 2.0);
        assert_eq!(editor.caret.slot.len(), editor.top_slot().len() + 2);
        let saved = document.export_docx().unwrap();
        let reopened = open_bytes(&saved);
        let zones = WordSession::math_zones(&reopened);
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].plan.text, "𝑥2");
    }

    fn open_bytes(bytes: &[u8]) -> NativeDocument {
        let font = core().join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf");
        let faces = ["Noto Sans JP", "Times New Roman", "serif"].map(|family| FaceSpec {
            family,
            weight: 400,
            path: &font,
        });
        NativeDocument::open(bytes, 816.0, 1056.0, &faces).unwrap()
    }

    #[test]
    fn an_inline_equation_goes_into_text() {
        let mut document = open(r#"<w:p><w:r><w:t xml:space="preserve">area </w:t></w:r></w:p>"#);
        let equation = built_in_equation(2).unwrap();
        let target = insert_new_equation(&mut document, (0, 5), Some(&equation)).unwrap();
        assert!(!target.display);
        assert!(target.run_index.is_some());
        let zones = WordSession::math_zones(&document);
        let zone = zone_for(&zones, &target).unwrap();
        // π is drawn from the italic Greek alphabet.
        assert!(zone.plan.text.contains('𝜋'), "{}", zone.plan.text);
    }
}

#[test]
fn matrix_rows_and_columns_are_inserted_deleted_and_justified() {
    let mut e = editor();
    // A 2 × 2 matrix, caret in its first cell.
    e.insert(&EquationInsert::Structure(StructureGroup::Matrix, 4));
    let context = e.table_context().expect("in the matrix");
    assert!(context.matrix);
    assert_eq!((context.rows, context.columns), (2, 2));
    assert!(e.insert_row(true));
    assert_eq!(e.table_context().unwrap().rows, 3);
    assert_eq!(
        e.table_context().unwrap().row,
        1,
        "the caret in the new row"
    );
    assert!(e.insert_column(true));
    let context = e.table_context().unwrap();
    assert_eq!(context.columns, 3);
    assert_eq!(context.column, 1);
    assert!(e.set_column_justification("left"));
    assert_eq!(e.table_context().unwrap().justification, "left");
    assert!(
        e.omml().contains(r#"<m:mcJc m:val="left"/>"#),
        "{}",
        e.omml()
    );
    assert!(e.delete_column());
    assert_eq!(e.table_context().unwrap().columns, 2);
    assert!(e.delete_row());
    assert_eq!(e.table_context().unwrap().rows, 2);
    let rows = e.omml().matches("<m:mr>").count();
    assert_eq!(rows, 2);
}

#[test]
fn equation_array_rows_and_manual_breaks() {
    let mut e = editor();
    type_all(&mut e, "█(x&=1@y&=2) ");
    // Into the array's first row, then a row after it.
    e.move_home();
    e.move_right();
    let context = e.table_context().expect("in the array");
    assert!(!context.matrix);
    assert!(e.insert_row(true));
    assert_eq!(e.table_context().unwrap().rows, 3);
    // A manual break and an alignment point in a long equation.
    let mut e = editor();
    type_all(&mut e, "a=b+c");
    e.move_left();
    e.move_left();
    assert!(e.insert_line_break());
    assert!(e.omml().contains("<m:brk/>"), "{}", e.omml());
    e.move_home();
    e.move_right();
    assert!(e.set_alignment_point());
    assert!(e.omml().contains("<m:aln/>"));
}

#[test]
fn home_formatting_reaches_every_run() {
    let mut e = editor();
    type_all(&mut e, "x/2 ");
    e.format_runs(EquationFormat {
        size: Some(14.0),
        color: Some("FF0000".into()),
        bold: Some(true),
    });
    let omml = e.omml();
    assert_eq!(omml.matches(r#"<w:sz w:val="28"/>"#).count(), 2, "{omml}");
    assert!(omml.contains(r#"<w:color w:val="FF0000"/>"#));
    assert!(omml.contains(r#"<m:sty m:val="bi"/>"#));
}
