//! Merge semantic text blocks without exposing or consuming their source markers.
use super::{Outcome, replacement};
use moino_document::{Origin, Selection, Snapshot};
use moino_markdown::{Node, Syntax};
use moino_text::ScalarOffset;

fn is_text_block(node: &Node) -> bool {
    matches!(node.syntax(), Syntax::Paragraph | Syntax::Heading(_))
}

fn pair(nodes: &[Node], position: ScalarOffset, forward: bool) -> Option<(&Node, &Node)> {
    let mut previous = None;
    for node in nodes {
        if matches!(
            node.syntax(),
            Syntax::Quote | Syntax::Callout(_) | Syntax::Item | Syntax::List(_)
        ) && let Some(pair) = pair(node.children(), position, forward)
        {
            return Some(pair);
        }
        if node.syntax() == &Syntax::Trivia {
            continue;
        }
        if let Some(left) = previous
            && is_text_block(left)
            && is_text_block(node)
            && position
                == if forward {
                    left.content().end
                } else {
                    node.content().start
                }
        {
            return Some((left, node));
        }
        previous = Some(node);
    }
    None
}

pub(super) fn at_boundary(
    snapshot: &Snapshot,
    selection: Selection,
    chars: &[char],
    forward: bool,
) -> Option<Outcome> {
    let (left, right) = pair(snapshot.cst().nodes(), selection.head().offset, forward)?;
    let start = left.content().end;
    let suffix: String = chars[start.value()..left.raw().end.value()]
        .iter()
        .collect();
    let mut body = String::new();
    if matches!(left.syntax(), Syntax::Heading(_)) {
        for child in right.children() {
            for ch in &chars[child.raw().start.value()..child.raw().end.value()] {
                body.push(if *ch == '\n' { ' ' } else { *ch });
            }
        }
    } else {
        body.extend(&chars[right.content().start.value()..right.content().end.value()]);
    }
    let trailing = if chars.get(right.raw().end.value().checked_sub(1)?) == Some(&'\n') {
        "\n"
    } else {
        ""
    };
    let value = format!("{body}{}{trailing}", suffix.trim_end_matches('\n'));
    Some(replacement(
        snapshot,
        start..right.raw().end,
        &value,
        start.value(),
        Origin::Command,
    ))
}
