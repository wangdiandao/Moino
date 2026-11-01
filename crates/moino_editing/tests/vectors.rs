//! Executes the formal editing, history and selection data against public core APIs.
use moino_document::{
    Affinity, Document, DocumentId, Edit, Endpoint, Epoch, HistoryGroup, Origin, Selection,
    Transaction,
};
use moino_editing::{BlockFormat, InlineFormat, Intent, Options, resolve};
use moino_text::ScalarOffset;
use std::{path::Path, time::Duration};

fn pos(value: usize) -> ScalarOffset {
    ScalarOffset::new(value)
}
fn marked(source: &str) -> (String, Selection) {
    let mut text = String::new();
    let mut anchor = None;
    let mut head = None;
    for ch in source.chars() {
        match ch {
            '█' => {
                anchor = Some(text.chars().count());
                head = anchor;
            }
            '｢' => anchor = Some(text.chars().count()),
            '｣' => head = Some(text.chars().count()),
            _ => text.push(ch),
        }
    }
    let anchor = anchor.expect("vector selection");
    let head = head.expect("vector selection end");
    (
        text,
        Selection::new(
            Endpoint {
                offset: pos(anchor),
                affinity: Affinity::After,
            },
            Endpoint {
                offset: pos(head),
                affinity: Affinity::After,
            },
        ),
    )
}
fn insert(action: &str) -> Option<String> {
    action.strip_prefix("insert:").map(|value| {
        serde_json::from_str::<String>(value.strip_suffix(" (group=1)").unwrap_or(value))
            .expect("quoted insertion")
    })
}
fn actions(source: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            quoted = !quoted;
        }
        if !quoted {
            let width = if source[index..].starts_with(", ") {
                2
            } else if source[index..].starts_with(" then ") {
                6
            } else {
                0
            };
            if width > 0 {
                result.push(&source[start..index]);
                start = index + width;
            }
        }
    }
    result.push(&source[start..]);
    result
}
#[test]
// Spec: docs/modules/editing.md#输入路由
fn executes_editing_and_history_vectors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/acceptance/vectors");
    let vectors = moino_testkit::load_vectors(&root).expect("formal vectors");
    let mut count = 0;
    for vector in vectors.iter().filter(|vector| {
        vector.source().file_name().is_some_and(|name| {
            name.to_string_lossy().starts_with("edit-") || name == "hist-basic.yaml"
        })
    }) {
        let (input, selection) = marked(vector.input());
        let mut document = Document::new(DocumentId::new(1), Epoch::new(0), &input);
        document.set_selection(selection).expect("selection");
        for (tick, action) in actions(vector.action().expect("action"))
            .into_iter()
            .enumerate()
        {
            if action == "undo" {
                document.undo().expect("undo");
                continue;
            }
            if action == "redo" {
                document.redo().expect("redo");
                continue;
            }
            let options =
                Options::new("    ", action.starts_with("type:")).expect("effective input options");
            let intent = if let Some(value) = action
                .strip_prefix("type:")
                .and_then(|value| value.strip_suffix(" (auto_pair=true)"))
            {
                Intent::Insert(serde_json::from_str(value).expect("keyboard text"))
            } else if let Some(value) = action.strip_prefix("composition:") {
                Intent::Composition(serde_json::from_str(value).expect("composition text"))
            } else if let Some(text) = insert(action) {
                Intent::Insert(text)
            } else {
                match action {
                    "enter" => Intent::Enter,
                    "backspace" => Intent::Backspace,
                    "edit.clearFormatting" => Intent::ClearFormatting,
                    "inline.bold" => Intent::Inline(InlineFormat::Bold),
                    "block.heading1" => Intent::Block(BlockFormat::Heading(1)),
                    _ => panic!("unsupported vector action {action}"),
                }
            };
            let outcome = resolve(&document.snapshot(), document.selection(), intent, &options)
                .expect("resolve");
            let selection = outcome.selection();
            if let Some(mut transaction) = outcome.into_transaction() {
                if action.contains("group=1") {
                    transaction = transaction.with_group(HistoryGroup::new(1));
                }
                document
                    .commit(
                        transaction,
                        Duration::from_millis(u64::try_from(tick).expect("tick") * 10),
                    )
                    .expect("commit");
            } else {
                document.set_selection(selection).expect("navigation");
            }
        }
        if vector.expected().contains('█') || vector.expected().contains('｢') {
            let (expected, selection) = marked(vector.expected());
            assert_eq!(
                document.snapshot().text().to_string(),
                expected,
                "{}",
                vector.name()
            );
            assert_eq!(document.selection(), selection, "{}", vector.name());
        } else {
            assert_eq!(
                document.snapshot().text().to_string(),
                vector.expected(),
                "{}",
                vector.name()
            );
        }
        count += 1;
    }
    assert_eq!(count, 20);
}
#[test]
// Spec: docs/contracts/document-model.md#选择与映射
fn executes_selection_vectors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/acceptance/vectors");
    let vectors = moino_testkit::load_vectors(&root).expect("formal vectors");
    let mut count = 0;
    for vector in vectors.iter().filter(|vector| {
        vector
            .source()
            .file_name()
            .is_some_and(|name| name == "sel-basic.yaml")
    }) {
        let action = vector.action().expect("selection action");
        let actual = if let Some(arguments) = action
            .strip_prefix("select(")
            .and_then(|value| value.strip_suffix(')'))
        {
            let (anchor, head) = arguments.split_once(", ").expect("endpoints");
            let selection = Selection::new(
                Endpoint {
                    offset: pos(anchor.parse().expect("anchor")),
                    affinity: Affinity::After,
                },
                Endpoint {
                    offset: pos(head.parse().expect("head")),
                    affinity: Affinity::After,
                },
            );
            format!(
                "anchor={} head={} forward={}\n",
                selection.anchor().offset.value(),
                selection.head().offset.value(),
                selection.is_forward()
            )
        } else if let Some(arguments) = action
            .strip_prefix("insert(")
            .and_then(|value| value.strip_suffix(')'))
        {
            let (value, at) = arguments.rsplit_once(" at ").expect("insertion action");
            let value: String = serde_json::from_str(value).expect("insertion text");
            let (_, cursor) = vector
                .input()
                .trim_end()
                .rsplit_once(" (cursor at ")
                .expect("cursor annotation");
            let cursor = cursor
                .strip_suffix(')')
                .expect("annotation end")
                .parse()
                .expect("cursor");
            let at = at.parse().expect("insertion position");
            let mapped =
                Selection::cursor(pos(cursor)).mapped(&[Edit::new(pos(at)..pos(at), &value)]);
            format!("cursor={}\n", mapped.head().offset.value())
        } else if let Some(arguments) = action
            .strip_prefix("delete(")
            .and_then(|value| value.strip_suffix(')'))
        {
            let (_, selection) = vector
                .input()
                .trim_end()
                .rsplit_once(" (select ")
                .expect("selection annotation");
            let (anchor, head) = selection
                .strip_suffix(')')
                .expect("annotation end")
                .split_once("..")
                .expect("selection range");
            let selection = Selection::new(
                Endpoint {
                    offset: pos(anchor.parse().expect("anchor")),
                    affinity: Affinity::After,
                },
                Endpoint {
                    offset: pos(head.parse().expect("head")),
                    affinity: Affinity::After,
                },
            );
            let (start, end) = arguments.split_once("..").expect("deletion range");
            let mapped = selection.mapped(&[Edit::new(
                pos(start.parse().expect("start"))..pos(end.parse().expect("end")),
                "",
            )]);
            format!(
                "anchor={} head={}\n",
                mapped.anchor().offset.value(),
                mapped.head().offset.value()
            )
        } else {
            panic!("unsupported selection action {action}")
        };
        assert_eq!(actual, vector.expected(), "{}", vector.name());
        count += 1;
    }
    assert_eq!(count, 4);
}
#[test]
// Spec: docs/modules/editing.md#enter
fn split_paragraph_remains_two_paragraphs_after_typing() {
    let mut document = Document::new(DocumentId::new(1), Epoch::new(0), "abcd");
    document
        .set_selection(Selection::cursor(pos(2)))
        .expect("cursor");
    let options = Options::new("  ", false).expect("options");
    for intent in [Intent::Enter, Intent::Insert("X".to_owned())] {
        let transaction = resolve(&document.snapshot(), document.selection(), intent, &options)
            .expect("intent")
            .into_transaction()
            .expect("text change");
        document
            .commit(transaction, Duration::ZERO)
            .expect("commit");
    }
    assert_eq!(document.snapshot().text().to_string(), "ab\n\nXcd");
    assert_eq!(
        document
            .snapshot()
            .cst()
            .nodes()
            .iter()
            .filter(|node| matches!(node.syntax(), moino_markdown::Syntax::Paragraph))
            .count(),
        2
    );
    let old = document.snapshot();
    let stale = Transaction::new(&old, Origin::Typing).with_edit(Edit::new(pos(0)..pos(0), "x"));
    document.undo().expect("undo");
    assert!(document.commit(stale, Duration::ZERO).is_err());
}
