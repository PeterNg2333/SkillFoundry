mod support;

use support::{Project, diagnostic};

const RULE: &str = "maintainable/similar-loops";
const ENABLED: &str = r#"{"rules":{"maintainable/similar-loops":"warn"}}"#;
const FIRST: &str = r#"for (const row of rows) {
    if (row.active) ids.push(normalizeId(row.id));
}"#;
const SECOND: &str = r#"for (const item of incoming) {
    if (item.active) ids.push(normalizeId(item.id));
}"#;
const DIFFERENT: &str = r#"for (const row of rows) {
    if (row.expired) await archive(row.id);
}"#;
const ARGS: &[&str] = &["check", "--json", "--rules", RULE];

#[test]
fn compares_renamed_bindings_across_selected_files_and_describes_import_edges() {
    let files = [
        ("import/collect.ts", FIRST),
        ("sync/collect.js", SECOND),
        ("sync/archive.ts", DIFFERENT),
        (
            "app.ts",
            "import './import/collect.js'; import('./sync/collect.js');",
        ),
        (".gitignore", "ignored.ts\n"),
        ("ignored.ts", SECOND),
        ("broken.ts", "const value: = 1;"),
    ];
    let default = Project::new(&files).run(&["check", "--json"], true).1;
    assert!(diagnostic(&default, RULE).is_none());

    let project = Project::new(&[&files[..], &[("checkignore.json", ENABLED)]].concat());
    let (status, report, _) = project.run(ARGS, true);
    assert_eq!(status, 1, "{report}");
    assert!(diagnostic(&report, "syntax").is_some());
    assert_eq!(report["summary"]["warnings"], 1, "{report}");
    let finding = diagnostic(&report, RULE).unwrap();
    assert_eq!(finding["file"], "import/collect.ts");
    assert_eq!(finding["related"]["file"], "sync/collect.js");
    assert!(
        finding["message"]
            .as_str()
            .unwrap()
            .contains("import/collect.ts <- app.ts -> sync/collect.js")
    );

    let args = [ARGS, &["--", "import/collect.ts", "sync/collect.js"]].concat();
    let scoped = project.success(&args, true);
    assert!(diagnostic(&scoped, RULE).is_some(), "{scoped}");
    let args = [ARGS, &["--", "sync"]].concat();
    assert!(diagnostic(&project.success(&args, true), RULE).is_none());
    let args = [ARGS, &["--checks", "typescript", "--", "import", "sync"]].concat();
    let partial = project.success(&args, true);
    assert!(diagnostic(&partial, RULE).is_none(), "{partial}");
    assert_eq!(partial["summary"]["unchecked"], 1, "{partial}");
}

#[test]
fn records_exceptions_on_either_loop_and_honors_scope_and_thresholds() {
    let commented = format!(
        "// @bypass {RULE} -- Different owners release these loops independently.\n{SECOND}"
    );
    let files = [("a.ts", FIRST), ("z.ts", &commented)];
    let project = Project::new(&[&files[..], &[("checkignore.json", ENABLED)]].concat());
    let report = project.success(ARGS, true);
    assert!(diagnostic(&report, RULE).is_none(), "{report}");
    assert_eq!(report["bypasses"][0]["finding"]["file"], "z.ts");
    assert_eq!(report["bypasses"][0]["directiveLine"], 1);

    let policy = r#"{"rules":{"maintainable/similar-loops":"warn"},"loopSimilarity":{"minSimilarity":60,"minBodyTokens":8},"bypasses":[{"rule":"maintainable/similar-loops","files":["z.ts"],"reason":"The modules have independent owners."}]}"#;
    let project = Project::new(&[
        ("a.ts", FIRST),
        ("z.ts", SECOND),
        ("checkignore.json", policy),
    ]);
    let report = project.success(ARGS, true);
    assert!(diagnostic(&report, RULE).is_none(), "{report}");
    assert_eq!(
        report["bypasses"][0]["reason"],
        "The modules have independent owners."
    );

    let policy =
        r#"{"rules":{"maintainable/similar-loops":"warn"},"loopSimilarity":{"minBodyTokens":100}}"#;
    let project = Project::new(&[
        ("a.ts", FIRST),
        ("z.ts", SECOND),
        ("checkignore.json", policy),
    ]);
    assert!(diagnostic(&project.success(ARGS, true), RULE).is_none());
}

#[test]
fn accepts_near_matches_across_loop_forms_and_can_require_exact_bodies() {
    let while_loop = "while (hasRows()) { const row = nextRow(); if (row.active) ids.push(normalizeId(row.id)); ids.push(normalizeId(row.parentId)); }";
    let for_loop = "for (; hasRows();) { const entry = nextRow(); if (entry.active) ids.push(normalizeId(entry.id)); ids.push(normalizeId(entry.ownerId)); }";
    let project = Project::new(&[
        ("a.ts", while_loop),
        ("b.ts", for_loop),
        ("checkignore.json", ENABLED),
    ]);
    let report = project.success(ARGS, true);
    let message = diagnostic(&report, RULE).unwrap()["message"]
        .as_str()
        .unwrap();
    assert!(!message.contains("100%"), "{message}");

    let strict =
        r#"{"rules":{"maintainable/similar-loops":"warn"},"loopSimilarity":{"minSimilarity":100}}"#;
    let project = Project::new(&[
        ("a.ts", while_loop),
        ("b.ts", for_loop),
        ("checkignore.json", strict),
    ]);
    assert!(diagnostic(&project.success(ARGS, true), RULE).is_none());
}

#[test]
fn audits_unused_inline_bypasses_and_validates_loop_policy() {
    let commented = format!("// @bypass {RULE} -- These modules have independent owners.\n{FIRST}");
    let project = Project::new(&[
        ("a.ts", &commented),
        ("b.ts", DIFFERENT),
        ("checkignore.json", ENABLED),
    ]);
    let (status, report, _) = project.run(ARGS, true);
    assert_eq!(status, 1, "{report}");
    assert_eq!(
        diagnostic(&report, "maintainable/unused-bypass").unwrap()["line"],
        1
    );
    let filtered = project.success(&["check", "--json", "--rules", "prefer-const"], true);
    assert!(diagnostic(&filtered, "maintainable/unused-bypass").is_none());

    for (field, value) in [("minSimilarity", 101), ("minBodyTokens", 2)] {
        let policy = format!(r#"{{"loopSimilarity":{{"{field}":{value}}}}}"#);
        let (status, _, error) = Project::new(&[("checkignore.json", &policy)]).run(ARGS, true);
        assert_eq!(status, 2, "{error}");
        assert!(error.contains(field), "{error}");
    }
}
