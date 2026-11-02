//! Exact byte edit windows supplied by the native source editor.
use gpui_kit::component::input::InputEdit;
use moino_text::ByteOffset;

#[derive(Clone, Copy, Debug)]
pub(super) struct SourceChange {
    pub start: ByteOffset,
    pub old_end: ByteOffset,
    pub new_end: ByteOffset,
}

impl SourceChange {
    /// Combines sequential replacements, including overlapping native preedit updates.
    pub fn include(previous: Option<Self>, edit: InputEdit) -> Self {
        let Some(previous) = previous else {
            return Self {
                start: ByteOffset::new(edit.start_byte),
                old_end: ByteOffset::new(edit.old_end_byte),
                new_end: ByteOffset::new(edit.new_end_byte),
            };
        };
        let old_end = if edit.old_end_byte >= previous.new_end.value() {
            previous.old_end.value() + edit.old_end_byte - previous.new_end.value()
        } else if edit.old_end_byte > previous.start.value() {
            previous.old_end.value()
        } else {
            edit.old_end_byte
        };
        Self {
            start: previous.start.min(ByteOffset::new(edit.start_byte)),
            old_end: previous.old_end.max(ByteOffset::new(old_end)),
            new_end: ByteOffset::new(
                edit.new_end_byte + previous.new_end.value().max(edit.old_end_byte)
                    - edit.old_end_byte,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SourceChange;
    use gpui_kit::component::input::{InputEdit, Point};

    fn edit(start: usize, end: usize, inserted: &str) -> InputEdit {
        InputEdit {
            start_byte: start,
            old_end_byte: end,
            new_end_byte: start + inserted.len(),
            start_position: Point::new(0, start),
            old_end_position: Point::new(0, end),
            new_end_position: Point::new(0, start + inserted.len()),
        }
    }

    #[test]
    // Spec: docs/contracts/markdown-dialect.md#增量解析与节点身份
    fn sequential_edit_windows_reconstruct_overlapping_and_disjoint_unicode_replacements() {
        let original = "abcdef";
        for start in 0..=original.len() {
            for end in start..=original.len() {
                for inserted in ["", "X", "中"] {
                    let mut intermediate = original.to_owned();
                    intermediate.replace_range(start..end, inserted);
                    let first = SourceChange::include(None, edit(start, end, inserted));
                    let boundaries: Vec<_> = intermediate
                        .char_indices()
                        .map(|(byte, _)| byte)
                        .chain([intermediate.len()])
                        .collect();
                    for &next_start in &boundaries {
                        for &next_end in boundaries.iter().filter(|&&end| end >= next_start) {
                            for inserted in ["", "Y", "😀"] {
                                let mut final_text = intermediate.clone();
                                final_text.replace_range(next_start..next_end, inserted);
                                let combined = SourceChange::include(
                                    Some(first),
                                    edit(next_start, next_end, inserted),
                                );
                                let mut reconstructed = original.to_owned();
                                reconstructed.replace_range(
                                    combined.start.value()..combined.old_end.value(),
                                    &final_text[combined.start.value()..combined.new_end.value()],
                                );
                                assert_eq!(reconstructed, final_text);
                            }
                        }
                    }
                }
            }
        }
    }
}
