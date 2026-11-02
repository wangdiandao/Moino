//! Public editing outcomes retain document validity and exact undo across syntax boundaries.
use moino_document::{Document, DocumentId, Epoch, Selection};
use moino_editing::{BlockFormat, InlineFormat, Intent, Options, resolve};
use moino_text::ScalarOffset;
use std::time::Duration;
#[test]
// Spec: docs/contracts/document-model.md#文档不变量
fn bounded_syntax_edits_remain_valid_and_exactly_reversible() {
    for source in [
        "",
        "#",
        "# ",
        "# 中😀",
        "##\t中",
        "Title\n===\n",
        "a\n\nb",
        "- # 中😀",
        "> # 中😀",
        "- [x] 中😀\n  tail",
        "10. > - 中😀",
        "```\n中😀\n```",
        "`中😀`",
        "**中😀**",
        "[中😀](url)",
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
        "a\n> b\n- c",
        "> - 中\n>   - 😀",
    ] {
        for start in 0..=source.chars().count() {
            for end in start..=source.chars().count() {
                for intent in [
                    Intent::Enter,
                    Intent::Backspace,
                    Intent::Delete,
                    Intent::Tab { reverse: false },
                    Intent::Tab { reverse: true },
                    Intent::Block(BlockFormat::Heading(1)),
                    Intent::Block(BlockFormat::Paragraph),
                    Intent::Inline(InlineFormat::Bold),
                    Intent::ClearFormatting,
                    Intent::Insert("'".into()),
                    Intent::Insert("*".into()),
                    Intent::Insert("=".into()),
                ] {
                    let mut document = Document::new(DocumentId::new(1), Epoch::new(0), source);
                    let selected = Selection::new(
                        moino_document::Endpoint {
                            offset: ScalarOffset::new(start),
                            affinity: moino_document::Affinity::After,
                        },
                        moino_document::Endpoint {
                            offset: ScalarOffset::new(end),
                            affinity: moino_document::Affinity::After,
                        },
                    );
                    document.set_selection(selected).expect("valid selection");
                    let outcome = resolve(
                        &document.snapshot(),
                        selected,
                        intent.clone(),
                        &Options::new("    ", true).expect("options"),
                    )
                    .expect("valid intent");
                    if let Some(transaction) = outcome.into_transaction() {
                        if let Err(error) = document.commit(transaction, Duration::ZERO) {
                            panic!(
                                "source={source:?} selection={start}..{end} intent={intent:?}: {error}"
                            );
                        }
                        assert!(document.snapshot().cst().is_valid());
                        let committed = document.snapshot().text().to_string();
                        assert_eq!(document.snapshot().cst().serialize(), committed);
                        document.undo().expect("undo");
                        assert_eq!(document.snapshot().text().to_string(), source);
                        assert_eq!(document.selection(), selected);
                        document.redo().expect("redo");
                        assert_eq!(document.snapshot().text().to_string(), committed);
                        assert!(document.snapshot().cst().is_valid());
                    }
                }
            }
        }
    }
}
