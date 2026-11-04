//! Enter command rules.
use super::{
    CommandContext, EditingError, Origin, Outcome, containers, heading, in_literal, replacement,
};

pub(super) fn run(context: &CommandContext) -> Result<Outcome, EditingError> {
    let snapshot = context.snapshot;
    let selection = context.selection;
    let chars = &context.chars;
    let range = context.selection.range();
    let start = context.selection.range().start.value();
    if in_literal(snapshot.cst().nodes(), start, chars.len()) {
        return Ok(replacement(
            snapshot,
            range,
            "\n",
            start + 1,
            Origin::Command,
        ));
    }
    if !context.has_unfinished_heading_prefix()
        && let Some(outcome) = heading::enter(snapshot, selection, chars)
    {
        return Ok(outcome);
    }
    if let Some(outcome) = containers::enter(snapshot, selection, chars) {
        return Ok(outcome);
    }
    Ok(replacement(
        snapshot,
        range,
        "\n\n",
        start + 2,
        Origin::Command,
    ))
}
