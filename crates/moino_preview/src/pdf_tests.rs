//! Independent inspection of the emitted PDF, rather than compiler-side layout claims.
use hayro::hayro_interpret::{
    BlendMode, CacheKey, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, SoftMask,
    font::{Glyph, GlyphRun},
    hayro_cmap::BfString,
    interpret_page,
};
use hayro::hayro_syntax::{
    Pdf,
    content::ops::TypedInstruction,
    object::{Array, Dict, Name, Object, Stream},
    page::Page,
};
use hayro::kurbo::{Affine, BezPath, Rect};

#[derive(Default)]
struct TextDevice {
    text: String,
    missing: Vec<String>,
    glyphs: Vec<String>,
    marked_text: std::collections::VecDeque<Option<String>>,
    actual_text_stack: Vec<bool>,
}

impl Device<'_> for TextDevice {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'_>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, '_>, _: DrawProps<'_>, _: &DrawMode) {
        for glyph in run.glyphs() {
            let identity = match &**glyph {
                Glyph::Outline(outline) => format!(
                    "font={:x} glyph={:?}",
                    outline.font_cache_key(),
                    outline.glyph_id()
                ),
                Glyph::Type3(_) => "type3".to_owned(),
            };
            let unicode = glyph.as_unicode();
            if self.glyphs.len() < 80 {
                self.glyphs.push(format!("{identity} unicode={unicode:?}"));
            }
            if self.actual_text_stack.contains(&true) {
                continue;
            }
            match unicode {
                Some(BfString::Char(character)) => self.text.push(character),
                Some(BfString::String(text)) => self.text.push_str(&text),
                None => self.missing.push(identity),
            }
        }
    }
    fn draw_image(&mut self, _: Image<'_, '_>, _: ImageDrawProps<'_>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}
    fn begin_marked_content(&mut self, _: &[u8], _: Option<i32>) {
        let text = self
            .marked_text
            .pop_front()
            .expect("fixture marked-content sequence");
        if !self.actual_text_stack.contains(&true)
            && let Some(text) = &text
        {
            self.text.push_str(text);
        }
        self.actual_text_stack.push(text.is_some());
    }
    fn end_marked_content(&mut self) {
        self.actual_text_stack
            .pop()
            .expect("balanced marked content");
    }
}

fn marked_text(page: &Page<'_>) -> std::collections::VecDeque<Option<String>> {
    let mut events = std::collections::VecDeque::new();
    let mut operations = page.typed_operations();
    while let Some(operation) = operations.next() {
        match operation {
            TypedInstruction::BeginMarkedContent(_) => events.push_back(None),
            TypedInstruction::BeginMarkedContentWithProperties(marked) => {
                let properties = match marked.1 {
                    Object::Dict(dict) => Some(dict.clone()),
                    Object::Name(name) => page.resources().properties.get::<Dict<'_>>(name),
                    _ => None,
                };
                events.push_back(properties.and_then(|properties| {
                    properties
                        .get::<hayro::hayro_syntax::object::String<'_>>(b"ActualText")
                        .map(|text| pdf_text_string(text.as_bytes()))
                }));
            }
            _ => {}
        }
    }
    events
}

fn page_text(pdf: &Pdf) -> Vec<TextDevice> {
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
            let mut device = TextDevice {
                marked_text: marked_text(page),
                ..Default::default()
            };
            interpret_page(page, &mut context, &mut device);
            assert!(
                device.marked_text.is_empty() && device.actual_text_stack.is_empty(),
                "all fixture marked content was interpreted"
            );
            device.text = device
                .text
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect();
            device
        })
        .collect()
}

fn font_diagnostics(pdf: &Pdf) -> String {
    let mut output = String::new();
    let mut visited = std::collections::HashSet::new();
    for page in pdf.pages().iter() {
        for name in page.resources().fonts.keys() {
            let font = page.resources().get_font(&name).expect("font resource");
            let key = font.cache_key();
            if !visited.insert(key) {
                continue;
            }
            output.push_str(&format!(
                "font={key:x} resource={name:?} base={:?} subtype={:?} encoding={:?}\n",
                font.get::<Name<'_>>(b"BaseFont"),
                font.get::<Name<'_>>(b"Subtype"),
                font.get::<Name<'_>>(b"Encoding")
            ));
            for field in [b"ToUnicode".as_slice(), b"Encoding"] {
                if let Some(stream) = font.get::<Stream<'_>>(field) {
                    let decoded = stream.decoded().expect("decoded font map");
                    output.push_str(&format!(
                        "{}:\n{}\n",
                        String::from_utf8_lossy(field),
                        String::from_utf8_lossy(&decoded)
                    ));
                }
            }
        }
        output.push_str(&format!(
            "page content:\n{}\n",
            String::from_utf8_lossy(page.page_stream().unwrap_or_default())
        ));
    }
    output
}

fn pdf_text_string(bytes: &[u8]) -> String {
    if let Some(bytes) = bytes.strip_prefix(&[0xfe, 0xff]) {
        assert_eq!(bytes.len() % 2, 0);
        let units = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|unit| u16::from_be_bytes([unit[0], unit[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).expect("valid UTF-16 PDF text")
    } else {
        String::from_utf8(bytes.to_vec()).expect("ASCII fixture PDF text")
    }
}

#[test]
// Spec: docs/acceptance/m0.md#技术原型
fn actual_text_overrides_glyph_mappings_for_nested_and_named_property_spans() {
    let content = "BT /F1 12 Tf 20 40 Td /Outer BMC /Span << /ActualText <FEFF03A92126> >> BDC (OO) Tj /Span << /ActualText (ignored) >> BDC (X) Tj EMC EMC /Span /Replacement BDC (Y) Tj EMC EMC ET";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << /Font << /F1 4 0 R >> /Properties << /Replacement << /ActualText (done) >> >> >> /Contents 5 0 R >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
    ];
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    let pdf = Pdf::new(bytes).expect("valid ActualText fixture PDF");
    let pages = page_text(&pdf);
    assert_eq!(pages[0].text, "ΩΩdone");
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
    let compiled = moino_export::compile_prototype(&source).unwrap_or_else(|error| {
        let probes = "中文测试。日本語の検証。한국어 검증."
            .chars()
            .filter(|character| !character.is_whitespace())
            .map(|character| {
                let result = moino_export::compile_prototype(&character.to_string());
                (character, result.err().map(|error| error.to_string()))
            })
            .collect::<Vec<_>>();
        panic!("pagination fixture: {error}; individual character diagnostics: {probes:?}");
    });
    assert!(compiled.warnings().is_empty(), "{:?}", compiled.warnings());
    let pdf = Pdf::new(compiled.bytes().to_vec()).expect("parsed emitted PDF");
    assert!(pdf.pages().len() >= 3);
    let decoded = page_text(&pdf);
    let pages = decoded
        .iter()
        .map(|page| page.text.as_str())
        .collect::<Vec<_>>();
    let text = pages.join("");
    let diagnostics = font_diagnostics(&pdf);
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/m0");
    std::fs::create_dir_all(&directory).expect("M0 evidence directory");
    std::fs::write(
        directory.join("cjk-search-regression.pdf"),
        compiled.bytes(),
    )
    .expect("PDF evidence");
    let glyphs = decoded
        .iter()
        .enumerate()
        .map(|(index, page)| {
            format!(
                "page={index} missing={:?}\nglyphs={}\n",
                page.missing,
                page.glyphs.join("\n")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(
        directory.join("cjk-search-fonts.txt"),
        format!("{diagnostics}\n{glyphs}"),
    )
    .expect("font evidence");
    assert!(
        decoded.iter().all(|page| page.missing.is_empty()),
        "missing glyph Unicode mappings; actual PDF, font maps, content and glyphs retained in target/m0/cjk-search-*"
    );
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
        titles.push(pdf_text_string(title.as_bytes()));
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
