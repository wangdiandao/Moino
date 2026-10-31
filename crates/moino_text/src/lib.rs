//! L0 normalized text, explicit offset conversion and Unicode boundaries; no internal dependencies.
#![forbid(unsafe_code)]

use std::{fmt, ops::Range};
use unicode_segmentation::UnicodeSegmentation;

macro_rules! offset {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(usize);
        impl $name {
            /// Constructs an offset; validate it against a buffer before use.
            pub const fn new(value: usize) -> Self {
                Self(value)
            }
            /// Returns the count in this offset's unit.
            pub const fn value(self) -> usize {
                self.0
            }
        }
    };
}
offset!(
    ScalarOffset,
    "A Unicode scalar position, independent of UTF-8 and UTF-16 encoding."
);
offset!(
    ByteOffset,
    "A UTF-8 byte position, required to lie on a scalar boundary."
);
offset!(
    Utf16Offset,
    "A UTF-16 code unit position, required not to split a surrogate pair."
);

/// A position or range does not belong to the current text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("text.invalid_offset")]
pub struct OffsetError;

impl OffsetError {
    /// Stable diagnostic code.
    pub const fn code(self) -> &'static str {
        "text.invalid_offset"
    }
    /// Localization key for the position diagnostic.
    pub const fn message_key(self) -> &'static str {
        "error.text.invalid_offset"
    }
    /// Invalid positions require a new input rather than retrying the same operation.
    pub const fn is_retryable(self) -> bool {
        false
    }
}

/// Normalizes CRLF and bare CR to LF without changing other Unicode separators.
pub fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// A cheap-to-clone rope snapshot containing only LF-normalized UTF-8 text.
#[derive(Clone, Debug)]
pub struct TextBuffer {
    rope: ropey::Rope,
}

impl fmt::Display for TextBuffer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.rope, formatter)
    }
}

impl TextBuffer {
    /// Creates a normalized buffer.
    pub fn new(text: &str) -> Self {
        Self {
            rope: ropey::Rope::from_str(&normalize_newlines(text)),
        }
    }
    /// Counts Unicode scalar values.
    pub fn len(&self) -> ScalarOffset {
        ScalarOffset(self.rope.len_chars())
    }
    /// Tests whether the buffer contains no text.
    pub fn is_empty(&self) -> bool {
        self.len().0 == 0
    }
    /// Converts a scalar boundary to a byte boundary, including end-of-buffer.
    pub fn to_byte(&self, offset: ScalarOffset) -> Result<ByteOffset, OffsetError> {
        if offset > self.len() {
            return Err(OffsetError);
        }
        Ok(ByteOffset(self.rope.char_to_byte_idx(offset.0)))
    }
    /// Converts an exact UTF-8 boundary; rejects the interior of a scalar.
    pub fn byte_to_scalar(&self, offset: ByteOffset) -> Result<ScalarOffset, OffsetError> {
        if offset.0 > self.rope.len() || !self.rope.is_char_boundary(offset.0) {
            return Err(OffsetError);
        }
        Ok(ScalarOffset(self.rope.byte_to_char_idx(offset.0)))
    }
    /// Converts a scalar position to UTF-16 code units.
    pub fn to_utf16(&self, offset: ScalarOffset) -> Result<Utf16Offset, OffsetError> {
        Ok(Utf16Offset(
            self.rope.byte_to_utf16_idx(self.to_byte(offset)?.0),
        ))
    }
    /// Converts an exact UTF-16 boundary; rejects split surrogate pairs.
    pub fn utf16_to_scalar(&self, offset: Utf16Offset) -> Result<ScalarOffset, OffsetError> {
        if offset.0 > self.rope.len_utf16() {
            return Err(OffsetError);
        }
        let byte = self.rope.utf16_to_byte_idx(offset.0);
        if self.rope.byte_to_utf16_idx(byte) != offset.0 {
            return Err(OffsetError);
        }
        self.byte_to_scalar(ByteOffset(byte))
    }
    /// Copies a scalar range; reversed and out-of-bounds ranges are rejected.
    pub fn slice(&self, range: Range<ScalarOffset>) -> Result<String, OffsetError> {
        let bytes = self.byte_range(range)?;
        Ok(self.rope.slice(bytes).to_string())
    }
    fn byte_range(&self, range: Range<ScalarOffset>) -> Result<Range<usize>, OffsetError> {
        if range.start > range.end {
            return Err(OffsetError);
        }
        Ok(self.to_byte(range.start)?.0..self.to_byte(range.end)?.0)
    }
    /// Replaces a validated range atomically and normalizes inserted newlines.
    pub fn replace(&mut self, range: Range<ScalarOffset>, text: &str) -> Result<(), OffsetError> {
        let bytes = self.byte_range(range)?;
        self.rope.remove(bytes.clone());
        self.rope.insert(bytes.start, &normalize_newlines(text));
        Ok(())
    }
    /// Finds the preceding extended grapheme boundary (or the start).
    pub fn previous_grapheme(&self, offset: ScalarOffset) -> Result<ScalarOffset, OffsetError> {
        let byte = self.to_byte(offset)?.0;
        let text = self.to_string();
        let previous = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .take_while(|i| *i < byte)
            .last()
            .unwrap_or(0);
        self.byte_to_scalar(ByteOffset(previous))
    }
    /// Finds the following extended grapheme boundary (or the end).
    pub fn next_grapheme(&self, offset: ScalarOffset) -> Result<ScalarOffset, OffsetError> {
        let byte = self.to_byte(offset)?.0;
        let text = self.to_string();
        let next = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i > byte)
            .unwrap_or(text.len());
        self.byte_to_scalar(ByteOffset(next))
    }
    /// Returns UAX #29 word ranges containing letters or numbers.
    pub fn words(&self) -> Vec<Range<ScalarOffset>> {
        let text = self.to_string();
        text.unicode_word_indices()
            .map(|(byte, word)| {
                let start = self.rope.byte_to_char_idx(byte);
                ScalarOffset(start)..ScalarOffset(start + word.chars().count())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    // Spec: docs/contracts/document-model.md#文本与位置
    fn rejects_invalid_boundaries_without_mutating_snapshots() {
        let mut text = TextBuffer::new("a\r\n墨😀\rb");
        let snapshot = text.clone();
        assert_eq!(text.to_string(), "a\n墨😀\nb");
        assert!(text.byte_to_scalar(ByteOffset(3)).is_err());
        assert!(text.utf16_to_scalar(Utf16Offset(4)).is_err());
        assert!(text.to_byte(ScalarOffset(99)).is_err());
        assert!(text.replace(ScalarOffset(4)..ScalarOffset(3), "x").is_err());
        assert!(
            text.replace(ScalarOffset(0)..ScalarOffset(99), "x")
                .is_err()
        );
        assert_eq!(text.to_string(), snapshot.to_string());
        text.replace(ScalarOffset(2)..ScalarOffset(4), "中\r\n")
            .expect("valid range");
        assert_eq!(text.to_string(), "a\n中\n\nb");
        assert_eq!(snapshot.to_string(), "a\n墨😀\nb");
    }
    #[test]
    // Spec: docs/modules/selection.md#逻辑选择
    fn moves_over_combining_marks_and_emoji_sequences() {
        let text = TextBuffer::new("e\u{301}👨‍👩‍👧‍👦墨");
        assert_eq!(text.next_grapheme(ScalarOffset(0)), Ok(ScalarOffset(2)));
        assert_eq!(text.next_grapheme(ScalarOffset(2)), Ok(ScalarOffset(9)));
        assert_eq!(text.previous_grapheme(ScalarOffset(9)), Ok(ScalarOffset(2)));
        assert_eq!(text.previous_grapheme(ScalarOffset(0)), Ok(ScalarOffset(0)));
        assert_eq!(text.next_grapheme(text.len()), Ok(text.len()));
        assert!(text.next_grapheme(ScalarOffset(99)).is_err());
        assert!(text.previous_grapheme(ScalarOffset(99)).is_err());
        assert_eq!(
            TextBuffer::new("hello 世界").words(),
            vec![
                ScalarOffset(0)..ScalarOffset(5),
                ScalarOffset(6)..ScalarOffset(7),
                ScalarOffset(7)..ScalarOffset(8)
            ]
        );
        assert!(TextBuffer::new("").is_empty());
        assert_eq!(OffsetError.code(), "text.invalid_offset");
        assert_eq!(OffsetError.message_key(), "error.text.invalid_offset");
        assert!(!OffsetError.is_retryable());
    }
    proptest! {
        #[test]
        fn encoding_round_trips(chars in prop::collection::vec(any::<char>(), 0..100)) {
            let source: String = chars.into_iter().collect();
            let text = TextBuffer::new(&source);
            for i in 0..=text.len().0 {
                let scalar = ScalarOffset(i);
                prop_assert_eq!(text.byte_to_scalar(text.to_byte(scalar).expect("scalar")), Ok(scalar));
                prop_assert_eq!(text.utf16_to_scalar(text.to_utf16(scalar).expect("scalar")), Ok(scalar));
            }
            prop_assert!(text.to_byte(ScalarOffset(text.len().0 + 1)).is_err());
        }
        #[test]
        fn replacements_match_string_model(source in ".{0,100}", replacement in ".{0,30}", a in any::<usize>(), b in any::<usize>()) {
            let mut text = TextBuffer::new(&source);
            let snapshot = text.clone();
            let mut model = normalize_newlines(&source);
            let len = text.len().value();
            let first = a % (len + 1);
            let second = b % (len + 1);
            let range = ScalarOffset(first.min(second))..ScalarOffset(first.max(second));
            let bytes = text.to_byte(range.start).expect("boundary").value()..text.to_byte(range.end).expect("boundary").value();
            model.replace_range(bytes, &normalize_newlines(&replacement));
            text.replace(range, &replacement).expect("valid range");
            prop_assert_eq!(text.to_string(), model);
            prop_assert_eq!(snapshot.to_string(), normalize_newlines(&source));
            prop_assert_eq!(text.slice(ScalarOffset(0)..text.len()).expect("whole buffer"), text.to_string());
        }
    }
}
