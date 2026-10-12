use std::collections::{BTreeMap, HashSet};

use crate::core::ir::{ApiSurface, ErrorDef};

#[derive(Clone, Copy)]
pub enum ExceptionNaming {
    Python,
    Nominal,
}

/// One error with shared classes assigned to their most specific compatible family. ~keep
pub struct ExceptionFamily {
    pub error: ErrorDef,
    pub parent: Option<String>,
}

/// Reject unknown families and cycles before emitting any binding artifact. ~keep
pub fn validate_exception_hierarchy(api: &ApiSurface) -> anyhow::Result<()> {
    let parents = exception_parents(api);
    for (child, parent) in &parents {
        anyhow::ensure!(
            api.errors.iter().any(|error| error.name == *parent),
            "unknown exception parent {parent}"
        );
        ancestors(child, &parents)?;
    }
    if !parents.is_empty() {
        variant_owners(api, ExceptionNaming::Python)?;
        variant_owners(api, ExceptionNaming::Nominal)?;
    }
    Ok(())
}

fn ancestors<'a>(name: &'a str, parents: &'a BTreeMap<String, String>) -> anyhow::Result<Vec<&'a str>> {
    let mut visited = HashSet::new();
    let mut names = Vec::new();
    let mut current = name;
    loop {
        anyhow::ensure!(visited.insert(current), "exception hierarchy cycle at {current}");
        names.push(current);
        let Some(parent) = parents.get(current) else {
            return Ok(names);
        };
        current = parent;
    }
}

/// Preserve legacy first-wins output unless a consumer explicitly declares a hierarchy. ~keep
pub fn exception_families(api: &ApiSurface, naming: ExceptionNaming) -> anyhow::Result<Vec<ExceptionFamily>> {
    validate_exception_hierarchy(api)?;
    let parents = exception_parents(api);
    if parents.is_empty() {
        return Ok(api
            .errors
            .iter()
            .cloned()
            .map(|error| ExceptionFamily { error, parent: None })
            .collect());
    }
    let owners = variant_owners(api, naming)?;
    let mut errors = api
        .errors
        .iter()
        .map(|error| Ok((ancestors(&error.name, &parents)?.len(), error.clone())))
        .collect::<anyhow::Result<Vec<_>>>()?;
    errors.sort_by_key(|(depth, _)| *depth);
    Ok(errors
        .into_iter()
        .map(|(_, mut error)| {
            error
                .variants
                .retain(|variant| owners[&class_name(&variant.name, &error.name, naming)] == error.name);
            ExceptionFamily {
                parent: parents.get(&error.name).cloned(),
                error,
            }
        })
        .collect())
}

fn class_name(variant: &str, error: &str, naming: ExceptionNaming) -> String {
    if matches!(naming, ExceptionNaming::Python) {
        super::python_exception_name(variant, error)
    } else {
        format!("{variant}Exception")
    }
}

fn variant_owners(api: &ApiSurface, naming: ExceptionNaming) -> anyhow::Result<BTreeMap<String, String>> {
    let parents = exception_parents(api);
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for error in &api.errors {
        for variant in &error.variants {
            let class = class_name(&variant.name, &error.name, naming);
            if let Some(owner) = owners.get(&class) {
                let owner_error = api
                    .errors
                    .iter()
                    .find(|error| error.name == *owner)
                    .ok_or_else(|| anyhow::anyhow!("missing exception owner {owner} for shared class {class}"))?;
                let layout = |error: &ErrorDef| {
                    error
                        .methods
                        .iter()
                        .map(|method| (method.name.clone(), method.return_type.clone()))
                        .collect::<Vec<_>>()
                };
                anyhow::ensure!(
                    layout(error) == layout(owner_error),
                    "incompatible exception introspection for {class} between {owner} and {}",
                    error.name
                );
                if ancestors(&error.name, &parents)?.contains(&owner.as_str()) {
                    owners.insert(class, error.name.clone());
                } else {
                    anyhow::ensure!(
                        ancestors(owner, &parents)?.contains(&error.name.as_str()),
                        "incompatible exception parents {owner} and {} for {class}",
                        error.name
                    );
                }
            } else {
                owners.insert(class, error.name.clone());
            }
        }
    }
    Ok(owners)
}

fn exception_parents(api: &ApiSurface) -> BTreeMap<String, String> {
    api.errors
        .iter()
        .filter_map(|error| {
            error
                .exception_base
                .as_ref()
                .map(|parent| (error.name.clone(), parent.clone()))
        })
        .collect()
}
