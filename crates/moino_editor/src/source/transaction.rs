//! Exact source edit publication into authoritative document transactions.
use crate::source_change::SourceChange;
use moino_document::{Edit, Origin, Selection, Snapshot, Transaction};
use moino_text::ByteOffset;

pub(crate) fn transaction(
    snapshot: &Snapshot,
    text: &str,
    selection: Selection,
    change: Option<SourceChange>,
    origin: Origin,
) -> Option<Transaction> {
    let old = snapshot.text().to_string();
    if old == text {
        return None;
    }
    let change = change
        .filter(|change| {
            old.get(..change.start.value()) == text.get(..change.start.value())
                && old.get(change.old_end.value()..) == text.get(change.new_end.value()..)
                && old.is_char_boundary(change.start.value())
                && old.is_char_boundary(change.old_end.value())
                && text.is_char_boundary(change.start.value())
                && text.is_char_boundary(change.new_end.value())
                && change.start <= change.old_end
                && change.start <= change.new_end
        })
        .unwrap_or(SourceChange {
            start: ByteOffset::new(0),
            old_end: ByteOffset::new(old.len()),
            new_end: ByteOffset::new(text.len()),
        });
    let start = snapshot
        .text()
        .byte_to_scalar(change.start)
        .expect("validated source edit");
    let end = snapshot
        .text()
        .byte_to_scalar(change.old_end)
        .expect("validated source edit");
    let inserted = &text[change.start.value()..change.new_end.value()];
    Some(
        Transaction::new(snapshot, origin)
            .with_edit(Edit::new(start..end, inserted))
            .with_selection(selection),
    )
}
