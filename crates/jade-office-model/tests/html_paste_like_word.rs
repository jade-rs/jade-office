//! 形式を選択して貼り付け → HTML 形式 for a copy from another application:
//! the clipboard's `text/html` read as Word reads a paste of it —
//! paragraphs from block elements and paragraph-internal breaks from `br`, the runs' bold, italic,
//! underline, strikethrough, super/subscript, size, face and colour from
//! the tags and the inline styles, entities decoded, white space collapsed.

use jade_office_model::word::clipboard::RichClip;

fn texts(clip: &RichClip) -> Vec<String> {
    clip.paragraphs
        .iter()
        .map(|paragraph| paragraph.runs.iter().map(|run| run.text.as_str()).collect())
        .collect()
}

#[test]
fn blocks_are_paragraphs_and_line_breaks_are_inline() {
    let clip = RichClip::from_html(
        "<html><head><style>p{color:red}</style><title>x</title></head><body>\
         <p>First   paragraph</p>\n<div>Second<br>third</div><ul><li>one</li><li>two</li></ul>\
         <!-- a comment --></body></html>",
    );
    assert_eq!(
        texts(&clip),
        ["First paragraph", "Second\nthird", "one", "two"]
    );
}

#[test]
fn the_tags_and_inline_styles_format_the_runs() {
    let clip = RichClip::from_html(
        "<p style=\"text-align:center\">plain <b>bold</b> <i>italic</i> <u>under</u> \
         <s>gone</s> H<sub>2</sub>O x<sup>2</sup></p>\
         <p><span style=\"font-weight:700; font-size:16px; color:#c00; \
         font-family:'Yu Gothic', sans-serif\">styled</span> \
         <span style=\"color: rgb(0, 112, 192); text-decoration: underline\">blue</span></p>",
    );
    assert_eq!(clip.paragraphs.len(), 2);
    let first = &clip.paragraphs[0];
    assert_eq!(first.alignment.as_deref(), Some("center"));
    let style_of = |paragraph: &jade_office_model::word::clipboard::ClipParagraph, text: &str| {
        paragraph
            .runs
            .iter()
            .find(|run| run.text == text)
            .unwrap_or_else(|| panic!("no run {text:?} in {:?}", paragraph.runs))
            .style
            .clone()
    };
    assert_eq!(style_of(first, "bold").bold, Some(true));
    assert_eq!(style_of(first, "italic").italic, Some(true));
    assert_eq!(style_of(first, "under").underline, Some(true));
    assert_eq!(style_of(first, "gone").strike, Some(true));
    assert_eq!(style_of(first, "2").baseline.as_deref(), Some("subscript"));
    assert!(first
        .runs
        .iter()
        .any(|run| run.style.baseline.as_deref() == Some("superscript")));
    let second = &clip.paragraphs[1];
    let styled = style_of(second, "styled");
    assert_eq!(styled.bold, Some(true));
    assert_eq!(styled.font_size, Some(12.0));
    assert_eq!(styled.color.as_deref(), Some("CC0000"));
    assert_eq!(styled.font_family.as_deref(), Some("Yu Gothic"));
    let blue = style_of(second, "blue");
    assert_eq!(blue.color.as_deref(), Some("0070C0"));
    assert_eq!(blue.underline, Some(true));
    assert_eq!(
        clip.plain_text(),
        "plain bold italic under gone H2O x2\nstyled blue"
    );
}

#[test]
fn entities_fragments_and_office_markup() {
    let clip = RichClip::from_html(
        "<html><body>ignored<!--StartFragment--><p class=MsoNormal>A &amp; B &lt;C&gt; \
         &#x3042;&#12356;&nbsp;end<o:p></o:p></p><script>alert(1)</script>\
         <table><tr><td>a</td><td>b</td></tr></table><!--EndFragment-->after</body></html>",
    );
    assert_eq!(texts(&clip), ["A & B <C> あい\u{a0}end", "a\tb"]);
}

#[test]
fn headings_are_bold_and_larger() {
    let clip = RichClip::from_html("<h1>Title</h1><p>body</p>");
    let title = &clip.paragraphs[0].runs[0].style;
    assert_eq!(title.bold, Some(true));
    assert_eq!(title.font_size, Some(24.0));
    assert_eq!(clip.paragraphs[1].runs[0].style.bold, None);
}

#[test]
fn empty_paragraphs_survive_at_the_edges_and_between_text() {
    let clip = RichClip::from_html(
        "<div><p style='text-align:center'></p><p>First</p><p> </p>\
         <p>Second</p><p><br></p></div>",
    );
    assert_eq!(texts(&clip), ["", "First", "", "Second", ""]);
    assert_eq!(clip.paragraphs[0].alignment.as_deref(), Some("center"));
    let restored = RichClip::from_html(&clip.to_html());
    assert_eq!(texts(&restored), texts(&clip));
    assert_eq!(restored.paragraphs[0].alignment.as_deref(), Some("center"));
}

#[test]
fn explicit_blank_lines_do_not_create_extra_wrapper_paragraphs() {
    for html in [
        "<div><p><br></p></div>",
        "<section><div><p></p></div></section>",
        "<div><br></div>",
    ] {
        assert_eq!(texts(&RichClip::from_html(html)), [""], "{html}");
    }
    assert_eq!(
        texts(&RichClip::from_html("<p>First<br><br></p><p>Last</p>")),
        ["First\n\n", "Last"]
    );
    assert!(RichClip::from_html("<div> \n </div>").paragraphs.is_empty());
}

#[test]
fn html_line_breaks_stay_inside_their_paragraph_and_keep_formatting() {
    let clip = RichClip::from_html(
        "<p style='text-align:center'>First<br><b>second</b><br><br>last</p><p style='text-align:left'>Next</p>",
    );
    assert_eq!(texts(&clip), ["First\nsecond\n\nlast", "Next"]);
    assert_eq!(clip.paragraphs[0].alignment.as_deref(), Some("center"));
    assert!(clip.paragraphs[0]
        .runs
        .iter()
        .any(|r| r.text == "second" && r.style.bold == Some(true)));
    assert_eq!(RichClip::from_html(&clip.to_html()), clip);
}

#[test]
fn copied_soft_breaks_roundtrip_even_without_surrounding_text() {
    use jade_office_model::word::clipboard::{ClipParagraph, ClipRun};
    for text in ["\n", "\n\n", "start\n", "\nend"] {
        let clip = RichClip {
            paragraphs: vec![ClipParagraph {
                alignment: Some("left".into()),
                runs: vec![ClipRun {
                    text: text.into(),
                    style: Default::default(),
                }],
                ..Default::default()
            }],
        };
        assert_eq!(RichClip::from_html(&clip.to_html()), clip, "{text:?}");
    }
}

#[test]
fn unicode_spaces_are_not_collapsed_or_changed_into_breakable_spaces() {
    let clip = RichClip::from_html(
        "<p>&nbsp;<b>10&#x202f;000&nbsp;円</b>&ensp;&emsp;　　&#x2009;&#x2009;&nbsp;</p>\
         <p>  ordinary \t\n spaces  </p>",
    );
    assert_eq!(
        texts(&clip),
        [
            "\u{a0}10\u{202f}000\u{a0}円\u{2002}\u{2003}　　\u{2009}\u{2009}\u{a0}",
            "ordinary spaces"
        ]
    );
    let amount = clip.paragraphs[0]
        .runs
        .iter()
        .find(|r| r.text.contains("10"))
        .unwrap();
    assert_eq!(amount.style.bold, Some(true));
    assert_eq!(texts(&RichClip::from_html(&clip.to_html())), texts(&clip));
}

#[test]
fn preformatted_text_keeps_nonbreaking_spaces_and_tabs() {
    let clip = RichClip::from_html(
        "<pre> A&nbsp; B\t　\nC </pre><p style='white-space:pre-wrap'>&nbsp; x&nbsp; </p>",
    );
    assert_eq!(texts(&clip), [" A\u{a0} B\t　\nC ", "\u{a0} x\u{a0} "]);
    assert_eq!(texts(&RichClip::from_html(&clip.to_html())), texts(&clip));
}

#[test]
fn quoted_attribute_delimiters_do_not_become_document_text() {
    let clip = RichClip::from_html(
        "<p title='a > b'><span title=\"x > y\" style='font-weight:bold;color:#123456'>正しい</span> text</p>",
    );
    assert_eq!(texts(&clip), ["正しい text"]);
    let run = &clip.paragraphs[0].runs[0];
    assert_eq!(run.style.bold, Some(true));
    assert_eq!(run.style.color.as_deref(), Some("123456"));
}

#[test]
fn attribute_names_inside_values_are_not_formatting_attributes() {
    let clip = RichClip::from_html(
        "<p title=\"example align=right style='font-weight:bold'\" align=center>plain\
         <span title=\"example style='color:red'\" style='color:#123456'>blue</span></p>",
    );
    assert_eq!(texts(&clip), ["plainblue"]);
    assert_eq!(clip.paragraphs[0].alignment.as_deref(), Some("center"));
    assert_eq!(clip.paragraphs[0].runs[0].style.bold, None);
    let blue = clip.paragraphs[0]
        .runs
        .iter()
        .find(|run| run.text == "blue")
        .unwrap();
    assert_eq!(blue.style.color.as_deref(), Some("123456"));
}

#[test]
fn unquoted_attribute_values_keep_slashes_and_apostrophes() {
    let clip = RichClip::from_html(
        "<p title=author's><font face=Foo/Bar color=#123456>text</font> after</p>",
    );
    assert_eq!(texts(&clip), ["text after"]);
    assert_eq!(
        clip.paragraphs[0].runs[0].style.font_family.as_deref(),
        Some("Foo/Bar")
    );
    assert_eq!(
        clip.paragraphs[0].runs[0].style.color.as_deref(),
        Some("123456")
    );
}
