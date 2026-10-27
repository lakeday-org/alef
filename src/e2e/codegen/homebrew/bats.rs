use crate::core::config::e2e::HomebrewCliTest;
use crate::core::config::shell::quote_word;
use crate::core::hash::{self, CommentStyle};

pub(super) fn render_suite(
    tap: &str,
    cli_formula: &str,
    ffi_formula: Option<&str>,
    version: &str,
    ffi_lib_name: &str,
    cli_tests: &[HomebrewCliTest],
) -> String {
    let tests: Vec<_> = cli_tests
        .iter()
        .map(|test| {
            crate::alef_context! {
                title => quote_word(&test.name), command => quote_word(&test.command),
                expected => test.expect_contains.as_deref().map(quote_word),
            }
        })
        .collect();
    crate::e2e::template_env::render(
        "homebrew/suite.bats.jinja",
        crate::alef_context! {
            header => hash::e2e_header(CommentStyle::Hash), tap => quote_word(tap),
            cli_formula => quote_word(cli_formula), ffi_formula => ffi_formula.map(quote_word),
            version => quote_word(version), ffi_lib_name => quote_word(ffi_lib_name), tests => tests,
        },
    )
}
