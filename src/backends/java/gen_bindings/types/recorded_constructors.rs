use crate::backends::java::gen_bindings::helpers::safe_java_field_name;
use crate::core::ir::{FieldDef, TypeDef};
use anyhow::Context;

pub(super) fn render_recorded_constructors(
    owner: &TypeDef,
    fields: &[&FieldDef],
    declarations: &[String],
) -> anyhow::Result<String> {
    if !owner.has_recorded_constructors() {
        return Ok(String::new());
    }
    anyhow::ensure!(
        fields.len() == declarations.len(),
        "Java constructor declarations for {} do not match its exposed fields",
        owner.name
    );
    let canonical = fields.iter().map(|field| field.name.clone()).collect::<Vec<_>>();
    let mut signatures = vec![(erased_signature(owner, declarations)?, canonical)];
    let mut output = String::new();
    for recorded in std::iter::once(owner.legacy_constructor_fields())
        .filter(|fields| !fields.is_empty())
        .chain(owner.constructor_history().iter().map(Vec::as_slice))
    {
        let parameters = recorded_parameters(owner, fields, declarations, recorded)?;
        let erased = erased_signature(owner, &parameters)?;
        if let Some((_, previous)) = signatures.iter().find(|(signature, _)| signature == &erased) {
            anyhow::ensure!(
                previous == recorded,
                "Java recorded constructor for {} collides with another constructor after type erasure",
                owner.name
            );
            continue;
        }
        signatures.push((erased, recorded.to_vec()));
        output.push_str(&render_constructor(owner, fields, recorded, &parameters)?);
    }
    Ok(output)
}

fn recorded_parameters(
    owner: &TypeDef,
    fields: &[&FieldDef],
    declarations: &[String],
    recorded: &[String],
) -> anyhow::Result<Vec<String>> {
    let mut seen = std::collections::HashSet::new();
    recorded
        .iter()
        .map(|name| {
            anyhow::ensure!(
                seen.insert(name),
                "Java recorded constructor for {} repeats field {name}",
                owner.name
            );
            let index = fields.iter().position(|field| &field.name == name).ok_or_else(|| {
                anyhow::anyhow!("Java recorded constructor field {}.{name} is not exposed", owner.name)
            })?;
            Ok(declarations[index].clone())
        })
        .collect()
}

fn erased_signature(owner: &TypeDef, declarations: &[String]) -> anyhow::Result<Vec<String>> {
    declarations
        .iter()
        .map(|declaration| {
            let (ty, _) = declaration.rsplit_once(' ').ok_or_else(|| {
                anyhow::anyhow!(
                    "Java recorded constructor for {} has invalid parameter declaration {declaration:?}",
                    owner.name
                )
            })?;
            let mut depth = 0usize;
            Ok(ty
                .chars()
                .filter(|character| match character {
                    '<' => {
                        depth += 1;
                        false
                    }
                    '>' => {
                        depth = depth.saturating_sub(1);
                        false
                    }
                    _ => depth == 0 && !character.is_whitespace(),
                })
                .collect())
        })
        .collect()
}

fn render_constructor(
    owner: &TypeDef,
    fields: &[&FieldDef],
    recorded: &[String],
    parameters: &[String],
) -> anyhow::Result<String> {
    let arguments = fields
        .iter()
        .map(|field| {
            if recorded.contains(&field.name) {
                Ok(safe_java_field_name(&field.name))
            } else {
                super::constructor_defaults::java_constructor_default(field)
                    .with_context(|| format!("Java recorded constructor for {} omits {}", owner.name, field.name))
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(crate::backends::java::template_env::render(
        "record_legacy_constructor.jinja",
        crate::alef_context! {
            type_name => &owner.name,
            parameters => parameters.join(", "),
            arguments => arguments.join(", "),
        },
    ))
}

#[cfg(test)]
#[path = "recorded_constructors_tests.rs"]
mod tests;
