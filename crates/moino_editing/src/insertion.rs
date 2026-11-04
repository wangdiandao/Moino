//! Keyboard-only transforms precede pairing; paste and composition bypass this module.
use super::{
    CommandContext, EditingError, Origin, Outcome, Selection, in_literal, is_inline_code_closer,
    navigation, normalize_newlines, offset, partner, replacement,
};

type PairRule = fn(&CommandContext, char) -> Option<Outcome>;
const PAIR_RULES: [PairRule; 4] = [upgrade_pair, complete_link, skip_closer, surround];

pub(super) fn insert(context: &CommandContext, input: String) -> Result<Outcome, EditingError> {
    let input = normalize_newlines(&input);
    let at = context.selection.range().start;
    let literal = in_literal(
        context.snapshot.cst().nodes(),
        at.value(),
        context.chars.len(),
    );
    let mut scalars = input.chars();
    let Some(character) = scalars.next() else {
        return Ok(context.insert_plain("", Origin::Typing));
    };
    if scalars.next().is_some() {
        if context.options.auto_pair && !literal && matches!(input.as_str(), "==" | "~~") {
            return Ok(wrap(context, &input, &input, Origin::Command));
        }
        return Ok(context.insert_plain(&input, Origin::Typing));
    }
    if context.selection.is_collapsed() && character == '`' {
        if context.options.auto_pair && is_inline_code_closer(context.snapshot.cst().nodes(), at) {
            return Ok(navigation(Selection::cursor(offset(at.value() + 1))));
        }
        if !literal
            && at.value() == context.line.start + 2
            && context.chars[context.line.start..at.value()] == ['`', '`']
        {
            return Ok(replacement(
                context.snapshot,
                offset(context.line.start)..at,
                "```\n\n```",
                context.line.start + 4,
                Origin::Command,
            ));
        }
    }
    if context.options.auto_pair && !literal {
        for rule in PAIR_RULES {
            if let Some(outcome) = rule(context, character) {
                return Ok(outcome);
            }
        }
    }
    Ok(context.insert_plain(&input, Origin::Typing))
}
fn wrap(context: &CommandContext, opening: &str, closing: &str, origin: Origin) -> Outcome {
    let range = context.selection.range();
    let body: String = context.chars[range.start.value()..range.end.value()]
        .iter()
        .collect();
    let value = format!("{opening}{body}{closing}");
    replacement(
        context.snapshot,
        range.clone(),
        &value,
        range.start.value() + opening.chars().count() + body.chars().count(),
        origin,
    )
}
fn upgrade_pair(context: &CommandContext, character: char) -> Option<Outcome> {
    let range = context.selection.range();
    let at = range.start.value();
    if at == 0 || context.chars[at - 1] != character || at > 1 && context.chars[at - 2] == character
    {
        return None;
    }
    if character == '=' {
        let body: String = context.chars[at..range.end.value()].iter().collect();
        return Some(replacement(
            context.snapshot,
            offset(at - 1)..range.end,
            &format!("=={body}=="),
            at + 1 + body.chars().count(),
            Origin::Command,
        ));
    }
    if !range.is_empty()
        || !matches!(character, '*' | '~' | '$')
        || context.chars.get(at) != Some(&character)
        || context.chars.get(at + 1) == Some(&character)
    {
        return None;
    }
    let (value, caret) = if character == '$' {
        if at - 1 != context.line.start {
            return None;
        }
        ("$$\n\n$$".to_owned(), at + 2)
    } else {
        (character.to_string().repeat(4), at + 1)
    };
    Some(replacement(
        context.snapshot,
        offset(at - 1)..offset(at + 1),
        &value,
        caret,
        Origin::Command,
    ))
}
fn complete_link(context: &CommandContext, character: char) -> Option<Outcome> {
    let at = context.selection.head().offset.value();
    if character != ']'
        || !context.selection.is_collapsed()
        || at == 0
        || context.chars[at - 1] != '['
        || context.chars.get(at) != Some(&']')
    {
        return None;
    }
    Some(replacement(
        context.snapshot,
        offset(at - 1)..offset(at + 1),
        "[]()",
        at,
        Origin::Command,
    ))
}
fn skip_closer(context: &CommandContext, character: char) -> Option<Outcome> {
    let at = context.selection.head().offset.value();
    if context.selection.is_collapsed()
        && ")]}`\"'*_~$".contains(character)
        && context.chars.get(at) == Some(&character)
    {
        Some(navigation(Selection::cursor(offset(at + 1))))
    } else {
        None
    }
}
fn surround(context: &CommandContext, character: char) -> Option<Outcome> {
    let closing = partner(character)?;
    let at = context.selection.head().offset;
    if character == '\''
        && context.selection.is_collapsed()
        && context
            .snapshot
            .text()
            .words()
            .iter()
            .any(|word| word.start < at && at <= word.end)
    {
        return None;
    }
    Some(wrap(
        context,
        &character.to_string(),
        &closing.to_string(),
        Origin::Typing,
    ))
}
