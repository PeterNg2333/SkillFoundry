mod support;

use support::{Project, diagnostic};

const CHECK: &[&str] = &["check", "--checks", "file-structure", "--json"];

#[test]
fn checks_filenames_and_inherits_matching_patterns() {
    let project = Project::new(&[
        (
            "check.json",
            r#"{
                "fileStructure": {
                    "$accepts": ["./*.json", "*.ts"],
                    "$ignore": ["*.generated.ts"],
                    "/": {
                        ".gitignore": {},
                        ".dockerignore": {},
                        "app": {
                            "$accepts": ["./*.ico"],
                            "/": {"routes": {"/": {}}}
                        },
                        "data": {
                            "$accepts": ["*.geojson", "read.generated.ts"],
                            "/": {
                                "keep.generated.ts": {},
                                "[floor]": {"/": {}}
                            }
                        },
                        "dist": {"$ignore": true, "/": {}},
                        "vendor": {"$ignore": true, "/": {}}
                    }
                }
            }"#,
        ),
        (".gitignore", "local/\n"),
        (".dockerignore", "data/\n"),
        ("app/favicon.ico", ""),
        ("app/routes/layout.ts", ""),
        (
            "data/[floor]/shape.geojson",
            "Contents are outside this check.",
        ),
        ("data/keep.generated.ts", ""),
        ("data/read.generated.ts", ""),
        ("data/skip.generated.ts", ""),
        ("dist/nested/cache.txt", ""),
        ("vendor/nested/cache.txt", ""),
        ("local/nested/cache.txt", ""),
    ]);
    let report = project.success(CHECK, true);
    assert_eq!(report["checks"][0]["id"], "file-structure", "{report}");
    assert_eq!(report["checks"][0]["files"], 8, "{report}");
    assert_eq!(report["summary"]["checked"], 8, "{report}");
    let skipped = report["skipped"].as_array().unwrap();
    for path in ["data/skip.generated.ts", "vendor"] {
        assert!(
            skipped.iter().any(|entry| {
                entry["path"] == path && entry["reason"] == "file-structure-ignore"
            }),
            "{report}"
        );
    }
    assert!(
        skipped
            .iter()
            .any(|entry| entry["path"] == "local" && entry["reason"] == "gitignore"),
        "{report}"
    );
}

#[test]
fn rejects_unlisted_types_and_folders_and_checks_only_requested_scope() {
    let project = Project::new(&[
        (
            "check.json",
            r#"{
                "fileStructure": {
                    "$root": "src",
                    "$accepts": ["*.ts"],
                    "/": {
                        "app": {
                            "$accepts": ["./*.ico"],
                            "/": {"nested": {"/": {}}, "optional.tsx": {}}
                        },
                        "data": {"/": {"pending": {"/": {}}}}
                    }
                }
            }"#,
        ),
        ("src/app/favicon.ico", ""),
        ("src/app/nested/icon.ico", ""),
        ("src/app/nested/another.ts", ""),
        ("src/data/pending", ""),
        ("src/data/script.txt", ""),
        ("src/data/unknown/module.ts", ""),
        ("outside.bin", ""),
    ]);
    let (status, report, stderr) = project.run(CHECK, true);
    assert_eq!(status, 1, "{stderr}\n{report}");
    let findings = report["diagnostics"].as_array().unwrap();
    assert_eq!(findings.len(), 4, "{report}");
    let expected = [
        ("src/app/nested/icon.ico", "*.ts"),
        ("src/data/pending", "expected a directory"),
        ("src/data/script.txt", "*.ts"),
        ("src/data/unknown", "directory is not declared"),
    ];
    for (finding, (path, explanation)) in findings.iter().zip(expected) {
        assert_eq!(finding["file"], path, "{report}");
        assert_eq!(finding["rule"], "file-structure/path", "{report}");
        assert!(
            finding["message"].as_str().unwrap().contains(explanation),
            "{finding}"
        );
    }
    let args = [CHECK, &["--", "src/app/favicon.ico"]].concat();
    project.success(&args, true);
    let args = [CHECK, &["--", "outside.bin"]].concat();
    let report = project.success(&args, true);
    assert_eq!(report["checks"][0]["status"], "not-applicable", "{report}");
}

#[test]
fn runs_opt_in_structure_by_default_and_preserves_source_routing() {
    let project = Project::new(&[
        (
            "check.json",
            r#"{"fileStructure": {"$root": "src", "$accepts": ["*.ts"], "/": {}}}"#,
        ),
        (".dockerignore", "src/\n"),
        ("src/invalid.ts", "const value = "),
        ("src/notes.txt", ""),
    ]);
    let (status, report, stderr) = project.run(&["check", "--json"], true);
    assert_eq!(status, 1, "{stderr}\n{report}");
    assert_eq!(report["summary"]["selected"], 3, "{report}");
    assert_eq!(report["summary"]["checked"], 3, "{report}");
    let findings = report["diagnostics"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{report}");
    assert_eq!(findings[0]["file"], "src/notes.txt", "{report}");
    assert!(
        diagnostic(&report, "file-structure/path").is_some(),
        "{report}"
    );
    project.success(&["check", "--checks", "typescript", "--json"], true);
}

#[test]
fn reports_missing_scope_and_invalid_policy() {
    let project = Project::new(&[(
        "check.json",
        r#"{"fileStructure": {"$root": "missing", "/": {}}}"#,
    )]);
    let (status, report, stderr) = project.run(CHECK, true);
    assert_eq!(status, 1, "{stderr}\n{report}");
    assert_eq!(report["diagnostics"][0]["file"], "missing", "{report}");

    for (policy, expected) in [
        (r#"{"$root": "../src", "/": {}}"#, "$root"),
        (r#"{"/": {"app/routes": {"/": {}}}}"#, "app/routes"),
        (r#"{"/": {"*.ico": {}}}"#, "*.ico"),
        (
            r#"{"$accepts": ["["], "/": {}}"#,
            "invalid fileStructure glob",
        ),
    ] {
        let config = format!(r#"{{"fileStructure": {policy}}}"#);
        let project = Project::new(&[("check.json", &config)]);
        let (status, _, stderr) = project.run(CHECK, true);
        assert_eq!(status, 2, "{stderr}");
        assert!(stderr.contains("fileStructure"), "{stderr}");
        assert!(stderr.contains(expected), "{stderr}");
    }
}
