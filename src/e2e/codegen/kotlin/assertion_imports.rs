use std::collections::BTreeSet;

use crate::core::ir::{EnumDef, FunctionDef, TypeDef};
use crate::e2e::config::E2eConfig;
use crate::e2e::fixture::Fixture;

pub(super) fn candidates(
    fixtures: &[&Fixture],
    config: &E2eConfig,
    types: &[TypeDef],
    enums: &[EnumDef],
    functions: &[FunctionDef],
) -> Vec<String> {
    let mut candidates = BTreeSet::new();
    for fixture in fixtures {
        let call = config.resolve_call_for_fixture(
            fixture.call.as_deref(),
            &fixture.id,
            &fixture.resolved_category(),
            &fixture.tags,
            &fixture.input,
        );
        let resolver = super::call_field_resolver::build_call_field_resolver(
            super::call_field_resolver::CallFieldResolverInputs {
                e2e_config: config,
                call_config: call,
                fixture,
                lang: "kotlin_android",
                type_defs: types,
                enums,
                functions,
            },
        );
        for field in fixture
            .assertions
            .iter()
            .filter_map(|assertion| assertion.field.as_deref())
        {
            candidates.extend(super::discriminated::android_imports(
                &resolver,
                resolver.resolve(field),
            ));
        }
    }
    candidates.into_iter().collect()
}

pub(super) fn code_identifiers(source: &str, include_members: bool) -> BTreeSet<&str> {
    // ~keep Skip comments and literals before matching identifiers: refusal diagnostics and
    // fixture descriptions can mention a type without using its import.
    let tokens = regex::Regex::new(
        r#"(?s)//[^\n]*|/\*.*?\*/|""".*?"""|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|([_\p{XID_Start}][_\p{XID_Continue}]*)"#,
    )
    .expect("Kotlin identifier pattern");
    tokens
        .captures_iter(source)
        .filter_map(|capture| capture.get(1))
        .filter(|identifier| include_members || !source[..identifier.start()].trim_end().ends_with('.'))
        .map(|identifier| identifier.as_str())
        .collect()
}

#[cfg(test)]
mod native_tests;
#[cfg(test)]
mod tests;
