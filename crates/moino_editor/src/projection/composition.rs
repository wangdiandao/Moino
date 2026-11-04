//! Splices preedit into projected runs without reparsing the document.
use super::*;

/// Overlays native preedit on its affected leaves without reparsing temporary text.
pub(crate) fn compose(mut leaves: Vec<Leaf>, range: Range<ScalarOffset>, text: &str) -> Vec<Leaf> {
    let first = leaves
        .iter()
        .rposition(|leaf| leaf.raw.start <= range.start)
        .unwrap_or(0);
    let last = leaves
        .iter()
        .rposition(|leaf| leaf.raw.start < range.end)
        .unwrap_or(first)
        .max(first);
    let before = &leaves[first];
    let after = &leaves[last];
    let start = before.byte(range.start, Affinity::After);
    let end = after.byte(range.end, Affinity::Before);
    let prefix = &before.text[..start];
    let suffix = &after.text[end..];
    let visible = format!("{prefix}{text}{suffix}");
    let inserted = text.chars().count();
    let shift = |at: ScalarOffset| {
        if at >= range.end {
            ScalarOffset::new(at.value() - range.end.value() + range.start.value() + inserted)
        } else if at >= range.start {
            ScalarOffset::new(range.start.value() + inserted)
        } else {
            at
        }
    };
    let mut boundaries = Vec::new();
    for (byte, _) in prefix.char_indices() {
        boundaries.push((byte, before.source(byte)));
    }
    for (scalar, (byte, _)) in text.char_indices().enumerate() {
        boundaries.push((
            start + byte,
            ScalarOffset::new(range.start.value() + scalar),
        ));
    }
    for (byte, _) in suffix.char_indices() {
        boundaries.push((start + text.len() + byte, shift(after.source(end + byte))));
    }
    boundaries.push((visible.len(), shift(after.source(after.text.len()))));
    let mut replacement = Leaf::plain(&visible, Some(start..start + text.len()));
    replacement.key = before.key;
    replacement.raw = before.raw.start..shift(after.raw.end);
    replacement.boundaries = boundaries;
    replacement.heading_badge = before.heading_badge;
    replacement.prefix = before.prefix.clone();
    replacement.content = before.content.as_ref().map(|content| {
        let start = if content.start <= range.start {
            content.start
        } else if content.start <= range.end {
            range.start
        } else {
            shift(content.start)
        };
        start..shift(content.end)
    });
    replacement.hidden_prefixes = before
        .hidden_prefixes
        .iter()
        .chain(after.hidden_prefixes.iter().filter(|_| first != last))
        .filter(|hidden| hidden.end <= range.start || hidden.start >= range.end)
        .map(|hidden| shift(hidden.start)..shift(hidden.end))
        .collect();
    let mut paragraph = Paragraph::default().with_leaf_key(before.key);
    append_slice(&mut paragraph, &before.block, 0..start);
    paragraph.push(
        InlineNode::new(text.to_owned())
            .marks(vec![(0..text.len(), TextMark::default().underline())]),
    );
    append_slice(&mut paragraph, &after.block, end..after.text.len());
    fn replace(block: &mut BlockNode, paragraph: &Paragraph) {
        match block {
            BlockNode::Paragraph(value)
            | BlockNode::Heading {
                children: value, ..
            } => *value = paragraph.clone(),
            BlockNode::Blockquote { children, .. }
            | BlockNode::List { children, .. }
            | BlockNode::ListItem { children, .. } => {
                for child in children {
                    replace(child, paragraph);
                }
            }
            _ => {}
        }
    }
    let mut block = before.block.clone();
    replace(&mut block, &paragraph.with_leaf_key(before.key));
    replacement.block = block;
    for leaf in &mut leaves[last + 1..] {
        leaf.raw = shift(leaf.raw.start)..shift(leaf.raw.end);
        leaf.content = leaf
            .content
            .as_ref()
            .map(|range| shift(range.start)..shift(range.end));
        leaf.hidden_prefixes = leaf
            .hidden_prefixes
            .iter()
            .map(|range| shift(range.start)..shift(range.end))
            .collect();
        for (_, offset) in &mut leaf.boundaries {
            *offset = shift(*offset);
        }
    }
    leaves.splice(first..=last, [replacement]);
    leaves
}

fn append_slice(output: &mut Paragraph, block: &BlockNode, range: Range<usize>) {
    match block {
        BlockNode::Paragraph(paragraph)
        | BlockNode::Heading {
            children: paragraph,
            ..
        } => {
            let mut offset = 0;
            for run in paragraph.children() {
                let start = range.start.saturating_sub(offset).min(run.text().len());
                let end = range.end.saturating_sub(offset).min(run.text().len());
                if start < end {
                    let marks = run
                        .mark_runs()
                        .iter()
                        .filter_map(|(range, mark)| {
                            let left = range.start.max(start);
                            let right = range.end.min(end);
                            (left < right).then(|| (left - start..right - start, mark.clone()))
                        })
                        .collect();
                    let mut slice = InlineNode::new(run.text()[start..end].to_owned()).marks(marks);
                    if let Some(color) = run.foreground() {
                        slice = slice.with_foreground(color);
                    }
                    if let Some(scale) = run.font_size_scale() {
                        slice = slice.with_font_size_scale(scale);
                    }
                    if let Some(shift) = run.baseline_shift() {
                        slice = slice.with_baseline_shift(shift);
                    }
                    output.push(slice);
                }
                offset += run.text().len();
            }
        }
        BlockNode::Blockquote { children, .. }
        | BlockNode::List { children, .. }
        | BlockNode::ListItem { children, .. } => {
            for child in children {
                append_slice(output, child, range.clone());
            }
        }
        _ => {}
    }
}
