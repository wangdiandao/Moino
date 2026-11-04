//! Tab resolves table navigation, list topology and text indentation in that order.
use super::{
    CommandContext, Edit, EditingError, Origin, Outcome, Selection, Syntax, edits_outcome,
    in_literal, indentation, navigation, offset, quote_prefix_width, replacement, table_cells,
    text_intersects,
};

pub(super) fn run(context: &CommandContext, reverse: bool) -> Result<Outcome, EditingError> {
    if !context.is_pending() {
        if let Some(outcome) = table(context, reverse) {
            return Ok(outcome);
        }
        if context.selection.is_collapsed()
            && !in_literal(
                context.snapshot.cst().nodes(),
                context.selection.head().offset.value(),
                context.chars.len(),
            )
            && let Some(outcome) =
                indentation::list_tab(context.snapshot, context.selection, &context.chars, reverse)
        {
            return Ok(outcome);
        }
    }
    if context.selection.is_collapsed() && !reverse {
        return Ok(context.insert_plain(&context.options.indent, Origin::Command));
    }
    let selected = context.selection.range();
    let stop = if !selected.is_empty() && context.chars.get(selected.end.value() - 1) == Some(&'\n')
    {
        selected.end.value() - 1
    } else {
        selected.end.value()
    };
    let mut start = context.line.start;
    let mut edits = Vec::new();
    while start <= stop {
        let end = context.chars[start..]
            .iter()
            .position(|ch| *ch == '\n')
            .map_or(context.chars.len(), |relative| start + relative);
        let line: String = context.chars[start..end].iter().collect();
        let quote = quote_prefix_width(&line);
        let prefix = if !in_literal(context.snapshot.cst().nodes(), start, context.chars.len())
            && !text_intersects(context.snapshot.cst().nodes(), start, start + quote)
        {
            quote
        } else {
            0
        };
        let position = offset(start + prefix);
        if reverse {
            if line[prefix..].starts_with(&context.options.indent) {
                edits.push(Edit::new(
                    position..offset(position.value() + context.options.indent.chars().count()),
                    "",
                ));
            }
        } else {
            edits.push(Edit::new(position..position, &context.options.indent));
        }
        if end == context.chars.len() {
            break;
        }
        start = end + 1;
    }
    Ok(edits_outcome(context.snapshot, context.selection, edits))
}
fn table(context: &CommandContext, reverse: bool) -> Option<Outcome> {
    let position = context.selection.head().offset;
    let path = context.path(position);
    let table = path
        .iter()
        .find(|node| matches!(node.syntax(), Syntax::Table(_)))?;
    let mut cells = Vec::new();
    table_cells(table.children(), &mut cells);
    let current = cells
        .iter()
        .position(|cell| cell.raw().start <= position && position <= cell.raw().end)?;
    let next = if reverse {
        current.saturating_sub(1)
    } else {
        current + 1
    };
    if let Some(cell) = cells.get(next) {
        return Some(navigation(Selection::cursor(cell.content().start)));
    }
    let Syntax::Table(columns) = table.syntax() else {
        return None;
    };
    let end = table.raw().end;
    let newline = end.value() > 0 && context.chars[end.value() - 1] == '\n';
    let leading = if newline { "" } else { "\n" };
    let value = format!(
        "{leading}|{}{}",
        "  |".repeat(*columns),
        if newline { "\n" } else { "" }
    );
    Some(replacement(
        context.snapshot,
        end..end,
        &value,
        end.value() + leading.len() + 2,
        Origin::Command,
    ))
}
