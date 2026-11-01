//! Formal vectors dispatched through the production GPUI surface.
use super::{LiveEditor, Options, selection};
use crate::commands;
use gpui_kit::test::{TestInput, TestWindowExt};
use gpui_kit::{AppContext, ElementInputHandler, TestAppContext};
use moino_document::Selection;
use moino_text::ScalarOffset;

fn marked(source: &str) -> (String, Selection) {
    let mut text = String::new();
    let mut anchor = None;
    let mut head = None;
    let mut count = 0;
    for ch in source.chars() {
        match ch {
            '█' => {
                anchor = Some(count);
                head = Some(count);
            }
            '｢' => anchor = Some(count),
            '｣' => head = Some(count),
            _ => {
                text.push(ch);
                count += 1;
            }
        }
    }
    (
        text,
        selection(
            ScalarOffset::new(anchor.expect("vector anchor")),
            ScalarOffset::new(head.expect("vector head")),
        ),
    )
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入路由
fn formal_editing_vectors_use_native_events_and_shared_actions(cx: &mut TestAppContext) {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/acceptance/vectors");
    let vectors = moino_testkit::load_vectors(&root).expect("formal vectors");
    cx.update(|cx| {
        gpui_kit::init(cx);
        moino_theme::register_fonts(cx).expect("bundled test fonts");
        moino_theme::apply(moino_theme::BuiltinTheme::Paper, cx);
        crate::init(cx);
    });
    let mut count = 0;
    for vector in vectors.iter().filter(|vector| {
        vector.source().file_name().is_some_and(|name| {
            name.to_string_lossy().starts_with("edit-") || name == "hist-basic.yaml"
        })
    }) {
        let (text, selected) = marked(vector.input());
        let action = vector.action().expect("vector actions");
        let (handle, editor) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                let editor = cx.new(|cx| {
                    LiveEditor::new(
                        &text,
                        Options::new("    ", action.contains("auto_pair=true")).expect("policy"),
                        cx,
                    )
                });
                editor.update(cx, |editor, _| {
                    editor
                        .document
                        .set_selection(selected)
                        .expect("fixture selection")
                });
                window.focus(&editor.read(cx).focus_handle(), cx);
                editor
            })
            .expect("vector window")
        });
        for action in action.replace(" then ", ", ").split(", ") {
            cx.update_window(handle, |_, window, cx| {
                if !action.contains("group=1") {
                    editor.update(cx, |editor, _| editor.document.break_history());
                }
                window.render_frame(cx);
                if let Some(value) = action
                    .strip_prefix("insert:")
                    .or_else(|| action.strip_prefix("type:"))
                {
                    let value = value.split(" (").next().expect("quoted text");
                    let value: String = serde_json::from_str(value).expect("input value");
                    window.input(&value, cx);
                } else if let Some(value) = action.strip_prefix("composition:") {
                    let value: String = serde_json::from_str(value).expect("composition value");
                    let bounds = window.find("document-editor").bounds();
                    let mut input =
                        TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
                    input.compose_text(None, &value, None, cx);
                    input.commit_text(&value, cx);
                } else {
                    match action {
                        "enter" | "tab" | "shift-tab" | "backspace" => window.press(action, cx),
                        "undo" => window.press("secondary-z", cx),
                        "redo" => window.press("secondary-shift-z", cx),
                        "inline.bold" => window.dispatch_action(Box::new(commands::Bold), cx),
                        "block.heading1" => {
                            window.dispatch_action(Box::new(commands::Heading1), cx)
                        }
                        "edit.clearFormatting" => {
                            window.dispatch_action(Box::new(commands::ClearFormatting), cx)
                        }
                        _ => panic!("unsupported formal action {action}"),
                    }
                }
            })
            .expect("vector interaction");
            cx.run_until_parked();
        }
        cx.update(|cx| {
            let editor = editor.read(cx);
            if vector.expected().contains('█') || vector.expected().contains('｢') {
                let (expected, selected) = marked(vector.expected());
                assert_eq!(
                    editor.snapshot().text().to_string(),
                    expected,
                    "{}",
                    vector.name()
                );
                assert_eq!(editor.selection(), selected, "{}", vector.name());
            } else {
                assert_eq!(
                    editor.snapshot().text().to_string(),
                    vector.expected(),
                    "{}",
                    vector.name()
                );
            }
            assert!(!editor.is_pending(), "{}", vector.name());
        });
        count += 1;
    }
    assert_eq!(count, 26);
}
