//! Atomic edit outcomes and selection mapping.
use super::{Edit, Origin, Outcome, Range, ScalarOffset, Selection, Snapshot, Transaction};

pub(super) fn offset(value: usize) -> ScalarOffset {
    ScalarOffset::new(value)
}

pub(super) fn replacement(
    snapshot: &Snapshot,
    range: Range<ScalarOffset>,
    value: &str,
    caret: usize,
    origin: Origin,
) -> Outcome {
    let selection = Selection::cursor(offset(caret));
    Outcome {
        transaction: Some(
            Transaction::new(snapshot, origin)
                .with_edit(Edit::new(range, value))
                .with_selection(selection),
        ),
        selection,
    }
}

pub(super) fn navigation(selection: Selection) -> Outcome {
    Outcome {
        transaction: None,
        selection,
    }
}

pub(super) fn edits_outcome(
    snapshot: &Snapshot,
    selection: Selection,
    edits: Vec<Edit>,
) -> Outcome {
    if edits.is_empty() {
        return navigation(selection);
    }
    let selection = selection.mapped(&edits);
    let mut transaction = Transaction::new(snapshot, Origin::Command).with_selection(selection);
    for edit in edits {
        transaction = transaction.with_edit(edit);
    }
    Outcome {
        transaction: Some(transaction),
        selection,
    }
}
