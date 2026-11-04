//! Container-aware continuation independent of the current physical line's spelling.
use super::{Outcome, next_marker, offset, prefix_len, replacement};
use moino_document::{Edit, Origin, Selection, Snapshot};
use moino_markdown::{Node, Syntax};

/// Source spans of one quote layer on physical lines intersecting the requested block.
pub(super) fn quote_markers(
    node: &Node,
    chars: &[char],
    scope: std::ops::Range<moino_text::ScalarOffset>,
) -> Vec<std::ops::Range<moino_text::ScalarOffset>> {
    let first = chars[..node.raw().start.value()]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |at| at + 1);
    let layer = chars[first..node.raw().start.value()]
        .iter()
        .filter(|ch| **ch == '>')
        .count();
    let mut cursor = first;
    let mut ranges = Vec::new();
    while cursor < node.raw().end.value() {
        let end = chars[cursor..]
            .iter()
            .position(|ch| *ch == '\n')
            .map_or(chars.len(), |at| cursor + at);
        if cursor < scope.end.value() && scope.start.value() <= end {
            let line: String = chars[cursor..end].iter().collect();
            let width = prefix_len(&line);
            let marker = if cursor == first {
                Some(node.raw().start.value())
            } else {
                chars[cursor..cursor + width]
                    .iter()
                    .enumerate()
                    .filter(|(_, ch)| **ch == '>')
                    .nth(layer)
                    .map(|(at, _)| cursor + at)
            };
            if let Some(marker) = marker {
                let space = usize::from(matches!(chars.get(marker + 1), Some(' ' | '\t')));
                ranges.push(offset(marker)..offset(marker + 1 + space));
            }
        }
        cursor = end + 1;
    }
    ranges
}

fn containing<'a>(nodes: &'a [Node], position: usize, chars: &[char]) -> Option<&'a Node> {
    for node in nodes {
        let raw = node.raw();
        if raw.start.value() <= position
            && (position < raw.end.value()
                || position == chars.len()
                    && raw.end.value() == position
                    && chars.last() != Some(&'\n'))
        {
            if let Some(container) = containing(node.children(), position, chars) {
                return Some(container);
            }
            if matches!(
                node.syntax(),
                Syntax::Item | Syntax::Quote | Syntax::Callout(_)
            ) {
                return Some(node);
            }
        }
    }
    None
}

pub(super) fn enter(snapshot: &Snapshot, selection: Selection, chars: &[char]) -> Option<Outcome> {
    let range = selection.range();
    let start = range.start.value();
    let container = containing(snapshot.cst().nodes(), start, chars)?;
    if range.end > container.raw().end {
        return None;
    }
    let header_start = chars[..container.raw().start.value()]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |index| index + 1);
    let outer: String = chars[header_start..container.raw().start.value()]
        .iter()
        .collect();
    let header: String = chars[container.raw().start.value()..]
        .iter()
        .take_while(|ch| **ch != '\n')
        .collect();
    let is_item = container.syntax() == &Syntax::Item;
    let prefix = if is_item {
        prefix_len(&header)
    } else {
        1 + usize::from(
            header
                .as_bytes()
                .get(1)
                .is_some_and(|ch| b" \t".contains(ch)),
        )
    };
    let marker = format!(
        "{}{}",
        super::heading::continuation(&outer),
        header.chars().take(prefix).collect::<String>()
    );
    let line_start = chars[..start]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |index| index + 1);
    let line_end = chars[start..]
        .iter()
        .position(|ch| *ch == '\n')
        .map_or(chars.len(), |index| start + index);
    let line: String = chars[line_start..line_end].iter().collect();
    let local_prefix = prefix_len(&line);
    let empty_item = container.text().trim().is_empty();
    let empty_line = empty_item || line.chars().skip(local_prefix).all(char::is_whitespace);
    if selection.is_collapsed() && empty_line && (!is_item || empty_item) {
        let outer = if line_start == header_start {
            outer
        } else {
            super::heading::continuation(&outer)
        };
        return Some(replacement(
            snapshot,
            offset(line_start)..offset(line_end),
            &outer,
            line_start + outer.chars().count(),
            Origin::Command,
        ));
    }
    let checked_task = container
        .children()
        .iter()
        .find(|node| node.syntax() == &Syntax::Task(true));
    let task_start = checked_task.is_some() && start == container.raw().start.value() + prefix;
    let continuation = if checked_task.is_some() && start < line_end {
        marker.clone()
    } else {
        next_marker(&marker)
    };
    let value = format!("\n{continuation}");
    let mut outcome = replacement(
        snapshot,
        range,
        &value,
        start + value.chars().count(),
        Origin::Command,
    );
    if is_item {
        if task_start
            && let Some(task) = checked_task
            && let Some(transaction) = outcome.transaction.take()
        {
            outcome.transaction = Some(transaction.with_edit(Edit::new(task.raw(), "[ ]")));
        }
        let digits = header.bytes().take_while(u8::is_ascii_digit).count();
        if let Ok(number) = header[..digits].parse::<u64>() {
            let next_width = (number + 1).to_string().len();
            if let Some(mut transaction) = outcome.transaction.take() {
                for edit in continuation_edits(
                    chars,
                    container,
                    selection.range().end.value(),
                    digits,
                    next_width,
                ) {
                    transaction = transaction.with_edit(edit);
                }
                outcome.transaction = Some(transaction);
            }
        }
        renumber_following(&mut outcome, snapshot, chars, container, &header);
    }
    Some(outcome)
}

fn following_items<'a>(nodes: &'a [Node], item: &Node) -> Option<&'a [Node]> {
    for node in nodes {
        if matches!(node.syntax(), Syntax::List(Some(_)))
            && let Some(index) = node
                .children()
                .iter()
                .position(|child| child.id() == item.id())
        {
            return Some(&node.children()[index + 1..]);
        }
        if let Some(items) = following_items(node.children(), item) {
            return Some(items);
        }
    }
    None
}

fn renumber_following(
    outcome: &mut Outcome,
    snapshot: &Snapshot,
    chars: &[char],
    item: &Node,
    header: &str,
) {
    let digits = header.bytes().take_while(u8::is_ascii_digit).count();
    let Some(mut ordinal) = header
        .get(..digits)
        .and_then(|text| text.parse::<u64>().ok())
        .map(|ordinal| ordinal + 2)
    else {
        return;
    };
    let Some(items) = following_items(snapshot.cst().nodes(), item) else {
        return;
    };
    let Some(mut transaction) = outcome.transaction.take() else {
        return;
    };
    for item in items.iter().filter(|node| node.syntax() == &Syntax::Item) {
        let start = item.raw().start.value();
        let digits = chars[start..]
            .iter()
            .take_while(|ch| ch.is_ascii_digit())
            .count();
        let number = ordinal.to_string();
        transaction =
            transaction.with_edit(Edit::new(offset(start)..offset(start + digits), &number));
        for edit in continuation_edits(chars, item, start, digits, number.len()) {
            transaction = transaction.with_edit(edit);
        }
        ordinal += 1;
    }
    outcome.transaction = Some(transaction);
}

fn continuation_edits(
    chars: &[char],
    item: &Node,
    after: usize,
    old_width: usize,
    new_width: usize,
) -> Vec<Edit> {
    if old_width == new_width {
        return Vec::new();
    }
    let start = item.raw().start.value();
    let line_start = chars[..start]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |at| at + 1);
    let outer_quotes = chars[line_start..start]
        .iter()
        .filter(|ch| **ch == '>')
        .count();
    let mut edits = Vec::new();
    for cursor in start..item.raw().end.value() {
        if chars[cursor] != '\n' || cursor < after || cursor + 1 >= item.raw().end.value() {
            continue;
        }
        let mut indent = cursor + 1;
        for _ in 0..outer_quotes {
            while chars
                .get(indent)
                .is_some_and(|ch| *ch == ' ' || *ch == '\t')
            {
                indent += 1;
            }
            if chars.get(indent) != Some(&'>') {
                break;
            }
            indent += 1;
            if chars
                .get(indent)
                .is_some_and(|ch| *ch == ' ' || *ch == '\t')
            {
                indent += 1;
            }
        }
        let mut end = indent;
        while chars.get(end).is_some_and(|ch| *ch == ' ' || *ch == '\t') {
            end += 1;
        }
        if end >= item.raw().end.value() || chars.get(end) == Some(&'\n') {
            continue;
        }
        let columns = |range: std::ops::Range<usize>| {
            chars[range].iter().fold(0, |column, ch| {
                if *ch == '\t' {
                    column + 4 - column % 4
                } else {
                    column + 1
                }
            })
        };
        let current = columns(cursor + 1..end) - columns(cursor + 1..indent);
        let width = if new_width >= old_width {
            current + new_width - old_width
        } else {
            current.saturating_sub(old_width - new_width)
        };
        edits.push(Edit::new(offset(indent)..offset(end), &" ".repeat(width)));
    }
    edits
}
