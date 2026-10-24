use crate::e2e::codegen::swift::values::escape_swift;
use crate::e2e::fixture::Fixture;
use std::fmt::Write as _;

/// Emit the `catch` block for an `error`-asserting test, closing the `do { … }` it follows.
///
/// ~keep When the fixture declares an expected error value, the check must match either
/// the caught error's description or its dynamic type name — never message-only — per the
/// shared contract in `declared_error_value`. With no declared value, output is byte-identical
/// to the old unconditional `// success` stub so untouched fixtures never see a diff. Which of
/// those two conventions applies, and whether Swift can ever satisfy the second, is decided once
/// by `declared_error_variant::classify` — see its doc for why Swift lands on "not yet" today
/// (swift-bridge's per-variant enum is never constructed for a real business-call failure).
pub(super) fn render_error_catch_block(out: &mut String, fixture: &Fixture, errors: &[crate::core::ir::ErrorDef]) {
    use crate::e2e::codegen::declared_error_variant::{DeclaredErrorAssertion, classify, skip_line};
    let _ = writeln!(out, "        }} catch {{");
    match classify("swift", fixture, errors) {
        DeclaredErrorAssertion::Assert(declared) => {
            let escaped = escape_swift(declared);
            let _ = writeln!(out, "            let _errorMessage = String(describing: error)");
            let _ = writeln!(out, "            let _errorType = String(describing: type(of: error))");
            let _ = writeln!(
                out,
                "            XCTAssertTrue(_errorMessage.contains(\"{escaped}\") || _errorType.contains(\"{escaped}\"), \"expected error to mention \\\"{escaped}\\\", got message: \\(_errorMessage), type: \\(_errorType)\")"
            );
        }
        DeclaredErrorAssertion::Unsubstantiable(variant) => {
            let _ = writeln!(
                out,
                "{}",
                skip_line("            ", "//", variant, &fixture.id, "swift")
            );
        }
        DeclaredErrorAssertion::Undeclared => {
            let _ = writeln!(out, "            // success");
        }
    }
    let _ = writeln!(out, "        }}");
}
