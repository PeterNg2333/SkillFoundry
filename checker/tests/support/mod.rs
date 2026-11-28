use serde_json::Value;
use std::{fs, process::Command};
use tempfile::TempDir;

pub struct Project(TempDir);

impl Project {
    pub fn new(files: &[(&str, &str)]) -> Self {
        let root = tempfile::tempdir().unwrap();
        for (name, contents) in files {
            let file = root.path().join(name);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, contents).unwrap();
        }
        Self(root)
    }

    pub fn run(&self, arguments: &[&str], isolated: bool) -> (i32, Value, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_maintainable-check"));
        command.arg("--root").arg(self.0.path()).args(arguments);
        if isolated {
            command.env("PATH", "");
        }
        let output = command.output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        (
            output.status.code().unwrap(),
            serde_json::from_str(&stdout).unwrap_or(Value::Null),
            stderr,
        )
    }

    pub fn success(&self, arguments: &[&str], isolated: bool) -> Value {
        let (status, report, stderr) = self.run(arguments, isolated);
        assert_eq!(status, 0, "{stderr}\n{report}");
        report
    }
}

// Each test crate compiles this module separately; not every crate uses this helper.
#[allow(dead_code)]
pub fn diagnostic<'a>(report: &'a Value, rule: &str) -> Option<&'a Value> {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["rule"] == rule)
}
