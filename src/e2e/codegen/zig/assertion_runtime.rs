use crate::e2e::field_access::FieldResolver;
use crate::e2e::fixture::Fixture;

pub(super) fn has_unavailable_fields(fixture: &Fixture, resolver: &FieldResolver) -> bool {
    fixture.assertions.iter().any(|assertion| {
        assertion
            .field
            .as_deref()
            .is_some_and(|field| !field.is_empty() && !resolver.is_valid_for_result(field))
    })
}

pub(super) fn append_assertions(out: &mut String, body: &str, fixture: &Fixture) {
    crate::e2e::codegen::fail_on_unavailable_field_markers(body, "zig", &fixture.id, &fixture.assertions);
    crate::e2e::codegen::fail_on_unsupported_assertion_type_markers(body, "zig", &fixture.id);
    out.push_str(body);
    out.push_str(&crate::e2e::codegen::inert_example::coverage_warning(
        body,
        "zig",
        &fixture.id,
    ));
}
