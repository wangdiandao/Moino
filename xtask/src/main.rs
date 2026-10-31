//! Repository checks and generated trace reports; never linked into the application.
mod docs;
mod layers;

use anyhow::{Context, Result, bail};
use moino_testkit::{Vector, load_vectors};
use serde_json::json;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace root")?;
    let mut args = env::args().skip(1);
    let command = args
        .next()
        .context("usage: cargo xtask <layers|docs|vectors|trace>")?;
    if args.next().is_some() {
        bail!("unexpected extra argument");
    }
    let count = match command.as_str() {
        "layers" => layers::check(root)?,
        "docs" => docs::check(root)?,
        "vectors" => vectors(root)?.len(),
        "trace" => {
            let rows = vectors(root)?;
            let report: Vec<_> = rows
                .iter()
                .map(|row| {
                    json!({
                        "name": row.name(), "source": relative(root, row.source()),
                        "specs": row.specs(), "scenario": row.scenario(),
                        "assertion": "not-executed"
                    })
                })
                .collect();
            fs::create_dir_all(root.join("target"))?;
            fs::write(
                root.join("target/trace.json"),
                serde_json::to_vec_pretty(&json!({
                    "schema": 1, "kind": "specification-links", "cases": report
                }))?,
            )?;
            rows.len()
        }
        _ => bail!("unknown check: {command}"),
    };
    writeln!(io::stdout().lock(), "{command}: checked {count}")?;
    Ok(())
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn vectors(root: &Path) -> Result<Vec<Vector>> {
    let rows = load_vectors(&root.join("docs/acceptance/vectors"))?;
    let scenarios = root.join("docs/acceptance/scenarios.md");
    for row in &rows {
        for link in row.specs() {
            docs::check_link(root, row.source(), link)?;
        }
        docs::check_link(root, &scenarios, &format!("#{}", row.scenario()))?;
    }
    Ok(rows)
}

fn files(directory: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path
            .file_name()
            .is_some_and(|name| name == ".git" || name == "target")
        {
            continue;
        }
        if path.is_dir() {
            result.extend(files(&path, extension)?);
        } else if path.extension().is_some_and(|ext| ext == extension) {
            result.push(path);
        }
    }
    result.sort();
    Ok(result)
}
