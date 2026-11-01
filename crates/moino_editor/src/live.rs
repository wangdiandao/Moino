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
    ]
);

enum Operation {
    Intent(Intent),
    Source { text: String, selection: Selection },
    Select(Selection, moino_document::Revision),
    Navigate { forward: bool, extend: bool },
    Vertical { forward: bool, extend: bool },
    LineEdge { end: bool, extend: bool },
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
    composition: Option<Composition>,
    dragging: bool,
    error: Option<SharedString>,
    list: ListState,
    reveal_caret: bool,
    preferred_x: Option<Pixels>,
    started: std::time::Instant,
    source_text: SharedString,
    projection_theme: moino_theme::BuiltinTheme,
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
        let leaves = projection::project(
            &document.snapshot(),
            document.selection(),
            projection_theme.marker(),
        );
        let list = ListState::new(leaves.len(), ListAlignment::Top, px(0.));
        Self {
            document,
            options,
            focus: cx.focus_handle().tab_stop(true),
            leaves,
            geometry: Rc::default(),
            pending: VecDeque::new(),
            worker: None,
            composition: None,
            dragging: false,
            error: None,
            list,
            reveal_caret: true,
            preferred_x: None,
            started: cx.background_executor().now(),
            source_text: source.into(),
            projection_theme,
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
        if let Some(composition) = &mut self.composition
            && composition.follows_pending
        {
            composition.range = self.document.selection().range();
            composition.revision = self.document.snapshot().revision();
        }
        self.source_text = self.document.snapshot().text().to_string().into();
        let top = self.list.logical_scroll_top();
        let key = self.leaves.get(top.item_ix).map(|leaf| leaf.key);
        self.leaves = if let Some(composition) = &self.composition {
            let display = self.display_text();
            let start = display
                .to_byte(composition.range.start)
                .expect("preedit boundary")
                .value();
            vec![Leaf::plain(
                &display.to_string(),
                Some(start..start + composition.text.len()),
            )]
        } else {
            projection::project(
                &self.document.snapshot(),
                self.document.selection(),
                self.projection_theme.marker(),
            )
        };
        self.list.reset(self.leaves.len());
        if let Some(ix) = key.and_then(|key| self.leaves.iter().position(|leaf| leaf.key == key)) {
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
        self.enqueue(Operation::Source { text, selection }, window, cx);
    }
    /// Moves the shared source selection without modifying text.
    pub fn select_source(
        &mut self,
        selection: Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select(selection, window, cx);
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
    }

    fn defer_visual_move(
        &mut self,
        operation: Operation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending.push_front(operation);
        let editor = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            let _ = editor.update(cx, |editor, cx| editor.start_next(window, cx)); // A closed surface has no navigation target.
        });
        cx.notify();
    }

    fn start_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.worker.is_some() {
            return;
        }
        while let Some(operation) = self.pending.pop_front() {
            match operation {
                Operation::Vertical { forward, extend } => {
                    let current = self.document.selection();
                    if let Some(caret) = self.caret(current.head().offset) {
                        let x = *self.preferred_x.get_or_insert(caret.left());
                        let target = point(
                            x,
                            caret.top()
                                + if forward {
                                    caret.size.height * 1.6
                                } else {
                                    -caret.size.height * 0.6
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
                    let snapshot = self.document.snapshot();
                    let selection = self.document.selection();
                    let options = self.options.clone();
                    let now = cx.background_executor().now().duration_since(self.started);
                    let before = snapshot.revision();
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
                                    Operation::Source { text, selection } => {
                                        if options.is_read_only() {
                                            return Err("editing.unavailable".to_owned());
                                        }
                                        (
                                            super::source::transaction(&snapshot, &text, selection),
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
                                Err(error) => this.error = Some(error.into()),
                            }
                            this.refresh(window, cx);
                            this.start_next(window, cx);
                        }); // Closed surfaces discard outstanding derived work.
                    }));
                    return;
                }
                Operation::Undo | Operation::Redo => {
                    let request = if matches!(operation, Operation::Undo) {
                        self.document.undo_request()
                    } else {
                        self.document.redo_request()
                    };
                    let Some(request) = request else {
                        continue;
                    };
                    self.worker = Some(cx.spawn_in(window, async move |this, cx| {
                        let prepared = cx.background_spawn(async move { request.prepare() }).await;
                        let _ = this.update_in(cx, |this, window, cx| {
                            this.worker = None;
                            if let Err(error) =
                                prepared.and_then(|prepared| this.document.apply_history(prepared))
                            {
                                this.error = Some(error.code().into());
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
        self.finish_composition(window, cx);
        self.enqueue(self.selection_operation(value), window, cx);
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
        let at = self.native_selection().and_then(|selected| {
            self.display_text()
                .utf16_to_scalar(Utf16Offset::new(selected.range.end))
                .ok()
        });
        if self.focus.is_focused(window)
            && let Some(at) = at
            && let Some(bounds) = self.caret(at)
        {
            window.paint_quad(fill(bounds, cx.theme().foreground));
        }
    }

    pub(super) fn ensure_caret_visible(&mut self, cx: &mut Context<Self>) {
        if !self.reveal_caret {
            return;
        }
        let at = self
            .native_selection()
            .and_then(|selection| {
                self.display_text()
                    .utf16_to_scalar(Utf16Offset::new(selection.range.end))
                    .ok()
            })
            .unwrap_or_else(|| self.document.selection().head().offset);
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

    fn render_leaf(&self, ix: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        let leaf = &self.leaves[ix];
        let context = RenderContext::from_app(cx)
            .with_style(moino_theme::text_style(cx))
            .on_link_click(|_, _, _, _| {})
            .with_body_font_size(moino_theme::body_size().to_pixels(window.rem_size()));
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
                            .when(leaf.heading_badge.is_some(), |this| this.pr_8())
                            .child(leaf.block.render_with_id(
                                ("block", leaf.key),
                                FlowPosition::at(&[&leaf.block], 0, true),
                                &context,
                                window,
                                cx,
                            )),
                    )
                    .when_some(leaf.heading_badge, |this, level| {
                        this.child(
                            div()
                                .absolute()
                                .top_1()
                                .right(moino_theme::content_inset())
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
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
    ];
    cx.bind_keys(primary);
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
