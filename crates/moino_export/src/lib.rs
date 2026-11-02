//! L0 isolated Typst prototype compiler; no internal dependencies yet.
#![forbid(unsafe_code)]

mod world;

/// A compiler diagnostic from the isolated prototype.
#[derive(Debug, thiserror::Error)]
#[error("export.compile: {0}")]
pub struct ExportError(String);

/// Compiled prototype PDF bytes and diagnostics.
pub struct CompiledPdf {
    bytes: Vec<u8>,
    pages: usize,
    warnings: Vec<String>,
}

impl CompiledPdf {
    /// Complete PDF bytes; callers own atomic file output.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Number of pages after pagination.
    pub fn pages(&self) -> usize {
        self.pages
    }
    /// Nonfatal compiler diagnostics.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

/// Compiles trusted validation markup with no file, package or clock access.
/// This is an M0 engine probe, not the product's semantic IR export API.
pub fn compile_prototype(source: &str) -> Result<CompiledPdf, ExportError> {
    compile_world(world::PrototypeWorld::new(source))
}

fn compile_world(world: world::PrototypeWorld) -> Result<CompiledPdf, ExportError> {
    let result = typst::compile::<typst_layout::PagedDocument>(&world);
    let warnings = result
        .warnings
        .iter()
        .map(|diagnostic| diagnostic.message.to_string())
        .collect();
    let document = result.output.map_err(|errors| {
        ExportError(
            errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        )
    })?;
    if let Some((info, character)) = document
        .pages()
        .iter()
        .find_map(|page| unavailable_font(&page.frame))
    {
        use typst::World;
        let candidate = world
            .book()
            .select_fallback(Some(info), info.variant, &character.to_string())
            .map(|index| {
                let candidate = world.book().info(index).expect("selected font metadata");
                let loadable = world.font(index).is_some();
                format!("{} (loadable {loadable})", candidate.family)
            });
        return Err(ExportError(format!(
            "font unavailable: U+{:04X} in {}; fallback {candidate:?}",
            u32::from(character),
            info.family,
        )));
    }
    let bytes = typst_pdf::pdf(
        &document,
        &typst_pdf::PdfOptions {
            creator: typst::foundations::Smart::Custom(Some("Moino".into())),
            ..Default::default()
        },
    )
    .map_err(|errors| ExportError(format!("{errors:?}")))?;
    Ok(CompiledPdf {
        bytes,
        pages: document.pages().len(),
        warnings,
    })
}

fn unavailable_font(frame: &typst::layout::Frame) -> Option<(&typst::text::FontInfo, char)> {
    use typst::{layout::FrameItem, text::is_default_ignorable};
    frame.items().find_map(|(_, item)| match item {
        FrameItem::Group(group) => unavailable_font(&group.frame),
        FrameItem::Text(text) => text.glyphs.iter().find_map(|glyph| {
            (glyph.id == 0 || text.font.info().is_last_resort())
                .then(|| {
                    text.text[glyph.range()].chars().find(|character| {
                        !character.is_whitespace() && !is_default_ignorable(*character)
                    })
                })
                .flatten()
                .map(|character| (text.font.info(), character))
        }),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn compiles_multipage_pdf_and_rejects_external_files() {
        let result =
            compile_prototype("= Moino\nHello\n#pagebreak()\nSecond page").expect("valid markup");
        assert!(result.bytes().starts_with(b"%PDF-"));
        assert_eq!(result.pages(), 2);
        assert!(compile_prototype("#read(\"secret.txt\")").is_err());
        assert!(compile_prototype("#import \"@preview/anything:1.0.0\": *").is_err());
    }

    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn missing_visible_glyph_is_a_diagnostic_without_relying_on_system_fonts() {
        let result = compile_world(world::PrototypeWorld::without_system_fonts("한국어"));
        assert!(matches!(result, Err(error) if error.to_string().contains("font unavailable")));
        assert!(compile_world(world::PrototypeWorld::without_system_fonts("Latin text")).is_ok());
    }
}
