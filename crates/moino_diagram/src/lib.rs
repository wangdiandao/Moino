//! L0 Mermaid SVG rendering seam; no internal dependencies yet.
#![forbid(unsafe_code)]

/// Diagram rendering failure, preserving the caller's source.
#[derive(Debug, thiserror::Error)]
#[error("diagram.render: {0}")]
pub struct DiagramError(String);

/// Renders Mermaid using the pure Rust, non-ELK engine configuration.
/// Callers must run this computation on a background executor.
pub fn render_svg(source: &str) -> Result<String, DiagramError> {
    let result = merman::Renderer::new()
        .render(merman::RenderRequest::svg(
            source,
            merman::OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .map_err(|error| DiagramError(error.to_string()))?;
    match result {
        merman::RenderOutput::Svg(Some(svg)) => Ok(svg.svg().to_owned()),
        _ => Err(DiagramError("no diagram in source".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renders_flowchart_and_sequence_with_real_engine() {
        for source in [
            "flowchart TD\nA[Start] --> B[Done]",
            "sequenceDiagram\nAlice->>Bob: Hello",
        ] {
            let svg = render_svg(source).expect("supported diagram");
            assert!(svg.contains("<svg"));
            assert!(svg.contains("Start") || svg.contains("Alice"));
        }
    }
    #[test]
    fn malformed_diagram_is_not_success() {
        assert!(render_svg("not a diagram").is_err());
    }
}
