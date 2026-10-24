use crate::e2e::codegen::inert_example::{self, InertCause};
use crate::e2e::escape::escape_java;

/// The body emitted in place of one whose declared assertions all funnelled into skip markers.
///
/// ~keep Which refusal is emitted follows who can fix it, exactly as in `ruby/examples.rs`. An
/// unresolved field path is the consumer's to repair, so it gets an `Assertions.fail` naming the
/// fixture — under the default strict setting the run has already failed, and the deliberately
/// disarmed run must still not go green. Everything else is alef's generator debt or a JNI/ABI
/// limit no consumer edit clears; failing their suite for it would only force a blanket opt-out,
/// so it gets JUnit's `Assumptions.assumeTrue(false, ..)`, which reports the test as skipped and
/// never as a pass. Both are fully qualified, which is what `test_method.jinja`'s fixed import set
/// makes necessary — the same spelling the api-key assumption above already uses.
pub(super) fn render_java_refusal(markers: &str, refusal: &inert_example::InertExample) -> String {
    let reason = escape_java(&refusal.reason());
    let statement = crate::e2e::template_env::render(
        "runtime_refusal.jinja",
        crate::alef_context! {
            language => "java", reason => reason, unresolved => refusal.cause == InertCause::UnresolvedFieldPath,
        },
    );
    inert_example::refusal_body(markers, &statement)
}
