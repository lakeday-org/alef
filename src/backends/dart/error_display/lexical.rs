use anyhow::{Context as _, Result, bail, ensure};

pub(super) fn mask(source: &str) -> Result<String> {
    let bytes = source.as_bytes();
    let mut result = bytes.to_vec();
    let mut index = 0;
    while index < bytes.len() {
        if let Some(end) = non_code(bytes, index)? {
            for byte in &mut result[index..end] {
                if *byte != b'\n' {
                    *byte = b' ';
                }
            }
            index = end;
        } else {
            index += 1;
        }
    }
    String::from_utf8(result).context("masking Dart literals and comments")
}

pub(super) fn closing(source: &str, start: usize) -> Result<usize> {
    closing_bytes(source.as_bytes(), start)
}

fn closing_bytes(bytes: &[u8], start: usize) -> Result<usize> {
    let close = match bytes.get(start) {
        Some(b'{') => b'}',
        Some(b'(') => b')',
        Some(b'[') => b']',
        _ => bail!("Dart delimiter expected"),
    };
    let mut index = start + 1;
    while let Some(byte) = bytes.get(index) {
        if let Some(end) = non_code(bytes, index)? {
            index = end;
            continue;
        }
        if *byte == close {
            return Ok(index);
        }
        if matches!(byte, b'{' | b'(' | b'[') {
            index = closing_bytes(bytes, index)?;
        } else if matches!(byte, b'}' | b')' | b']') {
            bail!("mismatched Dart delimiter at byte {index}");
        }
        index += 1;
    }
    bail!("unclosed Dart delimiter at byte {start}")
}

fn non_code(bytes: &[u8], start: usize) -> Result<Option<usize>> {
    if bytes[start..].starts_with(b"//") {
        return Ok(Some(
            bytes[start..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |end| start + end),
        ));
    }
    if bytes[start..].starts_with(b"/*") {
        return block_comment(bytes, start).map(Some);
    }
    let raw = bytes[start] == b'r' && matches!(bytes.get(start + 1), Some(b'\'' | b'"'));
    let quote_start = start + usize::from(raw);
    if matches!(bytes.get(quote_start), Some(b'\'' | b'"')) {
        return quoted(bytes, quote_start, raw).map(Some);
    }
    Ok(None)
}

fn block_comment(bytes: &[u8], start: usize) -> Result<usize> {
    let mut depth = 1;
    let mut index = start + 2;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"/*") {
            depth += 1;
            index += 2;
        } else if bytes[index..].starts_with(b"*/") {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return Ok(index);
            }
        } else {
            index += 1;
        }
    }
    bail!("unclosed Dart block comment at byte {start}")
}

fn quoted(bytes: &[u8], start: usize, raw: bool) -> Result<usize> {
    let quote = bytes[start];
    let width = if bytes.get(start..start + 3).is_some_and(|value| value == [quote; 3]) {
        3
    } else {
        1
    };
    let mut index = start + width;
    while index < bytes.len() {
        if bytes
            .get(index..index + width)
            .is_some_and(|value| value.iter().all(|byte| *byte == quote))
        {
            return Ok(index + width);
        }
        if !raw && bytes[index] == b'\\' {
            index += 2;
        } else if !raw && bytes[index..].starts_with(b"${") {
            index = closing_bytes(bytes, index + 1)? + 1;
        } else {
            index += 1;
        }
    }
    bail!("unclosed Dart string at byte {start}")
}

pub(super) fn parameters(source: &str) -> Result<Vec<String>> {
    let mask = mask(source)?;
    let mask = mask.trim();
    let mask = mask
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(mask);
    let mut result = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let mut generics = 0;
    while index <= mask.len() {
        if index == mask.len() || (mask.as_bytes()[index] == b',' && generics == 0) {
            let parameter = mask[start..index].trim();
            if !parameter.is_empty() {
                result.push(parameter_name(parameter)?);
            }
            start = index + 1;
        } else if mask.as_bytes()[index] == b'<' {
            generics += 1;
        } else if mask.as_bytes()[index] == b'>' {
            ensure!(generics > 0, "unmatched Dart generic delimiter");
            generics -= 1;
        } else if matches!(mask.as_bytes()[index], b'(' | b'[' | b'{') {
            index = closing(mask, index)?;
        }
        index += 1;
    }
    ensure!(generics == 0, "unclosed Dart generic delimiter");
    Ok(result)
}

fn parameter_name(source: &str) -> Result<String> {
    let source = source.split('=').next().unwrap_or(source).trim();
    regex::Regex::new(r"([$_\p{XID_Start}][$_\p{XID_Continue}]*)\s*$")?
        .captures(source)
        .and_then(|captures| captures.get(1))
        .map(|name| name.as_str().to_string())
        .context("reading actual FRB factory parameter name")
}
