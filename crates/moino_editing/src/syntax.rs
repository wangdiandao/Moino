//! Shared CST context queries for editing commands.
use super::{Node, ScalarOffset, Syntax};

pub(super) fn text_intersects(nodes: &[Node], start: usize, end: usize) -> bool {
    nodes.iter().any(|node| {
        node.raw().start.value() < end
            && start < node.raw().end.value()
            && (node.syntax() == &Syntax::Text || text_intersects(node.children(), start, end))
    })
}

pub(super) fn text_block<'a>(
    nodes: &'a [Node],
    position: usize,
    chars: &[char],
) -> Option<&'a Node> {
    for node in nodes {
        if node.raw().start.value() <= position
            && (position < node.raw().end.value()
                || (position == chars.len()
                    && position == node.raw().end.value()
                    && chars.last() != Some(&'\n')))
        {
            if let Some(block) = text_block(node.children(), position, chars) {
                return Some(block);
            }
            if matches!(
                node.syntax(),
                Syntax::Paragraph | Syntax::Heading(_) | Syntax::Item
            ) {
                return Some(node);
            }
        }
    }
    None
}

pub(super) fn table_cells<'a>(nodes: &'a [Node], result: &mut Vec<&'a Node>) {
    for node in nodes {
        if node.syntax() == &Syntax::TableCell {
            result.push(node);
        } else {
            table_cells(node.children(), result);
        }
    }
}

pub(super) fn marker_boundary(nodes: &[Node], position: usize, forward: bool) -> Option<&Node> {
    for node in nodes {
        if node.raw().start.value() > position || node.raw().end.value() < position {
            continue;
        }
        if let Some(child) = marker_boundary(node.children(), position, forward) {
            return Some(child);
        }
        if matches!(
            node.syntax(),
            Syntax::Emphasis
                | Syntax::Strong
                | Syntax::Strike
                | Syntax::Highlight
                | Syntax::Subscript
                | Syntax::Superscript
                | Syntax::Code
                | Syntax::Math
                | Syntax::Link(_)
                | Syntax::Image(_)
        ) {
            let opening = node.opening_marker();
            let closing = node.closing_marker();
            if (!forward && opening.start.value() < position && position <= opening.end.value())
                || (forward && closing.start.value() <= position && position < closing.end.value())
                || (matches!(node.syntax(), Syntax::Link(_) | Syntax::Image(_))
                    && closing.start.value() < position
                    && position < closing.end.value())
            {
                return Some(node);
            }
        }
    }
    None
}

pub(super) fn in_literal(nodes: &[Node], position: usize, document_end: usize) -> bool {
    nodes.iter().any(|node| {
        let literal = matches!(
            node.syntax(),
            Syntax::CodeBlock(_)
                | Syntax::MathBlock
                | Syntax::HtmlBlock
                | Syntax::FrontMatter
                | Syntax::Code
        );
        let open_end = position == document_end
            && position == node.raw().end.value()
            && literal
            && node.syntax() != &Syntax::Code
            && node.closing_marker().is_empty();
        node.raw().start.value() <= position
            && (position < node.raw().end.value()
                || position == document_end && position == node.raw().end.value())
            && ((literal && (position < node.raw().end.value() || open_end))
                || in_literal(node.children(), position, document_end))
    })
}

pub(super) fn is_inline_code_closer(nodes: &[Node], position: ScalarOffset) -> bool {
    nodes.iter().any(|node| {
        let closing = node.closing_marker();
        let raw = node.raw();
        raw.start <= position
            && position < raw.end
            && ((node.syntax() == &Syntax::Code
                && closing.start <= position
                && position < closing.end)
                || is_inline_code_closer(node.children(), position))
    })
}
