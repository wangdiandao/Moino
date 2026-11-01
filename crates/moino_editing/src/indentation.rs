use super::{Outcome, edits_outcome, navigation, offset, prefix_len, quote_prefix_width};
use moino_document::{Edit, Selection, Snapshot};
use moino_markdown::{Node, Syntax};

struct ListContext<'a> {
    item: &'a Node,
    previous: Option<&'a Node>,
    parent: Option<&'a Node>,
}
fn context<'a>(
    nodes: &'a [Node],
    position: usize,
    document_end: usize,
    parent: Option<&'a Node>,
) -> Option<ListContext<'a>> {
    for node in nodes {
        if node.raw().start.value() > position
            || (position >= node.raw().end.value()
                && !(position == document_end && node.raw().end.value() == document_end))
        {
            continue;
        }
        if matches!(node.syntax(), Syntax::List(_)) {
            let mut previous = None;
            for item in node
                .children()
                .iter()
                .filter(|item| item.syntax() == &Syntax::Item)
            {
                if item.raw().start.value() <= position
                    && (position < item.raw().end.value()
                        || (position == document_end && item.raw().end.value() == document_end))
                {
                    return context(item.children(), position, document_end, Some(item)).or(Some(
                        ListContext {
                            item,
                            previous,
                            parent,
                        },
                    ));
                }
                previous = Some(item);
            }
        } else {
            let parent = if matches!(node.syntax(), Syntax::Quote | Syntax::Callout(_)) {
                None
            } else {
                parent
            };
            if let Some(context) = context(node.children(), position, document_end, parent) {
                return Some(context);
            }
        }
    }
    None
}
fn line_start(chars: &[char], position: usize) -> usize {
    chars[..position]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |index| index + 1)
}
fn header(chars: &[char], item: &Node) -> String {
    chars[line_start(chars, item.raw().start.value())..]
        .iter()
        .take_while(|ch| **ch != '\n')
        .collect()
}
fn columns(text: &str) -> usize {
    text.chars().fold(0, |column, ch| {
        if ch == '\t' {
            column + 4 - column % 4
        } else {
            column + 1
        }
    })
}
fn leading(line: &str) -> (usize, usize) {
    let quote = quote_prefix_width(line);
    let count = line[quote..]
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .count();
    let end = quote + count;
    (count, columns(&line[..end]) - columns(&line[..quote]))
}
fn marker_width(line: &str) -> usize {
    let quote = quote_prefix_width(line);
    let (indent, _) = leading(line);
    let marker = quote + indent;
    let digits = line[marker..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    let width = if digits > 0 { digits + 1 } else { 1 };
    let marker_end = (marker + width).min(line.len());
    let whitespace = line[marker_end..]
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .count();
    let gap = columns(&line[..marker_end + whitespace]) - columns(&line[..marker_end]);
    width + if (1..=4).contains(&gap) { gap } else { 1 }
}
pub(super) fn list_tab(
    snapshot: &Snapshot,
    selection: Selection,
    chars: &[char],
    reverse: bool,
) -> Option<Outcome> {
    let position = selection.head().offset.value();
    let context = context(snapshot.cst().nodes(), position, chars.len(), None)?;
    if !reverse && context.previous.is_none() {
        return Some(navigation(selection));
    }
    let current = header(chars, context.item);
    let base = leading(&current).1;
    let unlist = reverse && context.parent.is_none();
    let target = if reverse {
        context
            .parent
            .map_or(0, |parent| leading(&header(chars, parent)).1)
    } else {
        let previous = header(chars, context.previous.expect("previous sibling checked"));
        leading(&previous).1 + marker_width(&previous)
    };
    let start = line_start(chars, context.item.raw().start.value());
    let mut end = context.item.raw().end.value();
    if end > start && chars[end - 1] == '\n' {
        end -= 1;
    }
    let quote = quote_prefix_width(&current);
    let marker_start = quote + leading(&current).0;
    let ordered_digits = current[marker_start..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    let restart_ordered = !reverse && ordered_digits > 0;
    let selected: String = chars[start..end].iter().collect();
    let mut cursor = start;
    let mut edits = Vec::new();
    for (index, line) in selected.split('\n').enumerate() {
        let quote = quote_prefix_width(line);
        let (count, old_columns) = leading(line);
        let (remove, value) = if unlist && index == 0 {
            (prefix_len(line).saturating_sub(quote), String::new())
        } else {
            let mut new_columns = if unlist {
                old_columns.saturating_sub(base + marker_width(&current))
            } else if target >= base {
                old_columns + target - base
            } else {
                old_columns.saturating_sub(base - target)
            };
            if restart_ordered && index > 0 {
                new_columns = new_columns.saturating_sub(ordered_digits - 1);
            }
            if restart_ordered && index == 0 {
                (
                    count + ordered_digits,
                    format!("{}1", " ".repeat(new_columns)),
                )
            } else {
                (count, " ".repeat(new_columns))
            }
        };
        if line[quote..quote + remove] != value {
            edits.push(Edit::new(
                offset(cursor + quote)..offset(cursor + quote + remove),
                &value,
            ));
        }
        cursor += line.chars().count() + 1;
    }
    Some(edits_outcome(snapshot, selection, edits))
}
