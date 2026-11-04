//! L0 editing commands resolve a frozen context into one atomic transaction.
#![forbid(unsafe_code)]

mod containers;
mod context;
mod deletion;
mod enter;
mod formatting;
mod heading;
mod indentation;
mod insertion;
mod merge;
mod prefixes;
mod syntax;
mod tab;
mod transaction;

use context::CommandContext;
use formatting::remove_markers;
use moino_document::{Edit, Origin, Selection, Snapshot, Transaction};
use moino_markdown::{Node, Syntax};
use moino_text::{ScalarOffset, normalize_newlines};
use prefixes::{next_marker, partner, prefix_len, quote_prefix_width};
use std::ops::Range;
use syntax::{
    in_literal, is_inline_code_closer, marker_boundary, table_cells, text_block, text_intersects,
};
use transaction::{edits_outcome, navigation, offset, replacement};

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
    pending_block: Option<Range<ScalarOffset>>,
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
            pending_block: None,
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
    /// Supplies the view's unconverted input block without changing authoritative syntax.
    pub fn with_pending_block(mut self, range: Range<ScalarOffset>) -> Self {
        self.pending_block = Some(range);
        self
    }
    pub(crate) fn pending_block(&self) -> Option<Range<ScalarOffset>> {
        self.pending_block.clone()
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
/// Resolves a command using frozen authoritative text, selection and transient input context.
pub fn resolve(
    snapshot: &Snapshot,
    selection: Selection,
    intent: Intent,
    options: &Options,
) -> Result<Outcome, EditingError> {
    let context = CommandContext::new(snapshot, selection, options)?;
    match intent {
        Intent::Composition(value) => Ok(context.insert_plain(&value, Origin::Composition)),
        Intent::Paste(value) => Ok(context.insert_plain(&value, Origin::Paste)),
        Intent::Insert(value) => insertion::insert(&context, value),
        Intent::Enter => enter::run(&context),
        Intent::Backspace | Intent::Delete if !selection.is_collapsed() => {
            deletion::selected(&context)
        }
        Intent::Backspace => deletion::backward(&context),
        Intent::Delete => deletion::forward(&context),
        Intent::Tab { reverse } => tab::run(&context, reverse),
        Intent::Inline(format) => formatting::inline(&context, format),
        Intent::Block(format) => formatting::block(&context, format),
        Intent::ClearFormatting => formatting::clear(&context),
    }
}
