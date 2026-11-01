//! L0 lossless Markdown syntax and node identities; depends only on moino_text internally.
#![forbid(unsafe_code)]

use finl_unicode::categories::CharacterCategories;
use moino_text::{ByteOffset, ScalarOffset, TextBuffer};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
};

/// Session-local identity of a syntax node.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(u64);
impl NodeId {
    /// Opaque identity value, not a text position.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Independently switchable product syntax extensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Extension {
    /// Pipe tables.
    Tables,
    /// Strikethrough.
    Strike,
    /// Task list checkboxes.
    Tasks,
    /// Bare URLs.
    Autolinks,
    /// Inline and block mathematics.
    Math,
    /// Highlight markers.
    Highlight,
    /// Subscript markers.
    Subscript,
    /// Superscript markers.
    Superscript,
    /// Table of contents.
    Toc,
    /// Typed block quotes.
    Callouts,
    /// YAML metadata.
    FrontMatter,
    /// Footnotes.
    Footnotes,
}
/// Effective dialect configuration; disabled extensions remain ordinary source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Extensions(u16);
impl Default for Extensions {
    fn default() -> Self {
        Self((1 << 12) - 1)
    }
}
impl Extensions {
    /// Selects only CommonMark syntax.
    pub const fn commonmark() -> Self {
        Self(0)
    }
    /// Sets a single dialect capability.
    pub fn with(mut self, extension: Extension, enabled: bool) -> Self {
        let bit = 1 << extension as u16;
        if enabled {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
        self
    }
    /// Whether this capability is enabled.
    pub const fn is_enabled(self, extension: Extension) -> bool {
        self.0 & (1 << extension as u16) != 0
    }
    fn options(self) -> Options {
        let mut options = Options::empty();
        for (extension, flag) in [
            (Extension::Tables, Options::ENABLE_TABLES),
            (Extension::Strike, Options::ENABLE_STRIKETHROUGH),
            (Extension::Tasks, Options::ENABLE_TASKLISTS),
            (Extension::Math, Options::ENABLE_MATH),
            (Extension::Subscript, Options::ENABLE_SUBSCRIPT),
            (Extension::Superscript, Options::ENABLE_SUPERSCRIPT),
            (Extension::Callouts, Options::ENABLE_GFM),
            (
                Extension::FrontMatter,
                Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
            ),
            (Extension::Footnotes, Options::ENABLE_FOOTNOTES),
        ] {
            if self.is_enabled(extension) {
                options.insert(flag);
            }
        }
        options
    }
}

/// Syntax recognized in the document dialect.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Syntax {
    /// Uninterpreted source between semantic nodes.
    Trivia,
    /// Paragraph.
    Paragraph,
    /// Heading with level.
    Heading(u8),
    /// Fenced or indented code with information string.
    CodeBlock(String),
    /// Quote container.
    Quote,
    /// Typed quote container.
    Callout(String),
    /// Ordered (starting ordinal) or unordered list.
    List(Option<u64>),
    /// List item.
    Item,
    /// Thematic separator.
    Rule,
    /// HTML block.
    HtmlBlock,
    /// Table and its column count.
    Table(usize),
    /// Header row.
    TableHead,
    /// Body row.
    TableRow,
    /// Cell.
    TableCell,
    /// YAML metadata.
    FrontMatter,
    /// Footnote definition with label.
    FootnoteDefinition(String),
    /// Reference definition preserved outside semantic events.
    ReferenceDefinition,
    /// Derived table of contents.
    Toc,
    /// Block mathematics.
    MathBlock,
    /// Plain or decoded inline text.
    Text,
    /// Emphasis.
    Emphasis,
    /// Strong emphasis.
    Strong,
    /// Strikethrough.
    Strike,
    /// Highlight.
    Highlight,
    /// Subscript.
    Subscript,
    /// Superscript.
    Superscript,
    /// Inline code.
    Code,
    /// Inline mathematics.
    Math,
    /// Link destination.
    Link(String),
    /// Image destination.
    Image(String),
    /// Footnote reference.
    FootnoteReference(String),
    /// Inline HTML data.
    Html,
    /// Soft line break.
    SoftBreak,
    /// Explicit line break.
    HardBreak,
    /// Task checkbox.
    Task(bool),
}

/// A source node; ranges use document scalar coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    id: NodeId,
    syntax: Syntax,
    raw: Range<ScalarOffset>,
    content: Range<ScalarOffset>,
    children: Vec<Node>,
    text: String,
    anchor: Option<String>,
}
impl Node {
    /// Stable session-local identity.
    pub fn id(&self) -> NodeId {
        self.id
    }
    /// Semantic syntax.
    pub fn syntax(&self) -> &Syntax {
        &self.syntax
    }
    /// Source including delimiters.
    pub fn raw(&self) -> Range<ScalarOffset> {
        self.raw.clone()
    }
    /// Source excluding outer delimiters.
    pub fn content(&self) -> Range<ScalarOffset> {
        self.content.clone()
    }
    /// Opening syntax marker.
    pub fn opening_marker(&self) -> Range<ScalarOffset> {
        self.raw.start..self.content.start
    }
    /// Closing syntax marker.
    pub fn closing_marker(&self) -> Range<ScalarOffset> {
        self.content.end..self.raw.end
    }
    /// Semantic child nodes in document order.
    pub fn children(&self) -> &[Node] {
        &self.children
    }
    /// Decoded text or code content.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Unique heading anchor within this parse.
    pub fn anchor(&self) -> Option<&str> {
        self.anchor.as_deref()
    }
}

/// A resolved reference definition; duplicate labels keep the first definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceDefinition {
    label: String,
    destination: String,
    title: Option<String>,
    raw: Range<ScalarOffset>,
}
/// One validated pre-edit range and its normalized inserted scalar count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextChange {
    old: Range<ScalarOffset>,
    inserted: ScalarOffset,
}
impl TextChange {
    /// Describes one replacement without carrying document text across the parser boundary.
    pub fn new(old: Range<ScalarOffset>, inserted: ScalarOffset) -> Self {
        Self { old, inserted }
    }
    /// Pre-edit source range.
    pub fn old(&self) -> Range<ScalarOffset> {
        self.old.clone()
    }
    /// Count of normalized inserted scalars.
    pub const fn inserted(&self) -> ScalarOffset {
        self.inserted
    }
}
impl ReferenceDefinition {
    /// Case-folded label with normalized whitespace.
    pub fn label(&self) -> &str {
        &self.label
    }
    /// Decoded destination as inert document data.
    pub fn destination(&self) -> &str {
        &self.destination
    }
    /// Optional decoded title.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    /// Definition source in document scalar coordinates.
    pub fn raw(&self) -> Range<ScalarOffset> {
        self.raw.clone()
    }
}

/// Frozen lossless syntax, sharing no mutable state with subsequent parses.
#[derive(Clone, Debug)]
pub struct Cst {
    text: TextBuffer,
    nodes: Arc<Vec<Node>>,
    extensions: Extensions,
    references: Arc<Vec<ReferenceDefinition>>,
}
impl Cst {
    /// Parses the complete product dialect from normalized text.
    pub fn parse(text: &TextBuffer) -> Self {
        Self::parse_with_extensions(text, Extensions::default())
    }
    /// Parses with explicit syntax capabilities, independently of application settings storage.
    pub fn parse_with_extensions(text: &TextBuffer, extensions: Extensions) -> Self {
        Self::parse_mode(text, true, extensions)
    }
    fn parse_mode(text: &TextBuffer, metadata: bool, extensions: Extensions) -> Self {
        let source = text.to_string();
        let mut options = extensions.options();
        if !metadata {
            options.remove(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        }
        let mut references = Vec::new();
        if source.contains("]:") {
            let parser = Parser::new_ext(&source, options);
            for (label, definition) in parser.reference_definitions().iter() {
                references.push(ReferenceDefinition {
                    label: normalize_label(label),
                    destination: definition.dest.to_string(),
                    title: definition.title.as_ref().map(ToString::to_string),
                    raw: text
                        .byte_to_scalar(ByteOffset::new(definition.span.start))
                        .expect("definition boundary")
                        ..text
                            .byte_to_scalar(ByteOffset::new(definition.span.end))
                            .expect("definition boundary"),
                });
            }
            references.sort_by_key(|definition| definition.raw.start);
        }
        let mut stack: Vec<Node> = Vec::new();
        let mut roots = Vec::new();
        let mut next_id = 1;
        for (event, bytes) in events(&source, options) {
            let raw = text
                .byte_to_scalar(ByteOffset::new(bytes.start))
                .expect("parser UTF-8 boundary")
                ..text
                    .byte_to_scalar(ByteOffset::new(bytes.end))
                    .expect("parser UTF-8 boundary");
            match event {
                Event::Start(tag) => {
                    let syntax = tag_syntax(tag);
                    stack.push(Node {
                        id: NodeId(next_id),
                        syntax,
                        raw: raw.clone(),
                        content: raw,
                        children: Vec::new(),
                        text: String::new(),
                        anchor: None,
                    });
                    next_id += 1;
                }
                Event::End(_) => {
                    if let Some(mut node) = stack.pop() {
                        finish(&mut node, text);
                        append(node, &mut stack, &mut roots);
                    }
                }
                event => {
                    let (syntax, value) = match event {
                        Event::Text(value) => (Syntax::Text, value.to_string()),
                        Event::Code(value) => (Syntax::Code, value.to_string()),
                        Event::InlineMath(value) => (Syntax::Math, value.to_string()),
                        Event::DisplayMath(value) => (Syntax::MathBlock, value.to_string()),
                        Event::Html(value) | Event::InlineHtml(value) => {
                            (Syntax::Html, value.to_string())
                        }
                        Event::FootnoteReference(label) => {
                            (Syntax::FootnoteReference(label.to_string()), String::new())
                        }
                        Event::SoftBreak => (Syntax::SoftBreak, "\n".to_owned()),
                        Event::HardBreak => (Syntax::HardBreak, "\n".to_owned()),
                        Event::Rule => (Syntax::Rule, String::new()),
                        Event::TaskListMarker(checked) => (Syntax::Task(checked), String::new()),
                        _ => continue,
                    };
                    let mut node = Node {
                        id: NodeId(next_id),
                        syntax,
                        raw: raw.clone(),
                        content: raw,
                        children: Vec::new(),
                        text: value,
                        anchor: None,
                    };
                    next_id += 1;
                    finish(&mut node, text);
                    append(node, &mut stack, &mut roots);
                }
            }
        }
        let mut nodes = Vec::new();
        let mut cursor = ScalarOffset::new(0);
        for node in roots {
            if cursor < node.raw.start {
                nodes.push(trivia(text, cursor..node.raw.start, next_id));
                next_id += 1;
            }
            cursor = node.raw.end;
            nodes.push(node);
        }
        if cursor < text.len() {
            nodes.push(trivia(text, cursor..text.len(), next_id));
        }
        inject_references(&mut nodes, &references, text);
        let mut anchors = HashMap::<String, usize>::new();
        for node in &mut nodes {
            extend(node, text, extensions);
        }
        let mut used = HashSet::new();
        for (ordinal, node) in nodes.iter_mut().enumerate() {
            fingerprint_ids(node, NodeId(0), ordinal, text, &mut used);
        }
        assign_anchors(&mut nodes, &mut anchors);
        Self {
            text: text.clone(),
            nodes: Arc::new(nodes),
            extensions,
            references: Arc::new(references),
        }
    }
    /// Ordered top-level nodes, including uninterpreted source gaps.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
    /// Definitions in source order, independent of hash-map traversal order.
    pub fn references(&self) -> &[ReferenceDefinition] {
        &self.references
    }
    /// Looks up a label with Unicode case folding and whitespace normalization.
    pub fn reference(&self, label: &str) -> Option<&ReferenceDefinition> {
        let label = normalize_label(label);
        self.references
            .iter()
            .find(|definition| definition.label == label)
    }
    /// Checks source partitioning, nested ranges and unique session identities.
    pub fn is_valid(&self) -> bool {
        let mut end = ScalarOffset::new(0);
        for node in self.nodes.iter() {
            if node.raw.start != end {
                return false;
            }
            end = node.raw.end;
        }
        if end != self.text.len() {
            return false;
        }
        let mut seen = HashSet::new();
        let mut pending: Vec<_> = self
            .nodes
            .iter()
            .map(|node| (node, ScalarOffset::new(0)..self.text.len()))
            .collect();
        while let Some((node, parent)) = pending.pop() {
            if node.raw.start < parent.start
                || node.raw.end > parent.end
                || node.raw.start > node.raw.end
                || node.content.start < node.raw.start
                || node.content.end > node.raw.end
                || node.content.start > node.content.end
                || !seen.insert(node.id)
            {
                return false;
            }
            let mut previous_end = node.raw.start;
            for child in &node.children {
                if child.raw.start < previous_end {
                    return false;
                }
                previous_end = child.raw.end;
            }
            pending.extend(node.children.iter().map(|child| (child, node.raw.clone())));
        }
        true
    }
    /// Reconstructs authoritative Markdown without a formatting pass.
    pub fn serialize(&self) -> String {
        self.nodes
            .iter()
            .map(|node| self.text.slice(node.raw()).expect("CST source range"))
            .collect()
    }
    /// Infers a single textual replacement; use explicit changes for transaction identities.
    pub fn reparse(&self, text: &TextBuffer) -> Self {
        let before: Vec<char> = self.text.to_string().chars().collect();
        let after: Vec<char> = text.to_string().chars().collect();
        let prefix = before
            .iter()
            .zip(&after)
            .take_while(|(a, b)| a == b)
            .count();
        if before == after {
            return self.clone();
        }
        let suffix = before[prefix..]
            .iter()
            .rev()
            .zip(after[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let old_end = before.len() - suffix;
        let new_end = after.len() - suffix;
        self.reparse_with_changes(
            text,
            &[TextChange::new(
                ScalarOffset::new(prefix)..ScalarOffset::new(old_end),
                ScalarOffset::new(new_end - prefix),
            )],
        )
        .expect("inferred replacement exactly describes the texts")
    }
    /// Reparses a transaction's actual edit windows and preserves all unaffected node identities.
    /// Returns None if ranges, inserted lengths or unchanged source do not describe the new text.
    pub fn reparse_with_changes(&self, text: &TextBuffer, changes: &[TextChange]) -> Option<Self> {
        let before: Vec<_> = self.text.to_string().chars().collect();
        let after: Vec<_> = text.to_string().chars().collect();
        let mut changes = changes.to_vec();
        changes.sort_by_key(|change| (change.old.start, change.old.end));
        let mut old_cursor = 0;
        let mut new_cursor: usize = 0;
        for (index, change) in changes.iter().enumerate() {
            if change.old.start > change.old.end
                || change.old.end.value() > before.len()
                || change.old.start.value() < old_cursor
                || (index > 0 && changes[index - 1].old.start == change.old.start)
            {
                return None;
            }
            let unchanged = change.old.start.value() - old_cursor;
            let next = new_cursor.checked_add(unchanged)?;
            if before.get(old_cursor..change.old.start.value())? != after.get(new_cursor..next)? {
                return None;
            }
            new_cursor = next.checked_add(change.inserted.value())?;
            if new_cursor > after.len() {
                return None;
            }
            old_cursor = change.old.end.value();
        }
        if before.get(old_cursor..)? != after.get(new_cursor..)? {
            return None;
        }
        if before == after {
            return Some(self.clone());
        }
        let prefix = changes.first()?.old.start.value();
        let old_end = changes.last()?.old.end.value();
        let removed: usize = changes
            .iter()
            .map(|change| change.old.end.value() - change.old.start.value())
            .sum();
        let inserted: usize = changes.iter().map(|change| change.inserted.value()).sum();
        let new_end = old_end - removed + inserted;
        let global = self.text.to_string().contains("]:") || text.to_string().contains("]:");
        let mut parsed = if self.nodes.is_empty() || global {
            Self::parse_with_extensions(text, self.extensions)
        } else {
            let mut first = self
                .nodes
                .iter()
                .position(|node| node.raw.end.value() >= prefix)
                .unwrap_or(self.nodes.len() - 1)
                .saturating_sub(2);
            while first > 0
                && self.nodes[first].raw.start.value() > 0
                && before[self.nodes[first].raw.start.value() - 1] != '\n'
            {
                first -= 1;
            }
            let mut last = self
                .nodes
                .iter()
                .position(|node| node.raw.start.value() >= old_end)
                .unwrap_or(self.nodes.len());
            last = (last + 3).min(self.nodes.len());
            let start = self.nodes[first].raw.start.value();
            loop {
                let stop = self.nodes[last - 1].raw.end.value();
                let new_stop = stop - old_end + new_end;
                let fragment = text
                    .slice(ScalarOffset::new(start)..ScalarOffset::new(new_stop))
                    .expect("mapped block window");
                let mut window =
                    Self::parse_mode(&TextBuffer::new(&fragment), start == 0, self.extensions);
                let stable = last == self.nodes.len()
                    || window.nodes.last().is_some_and(|node| {
                        node.syntax == self.nodes[last - 1].syntax
                            && window.text.slice(node.raw()).ok()
                                == self.text.slice(self.nodes[last - 1].raw()).ok()
                    });
                if stable {
                    for node in Arc::make_mut(&mut window.nodes) {
                        relocate(node, 0, start);
                    }
                    let mut nodes = self.nodes[..first].to_vec();
                    nodes.extend(window.nodes.iter().cloned());
                    for mut node in self.nodes[last..].iter().cloned() {
                        relocate(&mut node, old_end, new_end);
                        nodes.push(node);
                    }
                    break Self {
                        text: text.clone(),
                        nodes: Arc::new(nodes),
                        extensions: self.extensions,
                        references: self.references.clone(),
                    };
                }
                last = (last + (last - first).max(1)).min(self.nodes.len());
            }
        };
        let mut used = HashSet::new();
        reserve_ids(&self.nodes, &mut used);
        let mut old_nodes = HashMap::new();
        map_unchanged(&self.nodes, &changes, &mut old_nodes);
        let mut identities = IdentityReuse {
            old_text: &self.text,
            new_text: text,
            old_nodes,
            used,
            reused: HashSet::new(),
        };
        for (ordinal, node) in Arc::make_mut(&mut parsed.nodes).iter_mut().enumerate() {
            identities.apply(node, NodeId(0), ordinal);
        }
        assign_anchors(
            Arc::make_mut(&mut parsed.nodes).as_mut_slice(),
            &mut HashMap::new(),
        );
        Some(parsed)
    }
}

fn definition_node(definition: &ReferenceDefinition) -> Node {
    Node {
        id: NodeId(0),
        syntax: Syntax::ReferenceDefinition,
        raw: definition.raw.clone(),
        content: definition.raw.clone(),
        children: Vec::new(),
        text: String::new(),
        anchor: None,
    }
}
fn inject_references(
    nodes: &mut Vec<Node>,
    definitions: &[ReferenceDefinition],
    text: &TextBuffer,
) {
    if definitions.is_empty() {
        return;
    }
    let mut output = Vec::new();
    for mut node in std::mem::take(nodes) {
        let inside: Vec<_> = definitions
            .iter()
            .filter(|definition| {
                node.raw.start <= definition.raw.start && definition.raw.end <= node.raw.end
            })
            .cloned()
            .collect();
        if matches!(node.syntax, Syntax::Trivia | Syntax::ReferenceDefinition) && !inside.is_empty()
        {
            let mut cursor = node.raw.start;
            for definition in &inside {
                if cursor < definition.raw.start {
                    output.push(trivia(text, cursor..definition.raw.start, 0));
                }
                output.push(definition_node(definition));
                cursor = definition.raw.end;
            }
            if cursor < node.raw.end {
                output.push(trivia(text, cursor..node.raw.end, 0));
            }
        } else {
            inject_references(&mut node.children, &inside, text);
            for definition in inside {
                if !node.children.iter().any(|child| {
                    child.raw.start <= definition.raw.start && definition.raw.end <= child.raw.end
                }) {
                    node.children.push(definition_node(&definition));
                }
            }
            node.children.sort_by_key(|child| child.raw.start);
            output.push(node);
        }
    }
    *nodes = output;
}
fn relocate(node: &mut Node, old: usize, new: usize) {
    node.raw = ScalarOffset::new(node.raw.start.value() - old + new)
        ..ScalarOffset::new(node.raw.end.value() - old + new);
    node.content = ScalarOffset::new(node.content.start.value() - old + new)
        ..ScalarOffset::new(node.content.end.value() - old + new);
    for child in &mut node.children {
        relocate(child, old, new);
    }
}
fn reserve_ids(nodes: &[Node], used: &mut HashSet<u64>) {
    for node in nodes {
        used.insert(node.id.0);
        reserve_ids(&node.children, used);
    }
}
fn fingerprint_ids(
    node: &mut Node,
    parent: NodeId,
    ordinal: usize,
    text: &TextBuffer,
    used: &mut HashSet<u64>,
) {
    node.id = fingerprint_id(node, parent, ordinal, text, used);
    for (ordinal, child) in node.children.iter_mut().enumerate() {
        fingerprint_ids(child, node.id, ordinal, text, used);
    }
}
fn fingerprint_id(
    node: &Node,
    parent: NodeId,
    ordinal: usize,
    text: &TextBuffer,
    used: &mut HashSet<u64>,
) -> NodeId {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    parent.0.hash(&mut hash);
    ordinal.hash(&mut hash);
    node.syntax.hash(&mut hash);
    text.slice(node.raw())
        .expect("fingerprint range")
        .hash(&mut hash);
    let mut identity = hash.finish();
    while identity == 0 || !used.insert(identity) {
        identity = identity.wrapping_add(1);
    }
    NodeId(identity)
}
type MappedNodes<'a> = HashMap<(ScalarOffset, ScalarOffset), Vec<&'a Node>>;
fn map_unchanged<'a>(nodes: &'a [Node], changes: &[TextChange], result: &mut MappedNodes<'a>) {
    for node in nodes {
        let affected = changes.iter().any(|change| {
            if node.raw.start == node.raw.end {
                change.old.start <= node.raw.start && node.raw.start < change.old.end
            } else if change.old.start == change.old.end {
                node.raw.start < change.old.start && change.old.start < node.raw.end
            } else {
                node.raw.start < change.old.end && change.old.start < node.raw.end
            }
        });
        if !affected {
            let start = map_position(node.raw.start, changes, true);
            let end = map_position(node.raw.end, changes, node.raw.start == node.raw.end);
            result.entry((start, end)).or_default().push(node);
        }
        map_unchanged(&node.children, changes, result);
    }
}
fn map_position(position: ScalarOffset, changes: &[TextChange], after: bool) -> ScalarOffset {
    let mut removed = 0;
    let mut added = 0;
    for change in changes {
        if change.old.end < position
            || (change.old.end == position && (change.old.start < change.old.end || after))
        {
            removed += change.old.end.value() - change.old.start.value();
            added += change.inserted.value();
        }
    }
    ScalarOffset::new(position.value() - removed + added)
}
struct IdentityReuse<'a> {
    old_text: &'a TextBuffer,
    new_text: &'a TextBuffer,
    old_nodes: MappedNodes<'a>,
    used: HashSet<u64>,
    reused: HashSet<NodeId>,
}
impl IdentityReuse<'_> {
    fn apply(&mut self, node: &mut Node, parent: NodeId, ordinal: usize) {
        let previous = self
            .old_nodes
            .get(&(node.raw.start, node.raw.end))
            .and_then(|nodes| {
                nodes.iter().find(|old| {
                    !self.reused.contains(&old.id)
                        && old.syntax == node.syntax
                        && self.old_text.slice(old.raw()).ok()
                            == self.new_text.slice(node.raw()).ok()
                })
            })
            .map(|old| old.id);
        node.id = if let Some(identity) = previous {
            self.reused.insert(identity);
            identity
        } else {
            fingerprint_id(node, parent, ordinal, self.new_text, &mut self.used)
        };
        for (ordinal, child) in node.children.iter_mut().enumerate() {
            self.apply(child, node.id, ordinal);
        }
    }
}
fn append(node: Node, stack: &mut [Node], roots: &mut Vec<Node>) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else {
        roots.push(node);
    }
}
fn trivia(text: &TextBuffer, raw: Range<ScalarOffset>, id: u64) -> Node {
    let value = text.slice(raw.clone()).expect("source gap");
    let syntax = if value.trim_start().starts_with('[') && value.contains("]:") {
        Syntax::ReferenceDefinition
    } else {
        Syntax::Trivia
    };
    Node {
        id: NodeId(id),
        syntax,
        content: raw.clone(),
        raw,
        children: Vec::new(),
        text: value,
        anchor: None,
    }
}
fn tag_syntax(tag: Tag<'_>) -> Syntax {
    match tag {
        Tag::Paragraph => Syntax::Paragraph,
        Tag::Heading { level, .. } => Syntax::Heading(level as u8),
        Tag::BlockQuote(callout) => callout.map_or(Syntax::Quote, |value| {
            Syntax::Callout(format!("{value:?}").to_uppercase())
        }),
        Tag::CodeBlock(CodeBlockKind::Fenced(info)) => Syntax::CodeBlock(info.to_string()),
        Tag::CodeBlock(_) => Syntax::CodeBlock(String::new()),
        Tag::HtmlBlock => Syntax::HtmlBlock,
        Tag::List(ordinal) => Syntax::List(ordinal),
        Tag::Item => Syntax::Item,
        Tag::FootnoteDefinition(label) => Syntax::FootnoteDefinition(label.to_string()),
        Tag::Table(columns) => Syntax::Table(columns.len()),
        Tag::TableHead => Syntax::TableHead,
        Tag::TableRow => Syntax::TableRow,
        Tag::TableCell => Syntax::TableCell,
        Tag::Emphasis => Syntax::Emphasis,
        Tag::Strong => Syntax::Strong,
        Tag::Strikethrough => Syntax::Strike,
        Tag::Superscript => Syntax::Superscript,
        Tag::Subscript => Syntax::Subscript,
        Tag::Link { dest_url, .. } => Syntax::Link(dest_url.to_string()),
        Tag::Image { dest_url, .. } => Syntax::Image(dest_url.to_string()),
        Tag::MetadataBlock(_) => Syntax::FrontMatter,
        _ => Syntax::Text,
    }
}
fn finish(node: &mut Node, text: &TextBuffer) {
    if !node.children.is_empty() {
        node.text = node
            .children
            .iter()
            .map(|child| child.text.as_str())
            .collect();
        node.content = node.children.first().expect("nonempty").raw.start
            ..node.children.last().expect("nonempty").raw.end;
    }
    let source = text.slice(node.raw()).expect("parser range");
    if node.children.is_empty() {
        let empty = match node.syntax {
            Syntax::Heading(_) => {
                let leading = source
                    .chars()
                    .take_while(|ch| *ch == ' ' || *ch == '\t')
                    .count();
                let hashes = source
                    .chars()
                    .skip(leading)
                    .take_while(|ch| *ch == '#')
                    .count();
                let spaces = source
                    .chars()
                    .skip(leading + hashes)
                    .take_while(|ch| *ch == ' ' || *ch == '\t')
                    .count();
                Some(ScalarOffset::new(
                    node.raw.start.value() + leading + hashes + spaces,
                ))
            }
            Syntax::CodeBlock(_) | Syntax::FrontMatter => {
                Some(source.find('\n').map_or(node.raw.end, |byte| {
                    ScalarOffset::new(node.raw.start.value() + source[..=byte].chars().count())
                }))
            }
            Syntax::Quote
            | Syntax::Callout(_)
            | Syntax::Item
            | Syntax::Rule
            | Syntax::Task(_)
            | Syntax::TableCell => Some(node.raw.end),
            Syntax::Link(_) if source.starts_with('[') => {
                Some(ScalarOffset::new(node.raw.start.value() + 1))
            }
            Syntax::Image(_) => Some(ScalarOffset::new(node.raw.start.value() + 2)),
            _ => None,
        };
        if let Some(empty) = empty {
            node.content = empty..empty;
        }
    }
    let trim = match node.syntax {
        Syntax::Code => source.chars().take_while(|c| *c == '`').count(),
        Syntax::Math => 1,
        Syntax::MathBlock => 2,
        Syntax::FootnoteReference(_) => 2,
        _ => 0,
    };
    if trim > 0 && node.raw.end.value() >= node.raw.start.value() + trim * 2 {
        let end_trim = if matches!(node.syntax, Syntax::FootnoteReference(_)) {
            1
        } else {
            trim
        };
        node.content = ScalarOffset::new(node.raw.start.value() + trim)
            ..ScalarOffset::new(node.raw.end.value() - end_trim);
    }
    if matches!(node.syntax, Syntax::Paragraph) && source.trim().eq_ignore_ascii_case("[toc]") {
        node.syntax = Syntax::Toc;
    }
}
fn assign_anchors(nodes: &mut [Node], counts: &mut HashMap<String, usize>) {
    for node in nodes {
        if matches!(node.syntax, Syntax::Heading(_)) {
            let base = heading_anchor(&node.text);
            let mut suffix = counts.get(&base).copied().unwrap_or(0);
            let mut candidate = base.clone();
            while counts.contains_key(&candidate) {
                suffix += 1;
                candidate = format!("{base}-{suffix}");
            }
            counts.insert(base, suffix);
            counts.insert(candidate.clone(), 0);
            node.anchor = Some(candidate);
        }
        assign_anchors(&mut node.children, counts);
    }
}
/// Normalizes reference labels by case folding and collapsing whitespace.
pub fn normalize_label(label: &str) -> String {
    caseless::default_case_fold_str(&label.split_whitespace().collect::<Vec<_>>().join(" "))
}
/// Produces the base of a heading anchor; Cst assigns duplicate suffixes.
pub fn heading_anchor(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| !c.is_punctuation())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

fn extend(node: &mut Node, text: &TextBuffer, extensions: Extensions) {
    let child_extensions = if matches!(node.syntax, Syntax::Link(_) | Syntax::Image(_)) {
        extensions.with(Extension::Autolinks, false)
    } else {
        extensions
    };
    for child in &mut node.children {
        extend(child, text, child_extensions);
    }
    if matches!(node.syntax, Syntax::Table(_))
        && !node
            .children
            .iter()
            .any(|child| child.syntax == Syntax::TableRow)
    {
        let source = text.slice(node.raw()).expect("table source");
        let fallback = Cst::parse_with_extensions(
            &TextBuffer::new(&source),
            extensions
                .with(Extension::Tables, false)
                .with(Extension::FrontMatter, false),
        );
        if let Some(mut paragraph) = fallback
            .nodes
            .iter()
            .find(|child| child.syntax != Syntax::Trivia)
            .cloned()
        {
            relocate(&mut paragraph, 0, node.raw.start.value());
            paragraph.raw = node.raw.clone();
            *node = paragraph;
        }
        return;
    }
    if matches!(node.syntax, Syntax::Math) && node.text.contains('\n') {
        node.syntax = Syntax::Text;
        node.content = node.raw.clone();
        node.text = text.slice(node.raw()).expect("source range");
    }
    if matches!(node.syntax, Syntax::MathBlock) {
        let source = text.slice(node.raw()).expect("source range");
        let mut lines = source.lines();
        if lines.next().map(str::trim) != Some("$$")
            || source.lines().last().map(str::trim) != Some("$$")
        {
            node.syntax = Syntax::Text;
            node.content = node.raw.clone();
            node.text = source;
        } else if let Some(value) = node.text.strip_prefix('\n') {
            node.text = value.to_owned();
            node.content.start = ScalarOffset::new(node.content.start.value() + 1);
        }
    }
    if matches!(node.syntax, Syntax::Paragraph)
        && node.children.len() == 1
        && matches!(node.children[0].syntax, Syntax::MathBlock)
    {
        node.syntax = Syntax::MathBlock;
        node.content = node.children[0].content.clone();
        node.text = node.children[0].text.clone();
        node.children.clear();
    }
    if matches!(node.syntax, Syntax::Toc) {
        if extensions.is_enabled(Extension::Toc) {
            node.content = node.raw.end..node.raw.end;
            node.text.clear();
            node.children.clear();
        } else {
            node.syntax = Syntax::Paragraph;
        }
    }
    if extensions.is_enabled(Extension::Callouts) && matches!(node.syntax, Syntax::Quote) {
        let source = text.slice(node.raw()).expect("source range");
        if let Some(marker) = source
            .lines()
            .next()
            .and_then(|line| line.trim_start().strip_prefix('>'))
            .map(str::trim)
            .and_then(|line| line.strip_prefix("[!"))
            .and_then(|line| line.strip_suffix(']'))
            && !marker.is_empty()
            && marker.chars().all(|ch| ch.is_ascii_alphabetic())
        {
            node.syntax = Syntax::Callout(marker.to_uppercase());
            if let Some(first) = node.children.first_mut()
                && first
                    .children
                    .first()
                    .is_some_and(|child| child.text == format!("[!{marker}]"))
            {
                first.children.remove(0);
                if first
                    .children
                    .first()
                    .is_some_and(|child| child.syntax == Syntax::SoftBreak)
                {
                    first.children.remove(0);
                }
                first.text = first
                    .children
                    .iter()
                    .map(|child| child.text.as_str())
                    .collect();
                first.content.start = first
                    .children
                    .first()
                    .map_or(first.raw.end, |child| child.raw.start);
            }
        }
    }
    if matches!(
        node.syntax,
        Syntax::Paragraph
            | Syntax::Heading(_)
            | Syntax::Emphasis
            | Syntax::Strong
            | Syntax::Strike
            | Syntax::TableCell
    ) {
        let mut children = Vec::new();
        for child in std::mem::take(&mut node.children) {
            if child.syntax == Syntax::Text
                && text
                    .slice(child.raw())
                    .is_ok_and(|source| source == child.text)
            {
                split_extensions(child, &mut children, extensions);
            } else {
                children.push(child);
            }
        }
        node.children = if extensions.is_enabled(Extension::Highlight) {
            group_highlights(children, text)
        } else {
            children
        };
    }
    if !node.children.is_empty() {
        node.text = node
            .children
            .iter()
            .map(|child| child.text.as_str())
            .collect();
    }
}
fn group_highlights(children: Vec<Node>, text: &TextBuffer) -> Vec<Node> {
    let mut tokens = Vec::new();
    for child in children {
        if child.syntax != Syntax::Text
            || !child.text.contains("==")
            || !text
                .slice(child.raw())
                .is_ok_and(|source| source == child.text)
        {
            tokens.push(child);
            continue;
        }
        let mut start = 0;
        let chars: Vec<char> = child.text.chars().collect();
        let mut cursor = 0;
        while cursor + 1 < chars.len() {
            if chars[cursor..].starts_with(&['=', '=']) {
                if start < cursor {
                    tokens.push(text_part(&child, start, cursor));
                }
                tokens.push(text_part(&child, cursor, cursor + 2));
                cursor += 2;
                start = cursor;
            } else {
                cursor += 1;
            }
        }
        if start < chars.len() {
            tokens.push(text_part(&child, start, chars.len()));
        }
    }
    let mut output = Vec::new();
    let mut open = None;
    for token in tokens {
        if token.syntax == Syntax::Text && token.text == "==" {
            if let Some(index) = open.take() {
                let inner: Vec<Node> = output.drain(index + 1..).collect();
                let opening: Node = output.pop().expect("highlight opener");
                if inner.is_empty() {
                    output.push(opening);
                    output.push(token);
                } else {
                    output.push(Node {
                        id: opening.id,
                        syntax: Syntax::Highlight,
                        raw: opening.raw.start..token.raw.end,
                        content: opening.raw.end..token.raw.start,
                        text: inner.iter().map(|child| child.text.as_str()).collect(),
                        children: inner,
                        anchor: None,
                    });
                }
            } else {
                open = Some(output.len());
                output.push(token);
            }
        } else {
            output.push(token);
        }
    }
    output
}
fn split_extensions(node: Node, result: &mut Vec<Node>, extensions: Extensions) {
    let chars: Vec<char> = node.text.chars().collect();
    let mut cursor = 0;
    let mut plain_start = 0;
    while cursor < chars.len() {
        let email_start = extensions.is_enabled(Extension::Autolinks)
            && is_email_local(chars[cursor])
            && (cursor == 0 || !is_email_local(chars[cursor - 1]));
        if !chars[cursor..].starts_with(&['=', '='])
            && !chars[cursor..].starts_with(&['h', 't', 't', 'p', ':', '/', '/'])
            && !chars[cursor..].starts_with(&['h', 't', 't', 'p', 's', ':', '/', '/'])
            && !email_start
        {
            cursor += 1;
            continue;
        }
        let tail: String = chars[cursor..].iter().collect();
        let extension = if let Some(stripped) = tail
            .strip_prefix("==")
            .filter(|_| extensions.is_enabled(Extension::Highlight))
        {
            stripped.find("==").filter(|end| *end > 0).map(|end| {
                let content = &stripped[..end];
                (
                    Syntax::Highlight,
                    content.chars().count() + 4,
                    2,
                    2,
                    content.to_owned(),
                )
            })
        } else if extensions.is_enabled(Extension::Autolinks)
            && (tail.starts_with("https://") || tail.starts_with("http://"))
        {
            let mut length = tail
                .chars()
                .take_while(|ch| !ch.is_whitespace() && !['<', '>', '"', '\''].contains(ch))
                .count();
            while length > 0 && ['.', ',', ';', ':', '!', '?'].contains(&chars[cursor + length - 1])
            {
                length -= 1;
            }
            let destination: String = chars[cursor..cursor + length].iter().collect();
            let opens = destination.chars().filter(|ch| *ch == '(').count();
            let mut closes = destination.chars().filter(|ch| *ch == ')').count();
            while closes > opens && length > 0 && chars[cursor + length - 1] == ')' {
                length -= 1;
                closes -= 1;
            }
            let destination: String = chars[cursor..cursor + length].iter().collect();
            let host = destination.split_once("://").map_or("", |(_, host)| host);
            host.chars()
                .next()
                .filter(|ch| ch.is_alphanumeric())
                .map(|_| {
                    (
                        Syntax::Link(destination.clone()),
                        length,
                        0,
                        0,
                        destination.clone(),
                    )
                })
        } else if email_start {
            email_length(&tail).map(|length| {
                let value = tail[..length].to_owned();
                (
                    Syntax::Link(format!("mailto:{value}")),
                    value.chars().count(),
                    0,
                    0,
                    value,
                )
            })
        } else {
            None
        };
        if let Some((syntax, length, opening, closing, value)) = extension {
            if plain_start < cursor {
                result.push(text_part(&node, plain_start, cursor));
            }
            let mut part = text_part(&node, cursor, cursor + length);
            part.syntax = syntax;
            part.content = ScalarOffset::new(part.raw.start.value() + opening)
                ..ScalarOffset::new(part.raw.end.value() - closing);
            part.text = value;
            result.push(part);
            cursor += length;
            plain_start = cursor;
        } else {
            cursor += 1;
        }
    }
    if plain_start < chars.len() {
        result.push(text_part(&node, plain_start, chars.len()));
    }
}
fn is_email_local(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ['.', '_', '+', '-'].contains(&ch)
}
fn email_length(tail: &str) -> Option<usize> {
    let local = tail
        .bytes()
        .take_while(|byte| is_email_local(char::from(*byte)))
        .count();
    if local == 0 || tail.as_bytes().get(local) != Some(&b'@') {
        return None;
    }
    let domain_start = local + 1;
    let mut domain_len = tail[domain_start..]
        .bytes()
        .take_while(|byte| byte.is_ascii_alphanumeric() || b".-".contains(byte))
        .count();
    while domain_len > 0 && tail.as_bytes()[domain_start + domain_len - 1] == b'.' {
        domain_len -= 1;
    }
    let domain = &tail[domain_start..domain_start + domain_len];
    if !domain.contains('.')
        || !domain.split('.').all(|part| {
            !part.is_empty()
                && part.as_bytes()[0].is_ascii_alphanumeric()
                && part.as_bytes()[part.len() - 1].is_ascii_alphanumeric()
        })
    {
        return None;
    }
    Some(domain_start + domain_len)
}
fn text_part(node: &Node, start: usize, end: usize) -> Node {
    let mut part = node.clone();
    part.raw = ScalarOffset::new(node.raw.start.value() + start)
        ..ScalarOffset::new(node.raw.start.value() + end);
    part.content = part.raw.clone();
    part.text = node.text.chars().skip(start).take(end - start).collect();
    part
}

fn events(source: &str, options: Options) -> impl Iterator<Item = (Event<'static>, Range<usize>)> {
    // Block scanning needs expanded column-zero tabs; every event maps back to untouched source.
    let mut shadow = String::new();
    let mut positions = vec![0];
    let mut line_start = true;
    for (byte, ch) in source.char_indices() {
        if line_start && ch == '\t' {
            shadow.push_str("    ");
            positions.extend([byte, byte, byte, byte + 1]);
        } else {
            shadow.push(ch);
            positions.extend((1..=ch.len_utf8()).map(|part| byte + part));
        }
        line_start = ch == '\n';
    }
    // Inline code must retain literal tabs after container continuation prefixes.
    let original_code: HashMap<_, _> = if shadow != source {
        Parser::new_ext(source, options)
            .into_offset_iter()
            .filter_map(|(event, range)| match event {
                Event::Code(value) => Some(((range.start, range.end), value.into_static())),
                _ => None,
            })
            .collect()
    } else {
        HashMap::new()
    };
    let mut in_code = false;
    let mut heading_end = None;
    let mut code_has_text = false;
    let mut code_has_newline = true;
    let mut output = Vec::new();
    for (event, shadow_range) in Parser::new_ext(&shadow, options).into_offset_iter() {
        let mut range = positions[shadow_range.start]..positions[shadow_range.end];
        let original = &source[range.clone()];
        let event = match event {
            Event::Code(value) => Event::Code(
                original_code
                    .get(&(range.start, range.end))
                    .cloned()
                    .unwrap_or_else(|| value.into_static()),
            ),
            Event::Text(value) if value.as_ref() == &shadow[shadow_range.clone()] => {
                Event::Text(original.replace('\0', "\u{fffd}").into())
            }
            Event::Html(value) if value.as_ref() == &shadow[shadow_range.clone()] => {
                Event::Html(original.to_owned().into())
            }
            Event::InlineHtml(value) if value.as_ref() == &shadow[shadow_range.clone()] => {
                Event::InlineHtml(original.to_owned().into())
            }
            Event::HardBreak
                if !original.trim_end_matches(['\r', '\n']).starts_with('\\')
                    && !original.trim_end_matches(['\r', '\n']).ends_with("  ") =>
            {
                Event::SoftBreak
            }
            event => event.into_static(),
        };
        let event = match event {
            Event::Start(Tag::Heading { .. }) => {
                heading_end = Some(range.end);
                event
            }
            Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                heading_end = None;
                event
            }
            Event::Text(value)
                if heading_end.is_some_and(|end| {
                    source[range.end..end].trim().chars().all(|ch| ch == '#')
                }) =>
            {
                let trimmed = value.trim_end_matches([' ', '\t']);
                range.end -= value.len() - trimmed.len();
                Event::Text(trimmed.to_owned().into())
            }
            event => event,
        };
        match &event {
            Event::Start(Tag::CodeBlock(_)) => {
                in_code = true;
                code_has_text = false;
                code_has_newline = true;
            }
            Event::Text(value) if in_code && !value.is_empty() => {
                code_has_text = true;
                code_has_newline = value.ends_with('\n');
            }
            Event::End(pulldown_cmark::TagEnd::CodeBlock) => {
                // CommonMark gives nonempty code a semantic terminal LF even when source ends at EOF.
                if code_has_text && !code_has_newline {
                    output.push((Event::Text("\n".into()), range.end..range.end));
                }
                in_code = false;
            }
            _ => {}
        }
        output.push((event, range));
    }
    output.into_iter()
}
#[cfg(test)]
mod conformance;
