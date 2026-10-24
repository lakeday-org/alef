use std::process::{Command, Output};

pub(crate) struct Probe {
    directory: tempfile::TempDir,
    language: &'static str,
}
impl Probe {
    pub fn new(language: &'static str) -> Self {
        let directory = tempfile::tempdir().expect("owned native probe");
        let commands = match language {
            "c" => "      - cc probe.c -o probe\n      - ./probe\n",
            "go" => "      - go run probe.go\n",
            "node" => "      - node probe.js\n",
            "swift" => "      - swiftc probe.swift -o probe\n      - ./probe\n",
            "dart" => "      - dart run probe.dart\n",
            _ => panic!("unknown native probe"),
        };
        let tasks = ["version: '3'\ntasks:\n  test:\n    cmds:\n", commands].concat();
        std::fs::write(directory.path().join("Taskfile.yml"), tasks).expect("probe tasks");
        Self { directory, language }
    }
    pub fn run(&self, source: &str) -> Output {
        let extension = match self.language {
            "node" => "js",
            other => other,
        };
        let file = ["probe.", extension].concat();
        std::fs::write(self.directory.path().join(file), source).expect("probe source");
        let task = which::which_in("task", std::env::var_os("PATH"), self.directory.path()).expect("task executable");
        Command::new(task)
            .arg("test")
            .current_dir(self.directory.path())
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
