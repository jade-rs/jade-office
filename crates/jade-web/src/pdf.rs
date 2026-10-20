//! A PDF of pages as they are painted: each page one picture, at the size
//! of the paper. What the browser build exports — the pages exactly as the
//! editor shows them — where the native app records the painter's drawing
//! as PDF instead (`NativeDocument::export_pdf`).
//!
//! The pictures are the pages' pixels, Flate-compressed RGB; the text is
//! not selectable in them.

use std::io::Write;

/// One page: its paper in points, and its pixels, RGBA, row by row.
pub struct PdfPage {
    pub width_pt: f64,
    pub height_pt: f64,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub rgba: Vec<u8>,
}

/// The PDF holding `pages`, in order.
pub fn pdf_of_pages(pages: &[PdfPage]) -> Result<Vec<u8>, String> {
    if pages.is_empty() {
        return Err("a PDF needs a page".to_string());
    }
    // Objects: 1 catalog, 2 pages, then three per page — the page, its
    // content stream and its picture.
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
    let object = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    object(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages.len())
        .map(|index| format!("{} 0 R", 3 + index * 3))
        .collect();
    object(
        &mut out,
        &mut offsets,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .as_bytes(),
    );
    for (index, page) in pages.iter().enumerate() {
        let page_id = 3 + index * 3;
        let (width, height) = (page.width_pt, page.height_pt);
        object(
            &mut out,
            &mut offsets,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width:.2} {height:.2}] \
                 /Resources << /XObject << /P {} 0 R >> >> /Contents {} 0 R >>",
                page_id + 2,
                page_id + 1
            )
            .as_bytes(),
        );
        let drawing = format!("q {width:.2} 0 0 {height:.2} 0 0 cm /P Do Q");
        object(
            &mut out,
            &mut offsets,
            &stream(
                format!("<< /Length {} >>", drawing.len()).as_bytes(),
                drawing.as_bytes(),
            ),
        );
        let expected = page.pixel_width as usize * page.pixel_height as usize * 4;
        if page.rgba.len() != expected {
            return Err("a page's pixels do not fill it".to_string());
        }
        let rgb: Vec<u8> = page
            .rgba
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder
            .write_all(&rgb)
            .map_err(|error| format!("cannot compress a page: {error}"))?;
        let compressed = encoder
            .finish()
            .map_err(|error| format!("cannot compress a page: {error}"))?;
        object(
            &mut out,
            &mut offsets,
            &stream(
                format!(
                    "<< /Type /XObject /Subtype /Image /Width {} /Height {} \
                     /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode \
                     /Length {} >>",
                    page.pixel_width,
                    page.pixel_height,
                    compressed.len()
                )
                .as_bytes(),
                &compressed,
            ),
        );
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", offsets.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    Ok(out)
}

fn stream(dictionary: &[u8], data: &[u8]) -> Vec<u8> {
    let mut body = dictionary.to_vec();
    body.extend_from_slice(b"\nstream\n");
    body.extend_from_slice(data);
    body.extend_from_slice(b"\nendstream");
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_become_a_pdf_with_its_cross_reference() {
        let page = PdfPage {
            width_pt: 595.3,
            height_pt: 841.9,
            pixel_width: 2,
            pixel_height: 1,
            rgba: vec![255, 255, 255, 255, 0, 0, 0, 255],
        };
        let pdf = pdf_of_pages(&[page]).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Count 1"));
        assert!(text.contains("/MediaBox [0 0 595.30 841.90]"));
        assert!(text.trim_end().ends_with("%%EOF"));
        // Every object the table lists starts where it says.
        let xref = pdf
            .windows(5)
            .rposition(|window| window == b"xref\n")
            .unwrap();
        let table = String::from_utf8(pdf[xref..].to_vec()).unwrap();
        for (number, line) in table.lines().skip(3).take(5).enumerate() {
            let offset: usize = line[..10].parse().unwrap();
            let start = format!("{} 0 obj", number + 1);
            assert!(pdf[offset..].starts_with(start.as_bytes()), "{line}");
        }
        assert!(pdf_of_pages(&[]).is_err());
    }
}
