//! L0 RaTeX vector rendering seam; no internal dependencies yet.
#![forbid(unsafe_code)]

/// A formula that could not be rendered without losing its source.
#[derive(Debug, thiserror::Error)]
#[error("math.render: {0}")]
pub struct MathError(String);

/// Parses and lays out TeX into self-contained SVG paths for native rendering.
/// Callers must execute this synchronous computation on the background executor.
pub fn render_svg(source: &str) -> Result<String, MathError> {
    let nodes = ratex_parser::parse(source).map_err(|error| MathError(error.to_string()))?;
    let layout = ratex_layout::layout(&nodes, &ratex_layout::LayoutOptions::default());
    ratex_layout::validate_layout(&layout).map_err(|error| MathError(error.to_string()))?;
    let display = ratex_layout::to_display_list(&layout);
    Ok(ratex_svg::render_to_svg_with_color_syntax(
        &display,
        &ratex_svg::SvgOptions {
            embed_glyphs: true,
            ..Default::default()
        },
        ratex_svg::SvgColorSyntax::Rgb,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn formula_produces_paths_without_browser_fonts() {
        let svg = render_svg(r"\frac{-b \pm \sqrt{b^2-4ac}}{2a}").expect("supported formula");
        assert!(svg.contains("<path"));
        assert!(!svg.contains("<text"));
        assert!(!svg.contains("<script"));
    }
    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn invalid_formula_returns_diagnostic() {
        assert!(render_svg(r"\frac{").is_err());
    }
}
