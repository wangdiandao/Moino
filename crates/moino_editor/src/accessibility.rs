use gpui_kit::base::text::editor_host::LeafGeometry;
use gpui_kit::{
    A11ySubtreeBuilder,
    accesskit::{Node, NodeId, Rect, Role, TextPosition, TextSelection},
};
use std::{collections::HashMap, ops::Range};

pub(super) fn build_runs(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    selection: &Range<usize>,
    reversed: bool,
    geometry: &LeafGeometry,
) -> HashMap<NodeId, Range<usize>> {
    let mut lines: Vec<Range<usize>> = Vec::new();
    let mut start = 0;
    let mut y = geometry.position_for_index(0).y;
    for (byte, _) in text.char_indices() {
        let next_y = geometry.position_for_index(byte).y;
        if next_y != y && byte > start {
            lines.push(start..byte);
            start = byte;
            y = next_y;
        }
    }
    lines.push(start..text.len());
    let mut runs = HashMap::new();
    let mut anchor = None;
    let mut focus = None;
    for (line_index, range) in lines.iter().enumerate() {
        let id = builder.synthetic_node_id(("document-line", line_index));
        let value = &text[range.clone()];
        let mut node = Node::new(Role::TextRun);
        node.set_value(value);
        node.set_character_lengths(
            value
                .chars()
                .map(|ch| u8::try_from(ch.len_utf8()).expect("UTF-8 scalar is at most four bytes"))
                .collect::<Vec<_>>(),
        );
        let origin = geometry.position_for_index(range.start);
        let bounds = geometry.bounds();
        node.set_bounds(Rect {
            x0: f64::from(f32::from(origin.x)),
            y0: f64::from(f32::from(origin.y)),
            x1: f64::from(f32::from(bounds.right())),
            y1: f64::from(f32::from(origin.y + geometry.line_height())),
        });
        let position = |offset| {
            let byte = crate::byte_from_utf16(text, offset)?;
            if byte >= range.start
                && (byte < range.end || line_index + 1 == lines.len() && byte == range.end)
            {
                Some(TextPosition {
                    node: id,
                    character_index: text[range.start..byte].chars().count(),
                })
            } else {
                None
            }
        };
        anchor = anchor.or_else(|| {
            position(if reversed {
                selection.end
            } else {
                selection.start
            })
        });
        focus = focus.or_else(|| {
            position(if reversed {
                selection.start
            } else {
                selection.end
            })
        });
        builder.push_child(id, node);
        runs.insert(id, range.clone());
    }
    if let (Some(anchor), Some(focus)) = (anchor, focus) {
        builder
            .parent_node()
            .set_text_selection(TextSelection { anchor, focus });
    }
    runs
}

pub(super) fn native_selection(
    text: &str,
    selection: &TextSelection,
    runs: &HashMap<NodeId, Range<usize>>,
) -> Option<(Range<usize>, bool)> {
    let offset = |position: TextPosition| {
        let range = runs.get(&position.node)?;
        let value = text.get(range.clone())?;
        let byte = value
            .char_indices()
            .map(|(byte, _)| byte)
            .chain(std::iter::once(value.len()))
            .nth(position.character_index)?;
        Some(text[..range.start + byte].encode_utf16().count())
    };
    let anchor = offset(selection.anchor)?;
    let focus = offset(selection.focus)?;
    Some((anchor.min(focus)..anchor.max(focus), anchor > focus))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn accessible_scalar_positions_convert_to_native_utf16_and_reject_unknown_runs() {
        let id = NodeId(1);
        let runs = HashMap::from([(id, 0.."中😀".len())]);
        let selection = TextSelection {
            anchor: TextPosition {
                node: id,
                character_index: 1,
            },
            focus: TextPosition {
                node: id,
                character_index: 2,
            },
        };
        assert_eq!(
            native_selection("中😀", &selection, &runs),
            Some((1..3, false))
        );
        let backward = TextSelection {
            anchor: selection.focus,
            focus: selection.anchor,
        };
        assert_eq!(
            native_selection("中😀", &backward, &runs),
            Some((1..3, true))
        );
        assert!(native_selection("中😀", &selection, &HashMap::new()).is_none());
    }
}
