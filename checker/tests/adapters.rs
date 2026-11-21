mod support;

use support::{Project, diagnostic};

#[test]
fn explicitly_requested_missing_tools_report_incomplete_coverage() {
    let project = Project::new(&[
        ("main.py", "print('hello')\n"),
        ("main.go", "package main\n"),
    ]);
    let report = project.success(&["check", "--json"], true);
    assert_eq!(report["summary"]["checked"], 1, "{report}");
    let (code, report, stderr) =
        project.run(&["check", "--json", "--checks", "python,go-format"], true);
    assert_eq!(code, 2, "{stderr}\n{report}");
    assert_eq!(report["summary"]["checked"], 0, "{report}");
    for (check, tool) in [("python", "ruff"), ("go-format", "gofmt")] {
        let coverage = report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == check)
            .unwrap();
        assert_eq!(coverage["status"], "unsupported", "{report}");
        assert!(
            coverage["reason"].as_str().unwrap().contains(tool),
            "{report}"
        );
    }
}

#[test]
fn no_matching_files_do_not_require_tools() {
    let project = Project::new(&[("data.json", "{}")]);
    let report = project.success(
        &[
            "check",
            "--json",
            "--checks",
            "dependency-boundaries,duplication,python,go-format",
        ],
        true,
    );
    assert_eq!(report["checks"].as_array().unwrap().len(), 4, "{report}");
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["status"] == "not-applicable")
    );
}

#[test]
#[ignore = "requires dependency-cruiser, TypeScript, jscpd, Ruff and gofmt on PATH"]
fn declared_checks_use_real_tools_and_respect_selection() {
    let duplicate = "const tokenName = 'test';\nconst otherThing = tokenName + 'now';\nconsole.log(tokenName, otherThing);\nconsole.log(otherThing, tokenName);\nconsole.log(tokenName, otherThing + tokenName);\nconsole.log(otherThing, tokenName + otherThing);\n";
    let python_duplicate = "first = 'hello'\nsecond = first + 'world'\nprint(first, second)\nprint(second, first)\nprint(first, second + first)\nprint(second, first + second)\n";
    let go_duplicate = "package main\n\nfunc greet() {\n\tfirst := \"hello\"\n\tsecond := first + \"world\"\n\tprintln(first, second)\n\tprintln(second, first)\n\tprintln(first, second+first)\n\tprintln(second, first+second)\n}\n";
    let project = Project::new(&[
        (
            "checkignore.json",
            r#"{
            "externalChecks": ["dependency-boundaries", "duplication", "python", "go-format"],
            "dependencyBoundaries": {
                "tsConfig": "tsconfig.json",
                "forbidden": [
                    {"name": "ui-db", "from": "^ui/", "to": "^db/", "severity": "error"},
                    {"name": "ui-private", "from": "^ui/", "to": "^private/", "severity": "error"},
                    {"name": "no-cycle", "circular": true, "severity": "error"}
                ]
            },
            "duplication": {"minLines": 4, "minTokens": 20},
            "pythonRules": ["F821"],
            "ignore": ["ignore/**"],
            "bypasses": [{"files": ["legacy.py"], "rule": "ruff/F821", "reason": "Injected by host."}]
        }"#,
        ),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@db/*":["db/*"]}}}"#,
        ),
        (".dockerignore", "private/\n"),
        (
            "ui/report.ts",
            "import { value } from '@db/value';\nimport '../private/creds';\nconst display = () => value;\nexport { display };\n",
        ),
        ("db/value.ts", "const value = 42;\nexport { value };\n"),
        ("private/creds.ts", "export const secret = 0;\n"),
        ("loop/a.ts", "import './b';\n"),
        ("loop/b.ts", "import './a';\n"),
        ("clone/a.ts", duplicate),
        ("clone/b.ts", duplicate),
        ("clone/a.py", python_duplicate),
        ("clone/b.py", python_duplicate),
        ("clone/a.go", go_duplicate),
        ("clone/b.go", go_duplicate),
        ("ignore/c.ts", duplicate),
        ("legacy.py", "print(injected)\n"),
        ("main.py", "print(missing)\n"),
        (
            "main.go",
            "package main\nfunc main(){ println(\"hello\") }\n",
        ),
    ]);
    let (code, report, stderr) = project.run(&["check", "--json"], false);
    assert_eq!(code, 1, "{stderr}\n{report}");
    for rule in [
        "dependency-cruiser/ui-db",
        "dependency-cruiser/no-cycle",
        "jscpd/duplicate",
        "ruff/F821",
        "gofmt/format",
    ] {
        assert!(
            diagnostic(&report, rule).is_some(),
            "missing {rule}: {report}"
        );
    }
    assert!(
        diagnostic(&report, "dependency-cruiser/ui-private").is_none(),
        "{report}"
    );
    assert_eq!(diagnostic(&report, "ruff/F821").unwrap()["file"], "main.py");
    let clones: Vec<_> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["rule"] == "jscpd/duplicate")
        .map(|finding| finding["file"].as_str().unwrap())
        .collect();
    assert_eq!(clones, ["clone/a.go", "clone/a.py", "clone/a.ts"]);
    assert_eq!(report["bypasses"][0]["finding"]["file"], "legacy.py");
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["status"] != "partial"),
        "{report}"
    );
    for rule in [
        "dependency-boundaries",
        "duplication",
        "python",
        "go-format",
    ] {
        assert!(
            report["checks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["id"] == rule && entry["status"] == "checked"),
            "{report}"
        );
    }
    let scoped = project.success(
        &[
            "check",
            "--json",
            "--checks",
            "dependency-boundaries",
            "--",
            "ui",
        ],
        false,
    );
    assert_eq!(scoped["summary"]["checked"], 1, "{scoped}");
    assert!(
        scoped["diagnostics"].as_array().unwrap().is_empty(),
        "{scoped}"
    );

    let mixed_suffixes = Project::new(&[
        (
            "checkignore.json",
            r#"{"duplication":{"minLines":4,"minTokens":20}}"#,
        ),
        ("first.ts", duplicate),
        ("second.TS", duplicate),
    ]);
    let report = mixed_suffixes.success(&["check", "--json", "--checks", "duplication"], false);
    assert_eq!(report["summary"]["checked"], 2, "{report}");
    assert!(diagnostic(&report, "jscpd/duplicate").is_some(), "{report}");
}
