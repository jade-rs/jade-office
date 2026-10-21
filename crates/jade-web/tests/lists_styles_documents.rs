//! Paragraph options, lists, styles and the デザイン tab on the browser
//! build's Word document — the calls both shells make from their dialogs,
//! the ruler and the galleries. Each change is checked as the engine lays
//! it out, again after a save and a reopen (what Word would read), and for
//! undoing as one step.

mod common;

use common::open_bytes;
use jade_docx_editor::headless_api::execute_headless_queries_on_document;
use jade_docx_headless_api::{HeadlessQuery, HeadlessQueryRequest, HeadlessQueryResult};
use jade_office_model::word::design::{
    WordDesign, COLOR_SCHEMES, EFFECT_SCHEMES, FONT_SCHEMES, THEMES,
};
use jade_office_model::word::lists::{MultilevelPreset, Suffix, WordLists, NUMBER_FORMATS};
use jade_office_model::word::new::{blank_document, Blank};
use jade_office_model::word::package::Package;
use jade_office_model::word::paragraph_options::{
    BorderBox, BorderLine, DropCapPosition, ParagraphSwitch, ShadingSpec, Tab, TabAlignment,
    TabLeader, WordParagraphOptions,
};
use jade_office_model::word::styles::{NewStyle, StyleChange, StyleFormat, StyleKind, WordStyles};
use jade_web::editing::{EditableDocument, WordFormatting, WordSession};

/// A blank Japanese document with these paragraphs.
fn document(lines: &[&str]) -> EditableDocument {
    let mut document = open_bytes(&blank_document(Blank::Japanese));
    document.place_caret(0, 0);
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            document.insert_paragraph_break().unwrap();
        }
        document.insert_text(line).unwrap();
    }
    document
}

fn reopen(document: &mut EditableDocument) -> EditableDocument {
    open_bytes(&document.export_docx().unwrap())
}

fn part(document: &mut EditableDocument, kind: &str) -> String {
    let bytes = document.export_docx().unwrap();
    let package = Package::open(&bytes).unwrap();
    let name = if kind == "document" {
        package.main().to_string()
    } else {
        package.related(package.main(), kind).unwrap()
    };
    package.xml(&name).unwrap().to_xml()
}

/// The list label the engine draws before paragraph `index`.
fn label(document: &EditableDocument, index: usize) -> Option<String> {
    let results = execute_headless_queries_on_document(
        document.word_document(),
        &HeadlessQueryRequest {
            queries: vec![HeadlessQuery::ParagraphText { paragraph: index }],
        },
    )
    .unwrap();
    match results.results.into_iter().next()? {
        HeadlessQueryResult::ParagraphText { numbering_text, .. } => {
            numbering_text.map(|text| text.trim_end().to_string())
        }
        _ => None,
    }
}

/// Undo takes the whole edit back: the saved document is what it was.
fn assert_undoes(document: &mut EditableDocument, before: &[u8]) {
    document.undo().unwrap();
    assert_eq!(
        document.export_docx().unwrap(),
        before,
        "undo restores the document"
    );
}

#[test]
fn tab_stops_from_the_dialog_and_the_ruler_are_saved() {
    let mut document = document(&["名前\t値\t合計"]);
    let before = document.export_docx().unwrap();
    document
        .set_tab(Tab::new(72.0, TabAlignment::Right, TabLeader::Dot))
        .unwrap();
    document
        .set_tab(Tab::new(144.0, TabAlignment::Decimal, TabLeader::None))
        .unwrap();
    assert_eq!(document.paragraph_options().tabs.len(), 2);
    // Dragged along the ruler, then dragged off it.
    document.move_tab(144.0, Some(180.0)).unwrap();
    document.move_tab(72.0, None).unwrap();
    let tabs = document.paragraph_options().tabs;
    assert_eq!(
        tabs,
        vec![Tab::new(180.0, TabAlignment::Decimal, TabLeader::None)]
    );
    assert!(part(&mut document, "document").contains(r#"<w:tab w:val="decimal" w:pos="3600"/>"#));
    assert_eq!(reopen(&mut document).paragraph_options().tabs, tabs);
    document
        .set_tab(Tab::new(36.0, TabAlignment::Center, TabLeader::MiddleDot))
        .unwrap();
    document.clear_all_tabs().unwrap();
    assert!(document.paragraph_options().tabs.is_empty());

    document.set_default_tab_stop(42.0).unwrap();
    assert_eq!(document.default_tab_stop(), 42.0);
    assert_eq!(reopen(&mut document).default_tab_stop(), 42.0);
    for _ in 0..7 {
        document.undo().unwrap();
    }
    assert_eq!(document.export_docx().unwrap(), before);
}

#[test]
fn switches_borders_shading_alignment_and_drop_caps_are_saved() {
    let mut document = document(&["一行目の段落", "二行目の段落"]);
    document.select((0, 0), (1, 3)).unwrap();
    let before = document.export_docx().unwrap();
    document
        .set_paragraph_switch(ParagraphSwitch::ContextualSpacing, true)
        .unwrap();
    document
        .set_paragraph_switch(ParagraphSwitch::SuppressLineNumbers, true)
        .unwrap();
    document
        .set_paragraph_switch(ParagraphSwitch::DontHyphenate, true)
        .unwrap();
    let mut boxed = BorderBox::boxed(BorderLine::new("double", 1.5, Some("C00000")));
    boxed.between = Some(BorderLine::new("dotted", 0.5, None));
    document.set_border_box(&boxed).unwrap();
    let shading = ShadingSpec {
        pattern: "pct25".into(),
        color: None,
        fill: Some("FFF2CC".into()),
    };
    document.set_shading_spec(Some(&shading)).unwrap();
    WordFormatting::set_alignment(&mut document, "distribute").unwrap();

    let reopened = reopen(&mut document);
    for state in [document.paragraph_options(), reopened.paragraph_options()] {
        assert!(state.contextual_spacing && state.suppress_line_numbers && state.dont_hyphenate);
        assert_eq!(state.borders.top.as_ref().unwrap().style, "double");
        assert_eq!(
            state.borders.top.as_ref().unwrap().color.as_deref(),
            Some("C00000")
        );
        assert_eq!(state.borders.between.as_ref().unwrap().style, "dotted");
        assert_eq!(state.shading.as_ref(), Some(&shading));
        assert_eq!(state.alignment.as_deref(), Some("distribute"));
    }
    document.place_caret(0, 0);
    document
        .set_drop_cap(Some((DropCapPosition::Dropped, 2)))
        .unwrap();
    assert_eq!(
        reopen(&mut document).paragraph_options().drop_cap,
        Some((DropCapPosition::Dropped, 2))
    );
    for _ in 0..7 {
        document.undo().unwrap();
    }
    assert_eq!(document.export_docx().unwrap(), before);
}

#[test]
fn japanese_formats_start_values_continue_and_levels() {
    let mut document = document(&["項目", "項目", "項目", "別の段落", "項目", "項目"]);
    document.select((0, 0), (2, 2)).unwrap();
    let kanji = NUMBER_FORMATS
        .iter()
        .find(|format| format.format == "japaneseCounting" && format.text == "%1、")
        .unwrap();
    document
        .apply_list_format(kanji.format, kanji.text, None)
        .unwrap();
    assert_eq!(label(&document, 0).as_deref(), Some("一、"));
    assert_eq!(label(&document, 2).as_deref(), Some("三、"));

    // A second list after a plain paragraph, in circled numbers.
    document.select((4, 0), (5, 2)).unwrap();
    document
        .apply_list_format("decimalEnclosedCircle", "%1", None)
        .unwrap();
    assert_eq!(label(&document, 4).as_deref(), Some("①"));
    assert_eq!(label(&document, 5).as_deref(), Some("②"));
    // Changing an item's format changes the list's level.
    document.place_caret(5, 0);
    document
        .apply_list_format("aiueoFullWidth", "%1.", None)
        .unwrap();
    assert_eq!(label(&document, 4).as_deref(), Some("ア."));

    // 番号の設定: the second item and the rest of the first list from 5.
    document.place_caret(1, 0);
    let before = document.export_docx().unwrap();
    document.set_list_start(5).unwrap();
    assert_eq!(label(&document, 0).as_deref(), Some("一、"));
    assert_eq!(label(&document, 1).as_deref(), Some("五、"));
    assert_eq!(label(&document, 2).as_deref(), Some("六、"));
    let reopened = reopen(&mut document);
    assert_eq!(label(&reopened, 2).as_deref(), Some("六、"));
    assert_undoes(&mut document, &before);

    // 新しいリストを開始 then 前のリストから継続.
    document.set_list_start(1).unwrap();
    assert_eq!(label(&document, 1).as_deref(), Some("一、"));
    document.continue_list().unwrap();
    assert_eq!(label(&document, 1).as_deref(), Some("二、"));
    assert_eq!(label(&document, 2).as_deref(), Some("三、"));

    // Tab at the item's start: one level down, and back.
    document.place_caret(2, 0);
    document.change_list_level(1).unwrap();
    assert_eq!(document.list_state().unwrap().level, 1);
    document.set_list_level(0).unwrap();
    assert_eq!(document.list_state().unwrap().level, 0);
}

#[test]
fn a_defined_list_sets_alignment_indents_suffix_and_legal_numbering() {
    let mut document = document(&["第一", "第二", "第三"]);
    document.select((0, 0), (2, 1)).unwrap();
    document
        .apply_multilevel_preset(MultilevelPreset::Articles)
        .unwrap();
    assert_eq!(label(&document, 0).as_deref(), Some("Article I."));
    document.select((1, 0), (2, 1)).unwrap();
    document.change_list_level(1).unwrap();
    // Legal: Article I's numeral reads 1, the level keeps its 01.
    assert_eq!(label(&document, 1).as_deref(), Some("Section 1.01"));
    assert_eq!(label(&document, 2).as_deref(), Some("Section 1.02"));

    // リストのインデントの調整 on the caret's level.
    document.place_caret(1, 0);
    document
        .edit_list_level(|level| {
            level.alignment = "right".into();
            level.indent = 54.0;
            level.hanging = 18.0;
            level.suffix = Suffix::Nothing;
            level.legal = false;
        })
        .unwrap();
    let reopened = reopen(&mut document);
    let mut reopened = reopened;
    reopened.place_caret(1, 0);
    let state = reopened.list_state().unwrap();
    let level = &state.definition.levels[1];
    assert_eq!(level.alignment, "right");
    assert_eq!((level.indent, level.hanging), (54.0, 18.0));
    assert_eq!(level.suffix, Suffix::Nothing);
    assert_eq!(label(&reopened, 1).as_deref(), Some("Section I.01"));
}

#[test]
fn a_list_linked_to_headings_numbers_the_heading_paragraphs() {
    let mut document = document(&["はじめに", "本文", "目的"]);
    document.place_caret(0, 0);
    WordFormatting::set_paragraph_style(&mut document, Some("Heading1")).unwrap();
    document.place_caret(2, 0);
    WordFormatting::set_paragraph_style(&mut document, Some("Heading2")).unwrap();
    document.place_caret(0, 0);
    document
        .apply_multilevel_preset(MultilevelPreset::Chapters)
        .unwrap();
    assert_eq!(label(&document, 0).as_deref(), Some("第1章"));
    assert_eq!(label(&document, 2).as_deref(), Some("第1節"));
    assert_eq!(label(&document, 1), None);
    let styles = part(&mut document, "styles");
    assert!(styles.contains("<w:numPr>"), "{styles}");
    // A new heading paragraph is numbered through its style.
    let reopened = reopen(&mut document);
    assert_eq!(label(&reopened, 2).as_deref(), Some("第1節"));
}

#[test]
fn list_styles_are_created_and_applied() {
    let mut document = document(&["一", "二"]);
    document.select((0, 0), (1, 1)).unwrap();
    let definition = MultilevelPreset::Parenthesised.definition(&|_| None);
    let id = document.create_list_style("契約条項", &definition).unwrap();
    assert_eq!(label(&document, 1).as_deref(), Some("2)"));
    let styles = part(&mut document, "styles");
    assert!(styles.contains(&format!(
        r#"w:type="numbering" w:customStyle="1" w:styleId="{id}""#
    )));
    let numbering = part(&mut document, "numbering");
    assert!(numbering.contains(&format!(r#"<w:styleLink w:val="{id}"/>"#)));
}

#[test]
fn styles_are_created_modified_updated_and_applied() {
    let mut document = document(&["本文の段落です", "次の段落"]);
    document.place_caret(0, 0);
    let before = document.export_docx().unwrap();
    let catalog = document.style_catalog();
    let normal = catalog
        .iter()
        .find(|style| style.is_default)
        .unwrap()
        .id
        .clone();
    let id = document
        .create_style(&NewStyle {
            name: "要点".into(),
            kind: StyleKind::Paragraph,
            based_on: Some(normal.clone()),
            next: None,
            format: StyleFormat {
                size: Some(14.0),
                color: Some("C00000".into()),
                ..StyleFormat::default()
            },
            quick: true,
            auto_update: false,
        })
        .unwrap();
    assert_eq!(
        document.format_state().style_id.as_deref(),
        Some(id.as_str())
    );
    assert_eq!(document.format_state().font_size, Some(14.0));
    assert!(document.style_catalog().iter().any(|style| style.id == id
        && style.quick
        && style.based_on.as_deref() == Some(normal.as_str())));
    assert_undoes(&mut document, &before);
    document.redo().unwrap();

    // スタイルの変更: every paragraph in the style follows, and Enter at its
    // end gives the next style.
    document
        .modify_style(
            &id,
            &StyleChange {
                name: Some("要点 2".into()),
                next: Some(normal.clone()),
                format: StyleFormat {
                    size: Some(20.0),
                    bold: Some(true),
                    ..StyleFormat::default()
                },
                ..StyleChange::default()
            },
        )
        .unwrap();
    let state = document.format_state();
    assert_eq!((state.font_size, state.bold), (Some(20.0), true));
    let reopened = reopen(&mut document);
    let style = reopened
        .style_catalog()
        .into_iter()
        .find(|style| style.id == id)
        .unwrap();
    assert_eq!(style.name, "要点 2");
    assert_eq!(style.next.as_deref(), Some(normal.as_str()));
    let end = document.paragraph_text(0).unwrap().len();
    document.place_caret(0, end);
    document.insert_paragraph_break().unwrap();
    assert_eq!(
        document.format_state().style_id.as_deref(),
        Some(normal.as_str())
    );
    document.undo().unwrap();

    // 選択個所と一致するように更新.
    document.place_caret(0, 0);
    WordFormatting::set_alignment(&mut document, "center").unwrap();
    document.update_style_to_match(&id).unwrap();
    let style = document
        .style_catalog()
        .into_iter()
        .find(|style| style.id == id)
        .unwrap();
    assert_eq!(style.format.alignment.as_deref(), Some("center"));

    // 自動的に更新する: formatting the paragraph updates the style.
    document
        .modify_style(
            &id,
            &StyleChange {
                auto_update: Some(true),
                ..StyleChange::default()
            },
        )
        .unwrap();
    WordFormatting::set_alignment(&mut document, "right").unwrap();
    assert!(document.auto_update_style().unwrap());
    let style = document
        .style_catalog()
        .into_iter()
        .find(|style| style.id == id)
        .unwrap();
    assert_eq!(style.format.alignment.as_deref(), Some("right"));
    assert!(style.auto_update);
}

#[test]
fn character_and_linked_styles_apply_to_the_selection() {
    let mut document = document(&["重要な語を含む段落"]);
    document.select((0, 0), (0, "重要".len())).unwrap();
    let id = document
        .create_style(&NewStyle {
            name: "赤字".into(),
            kind: StyleKind::Character,
            based_on: None,
            next: None,
            format: StyleFormat {
                color: Some("FF0000".into()),
                bold: Some(true),
                ..StyleFormat::default()
            },
            quick: true,
            auto_update: false,
        })
        .unwrap();
    let styled = document.run_style_at((0, 1)).unwrap();
    assert_eq!(styled.bold, Some(true));
    assert_ne!(
        document.run_style_at((0, "重要な".len())).unwrap().bold,
        Some(true)
    );
    let reopened = reopen(&mut document);
    assert_eq!(reopened.run_style_at((0, 1)).unwrap().bold, Some(true));

    // A linked style on part of a paragraph formats the characters only.
    let linked = document
        .create_style(&NewStyle {
            name: "強調段落".into(),
            kind: StyleKind::Linked,
            based_on: None,
            next: None,
            format: StyleFormat {
                italic: Some(true),
                ..StyleFormat::default()
            },
            quick: true,
            auto_update: false,
        })
        .unwrap();
    // Creating it styled the paragraph; take that off and apply it to a part.
    WordFormatting::set_paragraph_style(&mut document, None).unwrap();
    document
        .select((0, "重要な".len()), (0, "重要な語".len()))
        .unwrap();
    document.apply_style(&linked).unwrap();
    assert_ne!(
        document.format_state().style_id.as_deref(),
        Some(linked.as_str())
    );
    assert_eq!(
        document.run_style_at((0, "重要な".len())).unwrap().italic,
        Some(true)
    );
    let _ = id;
}

#[test]
fn the_gallery_lists_quick_styles_by_priority() {
    let mut document = document(&["段落"]);
    document.place_caret(0, 0);
    for (name, quick) in [("後", true), ("隠れ", false)] {
        document
            .create_style(&NewStyle {
                name: name.into(),
                kind: StyleKind::Paragraph,
                based_on: None,
                next: None,
                format: StyleFormat::default(),
                quick,
                auto_update: false,
            })
            .unwrap();
    }
    let gallery = jade_office_model::word::styles::quick_style_gallery(&document.style_catalog());
    let names: Vec<_> = gallery
        .iter()
        .filter_map(|entry| entry.name.clone())
        .collect();
    assert!(names.contains(&"後".to_string()));
    assert!(!names.contains(&"隠れ".to_string()));
}

#[test]
fn themes_colors_fonts_effects_and_style_sets() {
    let mut document = document(&["見出し", "本文"]);
    let before = document.export_docx().unwrap();
    document.apply_theme(&THEMES[2]).unwrap();
    let state = document.design_state();
    assert_eq!(state.colors.as_deref(), Some("Office 2007 - 2010"));
    assert_eq!(state.fonts.as_deref(), Some("Office 2007 - 2010"));
    assert_eq!(state.effects.as_deref(), Some("Office 2007 - 2010"));
    assert_eq!(state.theme.as_deref(), Some("Office 2007 - 2010"));
    assert_undoes(&mut document, &before);

    let green = COLOR_SCHEMES
        .iter()
        .find(|scheme| scheme.name == "Green")
        .unwrap();
    document.apply_color_scheme(green).unwrap();
    document.apply_font_scheme(&FONT_SCHEMES[4]).unwrap();
    document.apply_effect_scheme(&EFFECT_SCHEMES[3]).unwrap();
    let reopened = reopen(&mut document);
    let state = reopened.design_state();
    assert_eq!(state.colors.as_deref(), Some("Green"));
    assert_eq!(state.accents[0], "549E39");
    assert_eq!(state.fonts.as_deref(), Some("Arial"));
    assert_eq!(state.effects.as_deref(), Some("Glow Edge"));
    // The engine reads the theme the runs' theme fonts resolve to.
    let theme = reopened.word_document().document_theme().unwrap();
    assert_eq!(theme.minor_font, "Arial");
    assert_eq!(theme.accent1, "549E39");

    // A style set restyles Normal and the headings.
    let set = &jade_office_model::word::design::style_sets()[1];
    document.apply_style_set(set).unwrap();
    let catalog = document.style_catalog();
    let heading = catalog
        .iter()
        .find(|style| style.name.eq_ignore_ascii_case("heading 1"))
        .unwrap();
    assert_eq!(heading.format.size, Some(18.0));
    assert_eq!(heading.format.bold, Some(true));
    let normal = catalog.iter().find(|style| style.is_default).unwrap();
    assert_eq!(normal.format.space_after, Some(10.0));
    document.place_caret(0, 0);
    document.apply_style(&heading.id).unwrap();
    assert_eq!(document.format_state().font_size, Some(18.0));

    // 段落の間隔.
    document.apply_paragraph_spacing(0.0, 6.0, 1.0).unwrap();
    let normal = document
        .style_catalog()
        .into_iter()
        .find(|style| style.is_default)
        .unwrap();
    assert_eq!(normal.format.space_after, Some(6.0));
}

#[test]
fn set_as_default_writes_the_document_defaults() {
    let mut document = document(&["段落"]);
    document.select((0, 0), (0, "段落".len())).unwrap();
    WordFormatting::set_font_size(&mut document, 16.0).unwrap();
    document.set_as_default().unwrap();
    let styles = part(&mut document, "styles");
    assert!(styles.contains(r#"<w:sz w:val="32"/>"#), "{styles}");
    let mut fresh = reopen(&mut document);
    fresh.place_caret(0, 0);
    fresh.insert_paragraph_break().unwrap();
    fresh.insert_text("新しい").unwrap();
    assert_eq!(fresh.run_style_at((1, 0)).unwrap().font_size, Some(16.0));
}

#[test]
fn a_template_s_styles_are_copied_when_attached() {
    // The template: an English blank with a style of its own and a red
    // heading 1.
    let mut template = document(&["テンプレート"]);
    template.place_caret(0, 0);
    template
        .create_style(&NewStyle {
            name: "Memo".into(),
            kind: StyleKind::Paragraph,
            based_on: None,
            next: None,
            format: StyleFormat {
                italic: Some(true),
                ..StyleFormat::default()
            },
            quick: true,
            auto_update: false,
        })
        .unwrap();
    let template_bytes = template.export_docx().unwrap();

    let mut document = document(&["本文"]);
    let copied = document
        .attach_template(&template_bytes, "/Users/me/Templates/契約書.dotx", true)
        .unwrap();
    assert!(copied >= 2, "{copied}");
    assert!(document
        .style_catalog()
        .iter()
        .any(|style| style.name == "Memo"));
    let settings = part(&mut document, "settings");
    assert!(settings.contains("<w:attachedTemplate r:id="), "{settings}");
    let bytes = document.export_docx().unwrap();
    let package = Package::open(&bytes).unwrap();
    let rels = String::from_utf8(
        package
            .bytes("word/_rels/settings.xml.rels")
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(rels.contains("TargetMode=\"External\""), "{rels}");
    assert!(rels.contains("file:///Users/me/Templates/"), "{rels}");
}
