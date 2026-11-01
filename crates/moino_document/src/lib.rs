//! L0 document transactions, selections, history and snapshots; depends on text and Markdown.
#![forbid(unsafe_code)]

use moino_markdown::{Cst, TextChange};
use moino_text::{ScalarOffset, TextBuffer, normalize_newlines};
use std::{ops::Range, sync::Arc, time::Duration};

macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
        pub struct $name(u64);
        impl $name {
            /// Constructs an identity at its owning boundary.
            pub const fn new(value: u64) -> Self {
                Self(value)
            }
            /// Opaque identity value.
            pub const fn value(self) -> u64 {
                self.0
            }
        }
    };
}
identity!(
    DocumentId,
    "Identity shared by every view of one document session."
);
identity!(Revision, "Monotonically increasing content revision.");
identity!(Epoch, "Generation of a loaded document.");
identity!(
    HistoryGroup,
    "Caller-owned merge key for a continuous typing operation."
);

/// Direction of a continuous typing stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryDirection {
    /// Forward insertion or deletion.
    Forward,
    /// Backward insertion or deletion.
    Backward,
}
/// Side of inserted text to which an endpoint sticks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Affinity {
    /// Remain before inserted text.
    Before,
    /// Follow inserted text.
    After,
}
/// One logical selection endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Endpoint {
    /// Unicode scalar position.
    pub offset: ScalarOffset,
    /// Insertion affinity.
    pub affinity: Affinity,
}
/// Selection direction is expressed by anchor and head, never by sorted endpoints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection {
    anchor: Endpoint,
    head: Endpoint,
}
impl Selection {
    /// Creates a directional selection.
    pub const fn new(anchor: Endpoint, head: Endpoint) -> Self {
        Self { anchor, head }
    }
    /// Creates a cursor following inserted text.
    pub const fn cursor(offset: ScalarOffset) -> Self {
        Self {
            anchor: Endpoint {
                offset,
                affinity: Affinity::After,
            },
            head: Endpoint {
                offset,
                affinity: Affinity::After,
            },
        }
    }
    /// Fixed selection endpoint.
    pub const fn anchor(self) -> Endpoint {
        self.anchor
    }
    /// Active selection endpoint.
    pub const fn head(self) -> Endpoint {
        self.head
    }
    /// Whether the selection is collapsed.
    pub fn is_collapsed(self) -> bool {
        self.anchor.offset == self.head.offset
    }
    /// Whether the head follows the anchor.
    pub fn is_forward(self) -> bool {
        self.anchor.offset < self.head.offset
    }
    /// Ordered text range without losing directional endpoint state.
    pub fn range(self) -> Range<ScalarOffset> {
        self.anchor.offset.min(self.head.offset)..self.anchor.offset.max(self.head.offset)
    }
    /// Maps each endpoint independently through validated edits in pre-edit coordinates.
    pub fn mapped(self, edits: &[Edit]) -> Self {
        Self {
            anchor: map_endpoint(self.anchor, edits),
            head: map_endpoint(self.head, edits),
        }
    }
}

/// Replacement in the document's pre-transaction scalar coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edit {
    range: Range<ScalarOffset>,
    text: String,
}
impl Edit {
    /// Normalizes replacement text when constructing an edit.
    pub fn new(range: Range<ScalarOffset>, text: &str) -> Self {
        Self {
            range,
            text: normalize_newlines(text),
        }
    }
    /// Pre-edit range.
    pub fn range(&self) -> Range<ScalarOffset> {
        self.range.clone()
    }
    /// Normalized inserted text.
    pub fn text(&self) -> &str {
        &self.text
    }
}
/// Explicit source of a document modification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Origin {
    /// Ordinary contiguous typing.
    Typing,
    /// Structural or formatting command.
    Command,
    /// Paste.
    Paste,
    /// Committed input-method composition.
    Composition,
    /// External replacement.
    External,
}
/// An atomic modification tied to a document generation and revision.
#[derive(Clone, Debug)]
pub struct Transaction {
    document: DocumentId,
    epoch: Epoch,
    revision: Revision,
    edits: Vec<Edit>,
    selection: Option<Selection>,
    origin: Origin,
    group: Option<HistoryGroup>,
    direction: HistoryDirection,
}
impl Transaction {
    /// Starts a transaction against a frozen document identity.
    pub fn new(snapshot: &Snapshot, origin: Origin) -> Self {
        Self {
            document: snapshot.document,
            epoch: snapshot.epoch,
            revision: snapshot.revision,
            edits: Vec::new(),
            selection: None,
            origin,
            group: None,
            direction: HistoryDirection::Forward,
        }
    }
    /// Adds a replacement; validation happens atomically at commit.
    pub fn with_edit(mut self, edit: Edit) -> Self {
        self.edits.push(edit);
        self
    }
    /// Sets the selection in post-edit coordinates.
    pub fn with_selection(mut self, selection: Selection) -> Self {
        self.selection = Some(selection);
        self
    }
    /// Allows ordinary typing with this key to merge at a history boundary.
    pub fn with_group(mut self, group: HistoryGroup) -> Self {
        self.group = Some(group);
        self
    }
    /// Sets the stream direction; direction changes prevent history merging.
    pub fn with_direction(mut self, direction: HistoryDirection) -> Self {
        self.direction = direction;
        self
    }
    /// Ordered replacement inputs.
    pub fn edits(&self) -> &[Edit] {
        &self.edits
    }
    /// Validates and derives candidate text and syntax offline, without changing a document.
    pub fn prepare(mut self, snapshot: &Snapshot) -> Result<PreparedTransaction, DocumentError> {
        if (self.document, self.epoch, self.revision)
            != (snapshot.document, snapshot.epoch, snapshot.revision)
        {
            return Err(DocumentError::Stale);
        }
        snapshot
            .revision
            .0
            .checked_add(1)
            .ok_or(DocumentError::Exhausted)?;
        self.edits
            .sort_by_key(|edit| (edit.range.start, edit.range.end));
        let (text, inverse) = apply(&snapshot.text, &self.edits)?;
        if let Some(selection) = self.selection {
            validate_selection(selection, text.len())?;
        }
        let cst = candidate_cst(&snapshot.cst, &text, &self.edits)?;
        Ok(PreparedTransaction {
            transaction: self,
            text,
            cst,
            inverse,
        })
    }
}
/// Validated offline work, sealed against mutation before its foreground commit.
pub struct PreparedTransaction {
    transaction: Transaction,
    text: TextBuffer,
    cst: Cst,
    inverse: Vec<Edit>,
}
/// Frozen text and syntax at a particular document revision.
#[derive(Clone, Debug)]
pub struct Snapshot {
    document: DocumentId,
    epoch: Epoch,
    revision: Revision,
    text: TextBuffer,
    cst: Cst,
}
impl Snapshot {
    /// Shared document identity.
    pub const fn document(&self) -> DocumentId {
        self.document
    }
    /// Loaded generation.
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }
    /// Frozen content revision.
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    /// Frozen authoritative text.
    pub fn text(&self) -> &TextBuffer {
        &self.text
    }
    /// Syntax derived from exactly this text.
    pub fn cst(&self) -> &Cst {
        &self.cst
    }
}
/// Rejected commit; no partially applied transaction escapes this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum DocumentError {
    /// Transaction targets a different document or revision.
    #[error("document.stale_transaction")]
    Stale,
    /// Edits overlap, duplicate an insertion point, or exceed text boundaries.
    #[error("document.invalid_transaction")]
    Invalid,
    /// Revision cannot advance without overflowing its identity.
    #[error("document.revision_exhausted")]
    Exhausted,
    /// Derived model differs from authoritative text.
    #[error("model.invariant")]
    Invariant,
}
impl DocumentError {
    /// Stable cross-layer diagnostic code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Stale => "document.stale_transaction",
            Self::Invalid => "document.invalid_transaction",
            Self::Exhausted => "document.revision_exhausted",
            Self::Invariant => "model.invariant",
        }
    }
    /// Localization key corresponding to the stable diagnostic.
    pub fn message_key(self) -> String {
        format!("error.{}", self.code())
    }
    /// A rejected transaction requires re-evaluation rather than blind retry.
    pub const fn is_retryable(self) -> bool {
        false
    }
}
/// Successful atomic commit and its inverse operation.
#[derive(Clone, Debug)]
pub struct Commit {
    revision: Revision,
    affected: Range<ScalarOffset>,
    inverse: Vec<Edit>,
}
impl Commit {
    /// New document revision.
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    /// Affected range in pre-edit coordinates.
    pub fn affected(&self) -> Range<ScalarOffset> {
        self.affected.clone()
    }
    /// Replacements restoring the pre-commit text, in post-edit coordinates.
    pub fn inverse(&self) -> &[Edit] {
        &self.inverse
    }
}
#[derive(Clone, Debug)]
struct Unit {
    forward: Vec<Vec<Edit>>,
    inverse: Vec<Vec<Edit>>,
    before: Selection,
    after: Selection,
    group: Option<HistoryGroup>,
    direction: HistoryDirection,
    tick: Duration,
}
/// Frozen history work which can be sent to an executor without borrowing the live document.
pub struct HistoryRequest {
    base: Snapshot,
    unit: Arc<Unit>,
    redo: bool,
}
impl HistoryRequest {
    /// Base identity used when discarding stale failures or protecting the live document.
    pub fn snapshot(&self) -> &Snapshot {
        &self.base
    }
    /// Replays text and derives syntax offline; does not mutate either history stack.
    pub fn prepare(self) -> Result<PreparedHistory, DocumentError> {
        let revision = Revision(
            self.base
                .revision
                .0
                .checked_add(1)
                .ok_or(DocumentError::Exhausted)?,
        );
        let mut text = self.base.text.clone();
        let mut cst = self.base.cst.clone();
        let steps: Box<dyn Iterator<Item = &Vec<Edit>>> = if self.redo {
            Box::new(self.unit.forward.iter())
        } else {
            Box::new(self.unit.inverse.iter().rev())
        };
        for edits in steps {
            let next = apply(&text, edits)?.0;
            cst = candidate_cst(&cst, &next, edits)?;
            text = next;
        }
        Ok(PreparedHistory {
            request: self,
            text,
            cst,
            revision,
        })
    }
}
/// Sealed history candidate, applicable only to its original document generation and revision.
pub struct PreparedHistory {
    request: HistoryRequest,
    text: TextBuffer,
    cst: Cst,
    revision: Revision,
}
fn candidate_cst(cst: &Cst, text: &TextBuffer, edits: &[Edit]) -> Result<Cst, DocumentError> {
    let changes: Vec<_> = edits
        .iter()
        .map(|edit| TextChange::new(edit.range(), ScalarOffset::new(edit.text.chars().count())))
        .collect();
    let cst = cst
        .reparse_with_changes(text, &changes)
        .ok_or(DocumentError::Invariant)?;
    if !cst.is_valid() {
        return Err(DocumentError::Invariant);
    }
    Ok(cst)
}
/// Headless authoritative document with one shared history stack.
pub struct Document {
    snapshot: Snapshot,
    selection: Selection,
    undo: Vec<Arc<Unit>>,
    redo: Vec<Arc<Unit>>,
    saved: TextBuffer,
    output: Vec<u8>,
    saved_output: Vec<u8>,
    merge_open: bool,
    protected: bool,
}
impl Document {
    /// Opens an initially clean document with empty history.
    pub fn new(id: DocumentId, epoch: Epoch, text: &str) -> Self {
        let text = TextBuffer::new(text);
        let cst = Cst::parse(&text);
        let protected = !cst.is_valid();
        Self {
            saved: text.clone(),
            snapshot: Snapshot {
                document: id,
                epoch,
                revision: Revision(0),
                text,
                cst,
            },
            selection: Selection::cursor(ScalarOffset::new(0)),
            undo: Vec::new(),
            redo: Vec::new(),
            output: Vec::new(),
            saved_output: Vec::new(),
            merge_open: false,
            protected,
        }
    }
    /// Freezes text, syntax and identity for offline consumers.
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.clone()
    }
    /// Current logical selection.
    pub const fn selection(&self) -> Selection {
        self.selection
    }
    /// Updates view selection and cuts typing merge history.
    pub fn set_selection(&mut self, selection: Selection) -> Result<(), DocumentError> {
        validate_selection(selection, self.snapshot.text.len())?;
        self.selection = selection;
        self.break_history();
        Ok(())
    }
    /// Explicitly cuts a merge boundary on save, focus change or external events.
    pub fn break_history(&mut self) {
        self.merge_open = false;
    }
    /// Whether current text or output properties differ from the successful save baseline.
    pub fn is_dirty(&self) -> bool {
        self.snapshot.text != self.saved || self.output != self.saved_output
    }
    /// Whether invariant protection has stopped further mutations.
    pub const fn is_protected(&self) -> bool {
        self.protected
    }
    /// Supplies the file service's canonical output-property fingerprint.
    pub fn with_output_properties(&mut self, fingerprint: Vec<u8>) {
        self.output = fingerprint;
        self.break_history();
    }
    /// Captures the baseline only after successful persistence.
    pub fn mark_saved(&mut self) {
        self.saved = self.snapshot.text.clone();
        self.saved_output = self.output.clone();
        self.break_history();
    }
    /// Whether an undo unit exists.
    pub fn has_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    /// Whether a redo unit exists.
    pub fn has_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// Synchronous headless convenience entry; UI callers prepare on a background executor.
    pub fn commit(
        &mut self,
        transaction: Transaction,
        tick: Duration,
    ) -> Result<Commit, DocumentError> {
        if self.protected {
            return Err(DocumentError::Invariant);
        }
        let prepared = transaction.prepare(&self.snapshot).inspect_err(|error| {
            if *error == DocumentError::Invariant {
                self.protected = true;
            }
        })?;
        self.commit_prepared(prepared, tick)
    }
    /// Publishes already derived work after revalidating its document generation and revision.
    pub fn commit_prepared(
        &mut self,
        prepared: PreparedTransaction,
        tick: Duration,
    ) -> Result<Commit, DocumentError> {
        let PreparedTransaction {
            transaction,
            text,
            cst,
            inverse,
        } = prepared;
        if self.protected {
            return Err(DocumentError::Invariant);
        }
        if (
            transaction.document,
            transaction.epoch,
            transaction.revision,
        ) != (
            self.snapshot.document,
            self.snapshot.epoch,
            self.snapshot.revision,
        ) {
            return Err(DocumentError::Stale);
        }
        let edits = transaction.edits;
        let selection = transaction
            .selection
            .unwrap_or_else(|| self.selection.mapped(&edits));
        validate_selection(selection, text.len())?;
        let revision = Revision(
            self.snapshot
                .revision
                .0
                .checked_add(1)
                .ok_or(DocumentError::Exhausted)?,
        );
        let before = self.selection;
        let affected = edits
            .first()
            .map_or(ScalarOffset::new(0)..ScalarOffset::new(0), |first| {
                first.range.start..edits.last().expect("nonempty").range.end
            });
        let group = if transaction.origin == Origin::Typing {
            transaction.group
        } else {
            None
        };
        let merge = self.merge_open
            && group.is_some()
            && self.undo.last().is_some_and(|unit| {
                unit.group == group
                    && unit.direction == transaction.direction
                    && unit.after == before
                    && tick
                        .checked_sub(unit.tick)
                        .is_some_and(|elapsed| elapsed <= Duration::from_millis(1000))
            });
        let unit = Unit {
            forward: vec![edits],
            inverse: vec![inverse.clone()],
            before,
            after: selection,
            group,
            tick,
            direction: transaction.direction,
        };
        if merge {
            let last = Arc::make_mut(self.undo.last_mut().expect("merge requires previous unit"));
            last.forward.extend(unit.forward);
            last.inverse.extend(unit.inverse);
            last.after = selection;
            last.tick = tick;
        } else {
            self.undo.push(Arc::new(unit));
            if self.undo.len() > 1000 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.merge_open = group.is_some();
        self.snapshot = Snapshot {
            text,
            cst,
            revision,
            ..self.snapshot.clone()
        };
        self.selection = selection;
        Ok(Commit {
            revision,
            affected,
            inverse,
        })
    }
    /// Reverts one complete history unit; returns false if no unit exists.
    pub fn undo(&mut self) -> Result<bool, DocumentError> {
        self.replay(false)
    }
    /// Reapplies one complete history unit; returns false if no unit exists.
    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        self.replay(true)
    }
    /// Captures an undo request without replaying or parsing on the calling thread.
    pub fn undo_request(&self) -> Option<HistoryRequest> {
        self.history_request(false)
    }
    /// Captures a redo request without replaying or parsing on the calling thread.
    pub fn redo_request(&self) -> Option<HistoryRequest> {
        self.history_request(true)
    }
    fn history_request(&self, redo: bool) -> Option<HistoryRequest> {
        let unit = if redo {
            self.redo.last()?
        } else {
            self.undo.last()?
        };
        Some(HistoryRequest {
            base: self.snapshot.clone(),
            unit: unit.clone(),
            redo,
        })
    }
    /// Applies an offline history result only while its captured revision remains current.
    pub fn apply_history(&mut self, prepared: PreparedHistory) -> Result<(), DocumentError> {
        if self.protected {
            return Err(DocumentError::Invariant);
        }
        let PreparedHistory {
            request,
            text,
            cst,
            revision,
        } = prepared;
        if !self.matches_snapshot(&request.base) {
            return Err(DocumentError::Stale);
        }
        let source = if request.redo { &self.redo } else { &self.undo };
        if !source
            .last()
            .is_some_and(|unit| Arc::ptr_eq(unit, &request.unit))
        {
            return Err(DocumentError::Stale);
        }
        self.selection = if request.redo {
            request.unit.after
        } else {
            request.unit.before
        };
        if request.redo {
            self.redo.pop();
            self.undo.push(request.unit);
        } else {
            self.undo.pop();
            self.redo.push(request.unit);
        }
        self.snapshot = Snapshot {
            text,
            cst,
            revision,
            ..self.snapshot.clone()
        };
        self.break_history();
        Ok(())
    }
    /// Protects only a still-current snapshot after an offline invariant failure.
    pub fn protect_snapshot(&mut self, snapshot: &Snapshot) -> bool {
        if !self.matches_snapshot(snapshot) {
            return false;
        }
        self.protected = true;
        self.break_history();
        true
    }
    fn matches_snapshot(&self, snapshot: &Snapshot) -> bool {
        (
            self.snapshot.document,
            self.snapshot.epoch,
            self.snapshot.revision,
        ) == (snapshot.document, snapshot.epoch, snapshot.revision)
    }
    fn replay(&mut self, redo: bool) -> Result<bool, DocumentError> {
        if self.protected {
            return Err(DocumentError::Invariant);
        }
        let Some(request) = self.history_request(redo) else {
            self.break_history();
            return Ok(false);
        };
        let prepared = request.prepare().inspect_err(|error| {
            if *error == DocumentError::Invariant {
                self.protected = true;
            }
        })?;
        self.apply_history(prepared)?;
        Ok(true)
    }
}
fn validate_selection(selection: Selection, len: ScalarOffset) -> Result<(), DocumentError> {
    if selection.anchor.offset > len || selection.head.offset > len {
        Err(DocumentError::Invalid)
    } else {
        Ok(())
    }
}
fn apply(text: &TextBuffer, edits: &[Edit]) -> Result<(TextBuffer, Vec<Edit>), DocumentError> {
    let mut previous: Option<&Edit> = None;
    for edit in edits {
        if edit.range.start > edit.range.end || edit.range.end > text.len() {
            return Err(DocumentError::Invalid);
        }
        if previous.is_some_and(|last| {
            last.range.end > edit.range.start || last.range.start == edit.range.start
        }) {
            return Err(DocumentError::Invalid);
        }
        previous = Some(edit);
    }
    let mut inverse = Vec::new();
    let mut removed = 0;
    let mut added = 0;
    for edit in edits {
        let start = edit.range.start.value() - removed + added;
        let inserted = edit.text.chars().count();
        inverse.push(Edit::new(
            ScalarOffset::new(start)..ScalarOffset::new(start + inserted),
            &text
                .slice(edit.range())
                .map_err(|_| DocumentError::Invalid)?,
        ));
        removed += edit.range.end.value() - edit.range.start.value();
        added += inserted;
    }
    let mut next = text.clone();
    for edit in edits.iter().rev() {
        next.replace(edit.range(), &edit.text)
            .map_err(|_| DocumentError::Invalid)?;
    }
    let mut merged: Vec<Edit> = Vec::new();
    for edit in inverse {
        if let Some(last) = merged
            .last_mut()
            .filter(|last| last.range.end == edit.range.start)
        {
            last.range.end = edit.range.end;
            last.text.push_str(&edit.text);
        } else {
            merged.push(edit);
        }
    }
    Ok((next, merged))
}
fn map_endpoint(endpoint: Endpoint, edits: &[Edit]) -> Endpoint {
    let position = endpoint.offset.value();
    let mut removed = 0;
    let mut added = 0;
    let mut edits: Vec<_> = edits.iter().collect();
    edits.sort_by_key(|edit| edit.range.start);
    for edit in edits {
        let start = edit.range.start.value();
        let end = edit.range.end.value();
        if position < start {
            break;
        }
        let inserted = edit.text.chars().count();
        if position < end || position == start {
            return Endpoint {
                offset: ScalarOffset::new(
                    start - removed
                        + added
                        + if position == start && endpoint.affinity == Affinity::After {
                            inserted
                        } else {
                            0
                        },
                ),
                ..endpoint
            };
        }
        removed += end - start;
        added += inserted;
    }
    Endpoint {
        offset: ScalarOffset::new(position - removed + added),
        ..endpoint
    }
}
