use crate::core::ir::EnumDef;
use crate::e2e::codegen::call_ir::CallIr;
use crate::e2e::config::{CallConfig, E2eConfig};
use crate::e2e::field_access::FieldResolver;

pub(super) fn build(
    config: &E2eConfig,
    call: &CallConfig,
    language: &str,
    ir: CallIr<'_>,
    enums: &[EnumDef],
) -> FieldResolver {
    let (reachable, excluded, optional) = FieldResolver::ir_field_sets(ir.type_defs);
    FieldResolver::new(
        config.effective_fields(call),
        config.effective_fields_optional(call),
        config.effective_result_fields(call),
        config.effective_fields_array(call),
        &std::collections::HashSet::new(),
    )
    .with_ir_fields(reachable, excluded, optional)
    .with_ir_enum_map(
        FieldResolver::ir_enum_fields(ir.type_defs, enums),
        crate::e2e::codegen::call_ir::resolve_declared_result_type(call, language, ir),
    )
}
