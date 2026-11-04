//! Source marker spelling within already identified syntax contexts.

pub(super) fn partner(ch: char) -> Option<char> {
    match ch {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '`' | '"' | '\'' | '*' | '_' | '~' | '$' => Some(ch),
        _ => None,
    }
}

pub(super) fn prefix_len(line: &str) -> usize {
    let indent = line
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .count();
    let rest: String = line.chars().skip(indent).collect();
    let heading = rest.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&heading)
        && rest
            .chars()
            .nth(heading)
            .is_some_and(|ch| ch == ' ' || ch == '\t')
    {
        return indent + heading + 1;
    }
    if rest.starts_with('>') {
        let width = if rest
            .as_bytes()
            .get(1)
            .is_some_and(|byte| *byte == b' ' || *byte == b'\t')
        {
            2
        } else {
            1
        };
        return indent + width + prefix_len(&rest[width..]);
    }
    if rest
        .as_bytes()
        .first()
        .is_some_and(|ch| b"-*+".contains(ch))
        && rest.as_bytes().get(1).is_some_and(|ch| b" \t".contains(ch))
    {
        return indent
            + if ["[ ]", "[x]", "[X]"]
                .iter()
                .any(|marker| rest[2..].starts_with(marker))
                && rest.as_bytes().get(5).is_some_and(|ch| b" \t".contains(ch))
            {
                6
            } else {
                2
            };
    }
    let digits = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if (1..=9).contains(&digits)
        && rest
            .as_bytes()
            .get(digits)
            .is_some_and(|ch| b".)".contains(ch))
        && rest
            .as_bytes()
            .get(digits + 1)
            .is_some_and(|ch| b" \t".contains(ch))
    {
        return indent + digits + 2;
    }
    0
}

pub(super) fn next_marker(marker: &str) -> String {
    let trimmed = &marker[container_prefix_len(marker)..];
    let digits = trimmed.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits > 0
        && let Ok(number) = trimmed[..digits].parse::<u64>()
    {
        return format!(
            "{}{}{}",
            &marker[..marker.len() - trimmed.len()],
            number + 1,
            &trimmed[digits..]
        );
    }
    marker.replace("[x]", "[ ]").replace("[X]", "[ ]")
}

pub(super) fn container_prefix_len(marker: &str) -> usize {
    marker
        .bytes()
        .take_while(|byte| b" \t>".contains(byte))
        .count()
}

pub(super) fn quote_prefix_width(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut cursor = bytes
        .iter()
        .take_while(|byte| b" \t".contains(byte))
        .count();
    let mut end = 0;
    while bytes.get(cursor) == Some(&b'>') {
        cursor += 1;
        if bytes.get(cursor).is_some_and(|byte| b" \t".contains(byte)) {
            cursor += 1;
        }
        end = cursor;
        while bytes.get(cursor).is_some_and(|byte| b" \t".contains(byte)) {
            cursor += 1;
        }
    }
    end
}
