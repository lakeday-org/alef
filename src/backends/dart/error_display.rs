use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

mod lexical;
mod metadata;
#[cfg(test)]
mod tests;

pub(crate) use metadata::metadata_comments;
#[cfg(test)]
use metadata::records;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    owner: String,
    factory: String,
    fields: usize,
    chunks: Vec<Chunk>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Chunk {
    Text { text: String },
    Field { field: usize },
}

pub(crate) fn rewrite(source: &str, bridge: &str, mirror: &str) -> Result<String> {
    let records = read_records(mirror)?;
    if records.is_empty() {
        return Ok(source.to_string());
    }
    let bridge_mask = lexical::mask(bridge)?;
    let source_mask = lexical::mask(source)?;
    let mut edits = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for record in records {
        ensure!(
            seen.insert((record.owner.clone(), record.factory.clone())),
            "duplicate Dart error display metadata"
        );
        edits.push(
            edit(source, &source_mask, &bridge_mask, &record)
                .with_context(|| format!("restoring Dart error display for {}.{}", record.owner, record.factory))?,
        );
    }
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    ensure!(
        edits.windows(2).all(|pair| pair[0].0.start >= pair[1].0.end),
        "overlapping Freezed error implementations"
    );
    let mut result = source.to_string();
    for (range, replacement) in edits {
        result.replace_range(range, &replacement);
    }
    Ok(result)
}

fn read_records(mirror: &str) -> Result<Vec<Record>> {
    mirror
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("// ~keep alef-error-display: "))
        .map(|json| serde_json::from_str(json).context("reading generated Dart error display metadata"))
        .collect()
}

fn one_match<'a>(source: &'a str, pattern: &str, description: &str) -> Result<regex::Match<'a>> {
    let regex = regex::Regex::new(pattern)?;
    let matches: Vec<_> = regex.find_iter(source).collect();
    ensure!(
        matches.len() == 1,
        "expected one {description}, found {}",
        matches.len()
    );
    Ok(matches[0])
}

fn factory_class(bridge: &str, record: &Record) -> Result<(String, Vec<String>)> {
    let pattern = format!(
        r"\bfactory\s+{}\.{}\s*\(",
        regex::escape(&record.owner),
        regex::escape(&record.factory)
    );
    let declaration = one_match(bridge, &pattern, "FRB error factory")?;
    let end = lexical::closing(bridge, declaration.end() - 1)?;
    let fields = lexical::parameters(&bridge[declaration.end()..end])?;
    ensure!(fields.len() == record.fields, "FRB factory field cardinality changed");
    let suffix = regex::Regex::new(r"^\s*=\s*([$_\p{XID_Start}][$_\p{XID_Continue}]*)\s*;")?;
    suffix
        .captures(&bridge[end + 1..])
        .and_then(|captures| captures.get(1))
        .map(|name| (name.as_str().to_string(), fields))
        .context("reading actual Freezed error class alias")
}

fn class_body<'a>(source: &'a str, name: &str) -> Result<(&'a str, usize)> {
    let pattern = format!(r"\bclass\s+{}\b[^{{}}]*\{{", regex::escape(name));
    let declaration = one_match(source, &pattern, "Freezed error class")?;
    let end = lexical::closing(source, declaration.end() - 1)?;
    Ok((&source[declaration.end()..end], declaration.end()))
}

fn field_names(body: &str, class: &str, count: usize) -> Result<Vec<String>> {
    let pattern = format!(r"\b{}\s*\(", regex::escape(class));
    let constructor = one_match(body, &pattern, "Freezed error constructor")?;
    let end = lexical::closing(body, constructor.end() - 1)?;
    let parameters = lexical::parameters(&body[constructor.end()..end])?;
    ensure!(
        parameters.len() == count,
        "expected {count} mirrored fields, found {}",
        parameters.len()
    );
    Ok(parameters)
}

fn expression(record: &Record, fields: &[String]) -> Result<String> {
    record
        .chunks
        .iter()
        .map(|chunk| {
            Ok(match chunk {
                Chunk::Text { text } => serde_json::to_string(text)?.replace('$', "\\$"),
                Chunk::Field { field } => format!(
                    "{}.toString()",
                    fields.get(*field).context("missing mirrored display field")?
                ),
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(|pieces| pieces.join(" + "))
}

fn edit(source: &str, mask: &str, bridge: &str, record: &Record) -> Result<(std::ops::Range<usize>, String)> {
    let (class, fields) = factory_class(bridge, record)?;
    let (body, offset) = class_body(mask, &class)?;
    let properties = field_names(body, &class, record.fields)?;
    ensure!(
        fields
            .iter()
            .all(|field| properties.iter().filter(|name| *name == field).count() == 1),
        "FRB factory fields do not uniquely bind Freezed constructor properties"
    );
    let method = one_match(
        body,
        r"\bString\s+toString\s*\(\s*\)\s*\{",
        "Freezed error toString method",
    )?;
    let start = offset + method.start();
    let end = lexical::closing(mask, offset + method.end() - 1)? + 1;
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let indent = &source[line_start..start];
    ensure!(
        indent.trim().is_empty(),
        "Freezed toString declaration must begin a line"
    );
    let replacement = super::template_env::render(
        "dart_error_display_method.jinja",
        crate::alef_context! {
            indent => indent, expression => expression(record, &fields)?,
        },
    );
    Ok((line_start..end, replacement.trim_end_matches('\n').to_string()))
}
