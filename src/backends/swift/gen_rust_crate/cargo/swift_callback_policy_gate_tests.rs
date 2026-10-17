use std::path::PathBuf;
use std::process::Command;

const FILTER: &str = "swift_callback_link_tests";
const TASK: &str = "gate:swift-callback-policy";
const STEP: &str = "Verify emitted Swift callback policy";
const TASK_COMMAND: &str = "task test:run -- --lib swift_callback_link_tests -- --ignored --test-threads=1";
const EXPECTED: [&str; 2] = [
    "backends::swift::gen_rust_crate::cargo::swift_callback_link_tests::should_allow_only_actual_swift_callbacks_in_apple_cdylibs",
    "backends::swift::gen_rust_crate::cargo::swift_callback_link_tests::should_validate_actual_emitted_swift_callback_exports",
];

fn selected_tests(listing: &str) -> Vec<&str> {
    let mut selected = listing
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect::<Vec<_>>();
    selected.sort_unstable();
    selected
}

fn validate_selection(selected: &[&str]) -> Result<(), &'static str> {
    if selected == EXPECTED {
        Ok(())
    } else {
        Err("Swift policy filter must select exactly the two named ignored tests")
    }
}

fn list_ignored(filter: &str) -> String {
    let output = Command::new(std::env::current_exe().expect("current test executable"))
        .args(["--ignored", "--list", filter])
        .output()
        .expect("list ignored tests in the actual current binary");
    assert!(
        output.status.success(),
        "listing failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("libtest listing is UTF-8")
}

#[test]
fn should_select_exactly_two_actual_ignored_swift_policy_tests() {
    let listing = list_ignored(FILTER);
    let selected = selected_tests(&listing);
    assert_eq!(
        validate_selection(&selected),
        Ok(()),
        "actual selected names: {selected:?}; listing: {listing}"
    );
}

#[test]
fn should_reject_an_actual_zero_match_libtest_selection() {
    let listing = list_ignored("definitely_no_such_swift_callback_policy_test_572");
    let selected = selected_tests(&listing);
    assert!(
        selected.is_empty(),
        "negative control unexpectedly matched {selected:?}"
    );
    assert_eq!(
        validate_selection(&selected),
        Err("Swift policy filter must select exactly the two named ignored tests")
    );
}

#[test]
fn should_reject_deleted_renamed_duplicate_or_extra_selected_tests() {
    for selected in [
        vec![EXPECTED[0]],
        vec![EXPECTED[0], "renamed::ignored_policy"],
        vec![EXPECTED[0], EXPECTED[0]],
        vec![EXPECTED[0], EXPECTED[1], "extra::ignored_policy"],
    ] {
        assert!(
            validate_selection(&selected).is_err(),
            "accepted wrong nonempty selection {selected:?}"
        );
    }
}

fn named_block<'a>(source: &'a str, header: &str, indent: usize) -> Option<Vec<&'a str>> {
    let prefix = " ".repeat(indent);
    let mut lines = source.lines().skip_while(|line| *line != header);
    lines.next()?;
    Some(
        lines
            .take_while(|line| {
                let trimmed = line.trim();
                trimmed.is_empty()
                    || trimmed.starts_with('#')
                    || !line.starts_with(&prefix)
                    || line.as_bytes().get(indent).is_some_and(u8::is_ascii_whitespace)
            })
            .collect(),
    )
}

fn step_block<'a>(job: &[&'a str]) -> Option<Vec<&'a str>> {
    let header = format!("      - name: {STEP}");
    let mut lines = job.iter().copied().skip_while(|line| *line != header);
    lines.next()?;
    Some(lines.take_while(|line| !line.starts_with("      - ")).collect())
}

fn fields<'a>(lines: &'a [&str]) -> Vec<&'a str> {
    lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn validate_wiring(workflow: &str, tasks: &str) -> Result<(), &'static str> {
    let job = named_block(workflow, "  test:", 2).ok_or("missing test job")?;
    if job.iter().any(|line| line.starts_with("    continue-on-error:")) {
        return Err("existing CI test job must not ignore failures");
    }
    let step = step_block(&job).ok_or("missing qualified policy step")?;
    let actual = fields(&step);
    let expected = [
        "if: runner.os == 'macOS'",
        "timeout-minutes: 10",
        "run: task gate:swift-callback-policy",
    ];
    if actual != expected {
        return Err("policy step must contain only its macOS condition, bounded timeout and named Task run");
    }
    let task = named_block(tasks, &format!("  {TASK}:"), 2).ok_or("missing named policy Task")?;
    let actual_task = fields(&task);
    if actual_task
        .iter()
        .any(|line| !line.starts_with("desc: ") && *line != "cmds:" && *line != format!("- {TASK_COMMAND}"))
        || actual_task.iter().filter(|line| **line == "cmds:").count() != 1
    {
        return Err("policy Task must not have skipping, ignored failure or unknown directives");
    }
    let commands = task
        .iter()
        .map(|line| line.trim())
        .filter_map(|line| line.strip_prefix("- "))
        .collect::<Vec<_>>();
    if commands != [TASK_COMMAND] {
        return Err("policy Task must run only the exact ignored two-test selector through test:run");
    }
    Ok(())
}

#[test]
fn should_wire_the_named_task_into_the_qualified_existing_ci_job() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workflow = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read CI workflow");
    let tasks = std::fs::read_to_string(root.join("Taskfile.yml")).expect("read Taskfile");
    assert_eq!(validate_wiring(&workflow, &tasks), Ok(()));
}

const VALID_WORKFLOW: &str = "jobs:\n  test:\n    steps:\n      - name: Verify emitted Swift callback policy\n        if: runner.os == 'macOS'\n        timeout-minutes: 10\n        run: task gate:swift-callback-policy\n  neighbour:\n    steps:\n      - run: unrelated\n";
const VALID_TASKS: &str = "tasks:\n  gate:swift-callback-policy:\n    desc: Compile emitted Swift callback policy\n    cmds:\n      - task test:run -- --lib swift_callback_link_tests -- --ignored --test-threads=1\n  neighbour:\n    cmds:\n      - unrelated\n";

#[test]
fn should_accept_the_actual_supported_step_and_task_shape() {
    assert_eq!(validate_wiring(VALID_WORKFLOW, VALID_TASKS), Ok(()));
    let preceding = VALID_WORKFLOW.replace(
        "      - name: Verify",
        "      - name: Target without run\n      - name: Verify",
    );
    assert_eq!(validate_wiring(&preceding, VALID_TASKS), Ok(()));
}

#[test]
fn should_reject_workflow_mutants_even_when_comments_or_neighbours_hold_the_command() {
    let mutants = [
        VALID_WORKFLOW.replace("  test:", "  wrong_job:"),
        VALID_WORKFLOW.replace("  test:\n", "  test:\n    continue-on-error: true\n"),
        VALID_WORKFLOW.replace(STEP, "Wrong step"),
        VALID_WORKFLOW.replace(
            "run: task gate:swift-callback-policy",
            "# run: task gate:swift-callback-policy",
        ),
        VALID_WORKFLOW.replace("run: task gate:swift-callback-policy", "run: task unrelated"),
        VALID_WORKFLOW.replace("if: runner.os == 'macOS'", "if: runner.os == 'Linux'"),
        VALID_WORKFLOW.replace("        timeout-minutes: 10\n", ""),
        VALID_WORKFLOW.replace("timeout-minutes: 10", "timeout-minutes: 0"),
        VALID_WORKFLOW.replace("        run:", "        continue-on-error: true\n        run:"),
    ];
    for (index, mutant) in mutants.iter().enumerate() {
        assert!(
            validate_wiring(mutant, VALID_TASKS).is_err(),
            "accepted workflow mutant {index}"
        );
    }
}

#[test]
fn should_reject_task_mutants_without_borrowing_comment_or_neighbour_commands() {
    let mutants = [
        VALID_TASKS.replace("  gate:swift-callback-policy:", "  other-task:"),
        VALID_TASKS.replace("--ignored ", ""),
        VALID_TASKS.replace(FILTER, "typo_callback_filter"),
        VALID_TASKS.replace("--lib ", ""),
        VALID_TASKS.replace("--test-threads=1", "--test-threads=8"),
        VALID_TASKS.replace("      - task test:run", "      # - task test:run"),
        VALID_TASKS.replace("    cmds:\n", "    ignore_error: true\n    cmds:\n"),
        VALID_TASKS
            .replace("      - unrelated", &format!("      - {TASK_COMMAND}"))
            .replace(
                &format!("      - {TASK_COMMAND}\n  neighbour:"),
                "      - task unrelated\n  neighbour:",
            ),
    ];
    for (index, mutant) in mutants.iter().enumerate() {
        assert!(
            validate_wiring(VALID_WORKFLOW, mutant).is_err(),
            "accepted Task mutant {index}"
        );
    }
}
