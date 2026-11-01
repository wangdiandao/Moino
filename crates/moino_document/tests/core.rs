//! Transaction and history invariants exercised through public APIs.
use moino_document::*;
use moino_text::ScalarOffset;
use proptest::prelude::*;
use std::time::Duration;
fn p(value: usize) -> ScalarOffset {
    ScalarOffset::new(value)
}
fn document(text: &str) -> Document {
    Document::new(DocumentId::new(1), Epoch::new(0), text)
}

#[test]
// Spec: docs/contracts/document-model.md#事务契约
fn rejects_stale_overlapping_and_out_of_bounds_commits_atomically() {
    let mut doc = document("a墨😀z");
    let snapshot = doc.snapshot();
    for edits in [
        vec![Edit::new(p(1)..p(3), "x"), Edit::new(p(2)..p(4), "y")],
        vec![Edit::new(p(0)..p(5), "x")],
        vec![Edit::new(p(1)..p(1), "x"), Edit::new(p(1)..p(1), "y")],
        vec![Edit::new(p(3)..p(1), "x")],
    ] {
        let mut transaction = Transaction::new(&snapshot, Origin::Command);
        for edit in edits {
            transaction = transaction.with_edit(edit);
        }
        assert_eq!(
            doc.commit(transaction, Duration::ZERO).expect_err("reject"),
            DocumentError::Invalid
        );
        assert_eq!(doc.snapshot().revision(), Revision::new(0));
        assert_eq!(doc.snapshot().text().to_string(), "a墨😀z");
        assert!(!doc.has_undo());
    }
    let invalid_selection = Transaction::new(&snapshot, Origin::Command)
        .with_edit(Edit::new(p(0)..p(1), ""))
        .with_selection(Selection::cursor(p(4)));
    assert!(doc.commit(invalid_selection, Duration::ZERO).is_err());
    let transaction =
        Transaction::new(&snapshot, Origin::Command).with_edit(Edit::new(p(1)..p(2), "中"));
    let event = doc
        .commit(transaction.clone(), Duration::ZERO)
        .expect("commit");
    assert_eq!(event.revision(), Revision::new(1));
    assert_eq!(event.affected(), p(1)..p(2));
    assert_eq!(
        doc.commit(transaction, Duration::ZERO).expect_err("stale"),
        DocumentError::Stale
    );
    assert_eq!(snapshot.text().to_string(), "a墨😀z");
    assert_eq!(snapshot.cst().serialize(), "a墨😀z");
    assert!(doc.undo().expect("undo"));
    assert_eq!(doc.snapshot().revision(), Revision::new(2));
    assert!(!doc.is_dirty());
    assert!(doc.redo().expect("redo"));
    assert!(doc.is_dirty());
}
#[test]
// Spec: docs/contracts/history.md#撤销单元
fn merge_timeout_boundaries_and_output_baseline_are_independent() {
    let mut doc = document("");
    for (index, tick) in [0, 1000, 2001].into_iter().enumerate() {
        let tx = Transaction::new(&doc.snapshot(), Origin::Typing)
            .with_edit(Edit::new(p(index)..p(index), "x"))
            .with_group(HistoryGroup::new(1));
        doc.commit(tx, Duration::from_millis(tick)).expect("typing");
    }
    doc.undo().expect("third input is separate");
    assert_eq!(doc.snapshot().text().to_string(), "xx");
    doc.undo().expect("first two merge at exact threshold");
    assert_eq!(doc.snapshot().text().to_string(), "");
    assert!(!doc.undo().expect("empty"));
    doc.redo().expect("redo");
    doc.mark_saved();
    assert!(!doc.is_dirty());
    doc.with_output_properties(vec![1]);
    assert!(doc.is_dirty());
    doc.mark_saved();
    assert!(!doc.is_dirty());
    doc.with_output_properties(vec![]);
    assert!(doc.is_dirty());
    let tx =
        Transaction::new(&doc.snapshot(), Origin::Command).with_edit(Edit::new(p(0)..p(0), "y"));
    doc.commit(tx, Duration::ZERO).expect("command");
    assert!(!doc.has_redo());
    assert!(!doc.is_protected());
    assert_eq!(DocumentError::Stale.code(), "document.stale_transaction");
    assert_eq!(
        DocumentError::Invariant.message_key(),
        "error.model.invariant"
    );
    assert!(!DocumentError::Invalid.is_retryable());
}
#[test]
// Spec: docs/contracts/document-model.md#选择与映射
fn maps_affinities_deletions_and_reversed_selections() {
    let before = Endpoint {
        offset: p(2),
        affinity: Affinity::Before,
    };
    let after = Endpoint {
        offset: p(2),
        affinity: Affinity::After,
    };
    let selection = Selection::new(after, before).mapped(&[Edit::new(p(2)..p(2), "墨😀")]);
    assert_eq!(selection.anchor().offset, p(4));
    assert_eq!(selection.head().offset, p(2));
    assert!(!selection.is_forward());
    let selection = Selection::new(
        Endpoint {
            offset: p(4),
            affinity: Affinity::After,
        },
        Endpoint {
            offset: p(2),
            affinity: Affinity::After,
        },
    );
    let mapped = selection.mapped(&[Edit::new(p(1)..p(5), "new")]);
    assert_eq!(mapped.range(), p(1)..p(1));
    let end = Selection::cursor(p(5)).mapped(&[Edit::new(p(1)..p(5), "new")]);
    assert_eq!(end.head().offset, p(4));
}
#[test]
// Spec: docs/contracts/history.md#历史预算
fn discards_only_oldest_history_units() {
    let mut doc = document("");
    for _ in 0..1001 {
        let tx = Transaction::new(&doc.snapshot(), Origin::Command)
            .with_edit(Edit::new(p(0)..p(0), "a"));
        doc.commit(tx, Duration::ZERO).expect("command");
    }
    for _ in 0..1000 {
        assert!(doc.undo().expect("undo"));
    }
    assert!(!doc.undo().expect("oldest discarded"));
    assert_eq!(doc.snapshot().text().to_string(), "a");
}
#[test]
// Spec: docs/contracts/document-model.md#事务契约
fn offline_candidates_reject_stale_results_and_preserve_snapshots() {
    let mut doc = document("墨");
    let snapshot = doc.snapshot();
    let clone = snapshot.clone();
    assert_eq!(
        snapshot.cst().nodes().as_ptr(),
        clone.cst().nodes().as_ptr()
    );
    let first = Transaction::new(&snapshot, Origin::Typing)
        .with_edit(Edit::new(p(1)..p(1), "a"))
        .prepare(&snapshot)
        .expect("prepare first");
    let stale = Transaction::new(&snapshot, Origin::Typing)
        .with_edit(Edit::new(p(1)..p(1), "b"))
        .prepare(&snapshot)
        .expect("prepare competing result");
    doc.commit_prepared(first, Duration::ZERO).expect("publish");
    assert_eq!(
        doc.commit_prepared(stale, Duration::ZERO)
            .expect_err("stale result"),
        DocumentError::Stale
    );
    assert!(!doc.protect_snapshot(&snapshot));
    assert!(!doc.is_protected());
    let request = doc.undo_request().expect("history request");
    assert_eq!(request.snapshot().revision(), doc.snapshot().revision());
    let stale_history = request.prepare().expect("offline undo");
    let tx =
        Transaction::new(&doc.snapshot(), Origin::Command).with_edit(Edit::new(p(0)..p(0), "new"));
    doc.commit(tx, Duration::ZERO).expect("new edit");
    assert_eq!(
        doc.apply_history(stale_history).expect_err("stale history"),
        DocumentError::Stale
    );
    let undo = doc
        .undo_request()
        .expect("request")
        .prepare()
        .expect("prepare");
    doc.apply_history(undo).expect("publish undo");
    let redo = doc
        .redo_request()
        .expect("request")
        .prepare()
        .expect("prepare");
    doc.apply_history(redo).expect("publish redo");
    assert_eq!(doc.snapshot().text().to_string(), "new墨a");
    assert_eq!(snapshot.text().to_string(), "墨");
    assert!(doc.protect_snapshot(&doc.snapshot()));
    assert!(doc.is_protected());
    assert_eq!(
        doc.undo().expect_err("protected history"),
        DocumentError::Invariant
    );
    let tx =
        Transaction::new(&doc.snapshot(), Origin::Typing).with_edit(Edit::new(p(0)..p(0), "x"));
    assert_eq!(
        doc.commit(tx, Duration::ZERO)
            .expect_err("protected commit"),
        DocumentError::Invariant
    );
}
#[test]
// Spec: docs/contracts/history.md#历史边界
fn direction_changes_and_explicit_selection_break_typing_merge() {
    let mut doc = document("");
    for (index, direction) in [HistoryDirection::Forward, HistoryDirection::Backward]
        .into_iter()
        .enumerate()
    {
        let tx = Transaction::new(&doc.snapshot(), Origin::Typing)
            .with_edit(Edit::new(p(index)..p(index), "x"))
            .with_group(HistoryGroup::new(1))
            .with_direction(direction);
        doc.commit(tx, Duration::ZERO).expect("type");
    }
    doc.undo().expect("separate directions");
    assert_eq!(doc.snapshot().text().to_string(), "x");
    doc.set_selection(Selection::cursor(p(1)))
        .expect("selection boundary");
    let tx = Transaction::new(&doc.snapshot(), Origin::Typing)
        .with_edit(Edit::new(p(1)..p(1), "y"))
        .with_group(HistoryGroup::new(1));
    doc.commit(tx, Duration::ZERO).expect("type");
    doc.undo().expect("new group");
    assert_eq!(doc.snapshot().text().to_string(), "x");
    assert_eq!(DocumentId::new(7).value(), 7);
    assert_eq!(Epoch::new(2).value(), 2);
    assert_eq!(Revision::new(3).value(), 3);
    assert_eq!(HistoryGroup::new(4).value(), 4);
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#增量解析与节点身份
fn actual_edit_coordinates_preserve_duplicate_and_nested_block_identities() {
    let mut doc = document("same\n\nsame\n");
    let old: Vec<_> = doc
        .snapshot()
        .cst()
        .nodes()
        .iter()
        .filter(|node| node.syntax() == &moino_markdown::Syntax::Paragraph)
        .map(|node| node.id())
        .collect();
    let tx = Transaction::new(&doc.snapshot(), Origin::Command)
        .with_edit(Edit::new(p(0)..p(0), "same\n\n"));
    doc.commit(tx, Duration::ZERO)
        .expect("insert duplicate before original");
    let ids: Vec<_> = doc
        .snapshot()
        .cst()
        .nodes()
        .iter()
        .filter(|node| node.syntax() == &moino_markdown::Syntax::Paragraph)
        .map(|node| node.id())
        .collect();
    assert_ne!(ids[0], old[0]);
    assert_eq!(&ids[1..], &old);
    doc.undo().expect("undo duplicate insertion");
    let restored: Vec<_> = doc
        .snapshot()
        .cst()
        .nodes()
        .iter()
        .filter(|node| node.syntax() == &moino_markdown::Syntax::Paragraph)
        .map(|node| node.id())
        .collect();
    assert_eq!(restored, old);
    let mut doc = document("a\n\nb\n\nc\n");
    let old = doc.snapshot();
    let middle = old
        .cst()
        .nodes()
        .iter()
        .find(|node| node.text() == "b")
        .expect("middle paragraph")
        .id();
    let tx = Transaction::new(&old, Origin::Command)
        .with_edit(Edit::new(p(0)..p(1), "A"))
        .with_edit(Edit::new(p(6)..p(7), "C"));
    doc.commit(tx, Duration::ZERO).expect("disjoint edits");
    assert_eq!(
        doc.snapshot()
            .cst()
            .nodes()
            .iter()
            .find(|node| node.text() == "b")
            .expect("unchanged middle")
            .id(),
        middle
    );
    let mut doc = document("- a\n- b\n");
    let old = doc.snapshot();
    let sibling = old.cst().nodes()[0].children()[1].id();
    let tx = Transaction::new(&old, Origin::Command).with_edit(Edit::new(p(2)..p(3), "A"));
    doc.commit(tx, Duration::ZERO)
        .expect("edit first list item");
    assert_eq!(doc.snapshot().cst().nodes()[0].children()[1].id(), sibling);
}
proptest! {
    #[test]
    fn multi_edit_inverse_restores_unicode(source in ".{0,80}", first in ".{0,15}", second in ".{0,15}", cut in any::<usize>()) {
        let mut doc = document(&source); let snapshot = doc.snapshot(); let len = snapshot.text().len().value(); let cut = cut % (len + 1);
        let transaction = if cut == 0 || cut == len {
            Transaction::new(&snapshot, Origin::Command).with_edit(Edit::new(p(0)..p(len), &first))
        } else {
            Transaction::new(&snapshot, Origin::Command).with_edit(Edit::new(p(0)..p(cut), &first)).with_edit(Edit::new(p(cut)..p(len), &second))
        };
        doc.commit(transaction, Duration::ZERO).expect("valid edits"); let after = doc.snapshot().text().to_string();
        doc.undo().expect("inverse"); prop_assert_eq!(doc.snapshot().text().to_string(), snapshot.text().to_string());
        doc.redo().expect("forward"); prop_assert_eq!(doc.snapshot().text().to_string(), after);
    }
}
