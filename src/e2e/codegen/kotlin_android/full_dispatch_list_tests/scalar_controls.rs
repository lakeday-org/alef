#![allow(clippy::print_stdout)]

use std::path::Path;

use super::native_task;

pub(super) fn run(root: &Path, classpath: &str) {
    let model = root.join("BodyValue.kt");
    let source = std::fs::read_to_string(&model).unwrap();
    shadow_negative(root, classpath, &source);
    guard_negative(root, classpath, &source);
    std::fs::write(&model, source).unwrap();
    let restored = native_task(root, "kotlin", classpath, "");
    assert!(
        restored.success
            && restored
                .stdout
                .contains("android_boolean_roundtrips=2 jni_boolean_cases=1 non_boolean_controls=2"),
        "{}\n{}",
        restored.stdout,
        restored.stderr,
    );
    println!("old_scalar_shadow_negative=1 old_boolean_guard_negative=1");
}

fn shadow_negative(root: &Path, classpath: &str, source: &str) {
    assert_eq!(source.matches("val value: kotlin.Boolean").count(), 1);
    let old = source.replace("val value: kotlin.Boolean", "val value: Boolean");
    assert_ne!(old, source);
    std::fs::write(root.join("BodyValue.kt"), old).unwrap();
    let compile = native_task(root, "kotlin-compile", classpath, "");
    assert!(
        !compile.success && compile.stderr.contains("BodyValue.Boolean"),
        "{}\n{}",
        compile.stdout,
        compile.stderr,
    );
}

fn guard_negative(root: &Path, classpath: &str, source: &str) {
    let correct = "if (node.isBoolean) return BodyValue.Boolean";
    assert_eq!(source.matches(correct).count(), 1);
    let old = source.replace(correct, "if (node.isNumber) return BodyValue.Boolean");
    assert_ne!(old, source);
    std::fs::write(root.join("BodyValue.kt"), old).unwrap();
    let compile = native_task(root, "kotlin-compile", classpath, "");
    assert!(
        compile.success,
        "the guard negative control must compile: {}\n{}",
        compile.stdout, compile.stderr,
    );
    let run = native_task(root, "kotlin-run", classpath, "");
    assert!(
        !run.success
            && run
                .stderr
                .contains("Cannot deserialize BodyValue: no matching variant for JSON shape"),
        "{}\n{}",
        run.stdout,
        run.stderr,
    );
}
