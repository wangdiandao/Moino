use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub(crate) fn tests(root: &Path) -> Result<Vec<Value>> {
    let mut associations = Vec::new();
    for path in crate::files(root, "rs")? {
        let source = fs::read_to_string(&path)?;
        let lines: Vec<_> = source.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let Some(spec) = line.trim().strip_prefix("// Spec: ") else {
                continue;
            };
            ensure!(
                index > 0 && ["#[test]", "#[gpui_kit::test]"].contains(&lines[index - 1].trim()),
                "{}:{}: Spec marker must follow a test attribute",
                path.display(),
                index + 1
            );
            let definition = lines
                .get(index + 1)
                .context("test marker without function")?
                .trim();
            let name = definition
                .strip_prefix("fn ")
                .and_then(|text| text.split_once('('))
                .map(|(name, _)| name)
                .context("test marker must precede a test function")?;
            crate::docs::check_link(root, &root.join("README.md"), spec)?;
            associations.push(
                json!({ "name": name, "source": crate::relative(root, &path),
                "line": index + 2, "spec": spec, "execution": "not-recorded" }),
            );
        }
    }
    ensure!(!associations.is_empty(), "no Rust test associations found");
    Ok(associations)
}
