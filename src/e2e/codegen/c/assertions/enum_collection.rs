use super::nested_access::{self, NativeAccessorArgs};
use super::*;
use crate::e2e::field_access::WireAccessStep;

pub(in crate::e2e::codegen::c) fn emit_ir_nested_accessor(
    out: &mut String,
    resolver: &FieldResolver,
    args: NativeAccessorArgs<'_>,
) -> anyhow::Result<Option<NestedLeafOutcome>> {
    let Some((prefix, owner, variant, suffix)) = resolver.ir_tagged_union_split(args.resolved) else {
        return nested_access::emit(out, args, false);
    };
    let supported = resolver.enum_definition(&owner).is_some_and(|item| {
        item.serde_untagged
            && item
                .variants
                .iter()
                .any(|definition| definition.name == variant && definition.is_tuple)
    }) && resolver.union_variant_payload_is_collection(&owner, &variant)
        && !prefix.is_empty();
    if !supported {
        return nested_access::emit(out, args, false);
    }
    let Some(steps) = resolver.enum_payload_wire_steps(&owner, &variant, &suffix) else {
        return nested_access::emit(out, args, false);
    };
    let base = if steps.is_empty() {
        args.local_var.to_string()
    } else {
        format!("{}_enum_json", args.local_var)
    };
    let outcome = nested_access::emit(
        out,
        NativeAccessorArgs {
            resolved: &prefix,
            local_var: &base,
            intermediate_handles: &mut *args.intermediate_handles,
            ..args
        },
        true,
    )?;
    anyhow::ensure!(
        outcome.is_none(),
        "C enum collection prefix {prefix} must expose JSON, got {outcome:?}"
    );
    if !steps.is_empty() {
        args.intermediate_handles.push((base.clone(), "free".to_string()));
    }
    let mut current = base.clone();
    let mut rendered_steps = Vec::new();
    let count = steps.len();
    for (index, step) in steps.into_iter().enumerate() {
        let name = if index + 1 == count {
            args.local_var.to_string()
        } else {
            format!("{}_enum_step_{index}", args.local_var)
        };
        let (kind, value) = match step {
            WireAccessStep::Index(index) => ("index", index.to_string()),
            WireAccessStep::Field(key) => (if index + 1 == count { "string" } else { "field" }, escape_c(&key)),
        };
        rendered_steps
            .push(crate::alef_context! { expression => current, name => &name, kind => kind, value => value });
        if index + 1 != count {
            args.intermediate_handles.push((name.clone(), "free".to_string()));
        }
        current = name;
    }
    out.push_str(&crate::e2e::template_env::render(
        "c/enum_collection_access.jinja",
        crate::alef_context! {
            expression => base, steps => rendered_steps,
        },
    ));
    Ok(None)
}

#[cfg(test)]
#[path = "enum_collection/tests.rs"]
mod tests;
