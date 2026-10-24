use super::escape_swift;
use crate::e2e::codegen::inert_example::{self, InertCause, InertExample};

/// The body emitted in place of one whose declared assertions all funnelled into skip markers.
///
/// ~keep Which refusal is emitted follows who can fix it, exactly as in `ruby/examples.rs`. An
/// unresolved field path is the consumer's to repair, so it gets `XCTFail` — under the default
/// strict setting the run has already failed, and the deliberately disarmed run must still not go
/// green. Everything else is alef's generator debt or a swift-bridge limit that no consumer edit
/// clears; failing their suite for it would only force a blanket opt-out, so it gets `XCTSkipIf`,
/// which XCTest reports as skipped and never as a pass. Both spellings are already emitted
/// elsewhere in this file, so neither introduces a construct the generated project cannot build.
pub(super) fn render_swift_refusal(markers: &str, refusal: &InertExample) -> String {
    let reason = escape_swift(&refusal.reason());
    let statement = crate::e2e::template_env::render(
        "runtime_refusal.jinja",
        crate::alef_context! {
            language => "swift", reason => reason, unresolved => refusal.cause == InertCause::UnresolvedFieldPath,
        },
    );
    inert_example::refusal_body(markers, &statement)
}
