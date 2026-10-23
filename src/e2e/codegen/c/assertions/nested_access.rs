use super::*;

pub(in crate::e2e::codegen::c) struct NativeAccessorArgs<'a> {
    pub prefix: &'a str,
    pub resolved: &'a str,
    pub local_var: &'a str,
    pub result_var: &'a str,
    pub fields_c_types: &'a HashMap<String, String>,
    pub fields_enum: &'a HashSet<String>,
    pub intermediate_handles: &'a mut Vec<(String, String)>,
    pub result_type_name: &'a str,
    pub raw_field: &'a str,
    pub type_defs: &'a [crate::core::ir::TypeDef],
    pub config_sources: &'a FieldConfigSources,
}

pub(super) fn emit(
    out: &mut String,
    mut args: NativeAccessorArgs<'_>,
    json_leaf: bool,
) -> anyhow::Result<Option<NestedLeafOutcome>> {
    let segments: Vec<&str> = args.resolved.split('.').collect();
    let mut walk = SegmentWalk {
        current_snake_type: args.result_type_name.to_snake_case(),
        current_handle: args.result_var.to_string(),
        current_type_from_ir: args
            .type_defs
            .iter()
            .any(|type_def| type_def.name == args.result_type_name),
        json_extract_mode: false,
        is_wildcard: false,
    };

    for (i, segment) in segments.iter().enumerate() {
        let is_leaf = i + 1 == segments.len();

        if json_leaf && walk.json_extract_mode && is_leaf {
            emit_json_leaf(out, args.local_var, &walk, segment);
            return Ok(None);
        }
        let step = advance(out, &mut args, &mut walk, &segments, i)?;
        match step {
            SegmentStep::Continue => continue,
            SegmentStep::Done(outcome) => return Ok(outcome),
        }
    }
    Ok(None)
}

fn advance(
    out: &mut String,
    args: &mut NativeAccessorArgs<'_>,
    walk: &mut SegmentWalk,
    segments: &[&str],
    index: usize,
) -> anyhow::Result<SegmentStep> {
    let segment = segments[index];
    let is_leaf = index + 1 == segments.len();
    if let Some(step) = step_prefixed_segment(
        &mut *out,
        args.prefix,
        &mut *args.intermediate_handles,
        &mut *walk,
        segment,
        is_leaf,
        args.local_var,
    ) {
        return Ok(step);
    }
    // cbindgen's `[export] prefix` is shouty-snake, not uppercase; re-deriving it here as
    // `to_uppercase` names types the generated header never declares for any prefix carrying an
    // internal word boundary (`SampleCore` -> `SAMPLECORE` vs the header's `SAMPLE_CORE`). ~keep
    let prefix_upper = crate::codegen::c_consumer::export_type_prefix(args.prefix);
    step_plain_segment(PlainSegmentArgs {
        out,
        prefix: args.prefix,
        prefix_upper: &prefix_upper,
        raw_field: args.raw_field,
        resolved: args.resolved,
        segment,
        is_leaf,
        segments,
        i: index,
        walk,
        local_var: args.local_var,
        fields_c_types: args.fields_c_types,
        fields_enum: args.fields_enum,
        intermediate_handles: &mut *args.intermediate_handles,
        result_type_name: args.result_type_name,
        type_defs: args.type_defs,
        config_sources: args.config_sources,
    })
}

fn emit_json_leaf(out: &mut String, local_var: &str, walk: &SegmentWalk, segment: &str) {
    out.push_str(&crate::e2e::template_env::render(
        "c/json_container_leaf.jinja",
        crate::alef_context! {
            local_var => local_var, expression => &walk.current_handle,
            key => escape_c(segment),
        },
    ));
}
