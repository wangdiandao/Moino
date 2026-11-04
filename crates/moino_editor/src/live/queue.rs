//! One ordered event stream owns text, block transitions, selection and history.
use super::*;
use moino_document::{DocumentError, Origin, Transaction};

pub(super) enum Operation {
    Intent(Intent),
    Source {
        text: String,
        selection: Selection,
        change: Option<crate::source_change::SourceChange>,
        origin: Origin,
    },
    Select(Selection, moino_document::Revision),
    SelectAfterPending(Selection),
    Extend(ScalarOffset, moino_document::Revision),
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
    CommitBlock,
    SettleSelection,
    Undo,
    Redo,
}

impl LiveEditor {
    pub(super) fn enqueue(
        &mut self,
        operation: Operation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reveal_caret = true;
        self.pending.push_back(operation);
        self.start_next(window, cx);
    }

    pub(super) fn selection_operation(&self, selected: Selection) -> Operation {
        Operation::Select(selected, self.document.snapshot().revision())
    }

    fn change_selection(&mut self, selected: Selection) -> Result<(), DocumentError> {
        if self.frozen_selection.is_none() && self.session.leaves(selected) {
            self.finalize_block()?;
        }
        if selected != self.document.selection() {
            self.document.set_selection(selected)?;
        }
        Ok(())
    }

    fn finalize_block(&mut self) -> Result<(), DocumentError> {
        if self.session.has_conversion(&self.document.snapshot()) {
            self.session.remember(&self.document);
            let before = self.document.snapshot().revision();
            // A semantic checkpoint carries no source edits and does no parsing.
            self.document.commit(
                Transaction::new(&self.document.snapshot(), Origin::Command),
                std::time::Duration::ZERO,
            )?;
            self.map_pending_selections(before, &[]);
        }
        self.session.finish(&self.document);
        self.document.break_history();
        Ok(())
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
            if let Operation::Extend(head, revision) = operation
                && *revision == before
            {
                *head = Selection::cursor(*head).mapped(edits).head().offset;
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

    fn yield_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.waiting_for_frame {
            return;
        }
        self.waiting_for_frame = true;
        let owner = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            let _ = owner.update(cx, |editor, cx| {
                editor.waiting_for_frame = false;
                editor.start_next(window, cx);
            });
        });
        cx.notify();
    }

    pub(super) fn start_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.worker.is_some() || self.waiting_for_frame {
            return;
        }
        let started = cx.background_executor().now();
        while let Some(operation) = self.pending.pop_front() {
            if cx.background_executor().now().duration_since(started)
                >= std::time::Duration::from_millis(8)
            {
                self.pending.push_front(operation);
                self.yield_queue(window, cx);
                return;
            }
            let selected = self.document.selection();
            let result = match operation {
                operation @ (Operation::Intent(_) | Operation::Source { .. }) => {
                    self.prepare_edit(operation, window, cx);
                    return;
                }
                Operation::Undo | Operation::Redo => {
                    if self.replay(matches!(operation, Operation::Redo), window, cx) {
                        return;
                    }
                    continue;
                }
                Operation::CommitBlock => self.finalize_block(),
                Operation::SettleSelection => {
                    if self.session.leaves(self.document.selection()) {
                        self.finalize_block()
                    } else {
                        Ok(())
                    }
                }
                Operation::Select(value, revision) => {
                    if revision != self.document.snapshot().revision() {
                        Err(DocumentError::Stale)
                    } else {
                        self.change_selection(value)
                    }
                }
                Operation::SelectAfterPending(value) => self.change_selection(value),
                Operation::Extend(head, revision) => {
                    if revision != self.document.snapshot().revision() {
                        Err(DocumentError::Stale)
                    } else {
                        self.change_selection(selection(selected.anchor().offset, head))
                    }
                }
                Operation::SelectAll => self.change_selection(selection(
                    ScalarOffset::new(0),
                    self.document.snapshot().text().len(),
                )),
                Operation::DocumentEdge { end, extend } => {
                    self.preferred_x = None;
                    let head = self.content_position(if end {
                        self.document.snapshot().text().len()
                    } else {
                        ScalarOffset::new(0)
                    });
                    self.change_selection(selection(
                        if extend {
                            selected.anchor().offset
                        } else {
                            head
                        },
                        head,
                    ))
                }
                Operation::Navigate { forward, extend } => {
                    self.preferred_x = None;
                    self.horizontal_selection(forward, extend)
                        .map_or(Ok(()), |value| self.change_selection(value))
                }
                operation @ (Operation::Vertical { .. } | Operation::LineEdge { .. }) => {
                    let Some(caret) = self.caret(selected.head().offset) else {
                        self.pending.push_front(operation);
                        self.yield_queue(window, cx);
                        return;
                    };
                    let (target, extend) = match operation {
                        Operation::Vertical { forward, extend } => {
                            let x = *self.preferred_x.get_or_insert(caret.left());
                            let height = self
                                .geometry
                                .borrow()
                                .values()
                                .find(|geometry| geometry.bounds().contains(&caret.origin))
                                .map_or(caret.size.height, LeafGeometry::line_height);
                            (
                                point(x, caret.top() + height * if forward { 1.5 } else { -0.5 }),
                                extend,
                            )
                        }
                        Operation::LineEdge { end, extend } => {
                            self.preferred_x = None;
                            (
                                point(
                                    if end {
                                        self.list.viewport_bounds().right()
                                    } else {
                                        self.list.viewport_bounds().left()
                                    },
                                    caret.top(),
                                ),
                                extend,
                            )
                        }
                        _ => unreachable!("visual movement"),
                    };
                    self.hit(target).map_or(Ok(()), |head| {
                        self.change_selection(selection(
                            if extend {
                                selected.anchor().offset
                            } else {
                                head
                            },
                            head,
                        ))
                    })
                }
                Operation::Copy { cut } => {
                    self.document.break_history();
                    let range = selected.range();
                    if !range.is_empty()
                        && let Ok(text) = self.document.snapshot().text().slice(range)
                    {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                        if cut {
                            self.pending
                                .push_front(Operation::Intent(Intent::Paste(String::new())));
                        }
                    }
                    Ok(())
                }
            };
            if let Err(error) = result {
                self.error = Some(error.code().into());
            }
            self.refresh(window, cx);
        }
    }

    fn prepare_edit(&mut self, operation: Operation, window: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if let Operation::Intent(intent) = &operation {
            let selected = self.document.selection();
            if selected.is_collapsed() {
                let at = self.content_position(selected.head().offset);
                if let Err(error) = self.change_selection(Selection::cursor(at)) {
                    self.error = Some(error.code().into());
                    self.start_next(window, cx);
                    return;
                }
            }
            self.session.begin(&self.document, intent);
        }
        let keep_draft = matches!(
            &operation,
            Operation::Intent(
                Intent::Insert(_)
                    | Intent::Composition(_)
                    | Intent::Backspace
                    | Intent::Delete
                    | Intent::Inline(_)
                    | Intent::Tab { .. }
            )
        );
        if let Operation::Source {
            change: Some(change),
            origin: Origin::Typing | Origin::Composition,
            ..
        } = &operation
            && change.start == change.old_end
            && let Ok(at) = self.document.snapshot().text().byte_to_scalar(change.start)
            && self.document.selection() != Selection::cursor(at)
        {
            let _ = self.document.set_selection(Selection::cursor(at));
        }
        let snapshot = self.document.snapshot();
        let selected = self.document.selection();
        let mut options = self.options.clone();
        if let Some(range) = self.session.pending() {
            options = options.with_pending_block(range);
        }
        let base = snapshot.clone();
        let before = snapshot.revision();
        let tick = cx.background_executor().now().duration_since(self.started);
        self.worker = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let (transaction, selected) = match operation {
                        Operation::Intent(intent) => {
                            let outcome =
                                moino_editing::resolve(&snapshot, selected, intent, &options)
                                    .map_err(|error| error.code().to_owned())?;
                            let selected = outcome.selection();
                            (outcome.into_transaction(), selected)
                        }
                        Operation::Source {
                            text,
                            selection,
                            change,
                            origin,
                        } => {
                            if options.is_read_only() && snapshot.text().to_string() != text {
                                return Err("editing.unavailable".to_owned());
                            }
                            (
                                crate::source::transaction(
                                    &snapshot, &text, selection, change, origin,
                                ),
                                selection,
                            )
                        }
                        _ => unreachable!("edit preparation"),
                    };
                    let edits = transaction
                        .as_ref()
                        .map_or_else(Vec::new, |transaction| transaction.edits().to_vec());
                    let prepared = transaction
                        .map(|transaction| {
                            transaction
                                .with_group(moino_document::HistoryGroup::new(1))
                                .prepare(&snapshot)
                        })
                        .transpose()
                        .map_err(|error| error.code().to_owned())?;
                    Ok((prepared, selected, edits))
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.worker = None;
                let applied = match result {
                    Ok((Some(prepared), _, edits)) => {
                        this.remember_scroll_anchor();
                        this.document
                            .commit_prepared(prepared, tick)
                            .map(|_| {
                                this.map_pending_selections(before, &edits);
                                this.session.applied(&this.document, &edits, keep_draft);
                            })
                            .map_err(|error| error.code().to_owned())
                    }
                    Ok((None, selected, _)) => this
                        .change_selection(selected)
                        .map_err(|error| error.code().to_owned()),
                    Err(error) => Err(error),
                };
                match applied {
                    Ok(()) => this.error = None,
                    Err(error) => {
                        this.session.restore(&this.document);
                        if error == "model.invariant" && this.document.protect_snapshot(&base) {
                            this.pending.clear();
                        }
                        this.error = Some(error.into());
                    }
                }
                this.refresh(window, cx);
                this.start_next(window, cx);
            });
        }));
    }

    fn replay(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.is_read_only() {
            self.error = Some("editing.unavailable".into());
            cx.notify();
            return false;
        }
        let request = if redo {
            self.document.redo_request()
        } else {
            self.document.undo_request()
        };
        let Some(request) = request else {
            return false;
        };
        let base = request.snapshot().clone();
        self.worker = Some(cx.spawn_in(window, async move |this, cx| {
            let prepared = cx.background_spawn(async move { request.prepare() }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.worker = None;
                this.remember_scroll_anchor();
                match prepared {
                    Ok(prepared) => {
                        let mapped: Vec<_> = this
                            .pending
                            .iter()
                            .map(|operation| match operation {
                                Operation::Select(selected, revision)
                                    if *revision == base.revision() =>
                                {
                                    Some(prepared.map_selection(*selected))
                                }
                                _ => None,
                            })
                            .collect();
                        let anchor = this.scroll_anchor.as_ref().map(|anchor| {
                            prepared
                                .map_selection(Selection::cursor(anchor.source))
                                .head()
                                .offset
                        });
                        let composition = this
                            .composition
                            .as_ref()
                            .filter(|composition| {
                                !composition.follows_pending
                                    && composition.revision == base.revision()
                            })
                            .map(|composition| {
                                prepared
                                    .map_selection(selection(
                                        composition.range.start,
                                        composition.range.end,
                                    ))
                                    .range()
                            });
                        match this.document.apply_history(prepared) {
                            Ok(()) => {
                                this.error = None;
                                this.session.restore(&this.document);
                                let revision = this.document.snapshot().revision();
                                for (operation, selected) in this.pending.iter_mut().zip(mapped) {
                                    if let Some(selected) = selected {
                                        *operation = Operation::Select(selected, revision);
                                    }
                                }
                                if let (Some(anchor), Some(source)) =
                                    (&mut this.scroll_anchor, anchor)
                                {
                                    anchor.source = source;
                                }
                                if let (Some(composition), Some(range)) =
                                    (&mut this.composition, composition)
                                {
                                    composition.range = range;
                                    composition.revision = revision;
                                }
                            }
                            Err(error) => this.error = Some(error.code().into()),
                        }
                    }
                    Err(error) => {
                        if error == DocumentError::Invariant
                            && this.document.protect_snapshot(&base)
                        {
                            this.pending.clear();
                        }
                        this.error = Some(error.code().into());
                    }
                }
                this.refresh(window, cx);
                this.start_next(window, cx);
            });
        }));
        true
    }
}
