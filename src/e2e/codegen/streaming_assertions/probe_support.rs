use std::process::Output;

pub(crate) struct Probe {
    directory: tempfile::TempDir,
    language: &'static str,
}
impl Probe {
    pub fn new(language: &'static str) -> Self {
        let directory = tempfile::tempdir().expect("owned native probe");
        let commands = match language {
            "c" => r#"["cc probe.c -o probe", "./probe"]"#,
            "go" => r#"["go run probe.go"]"#,
            "node" => r#"["node probe.js"]"#,
            "swift" => r#"["swiftc probe.swift -o probe", "./probe"]"#,
            "dart" => r#"["dart run probe.dart"]"#,
            _ => panic!("unknown native probe"),
        };
        let tasks = ["[tasks.test]\nrun = ", commands, "\n"].concat();
        std::fs::write(directory.path().join("mise.toml"), tasks).expect("probe tasks");
        Self { directory, language }
    }
    pub fn run(&self, source: &str) -> Output {
        let extension = match self.language {
            "node" => "js",
            other => other,
        };
        let file = ["probe.", extension].concat();
        std::fs::write(self.directory.path().join(file), source).expect("probe source");
        crate::test_support::mise_task::mise_run(self.directory.path(), "test")
            .env("GOCACHE", self.directory.path().join("go-cache"))
            .output()
            .expect("native probe")
    }
}
pub(crate) fn assert_status(output: &Output, success: bool) {
    assert_eq!(
        output.status.success(),
        success,
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
