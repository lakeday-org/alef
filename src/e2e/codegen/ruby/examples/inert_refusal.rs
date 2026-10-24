use super::ruby_string_literal;
use crate::e2e::codegen::inert_example::{InertCause, InertExample};

/// The example emitted in place of one whose assertions all funnelled into skip markers.
///
/// ~keep The rendered markers are carried into the refusal rather than dropped: they are the only
/// record IN THE GENERATED FILE of what the fixture asked for and why it could not run, and the
/// point of this change is to stop publishing a green test, not to restore the silence the markers
/// were added to break.
///
/// Which refusal is emitted follows who can fix it. An unresolved field path is the consumer's to
/// repair — under the default strict setting the run has already failed, so this shape only ever
/// reaches a generated file on a deliberately disarmed run, and that run must still not go green:
/// it gets a real expectation that always fails and names the fixture. Everything else is alef's
/// generator debt or a language limit that no consumer edit clears; failing their suite for it
/// would only force a blanket opt-out, so it gets RSpec's own `skip`, which reports the example as
/// pending and never as a pass. `skip` also keeps the example present, so the enclosing `describe`
/// does not go empty.
pub(super) fn render_refused_example(description_literal: &str, markers: &str, refusal: &InertExample) -> String {
    let markers: Vec<_> = markers
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::trim_end)
        .collect();
    crate::e2e::template_env::render(
        "ruby/refused_example.jinja",
        crate::alef_context! {
            description => description_literal, markers => markers, reason => ruby_string_literal(&refusal.reason()),
            unresolved => refusal.cause == InertCause::UnresolvedFieldPath,
        },
    )
}
