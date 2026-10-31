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
    /// Warnings, including missing glyphs and unresolved fonts.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

/// Compiles trusted validation markup with no file, package or clock access.
/// This is an M0 engine probe, not the product's semantic IR export API.
pub fn compile_prototype(source: &str) -> Result<CompiledPdf, ExportError> {
    let world = world::PrototypeWorld::new(source);
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn compiles_searchable_multipage_pdf_and_rejects_external_files() {
        let result =
            compile_prototype("= Moino\nHello\n#pagebreak()\nSecond page").expect("valid markup");
        assert!(result.bytes().starts_with(b"%PDF-"));
        assert_eq!(result.pages(), 2);
        assert!(compile_prototype("#read(\"secret.txt\")").is_err());
        assert!(compile_prototype("#import \"@preview/anything:1.0.0\": *").is_err());
    }
}
