//! Container traversal and retained leaf state have separate responsibilities.
use super::inline::{InlineCompiler, Style};
use super::*;

pub(super) struct Row<'a> {
    node: &'a Node,
    wrappers: Vec<Wrapper>,
    pub raw: Range<ScalarOffset>,
}
pub(super) fn rows(nodes: &[Node]) -> Vec<Row<'_>> {
    let mut output = Vec::new();
    visit(nodes, &[], &mut output);
    output
}
fn visit<'a>(nodes: &'a [Node], wrappers: &[Wrapper], output: &mut Vec<Row<'a>>) {
    for node in nodes {
        match node.syntax() {
            Syntax::Quote | Syntax::Callout(_) => {
                let mut wrappers = wrappers.to_vec();
                wrappers.push(Wrapper::Quote);
                if node.children().is_empty() {
                    output.push(Row {
                        node,
                        wrappers,
                        raw: node.raw(),
                    });
                } else {
                    visit(node.children(), &wrappers, output);
                }
            }
            Syntax::List(start) => {
                for (index, item) in node.children().iter().enumerate() {
                    let checked = item
                        .children()
                        .iter()
                        .chain(item.children().iter().flat_map(Node::children))
                        .find_map(|node| {
                            if let Syntax::Task(checked) = node.syntax() {
                                Some(*checked)
                            } else {
                                None
                            }
                        });
                    let mut wrappers = wrappers.to_vec();
                    wrappers.push(Wrapper::List(
                        start.and_then(|start| start.checked_add(u64::try_from(index).ok()?)),
                        checked,
                    ));
                    visit(std::slice::from_ref(item), &wrappers, output);
                }
            }
            Syntax::Item => {
                let children = node.children();
                let first_block = children
                    .iter()
                    .position(|child| is_block(child.syntax()))
                    .unwrap_or(children.len());
                let inline_row = first_block == children.len()
                    || children[..first_block]
                        .iter()
                        .any(|child| !matches!(child.syntax(), Syntax::Task(_)))
                    || children
                        .first()
                        .is_some_and(|child| matches!(child.syntax(), Syntax::List(_)));
                let mut continuation = wrappers.to_vec();
                if let Some(wrapper) = continuation
                    .iter_mut()
                    .rev()
                    .find(|wrapper| matches!(wrapper, Wrapper::List(_, _)))
                {
                    *wrapper = Wrapper::Indent;
                }
                if inline_row {
                    output.push(Row {
                        node,
                        wrappers: wrappers.to_vec(),
                        raw: node.raw().start
                            ..children
                                .get(first_block)
                                .map_or(node.raw().end, |child| child.raw().start),
                    });
                }
                for (index, child) in children[first_block..].iter().enumerate() {
                    visit(
                        std::slice::from_ref(child),
                        if inline_row || index > 0 {
                            &continuation
                        } else {
                            wrappers
                        },
                        output,
                    );
                }
            }
            _ => output.push(Row {
                node,
                wrappers: wrappers.to_vec(),
                raw: node.raw(),
            }),
        }
    }
}
fn is_block(syntax: &Syntax) -> bool {
    matches!(
        syntax,
        Syntax::Paragraph
            | Syntax::Heading(_)
            | Syntax::CodeBlock(_)
            | Syntax::Quote
            | Syntax::Callout(_)
            | Syntax::List(_)
            | Syntax::Rule
            | Syntax::MathBlock
            | Syntax::Table(_)
            | Syntax::HtmlBlock
            | Syntax::FrontMatter
            | Syntax::FootnoteDefinition(_)
            | Syntax::ReferenceDefinition
            | Syntax::Toc
    )
}
pub(super) fn compile(
    row: Row<'_>,
    source: &[char],
    selected: Selection,
    theme: moino_theme::BuiltinTheme,
    pending: bool,
) -> Leaf {
    let Row {
        node,
        wrappers,
        raw,
    } = row;
    let mut compiler = InlineCompiler::new(source, selected, theme);
    let style = Style::default();
    let end = editable_end(source, &raw);
    let heading = if let Syntax::Heading(level) = node.syntax() {
        Some(*level)
    } else {
        None
    };
    let committed_heading = heading.filter(|_| !pending);
    let rule = matches!(node.syntax(), Syntax::Rule) && !pending && !compiler.active(&raw);
    let children: Vec<_> = node
        .children()
        .iter()
        .filter(|child| !matches!(child.syntax(), Syntax::Task(_)) && child.raw().end <= raw.end)
        .collect();
    let range = if heading.is_some() {
        node.content()
    } else if matches!(node.syntax(), Syntax::Item) {
        children.first().map_or(end, |node| node.raw().start)..end
    } else {
        raw.start..end
    };
    let empty_container = node.children().is_empty()
        && matches!(
            node.syntax(),
            Syntax::Quote | Syntax::Callout(_) | Syntax::Item
        );
    if pending && heading.is_some() {
        let atx = source[raw.start.value()..end.value()]
            .iter()
            .find(|ch| !ch.is_whitespace())
            == Some(&'#');
        if atx {
            compiler.marker(raw.start..range.start, &style);
            compiler.children(node.children(), range.clone(), &style.clone().bold(), false);
            if range.is_empty() {
                compiler.source(range.clone(), &style);
            }
            compiler.marker(range.end..end, &style);
        } else {
            compiler.children(node.children(), range.clone(), &style, false);
            compiler.source(range.end..end, &style);
        }
    } else if empty_container || rule {
        compiler.source(end..end, &style);
    } else if matches!(
        node.syntax(),
        Syntax::Paragraph | Syntax::Heading(_) | Syntax::Item
    ) {
        compiler.children(node.children(), range.clone(), &style, false);
        if range.is_empty() {
            compiler.source(range.clone(), &style);
        }
    } else {
        compiler.source(raw.start..end, &style);
    }
    let badge = committed_heading.filter(|_| compiler.active(&raw));
    let (paragraph, text, boundaries) = compiler.finish(node.id().value(), end);
    let content = if pending && heading.is_some() {
        None
    } else if committed_heading.is_some() {
        Some(node.content())
    } else if matches!(node.syntax(), Syntax::Paragraph | Syntax::Item) && !children.is_empty() {
        Some(children[0].raw().start..children.last().expect("children").raw().end)
    } else if text.is_empty() {
        let at = boundaries[0].1;
        Some(at..at)
    } else {
        None
    };
    let hidden_prefixes = children
        .windows(2)
        .filter_map(|nodes| {
            let range = nodes[0].raw().end..nodes[1].raw().start;
            (!range.is_empty()).then_some(range)
        })
        .collect();
    let mut block = if let Some(level) = committed_heading {
        BlockNode::Heading {
            level,
            children: paragraph,
            span: None,
        }
    } else {
        BlockNode::Paragraph(paragraph)
    };
    let split = wrappers
        .iter()
        .rposition(|wrapper| matches!(wrapper, Wrapper::Indent))
        .map_or(0, |index| index + 1);
    for wrapper in wrappers[split..].iter().rev() {
        block = match wrapper {
            Wrapper::Quote => BlockNode::Blockquote {
                children: vec![block],
                span: None,
            },
            Wrapper::List(start, checked) => BlockNode::List {
                ordered: start.is_some(),
                start: start.and_then(|value| u32::try_from(value).ok()),
                span: None,
                children: vec![BlockNode::ListItem {
                    children: vec![block],
                    spread: false,
                    checked: *checked,
                    span: None,
                }],
            },
            Wrapper::Indent => unreachable!("continuation prefix"),
        };
    }
    Leaf {
        key: node.id().value(),
        block,
        text,
        raw,
        heading_badge: badge,
        prefix: wrappers[..split].to_vec(),
        rule,
        content,
        hidden_prefixes,
        boundaries,
    }
}
