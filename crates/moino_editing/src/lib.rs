//! L0 view-independent editing intents; depends on text, Markdown and document transactions.
#![forbid(unsafe_code)]

use moino_document::{Edit, Origin, Selection, Snapshot, Transaction};
use moino_markdown::{Node, Syntax};
use moino_text::{ScalarOffset, normalize_newlines};
use std::ops::Range;

/// Inline formatting supported by business commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InlineFormat {
    /// Strong emphasis.
    Bold,
    /// Emphasis.
    Italic,
    /// Code span.
    Code,
    /// Strikethrough.
    Strike,
    /// Highlight.
    Highlight,
    /// Subscript.
    Subscript,
    /// Superscript.
    Superscript,
}
impl InlineFormat {
    fn marker(self) -> &'static str {
        match self {
            Self::Bold => "**",
            Self::Italic => "*",
            Self::Code => "`",
            Self::Strike => "~~",
            Self::Highlight => "==",
            Self::Subscript => "~",
            Self::Superscript => "^",
        }
    }
}
/// Block conversion commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlockFormat {
    /// Remove a block prefix.
    Paragraph,
    /// ATX heading, with a level from one to six.
    Heading(u8),
    /// Quote.
    Quote,
    /// Bullet list item.
    BulletList,
    /// Ordered list item.
    OrderedList,
    /// Unchecked task list item.
    TaskList,
}
/// One editing operation before it becomes a document transaction.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Intent {
    /// Ordinary keyboard text, eligible for interactive pairing.
    Insert(String),
    /// Committed IME text, bypassing interactive transformations.
    Composition(String),
    /// Pasted source, bypassing keyboard-only pairing and conversion.
    Paste(String),
    /// Split or continue the current block.
    Enter,
    /// Delete the previous grapheme or structural marker.
    Backspace,
    /// Delete the next grapheme or merge the following block.
    Delete,
    /// Indent or outdent.
    Tab {
        /// Whether Shift is pressed.
        reverse: bool,
    },
    /// Toggle an inline format.
    Inline(InlineFormat),
    /// Toggle a block format.
    Block(BlockFormat),
    /// Remove selected inline delimiters and restore affected blocks to paragraphs.
    ClearFormatting,
}
/// Effective input policy, supplied by settings rather than invented by the core.
#[derive(Clone, Debug)]
pub struct Options {
    indent: String,
    auto_pair: bool,
    read_only: bool,
}
impl Options {
    /// Sets the exact indentation unit and effective automatic-pairing policy.
    pub fn new(indent: &str, auto_pair: bool) -> Result<Self, EditingError> {
        if indent.is_empty() || !indent.chars().all(|ch| ch == ' ' || ch == '\t') {
            return Err(EditingError);
        }
        Ok(Self {
            indent: indent.to_owned(),
            auto_pair,
            read_only: false,
        })
    }
    /// Prohibits modifications while preserving the available command descriptions.
    pub fn read_only(mut self, value: bool) -> Self {
        self.read_only = value;
        self
    }
    /// Whether document writes are prohibited.
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }
}
/// Invalid or unavailable editing operation.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("editing.unavailable")]
pub struct EditingError;
impl EditingError {
    /// Stable error code.
    pub const fn code(self) -> &'static str {
        "editing.unavailable"
    }
    /// Localization key.
    pub const fn message_key(self) -> &'static str {
        "error.editing.unavailable"
    }
    /// Re-evaluate the intent after context changes.
    pub const fn is_retryable(self) -> bool {
        false
    }
}
/// A document transaction or a selection-only navigation.
#[derive(Clone, Debug)]
pub struct Outcome {
    transaction: Option<Transaction>,
    selection: Selection,
}
impl Outcome {
    /// Consumes the planned modification, if this operation changes text.
    pub fn into_transaction(self) -> Option<Transaction> {
        self.transaction
    }
    /// Selection after the operation, also valid for navigation-only outcomes.
    pub const fn selection(&self) -> Selection {
        self.selection
    }
}
fn offset(value: usize) -> ScalarOffset {
    ScalarOffset::new(value)
}
fn replacement(
    snapshot: &Snapshot,
    range: Range<ScalarOffset>,
    value: &str,
    caret: usize,
    origin: Origin,
) -> Outcome {
    let selection = Selection::cursor(offset(caret));
    Outcome {
        transaction: Some(
            Transaction::new(snapshot, origin)
                .with_edit(Edit::new(range, value))
                .with_selection(selection),
        ),
        selection,
    }
}
fn navigation(selection: Selection) -> Outcome {
    Outcome {
        transaction: None,
        selection,
    }
}

/// Resolves an intent using frozen text and syntax, without changing the document.
pub fn resolve(
    snapshot: &Snapshot,
    selection: Selection,
    intent: Intent,
    options: &Options,
) -> Result<Outcome, EditingError> {
    if options.read_only || selection.range().end > snapshot.text().len() {
        return Err(EditingError);
    }
    let source = snapshot.text().to_string();
    let chars: Vec<char> = source.chars().collect();
    let range = selection.range();
    let start = range.start.value();
    let end = range.end.value();
    let line_start = chars[..start]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |index| index + 1);
    let line_end = chars[start..]
        .iter()
        .position(|ch| *ch == '\n')
        .map_or(chars.len(), |index| start + index);
    let line: String = chars[line_start..line_end].iter().collect();
    let prefix = prefix_len(&line);
    let insertion_origin = if matches!(&intent, Intent::Paste(_)) {
        Origin::Paste
    } else {
        Origin::Composition
    };
    match intent {
        Intent::Composition(value) | Intent::Paste(value) => {
            let value = normalize_newlines(&value);
            Ok(replacement(
                snapshot,
                range,
                &value,
                start + value.chars().count(),
                insertion_origin,
            ))
        }
        Intent::Insert(value) => {
            let value = normalize_newlines(&value);
            if options.auto_pair && !in_literal(snapshot.cst().nodes(), start) {
                if start == end
                    && value == "`"
                    && start >= 2
                    && start - 2 == line_start
                    && chars[start - 2..start] == ['`', '`']
                {
                    return Ok(replacement(
                        snapshot,
                        offset(start - 2)..offset(start),
                        "```\n\n```",
                        start + 2,
                        Origin::Command,
                    ));
                }
                if start == end
                    && value == "]"
                    && start > 0
                    && chars[start - 1] == '['
                    && chars.get(start) == Some(&']')
                {
                    return Ok(replacement(
                        snapshot,
                        offset(start - 1)..offset(start + 1),
                        "[]()",
                        start,
                        Origin::Command,
                    ));
                }
                if matches!(value.as_str(), "==" | "~~") {
                    let selected: String = chars[start..end].iter().collect();
                    return Ok(replacement(
                        snapshot,
                        range,
                        &format!("{value}{selected}{value}"),
                        start + 2 + selected.chars().count(),
                        Origin::Command,
                    ));
                }
            }
            if options.auto_pair
                && value.chars().count() == 1
                && !in_literal(snapshot.cst().nodes(), start)
            {
                let ch = value.chars().next().ok_or(EditingError)?;
                if start == end
                    && start > 0
                    && chars[start - 1] == ch
                    && chars.get(start) == Some(&ch)
                    && matches!(ch, '*' | '~' | '$')
                    && (start < 2 || chars[start - 2] != ch)
                    && chars.get(start + 1) != Some(&ch)
                {
                    if ch == '$' && start - 1 == line_start {
                        return Ok(replacement(
                            snapshot,
                            offset(start - 1)..offset(start + 1),
                            "$$\n\n$$",
                            start + 2,
                            Origin::Command,
                        ));
                    }
                    if ch != '$' {
                        return Ok(replacement(
                            snapshot,
                            offset(start - 1)..offset(start + 1),
                            &ch.to_string().repeat(4),
                            start + 1,
                            Origin::Command,
                        ));
                    }
                }
                if ch == '='
                    && start > 0
                    && chars[start - 1] == '='
                    && (start < 2 || chars[start - 2] != '=')
                {
                    return Ok(replacement(
                        snapshot,
                        offset(start - 1)..offset(start),
                        "====",
                        start + 1,
                        Origin::Command,
                    ));
                }
                if start == end && chars.get(start) == Some(&ch) && ")]}`\"'*_~$".contains(ch) {
                    return Ok(navigation(Selection::cursor(offset(start + 1))));
                }
                if let Some(close) = partner(ch) {
                    let selected: String = chars[start..end].iter().collect();
                    let wrapped = format!("{ch}{selected}{close}");
                    return Ok(replacement(
                        snapshot,
                        range,
                        &wrapped,
                        start + 1 + selected.chars().count(),
                        Origin::Typing,
                    ));
                }
            }
            Ok(replacement(
                snapshot,
                range,
                &value,
                start + value.chars().count(),
                Origin::Typing,
            ))
        }
        Intent::Enter => {
            if in_literal(snapshot.cst().nodes(), start) {
                return Ok(replacement(
                    snapshot,
                    range,
                    "\n",
                    start + 1,
                    Origin::Command,
                ));
            }
            let line_chars: Vec<char> = line.chars().collect();
            let marker: String = line_chars[..prefix].iter().collect();
            let body: String = line_chars[prefix..].iter().collect();
            let quote_width = quote_prefix_width(&marker);
            if quote_width > 0 && marker[quote_width..].trim_start().starts_with('#') {
                let quote = &marker[..quote_width];
                if start == line_start + prefix {
                    let value = format!("{quote}\n");
                    return Ok(replacement(
                        snapshot,
                        offset(line_start)..offset(line_start),
                        &value,
                        start + value.chars().count(),
                        Origin::Command,
                    ));
                }
                let value = format!("\n{quote}");
                return Ok(replacement(
                    snapshot,
                    range,
                    &value,
                    start + value.chars().count(),
                    Origin::Command,
                ));
            }
            if marker.trim_start().starts_with('>') || is_list(&marker) {
                if body.trim().is_empty() {
                    let keep = if is_list(&marker) { quote_width } else { 0 };
                    return Ok(replacement(
                        snapshot,
                        offset(line_start + keep)..offset(line_end),
                        "",
                        line_start + keep,
                        Origin::Command,
                    ));
                }
                let continuation = next_marker(&marker);
                let value = format!("\n{continuation}");
                let mut outcome = replacement(
                    snapshot,
                    range,
                    &value,
                    start + value.chars().count(),
                    Origin::Command,
                );
                if start == end {
                    renumber_following(&mut outcome, snapshot, &chars, start, line_end, &marker);
                }
                return Ok(outcome);
            }
            if marker.trim_start().starts_with('#') && start == line_start + prefix {
                return Ok(replacement(
                    snapshot,
                    offset(line_start)..offset(line_start),
                    "\n\n",
                    start + 2,
                    Origin::Command,
                ));
            }
            Ok(replacement(
                snapshot,
                range,
                "\n\n",
                start + 2,
                Origin::Command,
            ))
        }
        Intent::Backspace | Intent::Delete if start != end => {
            Ok(replacement(snapshot, range, "", start, Origin::Command))
        }
        Intent::Backspace => {
            if start == 0 {
                return Ok(navigation(selection));
            }
            if let Some(node) = marker_boundary(snapshot.cst().nodes(), start, false) {
                return Ok(remove_markers(snapshot, selection, node));
            }
            if prefix > 0 && start == line_start + prefix {
                let quote = quote_prefix_width(&line);
                let indent = line[quote..]
                    .chars()
                    .take_while(|ch| *ch == ' ' || *ch == '\t')
                    .count();
                if is_list(&line.chars().take(prefix).collect::<String>()) {
                    let amount = if indent > 0 {
                        indent.min(options.indent.chars().count())
                    } else {
                        prefix - quote
                    };
                    return Ok(replacement(
                        snapshot,
                        offset(line_start + quote)..offset(line_start + quote + amount),
                        "",
                        start - amount,
                        Origin::Command,
                    ));
                }
                return Ok(replacement(
                    snapshot,
                    offset(line_start)..offset(start),
                    "",
                    line_start,
                    Origin::Command,
                ));
            }
            if start == line_start {
                let mut previous = start - 1;
                while previous > 0 && chars[previous - 1] == '\n' {
                    previous -= 1;
                }
                return Ok(replacement(
                    snapshot,
                    offset(previous)..offset(start),
                    "",
                    previous,
                    Origin::Command,
                ));
            }
            let previous = snapshot
                .text()
                .previous_grapheme(offset(start))
                .map_err(|_| EditingError)?;
            Ok(replacement(
                snapshot,
                previous..offset(start),
                "",
                previous.value(),
                Origin::Command,
            ))
        }
        Intent::Delete => {
            if start == chars.len() {
                return Ok(navigation(selection));
            }
            if let Some(node) = marker_boundary(snapshot.cst().nodes(), start, true) {
                return Ok(remove_markers(snapshot, selection, node));
            }
            let mut next = snapshot
                .text()
                .next_grapheme(offset(start))
                .map_err(|_| EditingError)?
                .value();
            if start == line_end {
                while chars.get(next) == Some(&'\n') {
                    next += 1;
                }
                let tail: String = chars[next..].iter().take_while(|ch| **ch != '\n').collect();
                next += prefix_len(&tail);
            }
            Ok(replacement(
                snapshot,
                offset(start)..offset(next),
                "",
                start,
                Origin::Command,
            ))
        }
        Intent::Tab { reverse } => {
            if let Some(table) = containing_table(snapshot.cst().nodes(), start) {
                let mut cells = Vec::new();
                table_cells(table.children(), &mut cells);
                if let Some(index) = cells.iter().position(|cell| {
                    cell.raw().start.value() <= start && start <= cell.raw().end.value()
                }) {
                    let target = if reverse {
                        index.saturating_sub(1)
                    } else {
                        index + 1
                    };
                    if let Some(cell) = cells.get(target) {
                        return Ok(navigation(Selection::cursor(cell.content().start)));
                    }
                    let Syntax::Table(columns) = table.syntax() else {
                        return Err(EditingError);
                    };
                    let position = table.raw().end.value();
                    let has_newline = position > 0 && chars[position - 1] == '\n';
                    let prefix = if has_newline { "" } else { "\n" };
                    let row = format!(
                        "{prefix}|{}{}",
                        "  |".repeat(*columns),
                        if has_newline { "\n" } else { "" }
                    );
                    return Ok(replacement(
                        snapshot,
                        offset(position)..offset(position),
                        &row,
                        position + prefix.len() + 2,
                        Origin::Command,
                    ));
                }
            }
            let last_line_start = if end > start && chars.get(end - 1) == Some(&'\n') {
                end - 1
            } else {
                end
            };
            let end_line = chars[last_line_start..]
                .iter()
                .position(|ch| *ch == '\n')
                .map_or(chars.len(), |i| last_line_start + i);
            if start == end && !is_list(&line.chars().take(prefix).collect::<String>()) && !reverse
            {
                return Ok(replacement(
                    snapshot,
                    range,
                    &options.indent,
                    start + options.indent.chars().count(),
                    Origin::Command,
                ));
            }
            let selected: String = chars[line_start..end_line].iter().collect();
            let replacement_text = selected
                .split('\n')
                .map(|line| {
                    let quote = quote_prefix_width(line);
                    let (container, body) = line.split_at(quote);
                    if reverse {
                        format!(
                            "{container}{}",
                            body.strip_prefix(&options.indent).unwrap_or(body)
                        )
                    } else {
                        format!("{container}{}{body}", options.indent)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let caret = line_start + replacement_text.chars().count();
            Ok(replacement(
                snapshot,
                offset(line_start)..offset(end_line),
                &replacement_text,
                caret,
                Origin::Command,
            ))
        }
        Intent::Inline(format) => {
            let marker = format.marker();
            let width = marker.chars().count();
            let selected: String = chars[start..end].iter().collect();
            let before: String = chars[start.saturating_sub(width)..start].iter().collect();
            let after: String = chars[end..(end + width).min(chars.len())].iter().collect();
            if before == marker && after == marker {
                return Ok(replacement(
                    snapshot,
                    offset(start - width)..offset(end + width),
                    &selected,
                    end - width,
                    Origin::Command,
                ));
            }
            if selected.len() >= marker.len() * 2
                && selected.starts_with(marker)
                && selected.ends_with(marker)
            {
                let value = &selected[marker.len()..selected.len() - marker.len()];
                return Ok(replacement(
                    snapshot,
                    range,
                    value,
                    start + value.chars().count(),
                    Origin::Command,
                ));
            }
            let value = format!("{marker}{selected}{marker}");
            Ok(replacement(
                snapshot,
                range,
                &value,
                start + width + selected.chars().count(),
                Origin::Command,
            ))
        }
        Intent::Block(format) => {
            if let Some(outcome) = convert_text_block(snapshot, selection, format, &chars)? {
                return Ok(outcome);
            }
            let desired = match format {
                BlockFormat::Paragraph => String::new(),
                BlockFormat::Heading(level @ 1..=6) => {
                    format!("{} ", "#".repeat(usize::from(level)))
                }
                BlockFormat::Heading(_) => return Err(EditingError),
                BlockFormat::Quote => "> ".to_owned(),
                BlockFormat::BulletList => "- ".to_owned(),
                BlockFormat::OrderedList => "1. ".to_owned(),
                BlockFormat::TaskList => "- [ ] ".to_owned(),
            };
            let current: String = line.chars().take(prefix).collect();
            let value = if current == desired { "" } else { &desired };
            let caret = start.saturating_sub(prefix) + value.chars().count();
            Ok(replacement(
                snapshot,
                offset(line_start)..offset(line_start + prefix),
                value,
                caret,
                Origin::Command,
            ))
        }
        Intent::ClearFormatting => {
            let mut ranges = Vec::new();
            collect_markers(snapshot.cst().nodes(), &range, &mut ranges);
            let mut cursor = line_start;
            let limit = if end > start && chars.get(end - 1) == Some(&'\n') {
                end - 1
            } else {
                end
            };
            while cursor <= limit && cursor < chars.len() {
                let stop = chars[cursor..]
                    .iter()
                    .position(|ch| *ch == '\n')
                    .map_or(chars.len(), |i| cursor + i);
                let line: String = chars[cursor..stop].iter().collect();
                let prefix = prefix_len(&line);
                if prefix > 0 {
                    ranges.push(offset(cursor)..offset(cursor + prefix));
                }
                if stop == chars.len() {
                    break;
                }
                cursor = stop + 1;
            }
            ranges.retain(|range| range.start < range.end);
            ranges.sort_by_key(|range| range.start);
            let mut merged: Vec<Range<ScalarOffset>> = Vec::new();
            for range in ranges {
                if let Some(previous) = merged
                    .last_mut()
                    .filter(|previous| range.start <= previous.end)
                {
                    previous.end = previous.end.max(range.end);
                } else {
                    merged.push(range);
                }
            }
            if merged.is_empty() {
                return Ok(navigation(selection));
            }
            let edits: Vec<_> = merged
                .into_iter()
                .map(|range| Edit::new(range, ""))
                .collect();
            let selection = selection.mapped(&edits);
            let mut transaction =
                Transaction::new(snapshot, Origin::Command).with_selection(selection);
            for edit in edits {
                transaction = transaction.with_edit(edit);
            }
            Ok(Outcome {
                transaction: Some(transaction),
                selection,
            })
        }
    }
}
fn text_block(nodes: &[Node], position: usize, document_end: usize) -> Option<&Node> {
    for node in nodes {
        if node.raw().start.value() <= position
            && (position < node.raw().end.value() || position == document_end)
        {
            if let Some(block) = text_block(node.children(), position, document_end) {
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
fn convert_text_block(
    snapshot: &Snapshot,
    selection: Selection,
    format: BlockFormat,
    chars: &[char],
) -> Result<Option<Outcome>, EditingError> {
    let caret = selection.head().offset.value();
    let Some(block) = text_block(snapshot.cst().nodes(), caret, chars.len()) else {
        return Ok(None);
    };
    let raw_start = block.raw().start.value();
    let first_line = chars[..raw_start]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |i| i + 1);
    let first_end = chars[first_line..]
        .iter()
        .position(|ch| *ch == '\n')
        .map_or(chars.len(), |i| first_line + i);
    let line: String = chars[first_line..first_end].iter().collect();
    let prefix_width = prefix_len(&line);
    let current: String = line.chars().take(prefix_width).collect();
    let body_start = if matches!(block.syntax(), Syntax::Heading(_)) {
        block.content().start.value()
    } else {
        raw_start.max(first_line + prefix_width)
    };
    let mut body_end = if matches!(block.syntax(), Syntax::Heading(_)) {
        block.content().end.value()
    } else {
        block.raw().end.value()
    };
    let trailing_newline =
        block.raw().end.value() > 0 && chars[block.raw().end.value() - 1] == '\n';
    if body_end > body_start && chars[body_end - 1] == '\n' {
        body_end -= 1;
    }
    if body_start > body_end {
        return Ok(None);
    }
    let raw_body: String = chars[body_start..body_end].iter().collect();
    let mut body = String::new();
    let mut relative = 0;
    let mut source_position = body_start;
    for (index, line) in raw_body.split_inclusive('\n').enumerate() {
        let skip = if index == 0 {
            0
        } else {
            let structured = prefix_len(line);
            if structured > 0 {
                structured
            } else {
                line.chars()
                    .take_while(|ch| *ch == ' ' || *ch == '\t')
                    .count()
                    .min(body_start - first_line)
            }
        };
        for (index, ch) in line.chars().enumerate() {
            if index >= skip {
                body.push(ch);
                if source_position < caret {
                    relative += 1;
                }
            }
            source_position += 1;
        }
    }
    let trimmed = &current[container_prefix_len(&current)..];
    let is_task = trimmed.contains("[ ]") || trimmed.contains("[x]") || trimmed.contains("[X]");
    let same = match format {
        BlockFormat::Paragraph => current.is_empty(),
        BlockFormat::Heading(level) => block.syntax() == &Syntax::Heading(level),
        BlockFormat::Quote => current.contains('>'),
        BlockFormat::BulletList => {
            !is_task
                && ["- ", "* ", "+ "]
                    .iter()
                    .any(|marker| trimmed.starts_with(marker))
        }
        BlockFormat::OrderedList => trimmed.chars().next().is_some_and(|ch| ch.is_ascii_digit()),
        BlockFormat::TaskList => is_task,
    };
    let format = if same { BlockFormat::Paragraph } else { format };
    let (prefix, continuation, join) = match format {
        BlockFormat::Paragraph => (String::new(), String::new(), "\n"),
        BlockFormat::Heading(level @ 1..=6) => (
            format!("{} ", "#".repeat(usize::from(level))),
            String::new(),
            " ",
        ),
        BlockFormat::Heading(_) => return Err(EditingError),
        BlockFormat::Quote => ("> ".to_owned(), "> ".to_owned(), "\n"),
        BlockFormat::BulletList => ("- ".to_owned(), "  ".to_owned(), "\n"),
        BlockFormat::OrderedList => ("1. ".to_owned(), "   ".to_owned(), "\n"),
        BlockFormat::TaskList => ("- [ ] ".to_owned(), "  ".to_owned(), "\n"),
    };
    let prior_lines = body.chars().take(relative).filter(|ch| *ch == '\n').count();
    let new_caret =
        first_line + prefix.chars().count() + relative + continuation.chars().count() * prior_lines;
    let separator = format!("{join}{continuation}");
    let output = format!(
        "{prefix}{}{}",
        body.split('\n').collect::<Vec<_>>().join(&separator),
        if trailing_newline { "\n" } else { "" }
    );
    if snapshot
        .text()
        .slice(offset(first_line)..block.raw().end)
        .is_ok_and(|source| source == output)
    {
        return Ok(Some(navigation(Selection::cursor(offset(new_caret)))));
    }
    Ok(Some(replacement(
        snapshot,
        offset(first_line)..block.raw().end,
        &output,
        new_caret,
        Origin::Command,
    )))
}
fn containing_table(nodes: &[Node], position: usize) -> Option<&Node> {
    for node in nodes {
        if node.raw().start.value() <= position && position <= node.raw().end.value() {
            if matches!(node.syntax(), Syntax::Table(_)) {
                return Some(node);
            }
            if let Some(table) = containing_table(node.children(), position) {
                return Some(table);
            }
        }
    }
    None
}
fn table_cells<'a>(nodes: &'a [Node], result: &mut Vec<&'a Node>) {
    for node in nodes {
        if node.syntax() == &Syntax::TableCell {
            result.push(node);
        } else {
            table_cells(node.children(), result);
        }
    }
}
fn marker_boundary(nodes: &[Node], position: usize, forward: bool) -> Option<&Node> {
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
fn remove_markers(snapshot: &Snapshot, selection: Selection, node: &Node) -> Outcome {
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
fn collect_markers(
    nodes: &[Node],
    selected: &Range<ScalarOffset>,
    output: &mut Vec<Range<ScalarOffset>>,
) {
    for node in nodes {
        if node.raw().end < selected.start || node.raw().start > selected.end {
            continue;
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
        collect_markers(node.children(), selected, output);
    }
}
fn partner(ch: char) -> Option<char> {
    match ch {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '`' | '"' | '\'' | '*' | '_' | '~' | '$' => Some(ch),
        _ => None,
    }
}
fn is_list(marker: &str) -> bool {
    let marker = &marker[container_prefix_len(marker)..];
    marker.starts_with("- ")
        || marker.starts_with("* ")
        || marker.starts_with("+ ")
        || marker.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}
fn prefix_len(line: &str) -> usize {
    let indent = line
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .count();
    let rest: String = line.chars().skip(indent).collect();
    let heading = rest.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&heading) && rest.chars().nth(heading) == Some(' ') {
        return indent + heading + 1;
    }
    if rest.starts_with('>') {
        let width = if rest
            .as_bytes()
            .get(1)
            .is_some_and(|byte| *byte == b' ' || *byte == b'\t')
        {
            2
        } else {
            1
        };
        return indent + width + prefix_len(&rest[width..]);
    }
    if ["- ", "* ", "+ "]
        .iter()
        .any(|marker| rest.starts_with(marker))
    {
        return indent
            + if rest[2..].starts_with("[ ] ")
                || rest[2..].starts_with("[x] ")
                || rest[2..].starts_with("[X] ")
            {
                6
            } else {
                2
            };
    }
    let digits = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if (1..=9).contains(&digits)
        && (rest[digits..].starts_with(". ") || rest[digits..].starts_with(") "))
    {
        return indent + digits + 2;
    }
    0
}
fn next_marker(marker: &str) -> String {
    let trimmed = &marker[container_prefix_len(marker)..];
    let digits = trimmed.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits > 0
        && let Ok(number) = trimmed[..digits].parse::<u64>()
    {
        return format!(
            "{}{}{}",
            &marker[..marker.len() - trimmed.len()],
            number + 1,
            &trimmed[digits..]
        );
    }
    marker.replace("[x]", "[ ]").replace("[X]", "[ ]")
}
fn container_prefix_len(marker: &str) -> usize {
    marker
        .bytes()
        .take_while(|byte| b" \t>".contains(byte))
        .count()
}
fn quote_prefix_width(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut cursor = bytes
        .iter()
        .take_while(|byte| b" \t".contains(byte))
        .count();
    let mut end = 0;
    while bytes.get(cursor) == Some(&b'>') {
        cursor += 1;
        if bytes.get(cursor).is_some_and(|byte| b" \t".contains(byte)) {
            cursor += 1;
        }
        end = cursor;
        while bytes.get(cursor).is_some_and(|byte| b" \t".contains(byte)) {
            cursor += 1;
        }
    }
    end
}
fn ordered_end(nodes: &[Node], position: usize) -> Option<usize> {
    for node in nodes {
        if node.raw().start.value() <= position && position <= node.raw().end.value() {
            if let Some(end) = ordered_end(node.children(), position) {
                return Some(end);
            }
            if matches!(node.syntax(), Syntax::List(Some(_))) {
                return Some(node.raw().end.value());
            }
        }
    }
    None
}
fn renumber_following(
    outcome: &mut Outcome,
    snapshot: &Snapshot,
    chars: &[char],
    caret: usize,
    line_end: usize,
    marker: &str,
) {
    let container = container_prefix_len(marker);
    let digits = marker[container..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    let Some(mut ordinal) = marker
        .get(container..container + digits)
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value + 2)
    else {
        return;
    };
    let Some(end) = ordered_end(snapshot.cst().nodes(), caret) else {
        return;
    };
    let mut cursor = line_end + 1;
    let Some(mut transaction) = outcome.transaction.take() else {
        return;
    };
    while cursor < end && cursor < chars.len() {
        let stop = chars[cursor..]
            .iter()
            .position(|ch| *ch == '\n')
            .map_or(chars.len(), |index| cursor + index);
        let line: String = chars[cursor..stop].iter().collect();
        let prefix = container_prefix_len(&line);
        let digits = line[prefix..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        if prefix == container
            && digits > 0
            && line
                .as_bytes()
                .get(prefix + digits)
                .is_some_and(|byte| *byte == b'.' || *byte == b')')
        {
            transaction = transaction.with_edit(Edit::new(
                offset(cursor + prefix)..offset(cursor + prefix + digits),
                &ordinal.to_string(),
            ));
            ordinal += 1;
        }
        cursor = stop + 1;
    }
    outcome.transaction = Some(transaction);
}
fn in_literal(nodes: &[Node], position: usize) -> bool {
    nodes.iter().any(|node| {
        node.raw().start.value() <= position
            && position < node.raw().end.value()
            && (matches!(
                node.syntax(),
                Syntax::CodeBlock(_) | Syntax::MathBlock | Syntax::HtmlBlock | Syntax::Code
            ) || in_literal(node.children(), position))
    })
}
