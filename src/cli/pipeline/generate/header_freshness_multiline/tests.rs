use super::*;

const XBERG_GUARD: &str = r#"#if ((defined(XBERG_FEATURE_LATE_INTERACTION_PRESETS) ||                       \
      defined(XBERG_FEATURE_LATE_INTERACTION)) ||                              \
     defined(XBERG_FEATURE_LATE_INTERACTION_PRESETS))
uintptr_t xberg_max_sim_rank_len(XBERGAlefHandle _query, const char *_docs);
#endif
"#;

fn guard_problems(code: &str, exported: &[(&str, Option<&str>)]) -> Vec<String> {
    let exports = exported
        .iter()
        .map(|(name, cfg)| (name.to_string(), cfg.map(str::to_owned)))
        .collect();
    let occurrences = header_declared_function_occurrences(code, "xberg");
    assert_eq!(
        occurrences.len(),
        exported.len(),
        "every fixture export must be checked"
    );
    function_guard_drift(&exports, &occurrences, code, "XBERG")
}

#[test]
fn should_accept_formatted_xberg_guard_with_lf_and_crlf_continuations() {
    let exports = [(
        "xberg_max_sim_rank_len",
        Some("any(feature = \"late-interaction\", feature = \"late-interaction-presets\")"),
    )];
    for header in [XBERG_GUARD.to_owned(), XBERG_GUARD.replace('\n', "\r\n")] {
        assert_eq!(guard_problems(&header, &exports), Vec::<String>::new());
    }
}

#[test]
fn should_preserve_nested_branch_guards_and_unicode_byte_offsets() {
    let header = "const char *label = \"文档\";\n\
                  #if defined(XBERG_FEATURE_OUTER)\n\
                  #if defined(UNRELATED) || \\\n                  defined(XBERG_FEATURE_FIRST)\n\
                  void xberg_first(void);\n\
                  #elif defined(UNRELATED) || \\\n                  defined(XBERG_FEATURE_SECOND)\n\
                  void xberg_second(void);\n\
                  #else\n\
                  void xberg_other(void);\n\
                  #endif\n\
                  void xberg_outer(void);\n\
                  #endif\n\
                  void xberg_unconditional(void);";
    let exports = [
        ("xberg_first", Some("all(feature = \"outer\", feature = \"first\")")),
        ("xberg_second", Some("all(feature = \"outer\", feature = \"second\")")),
        ("xberg_other", Some("feature = \"outer\"")),
        ("xberg_outer", Some("feature = \"outer\"")),
        ("xberg_unconditional", None),
    ];
    assert_eq!(guard_problems(header, &exports), Vec::<String>::new());
    let stacks = guard_stack_by_line(header);
    for (name, cfg) in exports {
        let offset = header.find(name).expect("fixture declaration");
        let expected = cfg.map(|cfg| required_feature_macros(cfg, "XBERG")).unwrap_or_default();
        assert_eq!(active_prefix_macros(&stacks, offset, "XBERG"), expected, "{name}");
    }
    let expected_offsets: Vec<_> = std::iter::once(0)
        .chain(header.match_indices('\n').map(|(offset, _)| offset + 1))
        .collect();
    let actual_offsets: Vec<_> = guard_stack_by_line(header).iter().map(|(offset, _)| *offset).collect();
    assert_eq!(actual_offsets, expected_offsets);
}

#[test]
fn should_splice_continuations_without_inserting_whitespace() {
    let header = "#if de\\\nfined(XBERG_FEATURE_FIRST)\nvoid xberg_first(void);\n#endif\n";
    assert_eq!(
        guard_problems(header, &[("xberg_first", Some("feature = \"first\""))]),
        Vec::<String>::new()
    );
}

#[test]
fn should_reject_wrong_macro_in_a_continued_guard() {
    let header = XBERG_GUARD.replace("XBERG_FEATURE_LATE_INTERACTION)", "XBERG_FEATURE_WRONG)");
    let problems = guard_problems(
        &header,
        &[("xberg_max_sim_rank_len", Some("feature = \"late-interaction\""))],
    );
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("matching #if defined(XBERG_FEATURE_LATE_INTERACTION) guard"));
}
