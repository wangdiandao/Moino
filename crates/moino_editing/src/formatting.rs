//! Formatting command rules.
use super::{
    BlockFormat, CommandContext, Edit, EditingError, InlineFormat, Node, Origin, Outcome, Range,
    ScalarOffset, Selection, Snapshot, Syntax, Transaction, navigation, offset, prefix_len,
    replacement, text_block, text_intersects,
};

pub(super) fn inline(
    context: &CommandContext,
    format: InlineFormat,
) -> Result<Outcome, EditingError> {
    let selected = context.selection.range();
    if let Some(node) = active_style(context.snapshot.cst().nodes(), &selected, format) {
        return Ok(remove_markers(context.snapshot, context.selection, node));
    }
    let target = if selected.is_empty() {
        context
            .snapshot
            .text()
            .words()
            .into_iter()
            .find(|word| word.start <= selected.start && selected.start <= word.end)
            .unwrap_or(selected)
    } else {
        selected
    };
    let mut styles = Vec::new();
    matching_styles(context.snapshot.cst().nodes(), &target, format, &mut styles);
    if !target.is_empty()
        && !styles.is_empty()
        && (target.start.value()..target.end.value()).all(|at| {
            context.chars[at].is_whitespace()
                || styles
                    .iter()
                    .any(|node| node.raw().start.value() <= at && at < node.raw().end.value())
        })
    {
        let edits = styles
            .iter()
            .flat_map(|node| [node.opening_marker(), node.closing_marker()])
            .filter(|range| !range.is_empty())
            .map(|range| Edit::new(range, ""))
            .collect();
        return Ok(super::edits_outcome(
            context.snapshot,
            context.selection,
            edits,
        ));
    }
    let mut blocks = Vec::new();
    inline_ranges(context.snapshot.cst().nodes(), &target, &mut blocks);
    if blocks.is_empty() {
        blocks.push(target.clone());
    }
    let mut edits = Vec::new();
    let mut delimiters = Vec::new();
    let mut target_selection = context.selection;
    for range in blocks {
        let removals: Vec<_> = styles
            .iter()
            .filter(|node| range.start <= node.raw().start && node.raw().end <= range.end)
            .flat_map(|node| [node.opening_marker(), node.closing_marker()])
            .collect();
        let text: String = (range.start.value()..range.end.value())
            .filter(|at| {
                !removals
                    .iter()
                    .any(|range| range.start.value() <= *at && *at < range.end.value())
            })
            .map(|at| context.chars[at])
            .collect();
        let marker = delimiter(format, &text);
        let padding =
            if format == InlineFormat::Code && (text.starts_with('`') || text.ends_with('`')) {
                " "
            } else {
                ""
            };
        let value = format!("{marker}{padding}{text}{padding}{marker}");
        delimiters.push(marker.chars().count() + padding.len());
        if target.is_empty() || context.selection.is_collapsed() {
            let start = range.start.value() + marker.chars().count() + padding.len();
            target_selection = if target.is_empty() {
                Selection::cursor(offset(start))
            } else {
                Selection::new(
                    moino_document::Endpoint {
                        offset: offset(start),
                        affinity: moino_document::Affinity::After,
                    },
                    moino_document::Endpoint {
                        offset: offset(start + text.chars().count()),
                        affinity: moino_document::Affinity::Before,
                    },
                )
            };
        }
        edits.push(Edit::new(range, &value));
    }
    if !context.selection.is_collapsed() {
        let first = edits.first().expect("format target");
        let last = edits.last().expect("format target");
        let added: usize = edits.iter().map(|edit| edit.text().chars().count()).sum();
        let removed: usize = edits
            .iter()
            .map(|edit| edit.range().end.value() - edit.range().start.value())
            .sum();
        let low = first.range().start.value() + delimiters[0];
        let high =
            last.range().end.value() - removed + added - delimiters.last().expect("format target");
        let (anchor, head) = if context.selection.is_forward() {
            (low, high)
        } else {
            (high, low)
        };
        target_selection = Selection::new(
            moino_document::Endpoint {
                offset: offset(anchor),
                affinity: moino_document::Affinity::After,
            },
            moino_document::Endpoint {
                offset: offset(head),
                affinity: moino_document::Affinity::After,
            },
        );
    }
    let mut transaction =
        Transaction::new(context.snapshot, Origin::Command).with_selection(target_selection);
    for edit in edits {
        transaction = transaction.with_edit(edit);
    }
    Ok(Outcome {
        transaction: Some(transaction),
        selection: target_selection,
    })
}
fn matching_styles<'a>(
    nodes: &'a [Node],
    range: &Range<ScalarOffset>,
    format: InlineFormat,
    output: &mut Vec<&'a Node>,
) {
    for node in nodes {
        if node.raw().start >= range.end || node.raw().end <= range.start {
            continue;
        }
        if range.start <= node.raw().start
            && node.raw().end <= range.end
            && active_style(std::slice::from_ref(node), &node.raw(), format)
                .is_some_and(|found| found.id() == node.id())
        {
            output.push(node);
        } else {
            matching_styles(node.children(), range, format, output);
        }
    }
}
fn delimiter(format: InlineFormat, text: &str) -> String {
    if format != InlineFormat::Code {
        return format.marker().to_owned();
    }
    let longest = text.split(|ch| ch != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest + 1)
}
fn active_style<'a>(
    nodes: &'a [Node],
    range: &Range<ScalarOffset>,
    format: InlineFormat,
) -> Option<&'a Node> {
    for node in nodes {
        if node.raw().start <= range.start && range.end <= node.raw().end {
            if let Some(inner) = active_style(node.children(), range, format) {
                return Some(inner);
            }
            let matches = matches!(
                (format, node.syntax()),
                (InlineFormat::Bold, Syntax::Strong)
                    | (InlineFormat::Italic, Syntax::Emphasis)
                    | (InlineFormat::Code, Syntax::Code)
                    | (InlineFormat::Strike, Syntax::Strike)
                    | (InlineFormat::Highlight, Syntax::Highlight)
                    | (InlineFormat::Subscript, Syntax::Subscript)
                    | (InlineFormat::Superscript, Syntax::Superscript)
            );
            if matches {
                return Some(node);
            }
        }
    }
    None
}
fn inline_ranges(
    nodes: &[Node],
    selected: &Range<ScalarOffset>,
    output: &mut Vec<Range<ScalarOffset>>,
) {
    for node in nodes {
        if node.raw().start > selected.end || node.raw().end < selected.start {
            continue;
        }
        if matches!(
            node.syntax(),
            Syntax::Paragraph | Syntax::Heading(_) | Syntax::TableCell
        ) || node.syntax() == &Syntax::Item
            && node.children().iter().all(|child| {
                !matches!(
                    child.syntax(),
                    Syntax::Paragraph | Syntax::Heading(_) | Syntax::List(_) | Syntax::Quote
                )
            })
        {
            let content = node.content();
            let range = content.start.max(selected.start)..content.end.min(selected.end);
            if range.start < range.end || selected.is_empty() && range.start == range.end {
                output.push(range);
            }
        } else {
            inline_ranges(node.children(), selected, output);
        }
    }
}

pub(super) fn block(
    context: &CommandContext,
    requested: BlockFormat,
) -> Result<Outcome, EditingError> {
    if context.selection.is_collapsed() {
        return block_leaf(context, requested);
    }
    let selected = context.selection.range();
    let mut leaves = Vec::new();
    inline_ranges(context.snapshot.cst().nodes(), &selected, &mut leaves);
    if leaves.len() <= 1 {
        return block_leaf(context, requested);
    }
    let mut edits = Vec::new();
    let mut anchor = None;
    let mut head = None;
    let mut delta = 0_isize;
    for leaf in leaves {
        let at = leaf.start;
        let local = CommandContext::new(context.snapshot, Selection::cursor(at), context.options)?;
        let outcome = block_leaf(&local, requested)?;
        let Some(transaction) = outcome.transaction else {
            continue;
        };
        let local_edits = transaction.edits();
        if local_edits.iter().any(|edit| {
            edits
                .iter()
                .any(|previous: &Edit| previous.range().end > edit.range().start)
        }) {
            continue;
        }
        for (endpoint, resolved) in [
            (context.selection.anchor(), &mut anchor),
            (context.selection.head(), &mut head),
        ] {
            if leaf.start <= endpoint.offset && endpoint.offset <= leaf.end {
                let endpoint_context = CommandContext::new(
                    context.snapshot,
                    Selection::cursor(endpoint.offset),
                    context.options,
                )?;
                let endpoint_outcome = block_leaf(&endpoint_context, requested)?;
                *resolved = Some(offset(
                    endpoint_outcome
                        .selection
                        .head()
                        .offset
                        .value()
                        .saturating_add_signed(delta),
                ));
            }
        }
        for edit in local_edits {
            delta += edit.text().chars().count() as isize
                - (edit.range().end.value() - edit.range().start.value()) as isize;
            edits.push(edit.clone());
        }
    }
    let mapped = context.selection.mapped(&edits);
    let selection = Selection::new(
        moino_document::Endpoint {
            offset: anchor.unwrap_or(mapped.anchor().offset),
            affinity: context.selection.anchor().affinity,
        },
        moino_document::Endpoint {
            offset: head.unwrap_or(mapped.head().offset),
            affinity: context.selection.head().affinity,
        },
    );
    let mut transaction =
        Transaction::new(context.snapshot, Origin::Command).with_selection(selection);
    for edit in edits {
        transaction = transaction.with_edit(edit);
    }
    Ok(Outcome {
        transaction: Some(transaction),
        selection,
    })
}

fn block_leaf(context: &CommandContext, requested: BlockFormat) -> Result<Outcome, EditingError> {
    if matches!(requested, BlockFormat::Heading(level) if !(1..=6).contains(&level)) {
        return Err(EditingError);
    }
    let caret = context.selection.head().offset;
    let path = context.path(caret);
    let leaf = path.iter().rev().find(|node| {
        matches!(
            node.syntax(),
            Syntax::Paragraph | Syntax::Heading(_) | Syntax::Item
        )
    });
    let mut body = String::new();
    let mut relative = 0;
    let mut range = offset(context.line.start)..offset(context.line.end);
    let mut current = BlockFormat::Paragraph;
    if let Some(leaf) = leaf {
        range = leaf.raw();
        if let Syntax::Heading(level) = leaf.syntax() {
            current = BlockFormat::Heading(*level);
        }
        let first = leaf
            .children()
            .iter()
            .find(|child| !matches!(child.syntax(), Syntax::Task(_)));
        if leaf.syntax() == &Syntax::Item {
            range.start = first.map_or(leaf.content().end, |child| child.raw().start);
        }
        for child in leaf
            .children()
            .iter()
            .filter(|child| !matches!(child.syntax(), Syntax::Task(_)))
        {
            for at in child.raw().start.value()..child.raw().end.value() {
                body.push(context.chars[at]);
                if offset(at) < caret {
                    relative += 1;
                }
            }
        }
        if matches!(
            requested,
            BlockFormat::BulletList | BlockFormat::OrderedList | BlockFormat::TaskList
        ) && let Some(item) = path
            .iter()
            .rev()
            .find(|node| node.syntax() == &Syntax::Item)
        {
            range.start = item.raw().start;
            let task = item
                .children()
                .iter()
                .chain(item.children().iter().flat_map(|child| child.children()))
                .any(|child| matches!(child.syntax(), Syntax::Task(_)));
            let ordered = path
                .iter()
                .rev()
                .find_map(|node| {
                    if let Syntax::List(start) = node.syntax() {
                        Some(start.is_some())
                    } else {
                        None
                    }
                })
                .unwrap_or(false);
            current = if task {
                BlockFormat::TaskList
            } else if ordered {
                BlockFormat::OrderedList
            } else {
                BlockFormat::BulletList
            };
        }
        if requested == BlockFormat::Quote
            && path
                .iter()
                .any(|node| matches!(node.syntax(), Syntax::Quote | Syntax::Callout(_)))
        {
            let start = context.chars[..range.start.value()]
                .iter()
                .rposition(|ch| *ch == '\n')
                .map_or(0, |at| at + 1);
            if let Some(marker) = context.chars[start..range.start.value()]
                .iter()
                .rposition(|ch| *ch == '>')
            {
                range.start = offset(start + marker);
                current = BlockFormat::Quote;
            }
        }
    } else {
        body.extend(&context.chars[context.line.clone()]);
        relative = caret
            .value()
            .saturating_sub(context.line.start)
            .min(body.chars().count());
    }
    let trailing = range.end > range.start && context.chars[range.end.value() - 1] == '\n';
    let format = if requested == current {
        BlockFormat::Paragraph
    } else {
        requested
    };
    let prefix = match format {
        BlockFormat::Paragraph => String::new(),
        BlockFormat::Heading(level) => format!("{} ", "#".repeat(usize::from(level))),
        BlockFormat::Quote => "> ".into(),
        BlockFormat::BulletList => "- ".into(),
        BlockFormat::OrderedList => "1. ".into(),
        BlockFormat::TaskList => "- [ ] ".into(),
    };
    let line_start = context.chars[..range.start.value()]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |at| at + 1);
    let outer: String = context.chars[line_start..range.start.value()]
        .iter()
        .collect();
    let outer = super::heading::continuation(&outer);
    let continuation = match format {
        BlockFormat::Quote => "> ",
        BlockFormat::BulletList | BlockFormat::TaskList => "  ",
        BlockFormat::OrderedList => "   ",
        _ => "",
    };
    let separator = if matches!(format, BlockFormat::Heading(_)) {
        " ".to_owned()
    } else {
        format!("\n{outer}{continuation}")
    };
    let preceding_breaks = body.chars().take(relative).filter(|ch| *ch == '\n').count();
    let new_caret = range.start.value()
        + prefix.chars().count()
        + relative
        + preceding_breaks * separator.chars().count().saturating_sub(1);
    let value = format!(
        "{prefix}{}{}",
        body.split('\n').collect::<Vec<_>>().join(&separator),
        if trailing { "\n" } else { "" }
    );
    if context
        .snapshot
        .text()
        .slice(range.clone())
        .is_ok_and(|before| before == value)
    {
        return Ok(navigation(context.selection));
    }
    Ok(replacement(
        context.snapshot,
        range,
        &value,
        new_caret,
        Origin::Command,
    ))
}

pub(super) fn clear(context: &CommandContext) -> Result<Outcome, EditingError> {
    let selected = context.selection.range();
    let end = if selected.is_empty() {
        selected.end
    } else {
        offset(selected.end.value() - 1)
    };
    let first = text_block(
        context.snapshot.cst().nodes(),
        selected.start.value(),
        &context.chars,
    );
    let last = text_block(context.snapshot.cst().nodes(), end.value(), &context.chars);
    let scope = first.map_or(selected.start, |node| node.raw().start)
        ..last.map_or(selected.end, |node| node.raw().end);
    let mut ranges = Vec::new();
    collect_markers(
        context.snapshot.cst().nodes(),
        &scope,
        &context.chars,
        &mut ranges,
    );
    clear_containers(
        context.snapshot.cst().nodes(),
        &scope,
        &context.chars,
        &mut ranges,
    );
    ranges.retain(|range| range.start < range.end);
    ranges.sort_by_key(|range| range.start);
    let mut union: Vec<Range<ScalarOffset>> = Vec::new();
    for range in ranges {
        if let Some(previous) = union
            .last_mut()
            .filter(|previous| range.start <= previous.end)
        {
            previous.end = previous.end.max(range.end);
        } else {
            union.push(range);
        }
    }
    Ok(super::edits_outcome(
        context.snapshot,
        context.selection,
        union
            .into_iter()
            .map(|range| Edit::new(range, ""))
            .collect(),
    ))
}
fn clear_containers(
    nodes: &[Node],
    selected: &Range<ScalarOffset>,
    chars: &[char],
    output: &mut Vec<Range<ScalarOffset>>,
) {
    for node in nodes {
        if node.raw().start >= selected.end || node.raw().end <= selected.start {
            continue;
        }
        if matches!(
            node.syntax(),
            Syntax::CodeBlock(_) | Syntax::HtmlBlock | Syntax::MathBlock
        ) {
            continue;
        }
        if matches!(node.syntax(), Syntax::Quote | Syntax::Callout(_)) {
            output.extend(super::containers::quote_markers(
                node,
                chars,
                selected.clone(),
            ));
        }
        if node.syntax() == &Syntax::Item {
            let start = node.raw().start;
            let width = prefix_len(
                &chars[start.value()..]
                    .iter()
                    .take_while(|ch| **ch != '\n')
                    .collect::<String>(),
            );
            let end = offset(start.value() + width);
            if start < end && !text_intersects(node.children(), start.value(), end.value()) {
                output.push(start..end);
            }
        }
        clear_containers(node.children(), selected, chars, output);
    }
}

fn collect_markers(
    nodes: &[Node],
    selected: &Range<ScalarOffset>,
    chars: &[char],
    output: &mut Vec<Range<ScalarOffset>>,
) {
    for node in nodes {
        if node.raw().end < selected.start || node.raw().start > selected.end {
            continue;
        }
        if matches!(node.syntax(), Syntax::Heading(_)) {
            let mut end = node.raw().end.value();
            while end > node.raw().start.value() && chars[end - 1] == '\n' {
                end -= 1;
            }
            let intersects = if selected.is_empty() {
                node.raw().start <= selected.start && selected.start <= offset(end)
            } else {
                node.raw().start < selected.end && selected.start < offset(end)
            };
            if intersects {
                output.push(node.opening_marker());
                let closing = node.closing_marker();
                output.push(closing.start..closing.end.min(offset(end)));
            }
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
        ) && selected.start <= node.content().start
            && selected.end >= node.content().end
        {
            output.push(node.opening_marker());
            output.push(node.closing_marker());
        }
        collect_markers(node.children(), selected, chars, output);
    }
}

pub(super) fn remove_markers(snapshot: &Snapshot, selection: Selection, node: &Node) -> Outcome {
    let edits = [
        Edit::new(node.opening_marker(), ""),
        Edit::new(node.closing_marker(), ""),
    ];
    let selection = selection.mapped(&edits);
    let mut transaction = Transaction::new(snapshot, Origin::Command).with_selection(selection);
    for edit in edits {
        if edit.range().start != edit.range().end {
            transaction = transaction.with_edit(edit);
        }
    }
    Outcome {
        transaction: Some(transaction),
        selection,
    }
}
