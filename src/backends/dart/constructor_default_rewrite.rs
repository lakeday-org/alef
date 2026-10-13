use anyhow::Context as _;

/// Apply only defaults explicitly carried by the generated mirror. FRB attributes cannot express enum or null defaults. ~keep
pub fn apply_constructor_defaults(source: &str, mirror: &str) -> anyhow::Result<String> {
    let mut result = source.to_string();
    for line in mirror.lines() {
        let Some(metadata) = line.trim_start().strip_prefix("// ").and_then(|comment| {
            comment
                .strip_prefix("~keep ")
                .unwrap_or(comment)
                .strip_prefix("alef-constructor-default: ")
        }) else {
            continue;
        };
        let [owner, field, value]: [String; 3] =
            serde_json::from_str(metadata).context("reading generated FRB constructor default metadata")?;
        apply_default(&mut result, &owner, &field, &value)?;
    }
    Ok(result)
}

fn apply_default(source: &mut String, owner: &str, field: &str, value: &str) -> anyhow::Result<()> {
    let class = regex::Regex::new(&format!(r"\bclass\s+{}\b[^{{]*\{{", regex::escape(owner)))?;
    let class_start = class
        .find(source)
        .ok_or_else(|| anyhow::anyhow!("FRB class {owner} is missing"))?
        .end();
    let class_end = delimiters(&source[class_start..], '}')
        .last()
        .copied()
        .context("FRB class has no closing brace")?
        + class_start;
    let constructor = regex::Regex::new(&format!(r"\b{}\s*\(\s*\{{", regex::escape(owner)))?;
    let start = constructor
        .find(&source[class_start..class_end])
        .ok_or_else(|| anyhow::anyhow!("FRB constructor {owner} is missing"))?
        .end()
        + class_start;
    let ends = delimiters(&source[start..class_end], '}');
    let parameter = regex::Regex::new(&format!(r"^(?:required\s+)?this\.{}\b", regex::escape(field)))?;
    let mut previous = 0;
    for end in ends {
        let segment = &source[start + previous..start + end];
        if parameter.is_match(segment.trim()) {
            let indent = segment.len() - segment.trim_start().len();
            let rendered = crate::backends::dart::template_env::render(
                "constructor_default_param.jinja",
                crate::alef_context! { name => field, default_value => value },
            );
            source.replace_range(
                start + previous + indent..start + end,
                rendered.trim().trim_end_matches(','),
            );
            return Ok(());
        }
        previous = end + 1;
    }
    anyhow::bail!("FRB constructor {owner}.{field} is missing")
}

fn delimiters(source: &str, closing: char) -> Vec<usize> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut characters = source.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        if skip_literal_or_comment(character, &mut characters) {
            continue;
        }
        if depth == 0 && character == closing {
            result.push(index);
            break;
        }
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => result.push(index),
            _ => {}
        }
    }
    result
}

fn skip_literal_or_comment(character: char, characters: &mut std::iter::Peekable<std::str::CharIndices<'_>>) -> bool {
    if matches!(character, '\'' | '"') {
        while let Some((_, next)) = characters.next() {
            if next == '\\' {
                characters.next();
            } else if next == character {
                break;
            }
        }
        return true;
    }
    if character != '/' {
        return false;
    }
    match characters.peek().map(|(_, character)| *character) {
        Some('/') => {
            characters.find(|(_, character)| *character == '\n');
            true
        }
        Some('*') => {
            characters.next();
            while let Some((_, next)) = characters.next() {
                if next == '*' && characters.next_if(|(_, character)| *character == '/').is_some() {
                    break;
                }
            }
            true
        }
        _ => false,
    }
}
