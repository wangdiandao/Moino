//! L0 bounded PDF page rasterization via hayro; no internal dependencies yet.
#![forbid(unsafe_code)]

/// A PDF page that cannot be decoded within the prototype's resource bounds.
#[derive(Debug, thiserror::Error)]
#[error("preview.decode: {0}")]
pub struct PreviewError(String);

/// Decodes one page to PNG; caller must run this off the foreground executor.
pub fn render_pdf_page(bytes: Vec<u8>, page_index: usize) -> Result<Vec<u8>, PreviewError> {
    let pdf =
        hayro::hayro_syntax::Pdf::new(bytes).map_err(|error| PreviewError(format!("{error:?}")))?;
    let page = pdf
        .pages()
        .get(page_index)
        .ok_or_else(|| PreviewError("page out of range".into()))?;
    let (width, height) = page.render_dimensions();
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > 4096.0
        || height > 4096.0
    {
        return Err(PreviewError("page exceeds prototype raster budget".into()));
    }
    let pixmap = hayro::render(
        page,
        &hayro::RenderCache::new(),
        &Default::default(),
        &Default::default(),
        &hayro::PixmapSettings {
            bg_color: hayro::vello_cpu::color::palette::css::WHITE,
            ..Default::default()
        },
    );
    let rgba: Vec<u8> = pixmap
        .data()
        .iter()
        .flat_map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
        .collect();
    let mut output = std::io::Cursor::new(Vec::new());
    image::write_buffer_with_format(
        &mut output,
        &rgba,
        u32::from(pixmap.width()),
        u32::from(pixmap.height()),
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|error| PreviewError(error.to_string()))?;
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn malformed_pdf_returns_error() {
        assert!(render_pdf_page(b"invalid pdf".to_vec(), 0).is_err());
    }
    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn decodes_actual_typst_page_and_rejects_invalid_page_or_budget() {
        let pdf = moino_export::compile_prototype("#set page(width: 100pt, height: 100pt, margin: 10pt)\nHello\n#rect(width: 20pt, height: 20pt, fill: red)").expect("fixture PDF");
        let png = render_pdf_page(pdf.bytes().to_vec(), 0).expect("rasterized page");
        let image = image::load_from_memory(&png).expect("valid PNG").to_rgba8();
        assert_eq!(image.dimensions(), (100, 100));
        assert!(
            image
                .pixels()
                .any(|pixel| pixel.0[0] > 200 && pixel.0[1] < 100 && pixel.0[2] < 100)
        );
        assert!(render_pdf_page(pdf.bytes().to_vec(), 1).is_err());
        let large =
            moino_export::compile_prototype("#set page(width: 5000pt, height: 100pt)\nLarge")
                .expect("oversized fixture");
        assert!(render_pdf_page(large.bytes().to_vec(), 0).is_err());
    }
}
