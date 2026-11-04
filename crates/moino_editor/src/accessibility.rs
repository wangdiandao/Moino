//! Accessible text runs retain the same visible-to-source boundaries as native input.
use crate::native_input::native_range;
use crate::projection::Leaf;
use gpui_kit::base::text::editor_host::LeafGeometry;
use gpui_kit::{
    A11ySubtreeBuilder, UTF16Selection,
    accesskit::{Node, NodeId, Rect, Role, TextPosition, TextSelection},
};
use moino_text::TextBuffer;
use std::{collections::HashMap, ops::Range};

/// A published accessibility frame. Old text positions cannot target new text.
#[derive(Default)]
pub(super) struct TextRuns {
    text: String,
    positions: HashMap<NodeId, Vec<usize>>,
}

struct VisibleText {
    key: u64,
    text: String,
    positions: Vec<usize>,
}

/// Immutable render capture; source positions include hidden-marker and preedit mapping.
pub(super) struct ProjectedText {
    text: String,
    selection: UTF16Selection,
    leaves: Vec<VisibleText>,
}

impl ProjectedText {
    pub(super) fn new(text: &TextBuffer, selection: UTF16Selection, leaves: &[Leaf]) -> Self {
        let leaves = leaves
            .iter()
            .filter_map(|leaf| {
                let positions = leaf
                    .text
                    .char_indices()
                    .map(|(byte, _)| byte)
                    .chain(std::iter::once(leaf.text.len()))
                    .map(|byte| {
                        text.to_utf16(leaf.source(byte))
                            .ok()
                            .map(|offset| offset.value())
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(VisibleText {
                    key: leaf.key,
                    text: leaf.text.clone(),
                    positions,
                })
            })
            .collect();
        Self {
            text: text.to_string(),
            selection,
            leaves,
        }
    }

    pub(super) fn build(
        &self,
        builder: &mut A11ySubtreeBuilder,
        geometry: &HashMap<u64, LeafGeometry>,
    ) -> TextRuns {
        let mut runs = TextRuns {
            text: self.text.clone(),
            positions: HashMap::new(),
        };
        let mut ordered = Vec::new();
        for leaf in &self.leaves {
            let geometry = geometry.get(&leaf.key);
            let lines = geometry.map_or_else(
                || std::iter::once(0..leaf.text.len()).collect(),
                |geometry| line_ranges(&leaf.text, geometry),
            );
            for range in lines {
                let value = &leaf.text[range.clone()];
                let start = leaf.text[..range.start].chars().count();
                let end = start + value.chars().count();
                let positions = leaf.positions[start..=end].to_vec();
                // Reflow or syntax projection must not reuse a node with different offsets.
                let id = builder.synthetic_node_id(("document-line", leaf.key, &positions));
                let mut node = Node::new(Role::TextRun);
                node.set_value(value);
                node.set_character_lengths(
                    value
                        .chars()
                        .map(|ch| {
                            u8::try_from(ch.len_utf8()).expect("UTF-8 scalar is at most four bytes")
                        })
                        .collect::<Vec<_>>(),
                );
                if let Some(geometry) = geometry {
                    let origin = geometry.position_for_index(range.start);
                    let end_position = geometry.position_for_index(range.end);
                    let right = if end_position.y == origin.y {
                        end_position.x
                    } else {
                        geometry.bounds().right()
                    };
                    node.set_bounds(Rect {
                        x0: f64::from(f32::from(origin.x)),
                        y0: f64::from(f32::from(origin.y)),
                        x1: f64::from(f32::from(right.max(origin.x))),
                        y1: f64::from(f32::from(origin.y + geometry.line_height())),
                    });
                }
                ordered.extend(
                    positions
                        .iter()
                        .enumerate()
                        .map(|(character_index, offset)| {
                            (
                                *offset,
                                TextPosition {
                                    node: id,
                                    character_index,
                                },
                            )
                        }),
                );
                runs.positions.insert(id, positions);
                builder.push_child(id, node);
            }
        }
        if native_range(&TextBuffer::new(&self.text), self.selection.range.clone()).is_some() {
            let position = |offset: usize| {
                ordered
                    .iter()
                    .min_by_key(|(source, _)| source.abs_diff(offset))
                    .map(|(_, at)| *at)
            };
            let (anchor, focus) = if self.selection.reversed {
                (self.selection.range.end, self.selection.range.start)
            } else {
                (self.selection.range.start, self.selection.range.end)
            };
            if let (Some(anchor), Some(focus)) = (position(anchor), position(focus)) {
                builder
                    .parent_node()
                    .set_text_selection(TextSelection { anchor, focus });
            }
        }
        runs
    }
}

fn line_ranges(text: &str, geometry: &LeafGeometry) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut y = geometry.position_for_index(0).y;
    for byte in text
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
    {
        let next_y = geometry.position_for_index(byte).y;
        if next_y != y && byte > start {
            lines.push(start..byte);
            start = byte;
            y = next_y;
        }
    }
    lines.push(start..text.len());
    lines
}

pub(super) fn build_runs(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    selection: &Range<usize>,
    reversed: bool,
    geometry: &LeafGeometry,
) -> TextRuns {
    let mut units = 0;
    let mut positions = vec![0];
    for ch in text.chars() {
        units += ch.len_utf16();
        positions.push(units);
    }
    ProjectedText {
        text: text.to_owned(),
        selection: UTF16Selection {
            range: selection.clone(),
            reversed,
        },
        leaves: vec![VisibleText {
            key: 1,
            text: text.to_owned(),
            positions,
        }],
    }
    .build(builder, &HashMap::from([(1, geometry.clone())]))
}

pub(super) fn native_selection(
    text: &str,
    selection: &TextSelection,
    runs: &TextRuns,
) -> Option<(Range<usize>, bool)> {
    if runs.text != text {
        return None;
    }
    let offset = |position: TextPosition| {
        runs.positions
            .get(&position.node)?
            .get(position.character_index)
            .copied()
    };
    let anchor = offset(selection.anchor)?;
    let focus = offset(selection.focus)?;
    let range = anchor.min(focus)..anchor.max(focus);
    native_range(&TextBuffer::new(text), range.clone())?;
    Some((range, anchor > focus))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn accessible_positions_preserve_direction_and_reject_stale_or_unknown_runs() {
        let id = NodeId(1);
        let runs = TextRuns {
            text: "中😀".into(),
            positions: HashMap::from([(id, vec![0, 1, 3])]),
        };
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
        assert!(native_selection("a😀", &selection, &runs).is_none());
        assert!(native_selection("中😀", &selection, &TextRuns::default()).is_none());
        let invalid = TextSelection {
            focus: TextPosition {
                node: id,
                character_index: 3,
            },
            ..selection
        };
        assert!(native_selection("中😀", &invalid, &runs).is_none());
    }

    #[test]
    // Spec: docs/modules/selection.md#标记展开与折叠
    fn accessible_projected_boundaries_skip_hidden_heading_markers() {
        let document = moino_document::Document::new(
            moino_document::DocumentId::new(1),
            moino_document::Epoch::new(0),
            "# 😀中",
        );
        let snapshot = document.snapshot();
        let leaves = crate::projection::project(
            &snapshot,
            document.selection(),
            moino_theme::BuiltinTheme::Paper,
        );
        let projected = ProjectedText::new(
            snapshot.text(),
            UTF16Selection {
                range: 0..0,
                reversed: false,
            },
            &leaves,
        );
        assert_eq!(projected.leaves[0].text, "😀中");
        assert_eq!(projected.leaves[0].positions, vec![2, 4, 5]);
    }
}
