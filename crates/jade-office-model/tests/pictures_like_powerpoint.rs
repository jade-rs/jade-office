//! A picture as PowerPoint for Mac's 図の形式 tab edits it, through the
//! model both shells share: 図の変更, 修整, 色, アート効果, 透明度, 図の
//! スタイル, 回転 and 反転, サイズ with 縦横比を固定, トリミング and 縦横比,
//! 図のリセット, 図の圧縮, SVG and アイコン, フォト アルバム and animated
//! GIFs — each one Undo, and drawn on the slide.

use std::path::PathBuf;
use std::sync::Arc;

use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use jade_office_model::aliased_faces;
use jade_office_model::deck::{
    aspect_crop, recolor_presets, set_picture_clock, sized, Deck, PictureEdit, ShapeRect,
    ARTISTIC_EFFECTS, ICONS,
};
use jade_office_model::tiny::TinySkiaBackend;

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn deck() -> Deck<TinySkiaBackend> {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core().join("public/fixtures/sample-deck.pptx")).unwrap();
    Deck::open(&bytes, backend).unwrap()
}

fn png(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(RgbImage::from_pixel(width, height, Rgb(color)))
        .write_to(&mut out, ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

/// The colour drawn at the middle of shape `id` on slide 0.
fn pixel_at_centre(deck: &mut Deck<TinySkiaBackend>, id: u32) -> [u8; 4] {
    let rect = deck.shape_rect(0, id).unwrap();
    let (width, _) = deck.slide_size();
    let surface = deck.render_slide(0, width.round() as u32).unwrap();
    let png = surface.encode_png().unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    let (x, y) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    image.get_pixel(x.round() as u32, y.round() as u32).0
}

#[test]
fn colour_and_corrections_are_drawn_and_undone() {
    let mut deck = deck();
    let id = deck
        .insert_picture(0, &png(120, 80, [220, 40, 40]))
        .unwrap();
    let red = pixel_at_centre(&mut deck, id);
    assert!(red[0] > 180 && red[1] < 80, "{red:?}");
    deck.format_picture(0, id, &PictureEdit::Recolor(recolor_presets()[1].2.clone()))
        .unwrap();
    let grey = pixel_at_centre(&mut deck, id);
    assert!(grey[0] == grey[1] && grey[1] == grey[2], "{grey:?}");
    assert!(deck.undo().unwrap());
    deck.format_picture(
        0,
        id,
        &PictureEdit::Corrections {
            brightness: 0.4,
            contrast: 0.0,
        },
    )
    .unwrap();
    let brighter = pixel_at_centre(&mut deck, id);
    assert!(brighter[1] > red[1] + 60, "{brighter:?}");
    // Baked effects show in the picture the blip now names.
    deck.format_picture(0, id, &PictureEdit::Saturation(0.0))
        .unwrap();
    let flat = pixel_at_centre(&mut deck, id);
    assert!(
        (i32::from(flat[0]) - i32::from(flat[2])).abs() < 4,
        "{flat:?}"
    );
    deck.format_picture(0, id, &PictureEdit::Reset).unwrap();
    assert_eq!(pixel_at_centre(&mut deck, id), red);
    assert!(ARTISTIC_EFFECTS.len() >= 20);
}

#[test]
fn rotate_flip_size_and_aspect() {
    let mut deck = deck();
    let id = deck.insert_picture(0, &png(200, 100, [0, 128, 0])).unwrap();
    deck.rotate_picture(0, id, 90.0).unwrap();
    assert_eq!(
        deck.shape(0, id)
            .unwrap()
            .transform
            .unwrap()
            .rotation_60000ths,
        90 * 60_000
    );
    deck.rotate_picture(0, id, -90.0).unwrap();
    assert_eq!(
        deck.shape(0, id)
            .unwrap()
            .transform
            .unwrap()
            .rotation_60000ths,
        0
    );
    deck.flip_picture(0, id, true).unwrap();
    assert!(deck.shape(0, id).unwrap().transform.unwrap().flip_h);
    // 縦横比を固定 is on for a new picture: the height follows the width.
    let before = deck.shape_rect(0, id).unwrap();
    deck.set_picture_size(0, id, Some(before.width / 2.0), None)
        .unwrap();
    let after = deck.shape_rect(0, id).unwrap();
    assert!((after.height - before.height / 2.0).abs() < 0.5);
    deck.format_picture(0, id, &PictureEdit::LockAspect(false))
        .unwrap();
    deck.set_picture_size(0, id, None, Some(10.0)).unwrap();
    assert!((deck.shape_rect(0, id).unwrap().width - after.width).abs() < 0.5);
    let rect = ShapeRect {
        x: 0.0,
        y: 0.0,
        width: 40.0,
        height: 20.0,
    };
    assert_eq!(sized(rect, Some(80.0), None, true).height, 40.0);
    // 縦横比 1:1 on a 2:1 picture keeps its middle half.
    let crop = aspect_crop(200.0, 100.0, 1.0, 1.0);
    assert_eq!((crop.left, crop.right, crop.top), (25_000, 25_000, 0));
    deck.crop_picture_to_aspect(0, id, 1.0, 1.0).unwrap();
    let cropped = deck.shape_rect(0, id).unwrap();
    assert!((cropped.width - cropped.height).abs() < 1.0, "{cropped:?}");
}

#[test]
fn change_compress_svg_icons_album_and_gifs() {
    let mut deck = deck();
    let id = deck
        .insert_picture(0, &png(400, 200, [10, 10, 200]))
        .unwrap();
    deck.change_picture(0, id, &png(400, 200, [250, 250, 10]))
        .unwrap();
    let yellow = pixel_at_centre(&mut deck, id);
    assert!(yellow[0] > 200 && yellow[2] < 60, "{yellow:?}");
    let count = deck.compress_pictures(Some((0, id)), 96, true).unwrap();
    assert!(count <= 1);

    let icon = deck.insert_icon(0, 0).unwrap();
    let info = deck.picture_format(0, icon).unwrap();
    assert!(info.svg);
    assert_eq!(ICONS[0].0, "チェック");
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><rect width="100" height="50" fill="#00ff00"/></svg>"##;
    let drawn = deck.insert_picture_file(0, svg).unwrap();
    let green = pixel_at_centre(&mut deck, drawn);
    assert!(green[1] > 200 && green[0] < 60, "{green:?}");
    deck.format_picture(0, drawn, &PictureEdit::GraphicFill("#FF0000".into()))
        .unwrap();
    let red = pixel_at_centre(&mut deck, drawn);
    assert!(red[0] > 200 && red[1] < 60, "{red:?}");
    // 図形に変換: freeforms drawn as the picture was.
    let group = deck.convert_svg_to_shapes(0, drawn).unwrap();
    assert!(deck.picture_format(0, drawn).is_none());
    let shape = pixel_at_centre(&mut deck, group);
    assert!(shape[0] > 200 && shape[1] < 60, "{shape:?}");

    let slides = deck.slide_count();
    let first = deck
        .insert_photo_album(
            0,
            &[
                png(40, 30, [1, 2, 3]),
                png(30, 40, [4, 5, 6]),
                png(50, 50, [7, 8, 9]),
            ],
            2,
        )
        .unwrap();
    assert_eq!(first, 1);
    assert_eq!(deck.slide_count(), slides + 2);

    // An animated GIF plays: the frame drawn follows the clock.
    let mut gif = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut gif);
        for color in [[255u8, 0, 0, 255], [0, 0, 255, 255]] {
            encoder
                .encode_frame(image::Frame::from_parts(
                    image::RgbaImage::from_pixel(60, 60, image::Rgba(color)),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(500, 1),
                ))
                .unwrap();
        }
    }
    let animated = deck.insert_picture_file(0, &gif).unwrap();
    assert!(deck.has_animated_pictures(0));
    set_picture_clock(0);
    let first_frame = pixel_at_centre(&mut deck, animated);
    set_picture_clock(600);
    let second_frame = pixel_at_centre(&mut deck, animated);
    assert!(first_frame[0] > 200, "{first_frame:?}");
    assert!(second_frame[2] > 200, "{second_frame:?}");
}
