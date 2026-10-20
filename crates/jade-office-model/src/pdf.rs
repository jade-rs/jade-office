//! A PDF of pictures, one a page: what the browser build prints and
//! exports, where tiny-skia paints pixels and has no PDF of its own (the
//! native app records Skia's drawing as a vector PDF instead). Each picture
//! is the page's pixels, Flate-compressed RGB, filling paper of the page's
//! size; its text is not selectable.

use std::io::Write;

/// One page: its paper in points, and its pixels as RGB, row by row.
pub struct Picture {
    pub width_pt: f64,
    pub height_pt: f64,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub rgb: Vec<u8>,
}

/// The PDF of `pages`, in order.
pub fn of_pictures(pages: &[Picture]) -> Result<Vec<u8>, String> {
    if pages.is_empty() {
        return Err("a PDF needs a page".to_string());
    }
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    // Objects: 1 the catalog, 2 the page tree, then three a page — the
    // page, its drawing, its picture.
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
        let id = 3 + index * 3;
        let expected = page.pixel_width as usize * page.pixel_height as usize * 3;
        if page.rgb.len() != expected {
            return Err(format!("page {}'s pixels do not fill it", index + 1));
        }
        let (width, height) = (page.width_pt, page.height_pt);
        object(
            &mut out,
            &mut offsets,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width:.2} {height:.2}] \
                 /Resources << /XObject << /P {} 0 R >> >> /Contents {} 0 R >>",
                id + 2,
                id + 1
            )
            .as_bytes(),
        );
        let drawing = format!("q {width:.2} 0 0 {height:.2} 0 0 cm /P Do Q");
        object(
            &mut out,
            &mut offsets,
            &stream(
                &format!("<< /Length {} >>", drawing.len()),
                drawing.as_bytes(),
            ),
        );
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder
            .write_all(&page.rgb)
            .map_err(|error| error.to_string())?;
        let pixels = encoder.finish().map_err(|error| error.to_string())?;
        object(
            &mut out,
            &mut offsets,
            &stream(
                &format!(
                    "<< /Type /XObject /Subtype /Image /Width {} /Height {} \
                     /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode \
                     /Length {} >>",
                    page.pixel_width,
                    page.pixel_height,
                    pixels.len()
                ),
                &pixels,
            ),
        );
    }
    let table = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{table}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    Ok(out)
}

fn stream(dictionary: &str, data: &[u8]) -> Vec<u8> {
    let mut body = dictionary.as_bytes().to_vec();
    body.extend_from_slice(b"\nstream\n");
    body.extend_from_slice(data);
    body.extend_from_slice(b"\nendstream");
    body
}
