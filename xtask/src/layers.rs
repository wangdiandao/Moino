use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    process::Command,
};

// Read the authoritative table instead of maintaining another dependency graph.
fn allowed(source: &str) -> HashMap<String, HashSet<String>> {
    let rows: Vec<Vec<&str>> = source
        .lines()
        .filter(|line| line.starts_with("| `moino"))
        .map(|line| line.split('|').map(str::trim).collect())
        .collect();
    let names: Vec<String> = rows
        .iter()
        .map(|row| row[1].trim_matches('`').to_owned())
        .collect();
    rows.iter()
        .map(|row| {
            let name = row[1].trim_matches('`').to_owned();
            let dependencies = if row[4].starts_with("上述") {
                names
                    .iter()
                    .filter(|name| {
                        !["moino", "moino_workspace", "moino_testkit", "moino_editing"]
                            .contains(&name.as_str())
                    })
                    .cloned()
                    .collect()
            } else {
                row[4]
                    .split('`')
                    .skip(1)
                    .step_by(2)
                    .map(str::to_owned)
                    .collect()
            };
            (name, dependencies)
        })
        .collect()
}

pub(crate) fn check(root: &Path) -> Result<usize> {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(root)
        .output()?;
    ensure!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let packages = metadata["packages"]
        .as_array()
        .context("metadata packages")?;
    let graph = allowed(&fs::read_to_string(root.join("docs/architecture.md"))?);
    let mut edges = HashMap::<String, Vec<String>>::new();
    for package in packages {
        let name = package["name"].as_str().context("package name")?;
        ensure!(
            name == "xtask" || graph.contains_key(name),
            "undocumented crate {name}"
        );
        let path = Path::new(package["manifest_path"].as_str().context("manifest path")?);
        let manifest: toml::Value = toml::from_str(&fs::read_to_string(path)?)?;
        ensure!(
            manifest["lints"]["workspace"].as_bool() == Some(true),
            "{name} must inherit workspace lints"
        );
        check_inheritance(&manifest, name)?;
        for dep in package["dependencies"].as_array().context("dependencies")? {
            let dependency = dep["name"].as_str().context("dependency name")?;
            let dev = dep["kind"].as_str() == Some("dev");
            ensure!(
                dependency != "moino_testkit" || dev || name == "xtask",
                "testkit leaked into runtime: {name}"
            );
            ensure!(
                !dependency.starts_with("gpui") || dependency == "gpui-kit",
                "use Kit facade: {name} -> {dependency}"
            );
            let l0 = fs::read_to_string(root.join("docs/architecture.md"))?
                .lines()
                .any(|line| line.starts_with(&format!("| `{name}` | L0 |")));
            ensure!(
                !l0 || !dependency.starts_with("gpui"),
                "L0 cannot depend on GPUI: {name}"
            );
            if graph.contains_key(dependency) && name != "xtask" && !dev {
                ensure!(
                    graph[name].contains(dependency),
                    "forbidden edge {name} -> {dependency}"
                );
                edges
                    .entry(name.to_owned())
                    .or_default()
                    .push(dependency.to_owned());
            }
        }
    }
    fn visit(
        node: &str,
        edges: &HashMap<String, Vec<String>>,
        stack: &mut HashSet<String>,
        done: &mut HashSet<String>,
    ) -> Result<()> {
        if done.contains(node) {
            return Ok(());
        }
        ensure!(stack.insert(node.to_owned()), "dependency cycle at {node}");
        for child in edges.get(node).into_iter().flatten() {
            visit(child, edges, stack, done)?;
        }
        stack.remove(node);
        done.insert(node.to_owned());
        Ok(())
    }
    let mut done = HashSet::new();
    for name in edges.keys() {
        visit(name, &edges, &mut HashSet::new(), &mut done)?;
    }
    Ok(packages.len())
}

fn check_inheritance(value: &toml::Value, name: &str) -> Result<()> {
    if let Some(table) = value.as_table() {
        for (key, child) in table {
            if ["dependencies", "dev-dependencies", "build-dependencies"].contains(&key.as_str()) {
                for (dependency, setting) in child.as_table().context("dependency table")? {
                    ensure!(
                        setting.get("workspace").and_then(toml::Value::as_bool) == Some(true),
                        "{name}: {dependency} must inherit workspace version"
                    );
                }
            } else {
                check_inheritance(child, name)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // Spec: docs/acceptance/m0.md#自动检查
    fn reads_authoritative_allowlist() {
        let graph = allowed(include_str!("../../docs/architecture.md"));
        assert!(graph["moino_jobs"].is_empty());
        assert!(graph["moino"].contains("moino_workspace"));
        assert!(!graph["moino_workspace"].contains("moino_editing"));
        assert!(graph["moino_workspace"].contains("moino_document"));
    }
    #[test]
    // Spec: docs/acceptance/m0.md#自动检查
    fn rejects_member_version_overrides() {
        let bad: toml::Value = toml::from_str("[dependencies]\nserde = '1'").expect("toml");
        assert!(check_inheritance(&bad, "example").is_err());
    }
}
