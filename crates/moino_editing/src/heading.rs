//! Source-preserving heading commands across ATX, Setext and outer containers.
use super::{Outcome, replacement};
use moino_document::{Origin, Selection, Snapshot};
use moino_markdown::{Node, Syntax};
use moino_text::{ByteOffset, ScalarOffset};

fn at<'a>(nodes: &'a [Node], position: ScalarOffset, chars: &[char]) -> Option<&'a Node> {
    for node in nodes {
        let raw = node.raw();
        let at_end = position == raw.end
            && raw.end.value() > 0
            && chars.get(raw.end.value() - 1) != Some(&'\n');
        if raw.start <= position && (position < raw.end || at_end) {
            if let Some(heading) = at(node.children(), position, chars) {
                return Some(heading);
            }
            if matches!(node.syntax(), Syntax::Heading(_)) {
                return Some(node);
            }
        }
    }
    None
}
pub(super) fn continuation(prefix: &str) -> String {
    let mut continuation = String::new();
    let mut cursor = ByteOffset::new(0);
    while cursor.value() < prefix.len() {
        let rest = &prefix[cursor.value()..];
        let bytes = rest.as_bytes();
        if bytes.first() == Some(&b'[')
            && bytes
                .get(1)
                .is_some_and(|character| b" xX".contains(character))
            && bytes.get(2) == Some(&b']')
            && bytes
                .get(3)
                .is_some_and(|character| b" \t".contains(character))
        {
            cursor = ByteOffset::new(cursor.value() + 4);
            continue;
        }
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let list = if digits > 0
            && bytes
                .get(digits)
                .is_some_and(|character| b".)".contains(character))
            && bytes
                .get(digits + 1)
                .is_some_and(|character| b" \t".contains(character))
        {
            digits + 1
        } else if bytes
            .first()
            .is_some_and(|character| b"-*+".contains(character))
            && bytes
                .get(1)
                .is_some_and(|character| b" \t".contains(character))
        {
            1
        } else {
            0
        };
        if list > 0 {
            continuation.push_str(&" ".repeat(list));
            cursor = ByteOffset::new(cursor.value() + list);
        } else {
            let character = rest.chars().next().expect("prefix character remains");
            continuation.push(character);
            cursor = ByteOffset::new(cursor.value() + character.len_utf8());
        }
    }
    continuation
}
pub(super) fn enter(snapshot: &Snapshot, selection: Selection, chars: &[char]) -> Option<Outcome> {
    let range = selection.range();
    let heading = at(snapshot.cst().nodes(), range.start, chars)?;
    let content = heading.content();
    if !selection.is_collapsed() && (range.start < content.start || range.end > content.end) {
        return None;
    }
    let line_start = ScalarOffset::new(
        chars[..heading.raw().start.value()]
            .iter()
            .rposition(|character| *character == '\n')
            .map_or(0, |index| index + 1),
    );
    let prefix: String = chars[line_start.value()..heading.raw().start.value()]
        .iter()
        .collect();
    let separator = if prefix.is_empty() {
        "\n\n".to_owned()
    } else {
        format!("\n{}", continuation(&prefix))
    };
    if selection.is_collapsed() && range.start <= content.start && !content.is_empty() {
        return Some(replacement(
            snapshot,
            heading.raw().start..heading.raw().start,
            &separator,
            range.start.value() + separator.chars().count(),
            Origin::Command,
        ));
    }
    let start = range.start.min(content.end);
    let end = range.end.min(content.end);
    let tail: String = chars[end.value()..content.end.value()].iter().collect();
    let closing: String = chars[content.end.value()..heading.raw().end.value()]
        .iter()
        .collect();
    let marker = closing.trim_end_matches('\n');
    let trailing: String = closing[marker.len()..].to_owned();
    let opening: String = chars[heading.raw().start.value()..content.start.value()]
        .iter()
        .collect();
    let next = if tail.is_empty() {
        String::new()
    } else {
        format!("{opening}{tail}{marker}")
    };
    let value = format!("{marker}{separator}{next}{trailing}");
    Some(replacement(
        snapshot,
        start..heading.raw().end,
        &value,
        start.value()
            + marker.chars().count()
            + separator.chars().count()
            + if tail.is_empty() {
                0
            } else {
                opening.chars().count()
            },
        Origin::Command,
    ))
}
