//! Pointer gestures retain their initial geometry until release and restore logical endpoints.
use super::*;
use std::time::Duration;

impl LiveEditor {
    pub(super) fn pointer_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pointer_refresh = None;
        self.focus.focus(window, cx);
        self.frozen_selection = Some(self.document.selection());
        self.drag_position = Some(event.position);
        let Some(at) = self.hit(event.position) else {
            self.finish_composition(window, cx);
            return;
        };
        let selected = if event.click_count >= 3 {
            self.leaves
                .iter()
                .rev()
                .find(|leaf| leaf.raw.start <= at && at <= leaf.raw.end)
                .map(|leaf| selection(leaf.raw.start, leaf.raw.end))
        } else if event.click_count == 2 {
            self.document
                .snapshot()
                .text()
                .words()
                .into_iter()
                .find(|range| range.start <= at && at < range.end)
                .map(|range| selection(range.start, range.end))
        } else {
            Some(selection(
                if event.modifiers.shift {
                    self.document.selection().anchor().offset
                } else {
                    at
                },
                at,
            ))
        };
        if let Some(selected) = selected {
            self.select(selected, window, cx);
        }
        if event.click_count > 1 {
            self.frozen_selection = None;
            self.enqueue(Operation::SettleSelection, window, cx);
        }
    }

    pub(super) fn pointer_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(previous) = self.drag_position else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.pointer_up(window, cx);
            return;
        }
        if event.position == previous {
            return;
        }
        self.drag_position = Some(event.position);
        if let Some(at) = self.hit(event.position) {
            self.enqueue(
                Operation::Extend(at, self.document.snapshot().revision()),
                window,
                cx,
            );
        }
    }

    pub(super) fn pointer_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.drag_position.take().is_none() {
            return;
        }
        self.pointer_refresh = Some(cx.spawn_in(window, async |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.frozen_selection = None;
                this.refresh(window, cx);
            });
            cx.background_executor()
                .timer(Duration::from_millis(170))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.enqueue(Operation::SettleSelection, window, cx)
            });
        }));
    }

    pub(super) fn keyboard_boundary(&mut self) {
        self.pointer_refresh = None;
        self.drag_position = None;
        self.frozen_selection = None;
    }
}
