//! Ordered deletion rules over semantic boundaries, followed by a grapheme fallback.
use super::{
    CommandContext, Edit, EditingError, Origin, Outcome, Syntax, edits_outcome, in_literal,
    indentation, marker_boundary, merge, navigation, offset, partner, remove_markers, replacement,
};

pub(super) fn backward(context: &CommandContext) -> Result<Outcome, EditingError> {
    delete(context, false)
}
pub(super) fn forward(context: &CommandContext) -> Result<Outcome, EditingError> {
    delete(context, true)
}

/// Replace a cross-block selection using the surviving receiver's block syntax.
pub(super) fn selected(context: &CommandContext) -> Result<Outcome, EditingError> {
    let range = context.selection.range();
    let nodes = context.snapshot.cst().nodes();
    let Some(first) = super::text_block(nodes, range.start.value(), &context.chars) else {
        return Ok(context.insert_plain("", Origin::Command));
    };
    let Some(last) = super::text_block(nodes, range.end.value(), &context.chars) else {
        return Ok(context.insert_plain("", Origin::Command));
    };
    if first.id() == last.id() || context.is_pending() {
        return Ok(context.insert_plain("", Origin::Command));
    }
    let mut tail = String::new();
    for child in last
        .children()
        .iter()
        .filter(|node| !matches!(node.syntax(), Syntax::Task(_)))
    {
        let start = child.raw().start.max(range.end).value();
        let end = child.raw().end.value();
        if start < end {
            tail.extend(&context.chars[start..end]);
        }
    }
    if matches!(first.syntax(), Syntax::Heading(_)) {
        tail = tail.replace('\n', " ");
        let mut end = first.raw().end.value();
        while end > first.content().end.value() && context.chars[end - 1] == '\n' {
            end -= 1;
        }
        tail.extend(&context.chars[first.content().end.value()..end]);
    } else {
        let start = first.raw().start.value();
        let line = context.chars[..start]
            .iter()
            .rposition(|ch| *ch == '\n')
            .map_or(0, |at| at + 1);
        let body_start = first
            .children()
            .iter()
            .find(|node| !matches!(node.syntax(), Syntax::Task(_)))
            .map_or(start, |node| node.raw().start.value());
        let outer: String = context.chars[line..body_start].iter().collect();
        let continuation = super::heading::continuation(&outer);
        tail = tail.replace('\n', &format!("\n{continuation}"));
    }
    let mut end = last.raw().end.value();
    while end > range.end.value() && context.chars[end - 1] == '\n' {
        end -= 1;
    }
    Ok(replacement(
        context.snapshot,
        range.start..offset(end),
        &tail,
        range.start.value(),
        Origin::Command,
    ))
}
fn delete(context: &CommandContext, forward: bool) -> Result<Outcome, EditingError> {
    let at = context.selection.head().offset;
    let source = &context.chars;
    let snapshot = context.snapshot;
    if (forward && at == snapshot.text().len())
        || (!forward && at.value() == 0 && context.is_pending())
    {
        return Ok(navigation(context.selection));
    }
    let literal = in_literal(snapshot.cst().nodes(), at.value(), source.len());
    if !forward
        && !context.is_pending()
        && let Some(node) = context.path(at).into_iter().rev().find(|node| {
            matches!(node.syntax(), Syntax::CodeBlock(_) | Syntax::FrontMatter)
                && node.content().start == at
        })
    {
        let end = node.raw().end.value();
        let mut closing = node.closing_marker();
        if closing.end.value() == end && end > 0 && source[end - 1] == '\n' {
            closing.end = offset(end - 1);
        }
        let edits = [node.opening_marker(), closing]
            .into_iter()
            .filter(|range| !range.is_empty())
            .map(|range| Edit::new(range, ""))
            .collect();
        return Ok(edits_outcome(snapshot, context.selection, edits));
    }
    if !context.is_pending() && !literal {
        if !forward && let Some(outcome) = demote(context) {
            return Ok(outcome);
        }
        if let Some(outcome) = merge::at_boundary(snapshot, context.selection, source, forward) {
            return Ok(outcome);
        }
    }
    if let Some(node) = marker_boundary(snapshot.cst().nodes(), at.value(), forward) {
        return Ok(remove_markers(snapshot, context.selection, node));
    }
    if !forward
        && at.value() > 0
        && context.options.auto_pair
        && !literal
        && partner(source[at.value() - 1])
            .is_some_and(|right| source.get(at.value()) == Some(&right))
        && marker_boundary(snapshot.cst().nodes(), at.value(), true).is_none()
    {
        return Ok(replacement(
            snapshot,
            offset(at.value() - 1)..offset(at.value() + 1),
            "",
            at.value() - 1,
            Origin::Command,
        ));
    }
    let boundary = if forward {
        snapshot.text().next_grapheme(at)
    } else {
        snapshot.text().previous_grapheme(at)
    }
    .map_err(|_| EditingError)?;
    if boundary == at {
        return Ok(navigation(context.selection));
    }
    let mut range = boundary.min(at)..boundary.max(at);
    if !literal && source[range.start.value()..range.end.value()] == ['\n'] {
        if forward {
            while source.get(range.end.value()) == Some(&'\n') {
                range.end = offset(range.end.value() + 1);
            }
        } else {
            while range.start.value() > 0 && source[range.start.value() - 1] == '\n' {
                range.start = offset(range.start.value() - 1);
            }
        }
    }
    let caret = range.start.value();
    Ok(replacement(snapshot, range, "", caret, Origin::Command))
}
fn demote(context: &CommandContext) -> Option<Outcome> {
    let position = context.selection.head().offset;
    let path = context.path(position);
    if let Some(heading) = path.iter().rev().find(|node| {
        matches!(node.syntax(), Syntax::Heading(_)) && node.content().start == position
    }) {
        let mut end = heading.raw().end.value();
        while end > heading.raw().start.value() && context.chars[end - 1] == '\n' {
            end -= 1;
        }
        let edits = [
            heading.raw().start..heading.content().start,
            heading.content().end..offset(end),
        ]
        .into_iter()
        .filter(|range| !range.is_empty())
        .map(|range| Edit::new(range, ""))
        .collect();
        return Some(edits_outcome(context.snapshot, context.selection, edits));
    }
    let container = path.iter().rev().find(|node| {
        matches!(
            node.syntax(),
            Syntax::Item | Syntax::Quote | Syntax::Callout(_)
        )
    })?;
    let content_start = container
        .children()
        .iter()
        .find(|node| !matches!(node.syntax(), Syntax::Task(_)))?
        .content()
        .start;
    if content_start != position {
        return None;
    }
    if container.syntax() == &Syntax::Item {
        if let Some(task) = container
            .children()
            .iter()
            .find(|node| node.syntax() == &Syntax::Task(true))
        {
            return Some(edits_outcome(
                context.snapshot,
                context.selection,
                vec![Edit::new(task.raw(), "[ ]")],
            ));
        }
        return indentation::list_tab(context.snapshot, context.selection, &context.chars, true);
    }
    let edits = super::containers::quote_markers(container, &context.chars, container.raw())
        .into_iter()
        .map(|range| Edit::new(range, ""))
        .collect();
    Some(edits_outcome(context.snapshot, context.selection, edits))
}
