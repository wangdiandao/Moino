//! Retained editing surface; document derivation runs on the background executor.
use crate::commands;
use crate::projection::{self, Leaf};
use gpui_kit::base::text::editor_host::{FlowPosition, LeafGeometry, RenderContext};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{prelude::*, *};
use moino_document::{Affinity, Document, DocumentId, Endpoint, Epoch, Selection};
use moino_editing::{BlockFormat, InlineFormat, Intent, Options};
#[cfg(test)]
mod vectors;
use moino_text::{ScalarOffset, TextBuffer, Utf16Offset};
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
};

actions!(
    edit,
    [
        /// Splits the current block.
        Split,
        /// Deletes the preceding grapheme or marker.
        DeleteBackward,
        /// Deletes the following grapheme or marker.
        DeleteForward,
        /// Indents the current block.
        Indent,
        /// Outdents the current block.
        Outdent,
        /// Restores the preceding history unit.
        Undo,
        /// Reapplies the following history unit.
        Redo,
        /// Selects the entire document.
        SelectAll,
        /// Copies the source selection.
        Copy,
        /// Cuts the source selection.
        Cut,
        /// Pastes plain source.
        Paste,
        /// Moves to the beginning of the document.
        MoveToStart,
        /// Moves to the end of the document.
        MoveToEnd,
        /// Extends the selection to the beginning of the document.
        SelectToStart,
        /// Extends the selection to the end of the document.
        SelectToEnd,
    ]
);

enum Operation {
    Intent(Intent),
    Source {
        text: String,
        selection: Selection,
        change: Option<crate::source_change::SourceChange>,
        origin: moino_document::Origin,
    },
    Select(Selection, moino_document::Revision),
    SelectAfterPending(Selection),
    Navigate {
        forward: bool,
        extend: bool,
    },
    Vertical {
        forward: bool,
        extend: bool,
    },
    LineEdge {
        end: bool,
        extend: bool,
    },
    Copy {
        cut: bool,
    },
    SelectAll,
    DocumentEdge {
        end: bool,
        extend: bool,
    },
    Undo,
    Redo,
}

/// Owns one document, its native input surface and its derived block projection.
pub struct LiveEditor {
    document: Document,
    options: Options,
    focus: FocusHandle,
    leaves: Vec<Leaf>,
    geometry: Rc<RefCell<HashMap<u64, LeafGeometry>>>,
    pending: VecDeque<Operation>,
    worker: Option<Task<()>>,
    waiting_for_frame: bool,
    composition: Option<Composition>,
    dragging: bool,
    error: Option<SharedString>,
    list: ListState,
    reveal_caret: bool,
    preferred_x: Option<Pixels>,
    started: std::time::Instant,
    source_text: SharedString,
    projection_theme: moino_theme::BuiltinTheme,
    projection_rem: Pixels,
    scroll_anchor: Option<ScrollAnchor>,
}

struct ScrollAnchor {
    key: u64,
    offset: ScalarOffset,
    source: ScalarOffset,
    viewport_y: Pixels,
}

struct Composition {
    range: std::ops::Range<ScalarOffset>,
    text: String,
    selection: std::ops::Range<usize>,
    follows_pending: bool,
    revision: moino_document::Revision,
}

impl LiveEditor {
    /// Opens the initial empty document with the product input policy.
    pub fn empty(cx: &mut Context<Self>) -> Self {
        Self::new(
            "",
            Options::new("    ", true).expect("built-in input policy"),
            cx,
        )
    }
    /// Creates a surface with explicit input policy and a clean source document.
    pub fn new(source: &str, options: Options, cx: &mut Context<Self>) -> Self {
        let document = Document::new(DocumentId::new(1), Epoch::new(0), source);
        let projection_theme = effective_theme(cx);
        let leaves =
            projection::project(&document.snapshot(), document.selection(), projection_theme);
        let list = ListState::new(leaves.len(), ListAlignment::Top, px(0.));
        let owner = cx.entity().downgrade();
        list.set_scroll_handler(move |_, _, cx| {
            let _ = owner.update(cx, |editor, cx| {
                editor.scroll_anchor = None;
                editor.reveal_caret = false;
                cx.notify();
            }); // A closed editor has no user scroll target.
        });
        Self {
            document,
            options,
            focus: cx.focus_handle().tab_stop(true),
            leaves,
            geometry: Rc::default(),
            pending: VecDeque::new(),
            worker: None,
            waiting_for_frame: false,
            composition: None,
            dragging: false,
            error: None,
            list,
            reveal_caret: true,
            preferred_x: None,
            started: cx.background_executor().now(),
            source_text: source.into(),
            projection_theme,
            projection_rem: px(0.),
            scroll_anchor: None,
        }
    }

    /// Frozen authoritative document for status, persistence and other surfaces.
    pub fn snapshot(&self) -> moino_document::Snapshot {
        self.document.snapshot()
    }
    /// Current directional source selection.
    pub fn selection(&self) -> Selection {
        self.document.selection()
    }
    /// Whether the current source differs from its saved snapshot.
    pub fn is_dirty(&self) -> bool {
        self.document.is_dirty()
    }
    /// Whether content changes are unavailable, including invariant protection.
    pub fn is_read_only(&self) -> bool {
        self.options.is_read_only() || self.document.is_protected()
    }
    /// Whether background text derivation or queued input is outstanding.
    pub fn is_pending(&self) -> bool {
        self.worker.is_some() || !self.pending.is_empty()
    }
    /// Stable failure code, retained until a later successful modification.
    pub fn error_code(&self) -> Option<&str> {
        self.error.as_deref()
    }
    /// Stable native input focus target.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_scroll_anchor();
        if let Some(composition) = &mut self.composition
            && composition.follows_pending
        {
            composition.range = self.document.selection().range();
            composition.revision = self.document.snapshot().revision();
        }
        self.source_text = self.document.snapshot().text().to_string().into();
        let top = self.list.logical_scroll_top();
        let key = self.leaves.get(top.item_ix).map(|leaf| leaf.key);
        let old_source = self.leaves.get(top.item_ix).map(|leaf| leaf.raw.start);
        self.leaves = if let Some(composition) = &self.composition {
            projection::compose(
                projection::project(
                    &self.document.snapshot(),
                    selection(composition.range.start, composition.range.end),
                    self.projection_theme,
                ),
                composition.range.clone(),
                &composition.text,
            )
        } else {
            projection::project(
                &self.document.snapshot(),
                self.document.selection(),
                self.projection_theme,
            )
        };
        self.list.reset(self.leaves.len());
        if let Some(anchor) = &mut self.scroll_anchor
            && let Some(leaf) = self
                .leaves
                .iter()
                .find(|leaf| leaf.key == anchor.key)
                .or_else(|| {
                    self.leaves
                        .iter()
                        .rev()
                        .find(|leaf| leaf.raw.start <= anchor.source)
                })
        {
            anchor.key = leaf.key;
            anchor.offset = ScalarOffset::new(
                anchor
                    .source
                    .value()
                    .min(leaf.raw.end.value())
                    .saturating_sub(leaf.raw.start.value()),
            );
        }
        let target = key
            .and_then(|key| self.leaves.iter().position(|leaf| leaf.key == key))
            .or_else(|| {
                old_source.and_then(|at| self.leaves.iter().rposition(|leaf| leaf.raw.start <= at))
            });
        if let Some(ix) = target {
            self.list.scroll_to(ListOffset {
                item_ix: ix,
                offset_in_item: top.offset_in_item,
            });
        }
        self.geometry.borrow_mut().clear();
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn display_text(&self) -> TextBuffer {
        let mut text = self.document.snapshot().text().clone();
        if let Some(composition) = &self.composition {
            let _ = text.replace(composition.range.clone(), &composition.text); // Validated native source range.
        }
        text
    }

    fn native_range(&self, range: std::ops::Range<usize>) -> Option<std::ops::Range<ScalarOffset>> {
        let text = self.display_text();
        let start = text.utf16_to_scalar(Utf16Offset::new(range.start)).ok()?;
        let end = text.utf16_to_scalar(Utf16Offset::new(range.end)).ok()?;
        if start > end {
            return None;
        }
        if let Some(composition) = &self.composition {
            let displayed_end = composition.range.start.value() + composition.text.chars().count();
            let map = |at: ScalarOffset, after: bool| {
                if at <= composition.range.start {
                    at
                } else if at.value() >= displayed_end {
                    ScalarOffset::new(composition.range.end.value() + at.value() - displayed_end)
                } else if after {
                    composition.range.end
                } else {
                    composition.range.start
                }
            };
            Some(map(start, false)..map(end, true))
        } else {
            Some(start..end)
        }
    }

    fn native_selection(&self) -> Option<UTF16Selection> {
        let text = self.document.snapshot();
        if let Some(composition) = &self.composition {
            let start = text.text().to_utf16(composition.range.start).ok()?.value();
            return Some(UTF16Selection {
                range: start + composition.selection.start..start + composition.selection.end,
                reversed: false,
            });
        }
        let selected = self.document.selection();
        Some(UTF16Selection {
            range: text.text().to_utf16(selected.range().start).ok()?.value()
                ..text.text().to_utf16(selected.range().end).ok()?.value(),
            reversed: !selected.is_forward() && !selected.is_collapsed(),
        })
    }

    fn display_head(&self) -> ScalarOffset {
        if let Some(selected) = self.native_selection()
            && let Ok(at) =
                self.display_text()
                    .utf16_to_scalar(Utf16Offset::new(if selected.reversed {
                        selected.range.start
                    } else {
                        selected.range.end
                    }))
        {
            return at;
        }
        self.document.selection().head().offset
    }

    /// Routes a business editing intent through the same serialized command queue as native input.
    pub fn dispatch(&mut self, intent: Intent, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition(window, cx);
        self.enqueue(Operation::Intent(intent), window, cx);
    }

    /// Publishes a full-source edit through the shared document history.
    pub fn replace_source(
        &mut self,
        text: String,
        selection: Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_source_edit(
            text,
            selection,
            None,
            moino_document::Origin::Command,
            window,
            cx,
        );
    }
    pub(super) fn replace_source_edit(
        &mut self,
        text: String,
        selection: Selection,
        change: Option<crate::source_change::SourceChange>,
        origin: moino_document::Origin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.enqueue(
            Operation::Source {
                text,
                selection,
                change,
                origin,
            },
            window,
            cx,
        );
    }
    /// Moves the shared source selection without modifying text.
    pub fn select_source(
        &mut self,
        selection: Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_composition(window, cx);
        self.enqueue(self.selection_operation(selection), window, cx);
    }
    pub(super) fn select_source_after_pending(
        &mut self,
        selection: Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.enqueue(Operation::SelectAfterPending(selection), window, cx);
    }
    /// Restores a unit from the shared document history.
    pub fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition(window, cx);
        self.enqueue(Operation::Undo, window, cx);
    }
    /// Reapplies a unit from the shared document history.
    pub fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition(window, cx);
        self.enqueue(Operation::Redo, window, cx);
    }

    fn enqueue(&mut self, operation: Operation, window: &mut Window, cx: &mut Context<Self>) {
        self.reveal_caret = true;
        self.pending.push_back(operation);
        self.start_next(window, cx);
    }

    fn selection_operation(&self, selected: Selection) -> Operation {
        Operation::Select(selected, self.document.snapshot().revision())
    }

    fn map_pending_selections(
        &mut self,
        before: moino_document::Revision,
        edits: &[moino_document::Edit],
    ) {
        let after = self.document.snapshot().revision();
        for operation in &mut self.pending {
            if let Operation::Select(selected, revision) = operation
                && *revision == before
            {
                *selected = selected.mapped(edits);
                *revision = after;
            }
        }
        if let Some(composition) = &mut self.composition
            && composition.revision == before
            && !composition.follows_pending
        {
            composition.range = selection(composition.range.start, composition.range.end)
                .mapped(edits)
                .range();
            composition.revision = after;
        }
        if let Some(anchor) = &mut self.scroll_anchor {
            anchor.source = Selection::cursor(anchor.source).mapped(edits).head().offset;
        }
    }

    fn defer_visual_move(
        &mut self,
        operation: Operation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending.push_front(operation);
        self.yield_queue(window, cx);
    }

    fn yield_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.waiting_for_frame {
            return;
        }
        self.waiting_for_frame = true;
        let editor = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            let _ = editor.update(cx, |editor, cx| {
                editor.waiting_for_frame = false;
                editor.start_next(window, cx);
            }); // A closed surface has no queue target.
        });
        cx.notify();
    }

    fn start_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.worker.is_some() || self.waiting_for_frame {
            return;
        }
        let batch = cx.background_executor().now();
        while !self.pending.is_empty() {
            if cx.background_executor().now().duration_since(batch)
                >= std::time::Duration::from_millis(8)
            {
                self.yield_queue(window, cx);
                return;
            }
            let operation = self.pending.pop_front().expect("queue is nonempty");
            match operation {
                Operation::DocumentEdge { end, extend } => {
                    self.preferred_x = None;
                    let head = if end {
                        self.document.snapshot().text().len()
                    } else {
                        ScalarOffset::new(0)
                    };
                    let selected = selection(
                        if extend {
                            self.document.selection().anchor().offset
                        } else {
                            head
                        },
                        head,
                    );
                    if let Err(error) = self.document.set_selection(selected) {
                        self.error = Some(error.code().into());
                    }
                    self.refresh(window, cx);
                }
                Operation::SelectAll => {
                    if let Err(error) = self.document.set_selection(selection(
                        ScalarOffset::new(0),
                        self.document.snapshot().text().len(),
                    )) {
                        self.error = Some(error.code().into());
                    }
                    self.refresh(window, cx);
                }
                Operation::Copy { cut } => {
                    self.document.break_history();
                    let range = self.document.selection().range();
                    if !range.is_empty()
                        && let Ok(text) = self.document.snapshot().text().slice(range)
                    {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                        if cut {
                            self.pending
                                .push_front(Operation::Intent(Intent::Paste(String::new())));
                        }
                    }
                }
                Operation::Vertical { forward, extend } => {
                    let current = self.document.selection();
                    if let Some(caret) = self.caret(current.head().offset) {
                        let x = *self.preferred_x.get_or_insert(caret.left());
                        let line_height = self
                            .geometry
                            .borrow()
                            .values()
                            .find(|geometry| geometry.bounds().contains(&caret.origin))
                            .map_or(caret.size.height, LeafGeometry::line_height);
                        let target = point(
                            x,
                            caret.top()
                                + if forward {
                                    line_height * 1.5
                                } else {
                                    -line_height * 0.5
                                },
                        );
                        if let Some(head) = self.hit(target) {
                            let selected = selection(
                                if extend {
                                    current.anchor().offset
                                } else {
                                    head
                                },
                                head,
                            );
                            if let Err(error) = self.document.set_selection(selected) {
                                self.error = Some(error.code().into());
                            }
                            self.refresh(window, cx);
                        }
                    } else {
                        self.defer_visual_move(Operation::Vertical { forward, extend }, window, cx);
                        return;
                    }
                }
                Operation::LineEdge { end, extend } => {
                    self.preferred_x = None;
                    let current = self.document.selection();
                    if let Some(caret) = self.caret(current.head().offset) {
                        let x = if end {
                            self.list.viewport_bounds().right()
                        } else {
                            self.list.viewport_bounds().left()
                        };
                        if let Some(head) = self.hit(point(x, caret.top())) {
                            if let Err(error) = self.document.set_selection(selection(
                                if extend {
                                    current.anchor().offset
                                } else {
                                    head
                                },
                                head,
                            )) {
                                self.error = Some(error.code().into());
                            }
                            self.refresh(window, cx);
                        }
                    } else {
                        self.defer_visual_move(Operation::LineEdge { end, extend }, window, cx);
                        return;
                    }
                }
                Operation::Navigate { forward, extend } => {
                    self.preferred_x = None;
                    if let Some(selection) = self.horizontal_selection(forward, extend) {
                        if let Err(error) = self.document.set_selection(selection) {
                            self.error = Some(error.code().into());
                        }
                        self.refresh(window, cx);
                    }
                }
                Operation::SelectAfterPending(selection) => {
                    if let Err(error) = self.document.set_selection(selection) {
                        self.error = Some(error.code().into());
                    }
                    self.refresh(window, cx);
                }
                Operation::Select(selection, revision) => {
                    if revision != self.document.snapshot().revision() {
                        self.error = Some("document.stale".into());
                    } else if let Err(error) = self.document.set_selection(selection) {
                        self.error = Some(error.code().into());
                    }
                    self.refresh(window, cx);
                }
                operation @ (Operation::Intent(_) | Operation::Source { .. }) => {
                    self.preferred_x = None;
                    if let Operation::Source {
                        change: Some(change),
                        origin: moino_document::Origin::Typing,
                        ..
                    } = &operation
                        && change.start == change.old_end
                        && let Ok(at) = self.document.snapshot().text().byte_to_scalar(change.start)
                        && self.document.selection() != Selection::cursor(at)
                        && let Err(error) = self.document.set_selection(Selection::cursor(at))
                    {
                        self.error = Some(error.code().into());
                        continue;
                    }
                    let snapshot = self.document.snapshot();
                    let selection = self.document.selection();
                    let options = self.options.clone();
                    let now = cx.background_executor().now().duration_since(self.started);
                    let before = snapshot.revision();
                    let base = snapshot.clone();
                    self.worker = Some(cx.spawn_in(window, async move |this, cx| {
                        let result = cx
                            .background_spawn(async move {
                                let (transaction, selection) = match operation {
                                    Operation::Intent(intent) => {
                                        let outcome = moino_editing::resolve(
                                            &snapshot, selection, intent, &options,
                                        )
                                        .map_err(|error| error.code().to_owned())?;
                                        (outcome.clone().into_transaction(), outcome.selection())
                                    }
                                    Operation::Source {
                                        text,
                                        selection,
                                        change,
                                        origin,
                                    } => {
                                        if options.is_read_only()
                                            && snapshot.text().to_string() != text
                                        {
                                            return Err("editing.unavailable".to_owned());
                                        }
                                        (
                                            super::source::transaction(
                                                &snapshot, &text, selection, change, origin,
                                            ),
                                            selection,
                                        )
                                    }
                                    _ => unreachable!("only edit operations reach derivation"),
                                };
                                let edits =
                                    transaction.as_ref().map_or_else(Vec::new, |transaction| {
                                        transaction.edits().to_vec()
                                    });
                                let prepared = transaction
                                    .map(|transaction| {
                                        transaction
                                            .with_group(moino_document::HistoryGroup::new(1))
                                            .prepare(&snapshot)
                                    })
                                    .transpose()
                                    .map_err(|error| error.code().to_owned())?;
                                Ok::<_, String>((prepared, selection, edits))
                            })
                            .await;
                        let _ = this.update_in(cx, |this, window, cx| {
                            this.worker = None;
                            match result {
                                Ok((Some(prepared), _, edits)) => {
                                    this.remember_scroll_anchor();
                                    if let Err(error) = this.document.commit_prepared(prepared, now)
                                    {
                                        this.error = Some(error.code().into());
                                        this.pending.clear();
                                    } else {
                                        this.error = None;
                                        this.map_pending_selections(before, &edits);
                                    }
                                }
                                Ok((None, selection, _)) => {
                                    if let Err(error) = this.document.set_selection(selection) {
                                        this.error = Some(error.code().into());
                                    }
                                }
                                Err(error) => {
                                    if error == "model.invariant"
                                        && this.document.protect_snapshot(&base)
                                    {
                                        this.pending.clear();
                                    }
                                    this.error = Some(error.into());
                                }
                            }
                            this.refresh(window, cx);
                            this.start_next(window, cx);
                        }); // Closed surfaces discard outstanding derived work.
                    }));
                    return;
                }
                Operation::Undo | Operation::Redo => {
                    if self.is_read_only() {
                        self.error = Some("editing.unavailable".into());
                        cx.notify();
                        continue;
                    }
                    let request = if matches!(operation, Operation::Undo) {
                        self.document.undo_request()
                    } else {
                        self.document.redo_request()
                    };
                    let Some(request) = request else {
                        continue;
                    };
                    let base = request.snapshot().clone();
                    self.worker = Some(cx.spawn_in(window, async move |this, cx| {
                        let prepared = cx.background_spawn(async move { request.prepare() }).await;
                        let _ = this.update_in(cx, |this, window, cx| {
                            this.worker = None;
                            this.remember_scroll_anchor();
                            let mapped_anchor = prepared.as_ref().ok().and_then(|history| {
                                this.scroll_anchor.as_ref().map(|anchor| {
                                    history
                                        .map_selection(Selection::cursor(anchor.source))
                                        .head()
                                        .offset
                                })
                            });
                            let mapped = prepared.as_ref().ok().map(|history| {
                                this.pending
                                    .iter()
                                    .map(|operation| match operation {
                                        Operation::Select(selected, revision)
                                            if *revision == base.revision() =>
                                        {
                                            Some(history.map_selection(*selected))
                                        }
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                            });
                            let composition = prepared.as_ref().ok().and_then(|history| {
                                this.composition
                                    .as_ref()
                                    .filter(|composition| {
                                        !composition.follows_pending
                                            && composition.revision == base.revision()
                                    })
                                    .map(|composition| {
                                        history.map_selection(selection(
                                            composition.range.start,
                                            composition.range.end,
                                        ))
                                    })
                            });
                            if let Err(error) =
                                prepared.and_then(|prepared| this.document.apply_history(prepared))
                            {
                                if error == moino_document::DocumentError::Invariant
                                    && this.document.protect_snapshot(&base)
                                {
                                    this.pending.clear();
                                }
                                this.error = Some(error.code().into());
                            } else {
                                this.error = None;
                                if let Some(source) = mapped_anchor
                                    && let Some(anchor) = &mut this.scroll_anchor
                                {
                                    anchor.source = source;
                                }
                                if let Some(mapped) = mapped {
                                    let revision = this.document.snapshot().revision();
                                    for (operation, selected) in this.pending.iter_mut().zip(mapped)
                                    {
                                        if let Some(selected) = selected {
                                            *operation = Operation::Select(selected, revision);
                                        }
                                    }
                                    if let Some(mapped) = composition
                                        && let Some(composition) = &mut this.composition
                                    {
                                        composition.range = mapped.range();
                                        composition.revision = revision;
                                    }
                                }
                            }
                            this.refresh(window, cx);
                            this.start_next(window, cx);
                        }); // A dropped owner has no history target.
                    }));
                    return;
                }
            }
        }
    }

    fn finish_composition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(composition) = self.composition.take() {
            if !composition.follows_pending {
                self.enqueue(
                    self.selection_operation(selection(
                        composition.range.start,
                        composition.range.end,
                    )),
                    window,
                    cx,
                );
            }
            self.enqueue(
                Operation::Intent(Intent::Composition(composition.text)),
                window,
                cx,
            );
        }
    }

    fn select(&mut self, value: Selection, window: &mut Window, cx: &mut Context<Self>) {
        let composing = self.composition.is_some();
        self.finish_composition(window, cx);
        self.enqueue(
            if composing {
                Operation::SelectAfterPending(value)
            } else {
                self.selection_operation(value)
            },
            window,
            cx,
        );
    }

    fn hit(&self, point: Point<Pixels>) -> Option<ScalarOffset> {
        let geometry = self.geometry.borrow();
        let leaf = self
            .leaves
            .iter()
            .filter_map(|leaf| geometry.get(&leaf.key).map(|g| (leaf, g)))
            .min_by(|(_, a), (_, b)| {
                let distance = |g: &LeafGeometry| {
                    (point.y - point.y.clamp(g.bounds().top(), g.bounds().bottom())).abs()
                };
                distance(a)
                    .partial_cmp(&distance(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })?;
        Some(leaf.0.source(leaf.1.index_for_position(point)))
    }

    fn caret(&self, at: ScalarOffset) -> Option<Bounds<Pixels>> {
        let geometry = self.geometry.borrow();
        let leaf = self
            .leaves
            .iter()
            .rev()
            .find(|leaf| leaf.raw.start <= at && at <= leaf.raw.end)
            .or_else(|| self.leaves.iter().find(|leaf| at < leaf.raw.start))
            .or_else(|| self.leaves.last())?;
        let geometry = geometry.get(&leaf.key)?;
        Some(Bounds::new(
            geometry.position_for_index(leaf.byte(at, Affinity::After)),
            size(px(1.), geometry.caret_height()),
        ))
    }

    pub(super) fn decorations(
        &self,
        cx: &App,
    ) -> HashMap<u64, Vec<(std::ops::Range<usize>, Hsla)>> {
        let selected = self.document.selection().range();
        self.leaves
            .iter()
            .filter_map(|leaf| {
                let start = leaf.byte(selected.start.max(leaf.raw.start), Affinity::Before);
                let end = leaf.byte(selected.end.min(leaf.raw.end), Affinity::After);
                (selected.start < leaf.raw.end && leaf.raw.start < selected.end && start < end)
                    .then_some((leaf.key, vec![(start..end, cx.theme().selection)]))
            })
            .collect()
    }

    pub(super) fn paint_caret(&self, window: &mut Window, cx: &App) {
        if self.focus.is_focused(window)
            && let Some(bounds) = self.caret(self.display_head())
        {
            window.paint_quad(fill(bounds, cx.theme().foreground));
        }
    }

    pub(super) fn ensure_caret_visible(&mut self, cx: &mut Context<Self>) {
        if !self.reveal_caret {
            return;
        }
        let at = self.display_head();
        let ix = self
            .leaves
            .iter()
            .rposition(|leaf| leaf.raw.start <= at)
            .unwrap_or(0);
        if let Some(caret) = self.caret(at) {
            let viewport = self.list.viewport_bounds();
            let margin = caret.size.height;
            let delta = if caret.top() < viewport.top() + margin {
                caret.top() - viewport.top() - margin
            } else if caret.bottom() > viewport.bottom() - margin {
                caret.bottom() - viewport.bottom() + margin
            } else {
                self.reveal_caret = false;
                return;
            };
            self.list.scroll_by(delta);
        } else {
            self.list.scroll_to(ListOffset {
                item_ix: ix,
                offset_in_item: px(0.),
            });
        }
        cx.notify();
    }

    pub(super) fn remember_scroll_anchor(&mut self) {
        if self.scroll_anchor.is_some() {
            return;
        }
        let top = self.list.logical_scroll_top();
        let Some(leaf) = self.leaves.get(top.item_ix) else {
            return;
        };
        let geometry = self.geometry.borrow();
        let Some(geometry) = geometry.get(&leaf.key) else {
            return;
        };
        let byte = geometry.index_for_position(point(
            geometry.bounds().left(),
            self.list.viewport_bounds().top(),
        ));
        let at = leaf.source(byte);
        self.scroll_anchor = Some(ScrollAnchor {
            key: leaf.key,
            offset: ScalarOffset::new(at.value().saturating_sub(leaf.raw.start.value())),
            source: at,
            viewport_y: geometry.position_for_index(byte).y - self.list.viewport_bounds().top(),
        });
    }

    pub(super) fn restore_scroll_anchor(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(anchor) = self.scroll_anchor.take() else {
            return false;
        };
        let Some((ix, leaf)) = self
            .leaves
            .iter()
            .enumerate()
            .find(|(_, leaf)| leaf.key == anchor.key)
        else {
            return false;
        };
        let geometry = self.geometry.borrow();
        let Some(geometry) = geometry.get(&leaf.key) else {
            self.list.scroll_to(ListOffset {
                item_ix: ix,
                offset_in_item: px(0.),
            });
            self.scroll_anchor = Some(anchor);
            cx.notify();
            return true;
        };
        let at = ScalarOffset::new(
            leaf.raw
                .start
                .value()
                .saturating_add(anchor.offset.value())
                .min(leaf.raw.end.value()),
        );
        let delta = geometry
            .position_for_index(leaf.byte(at, Affinity::After))
            .y
            - self.list.viewport_bounds().top()
            - anchor.viewport_y;
        if delta.abs() <= px(0.5) {
            return false;
        }
        self.list.scroll_by(delta);
        cx.notify();
        true
    }

    fn render_leaf(&self, ix: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        let leaf = &self.leaves[ix];
        let context = RenderContext::from_app(cx)
            .with_style(moino_theme::text_style(cx))
            .on_link_click(|_, _, _, _| {})
            .with_body_font_size(moino_theme::body_size().to_pixels(window.rem_size()));
        let mut block = leaf
            .block
            .render_with_id(
                ("block", leaf.key),
                FlowPosition::at(&[&leaf.block], 0, true),
                &context,
                window,
                cx,
            )
            .into_any_element();
        for wrapper in leaf.prefix.iter().rev() {
            block = match wrapper {
                projection::Wrapper::Indent => div()
                    .w_full()
                    .min_w_0()
                    .pl(moino_theme::list_indent())
                    .child(block)
                    .into_any_element(),
                projection::Wrapper::Quote => div()
                    .w_full()
                    .min_w_0()
                    .border_l_2()
                    .border_color(effective_theme(cx).quote_border())
                    .text_color(effective_theme(cx).quote_text())
                    .pl(moino_theme::quote_inset())
                    .child(block)
                    .into_any_element(),
                projection::Wrapper::List(_, _) => {
                    unreachable!("only continuation containers precede a virtual leaf")
                }
            };
        }
        div()
            .id(("leaf", leaf.key))
            .test_support()
            .w_full()
            .flex()
            .justify_center()
            .child(
                div()
                    .w_full()
                    .max_w(moino_theme::content_width())
                    .px(moino_theme::content_inset())
                    .py_1()
                    .relative()
                    .child(
                        div()
                            .w_full()
                            .relative()
                            .when(leaf.heading_badge.is_some(), |this| this.pr_8())
                            .child(block)
                            .when(leaf.rule, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .right_0()
                                        .top(moino_theme::body_size() * 0.8)
                                        .border_b_1()
                                        .border_color(effective_theme(cx).rule()),
                                )
                            }),
                    )
                    .when_some(leaf.heading_badge, |this, level| {
                        this.child(
                            div()
                                .absolute()
                                .top_1()
                                .right(moino_theme::content_inset())
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .when(level >= 3, |badge| {
                                    badge
                                        .border_1()
                                        .border_color(effective_theme(cx).heading_badge_border())
                                        .text_color(effective_theme(cx).heading_badge_text())
                                        .px_1()
                                        .rounded_sm()
                                })
                                .child(format!("H{level}")),
                        )
                    }),
            )
            .into_any_element()
    }

    fn move_horizontal(
        &mut self,
        forward: bool,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_composition(window, cx);
        self.enqueue(Operation::Navigate { forward, extend }, window, cx);
    }

    fn horizontal_selection(&self, forward: bool, extend: bool) -> Option<Selection> {
        let current = self.document.selection();
        let text = self.document.snapshot();
        let head = if !extend && !current.is_collapsed() {
            Some(if forward {
                current.range().end
            } else {
                current.range().start
            })
        } else if forward {
            text.text().next_grapheme(current.head().offset).ok()
        } else {
            text.text().previous_grapheme(current.head().offset).ok()
        };
        head.map(|head| {
            selection(
                if extend {
                    current.anchor().offset
                } else {
                    head
                },
                head,
            )
        })
    }
}

pub(super) fn selection(anchor: ScalarOffset, head: ScalarOffset) -> Selection {
    Selection::new(
        Endpoint {
            offset: anchor,
            affinity: Affinity::After,
        },
        Endpoint {
            offset: head,
            affinity: Affinity::After,
        },
    )
}

fn effective_theme(cx: &App) -> moino_theme::BuiltinTheme {
    cx.try_global::<moino_theme::ThemeState>()
        .map_or(moino_theme::BuiltinTheme::Paper, |state| state.theme())
}

/// Registers document commands for the live surface.
pub fn init(cx: &mut App) {
    let primary_binding = |key: &str, action: Box<dyn Action>| {
        KeyBinding::load(
            key,
            action,
            Some(std::rc::Rc::new(
                KeyBindingContextPredicate::parse("MoinoEditor").expect("static editor context"),
            )),
            false,
            None,
            cx,
        )
        .expect("static primary binding")
    };
    let primary = [
        primary_binding("secondary-z", Box::new(Undo)),
        primary_binding("secondary-shift-z", Box::new(Redo)),
        primary_binding("secondary-a", Box::new(SelectAll)),
        primary_binding("secondary-c", Box::new(Copy)),
        primary_binding("secondary-x", Box::new(Cut)),
        primary_binding("secondary-v", Box::new(Paste)),
        primary_binding("secondary-b", Box::new(commands::Bold)),
        primary_binding("secondary-i", Box::new(commands::Italic)),
        primary_binding("secondary-home", Box::new(MoveToStart)),
        primary_binding("secondary-end", Box::new(MoveToEnd)),
        primary_binding("secondary-shift-home", Box::new(SelectToStart)),
        primary_binding("secondary-shift-end", Box::new(SelectToEnd)),
    ];
    cx.bind_keys(primary);
    for (key, action) in [
        ("secondary-z", Box::new(Undo) as Box<dyn Action>),
        ("secondary-shift-z", Box::new(Redo) as Box<dyn Action>),
        (
            "secondary-home",
            Box::new(gpui_kit::component::input::MoveToStart) as Box<dyn Action>,
        ),
        (
            "secondary-end",
            Box::new(gpui_kit::component::input::MoveToEnd) as Box<dyn Action>,
        ),
        (
            "secondary-shift-home",
            Box::new(gpui_kit::component::input::SelectToStart) as Box<dyn Action>,
        ),
        (
            "secondary-shift-end",
            Box::new(gpui_kit::component::input::SelectToEnd) as Box<dyn Action>,
        ),
    ] {
        cx.bind_keys([KeyBinding::load(
            key,
            action,
            Some(std::rc::Rc::new(
                KeyBindingContextPredicate::parse("MoinoSource").expect("static source context"),
            )),
            false,
            None,
            cx,
        )
        .expect("static source history binding")]);
    }
    cx.bind_keys([
        KeyBinding::new("enter", Split, Some("MoinoEditor")),
        KeyBinding::new("backspace", DeleteBackward, Some("MoinoEditor")),
        KeyBinding::new("delete", DeleteForward, Some("MoinoEditor")),
        KeyBinding::new("tab", Indent, Some("MoinoEditor")),
        KeyBinding::new("shift-tab", Outdent, Some("MoinoEditor")),
    ]);
}

mod input;
mod render;
#[cfg(test)]
mod tests;
