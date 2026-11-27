use jade_docx_layout::line_metrics::{set_word_layout_platform, WordLayoutPlatform};
use jade_opc::OpcPackage;
use jade_web::document::full_layout_debug;
use std::path::Path;

const WML: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const DISABLE_SPACING: &str = r#"<w:autoSpaceDE w:val="0"/><w:autoSpaceDN w:val="0"/>"#;

fn narrow_cell_fixture(donor: &[u8], defaults: &str) -> Vec<u8> {
    let mut package = OpcPackage::open(donor).unwrap();
    let styles = format!(
        r#"<w:styles xmlns:w="{WML}">
          <w:docDefaults><w:pPrDefault><w:pPr>{defaults}</w:pPr></w:pPrDefault></w:docDefaults>
        </w:styles>"#
    );
    let settings = format!(
        r#"<w:settings xmlns:w="{WML}"><w:compat>
          <w:balanceSingleByteDoubleByteWidth/><w:useFELayout/>
          <w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/>
        </w:compat></w:settings>"#
    );
    let document = format!(
        r#"<w:document xmlns:w="{WML}"><w:body><w:tbl>
          <w:tblPr><w:tblW w:w="1080" w:type="dxa"/><w:tblLayout w:type="fixed"/></w:tblPr>
          <w:tblGrid><w:gridCol w:w="1080"/></w:tblGrid>
          <w:tr><w:tc><w:tcPr>
            <w:tcW w:w="1080" w:type="dxa"/>
            <w:tcMar><w:left w:w="0" w:type="dxa"/><w:right w:w="0" w:type="dxa"/></w:tcMar>
          </w:tcPr><w:p>
            <w:pPr><w:ind w:left="20"/><w:spacing w:line="240" w:lineRule="exact"/></w:pPr>
            <w:r><w:rPr>
              <w:rFonts w:ascii="Noto Sans JP" w:hAnsi="Noto Sans JP" w:eastAsia="Noto Sans JP"/>
              <w:sz w:val="24"/>
            </w:rPr><w:t>様A第5号</w:t></w:r>
          </w:p></w:tc></w:tr>
        </w:tbl><w:sectPr/></w:body></w:document>"#
    );
    package.replace("word/styles.xml", styles.into_bytes());
    package.replace("word/settings.xml", settings.into_bytes());
    package.replace("word/document.xml", document.into_bytes());
    package.write().unwrap()
}

#[test]
fn word_table_respects_document_default_east_asian_spacing() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jade-core");
    let donor = std::fs::read(core.join("contract.docx")).unwrap();
    let regular =
        std::fs::read(core.join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf")).unwrap();
    let (backend, measurer) = jade_web::bundled_fonts(regular, None, None, None).unwrap();
    let control = narrow_cell_fixture(&donor, "");
    let disabled = narrow_cell_fixture(&donor, DISABLE_SPACING);
    for platform in [WordLayoutPlatform::Mac, WordLayoutPlatform::Windows] {
        set_word_layout_platform(platform);
        let lines = |bytes: &[u8]| {
            let snapshot = full_layout_debug(bytes, backend.fonts(), measurer.clone()).unwrap();
            assert!(snapshot.complete);
            snapshot
                .body_lines
                .into_iter()
                .map(|line| line.text)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
        };
        assert_eq!(lines(&control), ["様A第5", "号"], "{platform:?}");
        assert_eq!(lines(&disabled), ["様A第5号"], "{platform:?}");
    }
}
