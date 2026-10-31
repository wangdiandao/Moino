use saphyr::{LoadableYamlNode, Yaml};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

/// One formal acceptance case; source text is preserved verbatim.
#[derive(Clone, Debug)]
pub struct Vector {
    name: String,
    specs: Vec<String>,
    scenario: String,
    input: String,
    action: Option<String>,
    expected: String,
    description: String,
    source: PathBuf,
}

impl Vector {
    /// Globally unique behavior name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Specification links relative to the YAML source.
    pub fn specs(&self) -> &[String] {
        &self.specs
    }
    /// Anchor in the acceptance scenario matrix.
    pub fn scenario(&self) -> &str {
        &self.scenario
    }
    /// Exact input, including trailing whitespace.
    pub fn input(&self) -> &str {
        &self.input
    }
    /// Operation, omitted for pure parsing cases.
    pub fn action(&self) -> Option<&str> {
        self.action.as_deref()
    }
    /// Exact expected output.
    pub fn expected(&self) -> &str {
        &self.expected
    }
    /// Human-readable assertion intent.
    pub fn description(&self) -> &str {
        &self.description
    }
    /// Source filename used to resolve links and diagnostics.
    pub fn source(&self) -> &Path {
        &self.source
    }
}

/// Invalid acceptance data; never interpreted as a passing product assertion.
#[derive(Debug, thiserror::Error)]
#[error("vectors.invalid: {0}")]
pub struct VectorError(String);

fn invalid(message: impl Into<String>) -> VectorError {
    VectorError(message.into())
}

/// Parses a single YAML sequence and validates the formal string schema.
pub fn parse_vectors(source: &str, path: &Path) -> Result<Vec<Vector>, VectorError> {
    let documents = Yaml::load_from_str(source).map_err(|error| invalid(error.to_string()))?;
    if documents.len() != 1 {
        return Err(invalid("expected exactly one YAML document"));
    }
    let rows = documents[0]
        .as_sequence()
        .ok_or_else(|| invalid("root must be a sequence"))?;
    if rows.is_empty() {
        return Err(invalid("vector file must not be empty"));
    }
    let mut names = HashSet::new();
    rows.iter()
        .map(|row| {
            let fields = row
                .as_mapping()
                .ok_or_else(|| invalid("case must be a mapping"))?;
            let allowed = [
                "name",
                "specs",
                "scenario",
                "input",
                "action",
                "expected",
                "description",
            ];
            for key in fields.keys() {
                if !key.as_str().is_some_and(|name| allowed.contains(&name)) {
                    return Err(invalid("unknown vector field"));
                }
            }
            let string = |key: &str| -> Result<String, VectorError> {
                row[key]
                    .as_str()
                    .filter(|text| !text.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| invalid(format!("{key} must be a nonempty string")))
            };
            let name = string("name")?;
            if !name.split('-').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            }) || !names.insert(name.clone())
            {
                return Err(invalid(format!("invalid or duplicate name: {name}")));
            }
            let specs = row["specs"]
                .as_sequence()
                .filter(|list| !list.is_empty())
                .ok_or_else(|| invalid("specs must be a nonempty sequence"))?
                .iter()
                .map(|link| {
                    link.as_str()
                        .filter(|link| link.contains('#'))
                        .map(str::to_owned)
                        .ok_or_else(|| invalid("spec must link to a section"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let action = if fields.keys().any(|key| key.as_str() == Some("action")) {
                Some(string("action")?)
            } else {
                None
            };
            Ok(Vector {
                name,
                specs,
                scenario: string("scenario")?,
                input: string("input")?,
                action,
                expected: string("expected")?,
                description: string("description")?,
                source: path.to_owned(),
            })
        })
        .collect()
}

/// Loads all `.yaml` files in lexical order and rejects duplicate names across files.
pub fn load_vectors(directory: &Path) -> Result<Vec<Vector>, VectorError> {
    let mut paths = fs::read_dir(directory)
        .map_err(|error| invalid(error.to_string()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| invalid(error.to_string()))?;
    paths.retain(|path| path.extension().is_some_and(|ext| ext == "yaml"));
    paths.sort();
    if paths.is_empty() {
        return Err(invalid("no formal vector files"));
    }
    let mut vectors = Vec::new();
    let mut names = HashSet::new();
    for path in paths {
        let source = fs::read_to_string(&path).map_err(|error| invalid(error.to_string()))?;
        for vector in parse_vectors(&source, &path)? {
            if !names.insert(vector.name.clone()) {
                return Err(invalid(format!("duplicate name: {}", vector.name)));
            }
            vectors.push(vector);
        }
    }
    Ok(vectors)
}

#[cfg(test)]
mod tests {
    use super::*;
    const CASE: &str = "- name: sample\n  specs: [../../modules/editing.md#enter]\n  scenario: enter-splits-paragraph\n  input: |\n    a  \n  expected: |\n    b\n  description: 测试\n";
    #[test]
    // Spec: docs/acceptance/m0.md#自动检查
    fn preserves_whitespace_and_optional_action() {
        let rows = parse_vectors(CASE, Path::new("sample.yaml")).expect("valid case");
        assert_eq!(rows[0].input(), "a  \n");
        assert_eq!(rows[0].expected(), "b\n");
        assert!(rows[0].action().is_none());
    }
    #[test]
    // Spec: docs/acceptance/m0.md#自动检查
    fn rejects_invalid_shapes_fields_and_duplicate_names() {
        for source in [
            "{}".to_owned(),
            "[]".to_owned(),
            format!("{CASE}{CASE}"),
            CASE.replace("sample", "Bad_Name"),
            CASE.replace("specs:", "unknown:"),
            CASE.replace("description: 测试", "description: 1"),
            format!("{CASE}---\n[]"),
        ] {
            assert!(
                parse_vectors(&source, Path::new("bad.yaml")).is_err(),
                "{source}"
            );
        }
    }
    #[test]
    // Spec: docs/acceptance/m0.md#自动检查
    fn repository_cases_load_without_running_product_assertions() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/acceptance/vectors");
        let rows = load_vectors(&directory).expect("formal vectors");
        assert_eq!(rows.len(), 26);
        assert_eq!(rows.iter().filter(|row| row.action().is_none()).count(), 10);
    }
}
