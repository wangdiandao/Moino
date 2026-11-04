//! Heading recognition and source mapping for the product dialect.
//! The backend still owns container structure, inline syntax and reference resolution.
use super::{ReferenceDefinition, normalize_label};
use moino_text::{ByteOffset, TextBuffer};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::{collections::BTreeMap, ops::Range};

type Events = Vec<(Event<'static>, Range<usize>)>;

#[derive(Clone, Debug)]
struct Heading {
    raw: Range<usize>,
    body: Range<usize>,
    level: usize,
}

/// Recognizes a single ATX line without changing its text or delimiter boundaries.
fn atx(source: &str, start: usize, end: usize) -> Option<Heading> {
    let line = &source[start..end];
    let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
    if hashes == 0 || line.len() == 1 {
        return None;
    }
    let level = hashes.min(6).min(line.len() - 1);
    let mut body_start = start + level;
    for (offset, ch) in source[body_start..end].char_indices() {
        if !ch.is_whitespace() {
            body_start += offset;
            break;
        }
        if body_start + offset + ch.len_utf8() == end {
            body_start += offset;
            break;
        }
    }
    let mut body_end = end;
    // A closing group requires preceding whitespace and at least one body character.
    // Several whitespace-separated groups are one trailing delimiter region.
    let mut cursor = end;
    loop {
        while cursor > body_start {
            let ch = source[body_start..cursor]
                .chars()
                .next_back()
                .expect("character");
            if !ch.is_whitespace() {
                break;
            }
            cursor -= ch.len_utf8();
        }
        let hashes_end = cursor;
        while cursor > body_start && source.as_bytes()[cursor - 1] == b'#' {
            cursor -= 1;
        }
        if hashes_end == cursor {
            break;
        }
        let separator_end = cursor;
        while cursor > body_start {
            let ch = source[body_start..cursor]
                .chars()
                .next_back()
                .expect("character");
            if !ch.is_whitespace() {
                break;
            }
            cursor -= ch.len_utf8();
        }
        if cursor == separator_end || cursor == body_start {
            break;
        }
        body_end = cursor;
    }
    Some(Heading {
        raw: start..end,
        body: body_start..body_end,
        level,
    })
}

struct Block {
    raw: Range<usize>,
    heading: bool,
    lines: BTreeMap<usize, usize>,
}

fn line_start(source: &str, byte: usize) -> usize {
    source[..byte].rfind('\n').map_or(0, |offset| offset + 1)
}
fn line_end(source: &str, byte: usize) -> usize {
    source[byte..]
        .find('\n')
        .map_or(source.len(), |offset| byte + offset)
}
fn setext_level(underline: &str) -> Option<usize> {
    let underline = underline.trim_end();
    (underline.len() >= 2 && underline.bytes().all(|byte| b"=-".contains(&byte)))
        .then(|| if underline.ends_with('=') { 1 } else { 2 })
}
fn contains(headings: &BTreeMap<usize, Heading>, byte: usize) -> bool {
    headings
        .range(..=byte)
        .next_back()
        .is_some_and(|(_, heading)| heading.raw.contains(&byte))
}

fn blocks(source: &str, events: &Events) -> Vec<Block> {
    let mut result = Vec::new();
    let mut current: Option<Block> = None;
    let mut items: Vec<Block> = Vec::new();
    let mut literal_depth = 0;
    for (event, range) in events {
        match event {
            Event::Start(Tag::Item) => items.push(Block {
                raw: range.clone(),
                heading: false,
                lines: BTreeMap::new(),
            }),
            Event::End(TagEnd::Item) => {
                if let Some(item) = items.pop() {
                    result.push(item);
                }
            }
            Event::Start(
                Tag::CodeBlock(_) | Tag::HtmlBlock | Tag::MetadataBlock(_) | Tag::Table(_),
            ) => literal_depth += 1,
            Event::End(
                TagEnd::CodeBlock | TagEnd::HtmlBlock | TagEnd::MetadataBlock(_) | TagEnd::Table,
            ) => literal_depth -= 1,
            Event::Start(Tag::Paragraph | Tag::Heading { .. }) => {
                let mut block = Block {
                    raw: range.clone(),
                    heading: matches!(event, Event::Start(Tag::Heading { .. })),
                    lines: BTreeMap::new(),
                };
                block
                    .lines
                    .insert(line_start(source, range.start), range.start);
                current = Some(block);
            }
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_)) => {
                if let Some(block) = current.take() {
                    result.push(block);
                }
            }
            Event::SoftBreak | Event::HardBreak | Event::TaskListMarker(_) => {}
            Event::End(_) => {}
            _ => {
                if literal_depth == 0
                    && let Some(block) = current.as_mut().or_else(|| items.last_mut())
                {
                    if range.start < block.raw.start {
                        continue;
                    }
                    if matches!(event, Event::Start(tag) if !matches!(tag, Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. } | Tag::Subscript | Tag::Superscript))
                    {
                        continue;
                    }
                    let line = line_start(source, range.start);
                    block
                        .lines
                        .entry(line)
                        .and_modify(|start| *start = (*start).min(range.start))
                        .or_insert(range.start);
                }
            }
        }
    }
    result
}

struct Shadow {
    text: String,
    positions: Vec<usize>,
}
impl Shadow {
    fn new() -> Self {
        Self {
            text: String::new(),
            positions: vec![0],
        }
    }
    fn copy(&mut self, source: &str, range: Range<usize>) {
        self.text.push_str(&source[range.clone()]);
        self.positions.extend(range.map(|byte| byte + 1));
    }
    fn insert(&mut self, value: &str, byte: usize) {
        self.text.push_str(value);
        self.positions
            .extend(std::iter::repeat_n(byte, value.len()));
    }
    fn blank(&mut self, range: Range<usize>) {
        for byte in range {
            self.text.push(' ');
            self.positions.push(byte + 1);
        }
    }
}

pub(super) fn events(
    source: &str,
    options: Options,
    text: &TextBuffer,
    references: &mut Vec<ReferenceDefinition>,
) -> Events {
    let baseline: Events = super::events(source, options).collect();
    let task_markers: BTreeMap<_, _> = baseline
        .iter()
        .filter(|(event, _)| matches!(event, Event::TaskListMarker(_)))
        .map(|(event, range)| {
            (
                line_start(source, range.start),
                (event.clone(), range.clone()),
            )
        })
        .collect();
    let mut headings = BTreeMap::new();
    let mut escapes = Vec::new();
    for block in blocks(source, &baseline) {
        for &start in block.lines.values() {
            let end = line_end(source, start).min(block.raw.end);
            if source[start..end].starts_with('#') {
                if let Some(heading) = atx(source, start, end) {
                    headings.insert(start, heading);
                } else {
                    escapes.push(start);
                }
            }
        }
        let lines: Vec<_> = block.lines.values().copied().collect();
        for pair in lines.windows(2) {
            let (previous, underline) = (pair[0], pair[1]);
            let end = line_end(source, underline).min(block.raw.end);
            if let Some(level) = setext_level(&source[underline..end])
                && !contains(&headings, previous)
                && !source[previous..line_end(source, previous)]
                    .trim()
                    .is_empty()
            {
                headings.insert(
                    previous,
                    Heading {
                        raw: previous..(end + 1).min(source.len()),
                        body: previous..line_end(source, previous),
                        level,
                    },
                );
            }
        }
        if block.heading && !source[block.raw.start..].starts_with('#') {
            let end = source[..block.raw.end].trim_end_matches('\n').len();
            let underline_start = line_start(source, end);
            let underline = source[underline_start..end]
                .trim_start_matches([' ', '>', '\t'])
                .trim_end();
            if !underline.is_empty() && underline.bytes().all(|byte| b"=-".contains(&byte)) {
                if underline.len() < 2 {
                    escapes.push(end - underline.len());
                } else if let Some((&_, &start)) = block.lines.iter().next_back()
                    && !contains(&headings, start)
                {
                    headings.insert(
                        start,
                        Heading {
                            raw: start..block.raw.end,
                            body: start..line_end(source, start),
                            level: setext_level(underline).expect("validated underline"),
                        },
                    );
                }
            }
        }
    }
    if headings.is_empty() && escapes.is_empty() {
        return baseline;
    }
    // Unicode whitespace shields the exact body from the backend's ASCII heading
    // trimming without altering inline delimiter flanking. Choose an absent sentinel.
    let decoded: String = baseline
        .iter()
        .filter_map(|(event, _)| match event {
            Event::Text(value)
            | Event::Code(value)
            | Event::Html(value)
            | Event::InlineHtml(value) => Some(value.as_ref()),
            _ => None,
        })
        .collect();
    let mut sentinel = "\u{a0}".to_owned();
    while source.contains(&sentinel) || decoded.contains(&sentinel) {
        sentinel.push('\u{a0}');
    }
    let mut shadow = Shadow::new();
    let mut cursor = 0;
    let mut tasks = BTreeMap::new();
    escapes.sort_unstable();
    for heading in headings.values() {
        let task = task_markers
            .get(&line_start(source, heading.raw.start))
            .filter(|(_, range)| cursor <= range.start && range.end <= heading.raw.start);
        if let Some((event, range)) = task {
            copy_escaped(&mut shadow, source, cursor..range.start, &escapes);
            *shadow.positions.last_mut().expect("task boundary") = heading.raw.start;
            tasks.insert(heading.raw.start, (event.clone(), range.clone()));
        } else {
            copy_escaped(&mut shadow, source, cursor..heading.raw.start, &escapes);
        }
        shadow.insert(&"#".repeat(heading.level), heading.raw.start);
        shadow.insert(" ", heading.body.start);
        shadow.insert(&sentinel, heading.body.start);
        shadow.copy(source, heading.body.clone());
        shadow.insert(&sentinel, heading.body.end);
        let end = line_end(source, heading.raw.start);
        shadow.blank(heading.body.end..end);
        if end < source.len() {
            shadow.copy(source, end..end + 1);
            // Setext consumes its underline without introducing a synthetic blank line.
            if heading.raw.end > end + 1 {
                *shadow.positions.last_mut().expect("newline boundary") = heading.raw.end;
            }
        }
        cursor = heading.raw.end.max((end + 1).min(source.len()));
    }
    copy_escaped(&mut shadow, source, cursor..source.len(), &escapes);
    // Heading boundaries can expose or suppress reference definitions. Resolve
    // them from the same block stream as links, then return original coordinates.
    references.clear();
    if shadow.text.contains("]:") {
        let parser = Parser::new_ext(&shadow.text, options);
        for (label, definition) in parser.reference_definitions().iter() {
            references.push(ReferenceDefinition {
                label: normalize_label(label),
                destination: definition.dest.to_string(),
                title: definition.title.as_ref().map(ToString::to_string),
                raw: text
                    .byte_to_scalar(ByteOffset::new(shadow.positions[definition.span.start]))
                    .expect("mapped definition start")
                    ..text
                        .byte_to_scalar(ByteOffset::new(shadow.positions[definition.span.end]))
                        .expect("mapped definition end"),
            });
        }
        references.sort_by_key(|definition| definition.raw.start);
    }
    let mut output = Vec::new();
    for (event, range) in super::events(&shadow.text, options) {
        let mapped = shadow.positions[range.start]..shadow.positions[range.end];
        if matches!(event, Event::Start(Tag::Heading { .. }))
            && let Some(task) = tasks.remove(&mapped.start)
        {
            output.push(task);
        }
        let event = match event {
            Event::Text(value) if value.contains(&sentinel) => {
                let value = value.replace(&sentinel, "");
                if value.is_empty() {
                    continue;
                }
                Event::Text(value.into())
            }
            event => event,
        };
        output.push((event, mapped));
    }
    output
}

fn copy_escaped(shadow: &mut Shadow, source: &str, range: Range<usize>, escapes: &[usize]) {
    let mut cursor = range.start;
    for &escape in escapes.iter().filter(|&&byte| range.contains(&byte)) {
        shadow.copy(source, cursor..escape);
        shadow.insert("\\", escape);
        cursor = escape;
    }
    shadow.copy(source, cursor..range.end);
}
