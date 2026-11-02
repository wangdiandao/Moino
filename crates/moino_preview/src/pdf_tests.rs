//! Independent inspection of the emitted PDF, rather than compiler-side layout claims.
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, SoftMask, font::GlyphRun, hayro_cmap::BfString, interpret_page,
};
use hayro::hayro_syntax::{
    Pdf,
    object::{Array, Dict, Name, Stream},
};
use hayro::kurbo::{Affine, BezPath, Rect};

#[derive(Default)]
struct TextDevice {
    text: String,
}

impl Device<'_> for TextDevice {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'_>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, '_>, _: DrawProps<'_>, _: &DrawMode) {
        for glyph in run.glyphs() {
            match glyph
                .as_unicode()
                .expect("fixture glyph has a Unicode mapping")
            {
                BfString::Char(character) => self.text.push(character),
                BfString::String(text) => self.text.push_str(&text),
            }
        }
    }
    fn draw_image(&mut self, _: Image<'_, '_>, _: ImageDrawProps<'_>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

fn page_text(pdf: &Pdf) -> Vec<String> {
    let cache = InterpreterCache::new();
    pdf.pages()
        .iter()
        .map(|page| {
            let (width, height) = page.render_dimensions();
            let mut context = Context::new(
                Affine::IDENTITY,
                Rect::new(0., 0., f64::from(width), f64::from(height)),
                &cache,
                page.xref(),
                Default::default(),
            );
            let mut device = TextDevice::default();
            interpret_page(page, &mut context, &mut device);
            device
                .text
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect()
        })
        .collect()
}

fn bookmark_title(bytes: &[u8]) -> String {
    if let Some(bytes) = bytes.strip_prefix(&[0xfe, 0xff]) {
        assert_eq!(bytes.len() % 2, 0);
        let units = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|unit| u16::from_be_bytes([unit[0], unit[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).expect("valid UTF-16 bookmark")
    } else {
        String::from_utf8(bytes.to_vec()).expect("ASCII fixture bookmark")
    }
}

#[test]
// Spec: docs/acceptance/m0.md#技术原型
fn emitted_pdf_preserves_cjk_search_text_embedded_fonts_bookmarks_and_repeated_table_headers() {
    let mut source = String::from(
        "#set page(width: 220pt, height: 190pt, margin: 12pt)\n#set text(size: 10pt)\n= Moino\n中文测试。日本語の検証。한국어 검증.\n\n= Repeated table header\n#table(columns: 2, table.header([M0 header], [Value]),\n",
    );
    for row in 0..50 {
        source.push_str(&format!("[Row {row}], [Validation],\n"));
    }
    source.push_str(")\n");
    let compiled = moino_export::compile_prototype(&source).expect("pagination fixture");
    assert!(compiled.warnings().is_empty(), "{:?}", compiled.warnings());
    let pdf = Pdf::new(compiled.bytes().to_vec()).expect("parsed emitted PDF");
    assert!(pdf.pages().len() >= 3);
    let pages = page_text(&pdf);
    let text = pages.join("");
    for expected in [
        "Moino",
        "中文测试。",
        "日本語の検証。",
        "한국어검증.",
        "Row0",
        "Row49",
    ] {
        assert!(
            text.contains(expected),
            "missing searchable text {expected}: {text}"
        );
    }
    let mut table_pages = 0;
    for text in &pages {
        if text.contains("Validation") {
            table_pages += 1;
            assert!(
                text.contains("M0headerValue"),
                "table page lost its repeated header: {text}"
            );
        }
    }
    assert!(table_pages >= 3, "fixture must exercise repeated headers");

    let objects = pdf
        .objects()
        .into_iter()
        .filter_map(|object| object.into_dict())
        .collect::<Vec<_>>();
    let mut fonts = 0;
    for page in pdf.pages().iter() {
        for name in page.resources().fonts.keys() {
            let font = page.resources().get_font(&name).expect("referenced font");
            let descendants = font.get::<Array<'_>>(b"DescendantFonts");
            let referenced =
                descendants.map_or_else(|| vec![font], |array| array.iter::<Dict<'_>>().collect());
            assert!(!referenced.is_empty(), "font must resolve to a face");
            for font in referenced {
                let descriptor = font
                    .get::<Dict<'_>>(b"FontDescriptor")
                    .expect("referenced font descriptor");
                let stream = [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"]
                    .into_iter()
                    .find_map(|key| descriptor.get::<Stream<'_>>(key))
                    .expect("referenced font is embedded");
                assert!(!stream.decoded().expect("decoded embedded font").is_empty());
                fonts += 1;
            }
        }
    }
    assert!(fonts >= 2, "fixture must include Latin and CJK fonts");
    let catalog = objects
        .iter()
        .find(|dict| dict.get::<Name<'_>>(b"Type").as_deref() == Some(b"Catalog"))
        .expect("catalog");
    let outlines = catalog.get::<Dict<'_>>(b"Outlines").expect("outline tree");
    let mut bookmark = outlines.get::<Dict<'_>>(b"First");
    let mut titles = Vec::new();
    while let Some(node) = bookmark {
        assert!(
            titles.len() < 3,
            "outline cycle or unexpected extra fixture bookmark"
        );
        let title = node
            .get::<hayro::hayro_syntax::object::String<'_>>(b"Title")
            .expect("bookmark title");
        titles.push(bookmark_title(title.as_bytes()));
        let destination = node
            .get::<Array<'_>>(b"Dest")
            .expect("bookmark destination");
        let page = destination
            .iter::<Dict<'_>>()
            .next()
            .expect("destination page");
        assert_eq!(
            page.get::<Name<'_>>(b"Type").as_deref(),
            Some(b"Page".as_slice())
        );
        bookmark = node.get::<Dict<'_>>(b"Next");
    }
    assert_eq!(titles, ["Moino", "Repeated table header"]);
}
