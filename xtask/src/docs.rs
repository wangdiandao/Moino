use anyhow::{Context, Result, ensure};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

pub(crate) fn check(root: &Path) -> Result<usize> {
    let paths = crate::files(root, "md")?;
    let mut graph = HashMap::new();
    for path in &paths {
        let source = fs::read_to_string(path)?;
        ensure!(
            !source.starts_with("---\n"),
            "front matter is not permitted: {}",
            path.display()
        );
        let mut targets = Vec::new();
        for event in Parser::new_ext(&source, Options::all()) {
            if let Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) = event
                && let Some(target) = check_link(root, path, &dest_url)?
            {
                targets.push(target);
            }
        }
        graph.insert(path.canonicalize()?, targets);
    }
    let mut reachable = HashSet::new();
    let mut pending = vec![
        root.join("README.md").canonicalize()?,
        root.join("AGENTS.md").canonicalize()?,
    ];
    while let Some(path) = pending.pop() {
        if reachable.insert(path.clone()) {
            pending.extend(graph.get(&path).into_iter().flatten().cloned());
        }
    }
    for path in &paths {
        ensure!(
            reachable.contains(&path.canonicalize()?),
            "unreachable document: {}",
            path.display()
        );
    }
    Ok(paths.len())
}

pub(crate) fn check_link(
    root: &Path,
    source: &Path,
    link: &str,
) -> Result<Option<std::path::PathBuf>> {
    if link.starts_with("https://") || link.starts_with("http://") || link.starts_with("mailto:") {
        return Ok(None);
    }
    ensure!(!link.contains("://"), "unsupported link scheme: {link}");
    let (file, fragment) = link
        .split_once('#')
        .map_or((link, None), |(file, fragment)| (file, Some(fragment)));
    let target = if file.is_empty() {
        source.to_owned()
    } else {
        source.parent().context("link parent")?.join(decode(file)?)
    };
    let canonical = target
        .canonicalize()
        .with_context(|| format!("{}: missing target {link}", source.display()))?;
    ensure!(
        canonical.starts_with(root.canonicalize()?),
        "link escapes repository: {link}"
    );
    if let Some(fragment) = fragment {
        let text = fs::read_to_string(&canonical)?;
        ensure!(
            anchors(&text).contains(&decode(fragment)?),
            "{}: missing anchor {link}",
            source.display()
        );
    }
    Ok(Some(canonical))
}

fn decode(text: &str) -> Result<String> {
    let mut bytes = Vec::new();
    let mut iter = text.bytes();
    while let Some(byte) = iter.next() {
        if byte == b'%' {
            let high = iter.next().context("incomplete URL escape")?;
            let low = iter.next().context("incomplete URL escape")?;
            let hex = [high, low];
            bytes.push(u8::from_str_radix(std::str::from_utf8(&hex)?, 16)?);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).context("invalid URL UTF-8")
}

fn anchors(source: &str) -> HashSet<String> {
    let mut anchors = HashSet::new();
    let mut counts = HashMap::<String, usize>::new();
    let mut heading = None;
    for event in Parser::new_ext(source, Options::all()) {
        match event {
            Event::Start(Tag::Heading { id, .. }) => {
                if let Some(id) = id {
                    anchors.insert(id.to_string());
                }
                heading = Some(String::new());
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(heading) = &mut heading {
                    heading.push_str(&text);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(text) = heading.take() {
                    let slug: String = text
                        .to_lowercase()
                        .chars()
                        .filter_map(|c| {
                            if c.is_whitespace() {
                                Some('-')
                            } else if c.is_alphanumeric() || c == '-' || c == '_' {
                                Some(c)
                            } else {
                                None
                            }
                        })
                        .collect();
                    let count = counts.entry(slug.clone()).or_default();
                    anchors.insert(if *count == 0 {
                        slug
                    } else {
                        format!("{slug}-{count}")
                    });
                    *count += 1;
                }
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                for quote in ['\"', '\''] {
                    let pattern = format!("id={quote}");
                    for tail in html.split(&pattern).skip(1) {
                        if let Some(id) = tail.split(quote).next() {
                            anchors.insert(id.to_owned());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    anchors
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anchors_follow_heading_text_and_explicit_ids() {
        let ids = anchors("## 逻辑选择\n## `A` / B\n## Same\n## Same\n<a id=\"named\"></a>\n");
        for id in ["逻辑选择", "a--b", "same", "same-1", "named"] {
            assert!(ids.contains(id), "{id}");
        }
    }
    #[test]
    fn decodes_utf8_and_rejects_invalid_escapes() {
        assert_eq!(decode("a%20b").expect("valid escape"), "a b");
        assert!(decode("%").is_err());
        assert!(decode("%ff").is_err());
        assert!(decode("%zz").is_err());
    }
    #[test]
    fn detects_broken_targets_and_anchors() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace");
        assert!(check_link(root, &root.join("README.md"), "missing.md").is_err());
        assert!(check_link(root, &root.join("README.md"), "docs/product.md#missing").is_err());
    }
}
