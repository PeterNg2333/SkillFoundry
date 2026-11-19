mod support;

use serde_json::json;
use support::{Project, diagnostic};

const SINGLE_USE: &str = "maintainable/single-use-helper";
const OBJECT_SPREAD_NEWLINE: &str = "maintainable/object-spread-newline";

#[test]
fn object_spread_newline_is_an_opt_in_warning() {
    let inline = (
        "inline.ts",
        "const reverse = { ...data, id, source, target };",
    );
    let args = ["check", "--json", "--rules", OBJECT_SPREAD_NEWLINE];
    let default = Project::new(&[inline]).success(&args, true);
    assert!(diagnostic(&default, OBJECT_SPREAD_NEWLINE).is_none());

    let enabled = Project::new(&[
        (
            "checkignore.json",
            r#"{"rules":{"maintainable/object-spread-newline":"warn"}}"#,
        ),
        inline,
        (
            "partial.js",
            "const reverse = {\r\n  ...data, id,\r\n  source,\r\n  target,\r\n};",
        ),
        (
            "allowed.ts",
            r#"const separated = {
    ...data,
    id,
    // Keep the source and target together in the data model.
    source,
    target,
};
const compact = { ...data, id };
const plain = { id, source, target, cost };
const fewerFields = { ...first, ...last, id, target };
const { id: key, source: from, target: to, ...rest } = data;"#,
        ),
    ])
    .success(&args, true);

    assert_eq!(enabled["summary"]["warnings"], 2);
    assert_eq!(enabled["summary"]["errors"], 0);
    let findings = enabled["diagnostics"].as_array().unwrap();
    assert_eq!(findings.len(), 2);
    for (finding, file) in findings.iter().zip(["inline.ts", "partial.js"]) {
        assert_eq!(finding["rule"], OBJECT_SPREAD_NEWLINE);
        assert_eq!(finding["severity"], "warning");
        assert_eq!(finding["file"], file);
    }
}

#[test]
fn filters_rules_and_counts_direct_calls_by_binding() {
    let project = Project::new(&[
        (
            "src/one.ts",
            "var state = 0; const helper = () => 1; export const run = () => helper();",
        ),
        (
            "src/two.ts",
            "const helper = () => 1; const run = () => helper() + helper(); export { run };",
        ),
        (
            "src/shadow.ts",
            "const helper = () => 1; const run = () => { const helper = () => 2; return helper() + helper(); }; const value = helper(); export { run, value };",
        ),
        (
            "src/component.tsx",
            "const Component = () => <div />; const call = () => Component(); const render = () => <Component />; export { call, render };",
        ),
        (
            "src/callback.ts",
            "const callback = () => 1; const call = () => callback(); const register = [callback]; export { call, register };",
        ),
        (
            "src/recursive.ts",
            "const walk = (n: number): number => n === 0 ? 0 : walk(n - 1); export const run = () => walk(2);",
        ),
        (
            "src/public.ts",
            "const helper = () => 1; const run = () => helper(); export { helper as publicHelper, run };",
        ),
        (
            "src/framework.cjs",
            "const plugin = () => 1; module.exports = plugin; plugin();",
        ),
        (
            "src/nested.js",
            "const run = () => { const helper = () => 1; return helper(); }; export { run };",
        ),
        (
            "outside.ts",
            "const helper = () => 1; export const run = () => helper();",
        ),
        ("bad.json", "not json"),
    ]);
    let (status, report, stderr) = project.run(
        &["check", "--json", "--rules", SINGLE_USE, "--", "src"],
        true,
    );
    assert_eq!(status, 1, "{stderr}\n{report}");
    assert_eq!(report["ruleFilter"], json!([SINGLE_USE]));
    assert_eq!(report["metrics"], json!([]));
    let findings = report["diagnostics"].as_array().unwrap();
    let actual: Vec<_> = findings
        .iter()
        .map(|finding| (&finding["file"], &finding["rule"]))
        .collect();
    assert_eq!(
        actual,
        [
            (&json!("src/nested.js"), &json!(SINGLE_USE)),
            (&json!("src/one.ts"), &json!(SINGLE_USE)),
            (&json!("src/shadow.ts"), &json!(SINGLE_USE))
        ]
    );
    let capabilities = project.success(&["capabilities", "--json"], true);
    assert!(
        capabilities["rules"]
            .as_array()
            .unwrap()
            .contains(&json!(SINGLE_USE))
    );
    let (status, report, stderr) = project.run(&["check", "--json", "src/one.ts"], true);
    assert_eq!(status, 1, "{stderr}\n{report}");
    assert!(diagnostic(&report, SINGLE_USE).is_some());
    assert!(diagnostic(&report, "no-var").is_some());
    for args in [
        vec!["check", "--rules", "unknown"],
        vec!["check", "--rules", SINGLE_USE, "--checks", "json"],
    ] {
        assert_eq!(project.run(&args, true).0, 2);
    }
}

#[test]
fn single_use_helpers_preserve_inline_exports_and_check_nested_private_bindings() {
    let project = Project::new(&[
        (
            "api.ts",
            r#"export function shared() { return 1; }
export const arrow = () => 2;
export default function run() {
    const nested = () => 3;
    return shared() + arrow() + nested();
}
run();"#,
        ),
        (
            "consumer.ts",
            "import run, { shared, arrow } from './api'; console.log(run(), shared(), arrow());",
        ),
    ]);
    let (status, report, stderr) = project.run(&["check", "--json", "--rules", SINGLE_USE], true);
    assert_eq!(status, 1, "{stderr}\n{report}");
    let findings = report["diagnostics"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{report}");
    assert_eq!(findings[0]["rule"], SINGLE_USE);
    assert!(
        findings[0]["message"]
            .as_str()
            .unwrap()
            .starts_with("nested ")
    );
}

#[test]
fn bypasses_only_the_attached_rule_and_checks_for_unused_bypasses() {
    let project = Project::new(&[
        (
            "suppressed.ts",
            "/* @bypass maintainable/single-use-helper -- Keep the retry boundary. */\nconst retry = () => { let value = 1; return value; };\nconst other = () => 2;\nexport const run = () => retry() + other();",
        ),
        (
            "stale.ts",
            "// @bypass maintainable/single-use-helper -- Keep the retry boundary.\nconst retry = () => 1;\nexport const run = () => retry() + retry();",
        ),
        (
            "invalid.ts",
            "// @bypass maintainable/single-use-helper --\nconst retry = () => 1;\nconst run = () => retry(); export { run };",
        ),
        (
            "off.ts",
            "// @bypass maintainable/single-use-helper -- Keep the retry boundary.\nconst retry = () => 1;\nconst run = () => retry(); export { run };",
        ),
        (
            "checkignore.json",
            r#"{"overrides":[{"files":["off.ts"],"rules":{"maintainable/single-use-helper":"off"}}]}"#,
        ),
    ]);
    let (status, report, stderr) = project.run(&["check", "--json", "--rules", SINGLE_USE], true);
    assert_eq!(status, 1, "{stderr}\n{report}");
    let bypasses = report["bypasses"].as_array().unwrap();
    assert_eq!(bypasses.len(), 1);
    assert_eq!(bypasses[0]["finding"]["line"], 2);
    assert_eq!(bypasses[0]["reason"], "Keep the retry boundary.");
    assert_eq!(
        diagnostic(&report, "maintainable/unused-bypass").unwrap()["file"],
        "stale.ts"
    );
    assert_eq!(
        diagnostic(&report, "maintainable/invalid-bypass").unwrap()["file"],
        "invalid.ts"
    );
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["file"] == "suppressed.ts"
                && finding["line"] == 3
                && finding["rule"] == SINGLE_USE)
    );
    let other_rule = project.success(
        &["check", "--json", "--rules", "prefer-const", "stale.ts"],
        true,
    );
    assert_eq!(other_rule["diagnostics"], json!([]));
    let combined_rules = format!("{SINGLE_USE},prefer-const");
    let (status, report, stderr) = project.run(
        &[
            "check",
            "--json",
            "--rules",
            &combined_rules,
            "suppressed.ts",
        ],
        true,
    );
    assert_eq!(status, 1, "{stderr}\n{report}");
    assert_eq!(diagnostic(&report, "prefer-const").unwrap()["line"], 2);
    assert_eq!(report["bypasses"].as_array().unwrap().len(), 1);
}

#[test]
fn lists_only_real_comment_directives_in_selected_files() {
    let project = Project::new(&[
        (
            "src/route.ts",
            "const sample = '// @impl Ignore this string';\n// @impl Validate the body.\n// @check Return 400 for a missing ID.\n/**\n * @refactor Keep the response contract.\n */\nvar state = 0;",
        ),
        ("outside.ts", "// @impl Outside the selected directory."),
    ]);
    let report = project.success(&["check", "--json", "--checks", "directives", "src"], true);
    let directives = report["directives"].as_array().unwrap();
    let actual: Vec<_> = directives
        .iter()
        .map(|directive| (&directive["kind"], &directive["line"], &directive["text"]))
        .collect();
    assert_eq!(
        actual,
        [
            (&json!("impl"), &json!(2), &json!("Validate the body.")),
            (
                &json!("check"),
                &json!(3),
                &json!("Return 400 for a missing ID.")
            ),
            (
                &json!("refactor"),
                &json!(5),
                &json!("Keep the response contract.")
            )
        ]
    );
    assert_eq!(report["diagnostics"], json!([]));
    assert_eq!(report["checks"][0]["files"], 1);
}

#[test]
fn matches_moby_docker_reference_patterns() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/docker-ignore.json")).unwrap();
    let files: Vec<(&str, &str)> = fixture["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| (file.as_str().unwrap(), "invalid !!!"))
        .collect();
    for case in fixture["cases"].as_array().unwrap() {
        let mut files = files.clone();
        files.push((".dockerignore", case["pattern"].as_str().unwrap()));
        let project = Project::new(&files);
        let report = project.success(&["select", "--json"], true);
        let actual: Vec<_> = report["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["path"].clone())
            .collect();
        assert_eq!(
            serde_json::json!(actual),
            case["expected"],
            "pattern {}",
            case["pattern"]
        );
    }
}

#[test]
fn checks_js_ts_jsx_decorators_overloads_and_strict_json_without_node() {
    let project = Project::new(&[
        (
            "value.ts",
            "type Label = string; class Model { label(): Label { return 'ok'; } } const value = new Model().label(); export { Model, value }; export type { Label };",
        ),
        (
            "Page.tsx",
            "const Page = () => <p>ok</p>; export default Page;",
        ),
        (
            "Page.jsx",
            "const Page = () => <p>ok</p>; export default Page;",
        ),
        ("module.cjs", "const value = 1; module.exports = { value };"),
        (
            "di.ts",
            "import { Injectable } from '@nestjs/common'; import { Service } from '@/Service'; @Injectable() class Model { constructor(private readonly service: Service) {} } export { Model };",
        ),
        (
            "overload.ts",
            "function choose(value: string): string; function choose(value: number): number; function choose(value: string | number) { return value; } export { choose };",
        ),
        ("settings.json", "\u{feff}{\"large\":1e400}"),
    ]);
    let report = project.success(&["check", "--json"], true);
    assert_eq!(report["summary"]["checked"], 7);
    assert_eq!(report["diagnostics"], json!([]));
}

#[test]
fn reports_keywords_exports_syntax_aliases_and_type_only_imports() {
    let project = Project::new(&[
        ("style.ts", "export function choose() { return 1; }"),
        (
            "exports.ts",
            "const a = 1; export { a }; const b = 2; export { b };",
        ),
        (
            "function.ts",
            "const choose = function () { return 1; }; export { choose };",
        ),
        ("binding.ts", "let choose = () => 1; export { choose };"),
        ("syntax.js", "const value: number = 1;"),
        ("syntax.ts", "const = ;"),
        ("broken.json", "\u{feff}{\n\"enabled\":\n}"),
        ("trailing.json", "{\"enabled\":true,}"),
        (
            "source.ts",
            "import { Label } from '@/model'; const label: Label = 'ok'; export { label };",
        ),
        ("parent.ts", "export type { Label } from '../model';"),
        (
            "dynamic.ts",
            "const load = () => import('../model'); export { load };",
        ),
    ]);
    let (status, report, stderr) = project.run(&["check", "--json"], false);
    assert_eq!(status, 1, "{stderr}");
    for rule in [
        "func-style",
        "no-restricted-syntax",
        "syntax",
        "json/syntax",
        "maintainable/cross-directory-imports",
        "@typescript-eslint/consistent-type-imports",
    ] {
        assert!(
            diagnostic(&report, rule).is_some(),
            "Missing {rule}: {report}"
        );
    }
    let json_error = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["file"] == "broken.json")
        .unwrap();
    assert_eq!(json_error["line"], 3);
    assert_eq!(json_error["column"], 1);
}

#[test]
fn preserves_semantic_prefer_const_and_variable_shadowing() {
    let project = Project::new(&[
        ("immutable.ts", "let value = 1; console.log(value);"),
        (
            "mutable.ts",
            "let value = 1; value += 1; console.log(value);",
        ),
        (
            "shadow.ts",
            "let value = 1; const change = () => { let value = 2; value++; return value; }; console.log(value, change());",
        ),
    ]);
    let (_, report, _) = project.run(&["check", "--json"], false);
    let files: Vec<_> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["rule"] == "prefer-const")
        .map(|finding| finding["file"].as_str().unwrap())
        .collect();
    assert_eq!(files, ["immutable.ts", "shadow.ts"]);
}

#[test]
fn combines_git_docker_and_config_ignores_before_parsing() {
    let project = Project::new(&[
        (
            ".gitignore",
            "blocked/\n*.secret.ts\n!keep.secret.ts\n/only-root.ts\n",
        ),
        (
            ".dockerignore",
            "excluded/\n!excluded/keep.ts\n**/temp*.ts\n",
        ),
        (
            "checkignore.json",
            "{\"ignore\":[\"skipped/**\"],\"extensions\":{\".generated.ts\":false,\".source\":\"typescript\"}}",
        ),
        ("blocked/.gitignore", "!bad.ts\n"),
        ("blocked/bad.ts", "invalid !!!"),
        ("bad.secret.ts", "invalid !!!"),
        ("keep.secret.ts", "const value = 1; export { value };"),
        ("only-root.ts", "invalid !!!"),
        ("nested/only-root.ts", "const value = 1; export { value };"),
        ("excluded/bad.ts", "invalid !!!"),
        ("excluded/keep.ts", "const value = 1; export { value };"),
        ("nested/temp.ts", "invalid !!!"),
        ("skipped/bad.ts", "invalid !!!"),
        ("node_modules/bad.ts", "invalid !!!"),
        ("view.generated.ts", "invalid !!!"),
        ("custom.source", "const value = 1; export { value };"),
    ]);
    let report = project.success(&["check", "--json"], false);
    assert_eq!(report["summary"]["checked"], 5);
    for reason in ["gitignore", "dockerignore", "ignore", "disabled-extension"] {
        assert!(
            report["skipped"]
                .as_array()
                .unwrap()
                .iter()
                .any(|skip| skip["reason"] == reason),
            "{report}"
        );
    }
}

#[test]
fn respects_nested_git_scopes_and_explicit_paths() {
    let project = Project::new(&[
        (".gitignore", "*.secret.ts\n"),
        ("src/.gitignore", "*.ts\n!keep.ts\n!keep.secret.ts\n"),
        ("src/hidden.ts", "invalid !!!"),
        ("src/keep.ts", "const keep = 1;"),
        ("src/keep.secret.ts", "const keep = 1;"),
        ("other/hidden.ts", "const keep = 1;"),
        ("other/hidden.secret.ts", "invalid !!!"),
    ]);
    let report = project.success(&["select", "--json"], false);
    let files: Vec<_> = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        ["other/hidden.ts", "src/keep.secret.ts", "src/keep.ts"]
    );
    let report = project.success(&["select", "--json", "src/keep.ts"], false);
    assert_eq!(report["summary"]["selected"], 1);
    assert_eq!(report["summary"]["checked"], 0);
    assert_eq!(
        project.run(&["select", "--json", "../outside.ts"], false).0,
        2
    );
}

#[test]
fn preserves_rule_overrides_and_warning_thresholds() {
    let branches = (0..10)
        .map(|index| format!("if (value === {index}) return {index};"))
        .collect::<Vec<_>>();
    let complex = format!(
        "const choose = (value: number) => {{ {} return -1; }}; export {{ choose }};",
        branches.join("\n")
    );
    let boundary = format!(
        "const choose = (value: number) => {{ {} return -1; }}; export {{ choose }};",
        branches[..9].join("\n")
    );
    let long = "console.info('review');\n".repeat(301);
    let project = Project::new(&[
        (
            "checkignore.json",
            "{\"overrides\":[{\"files\":[\"fixture.js\"],\"rules\":{\"func-style\":\"off\"}}]}",
        ),
        (
            "fixture.js",
            "function choose() { return 1; } export { choose };",
        ),
        (
            "nested.ts",
            "const choose = (a: boolean, b: boolean, c: boolean) => { if (a) { if (b) { if (c) return 1; } } return 0; }; export { choose };",
        ),
        ("complex.ts", &complex),
        ("boundary.ts", &boundary),
        ("long.mjs", &long),
    ]);
    let report = project.success(&["check", "--json"], false);
    for rule in ["max-depth", "complexity", "max-lines"] {
        assert_eq!(diagnostic(&report, rule).unwrap()["severity"], "warning");
    }
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|finding| finding["file"] != "boundary.ts" && finding["file"] != "fixture.js")
    );
}

#[test]
fn native_checks_report_dependencies_comments_and_partial_coverage() {
    let project = Project::new(&[
        (
            "package.json",
            "{\"dependencies\":{\"a\":\"1\",\"b\":\"1\"},\"devDependencies\":{\"a\":\"1\",\"c\":\"1\"}}",
        ),
        (
            "go.mod",
            "module example.com/sample\n\ngo 1.26\nrequire (\n example.com/direct v1.0.0\n example.com/indirect v1.0.0 // indirect\n)\n",
        ),
        (
            "sample.go",
            "package sample\nimport \"fmt\"\n// Explains the side effect.\nfunc Print() { fmt.Println(\"// string\") }\n",
        ),
        ("value.ts", "const value = 1; export { value };"),
    ]);
    let report = project.success(&["check", "--checks", "dependencies,json", "--json"], true);
    assert_eq!(report["summary"]["checked"], 2);
    assert_eq!(report["summary"]["unchecked"], 2);
    assert!(
        report["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|metric| metric["name"] == "uniqueDeclaredDependencies" && metric["value"] == 3)
    );
    let report = project.success(&["check", "--checks", "go,comments", "--json"], true);
    for (name, value) in [
        ("imports", 1),
        ("functionDeclarations", 1),
        ("commentGroups", 1),
    ] {
        assert!(
            report["metrics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|metric| metric["name"] == name && metric["value"] == value),
            "{report}"
        );
    }
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["id"] == "go-format" && check["status"] == "unsupported")
    );
}

#[test]
fn rejects_unknown_configuration_rules_checks_and_invalid_json() {
    for settings in [
        "{",
        "{\"extensions\":{\".py\":\"python\"}}",
        "{\"rules\":{\"misspelled-rule\":\"off\"}}",
        "{\"limits\":{\"importsPerFile\":-1}}",
    ] {
        let project = Project::new(&[("checkignore.json", settings)]);
        let (status, _, stderr) = project.run(&["check", "--json"], false);
        assert_eq!(status, 2, "{settings}: {stderr}");
        assert!(stderr.contains("configuration"), "{stderr}");
    }
    let project = Project::new(&[("value.ts", "const value = 1;")]);
    assert_eq!(
        project
            .run(&["check", "--checks", "llm", "--json"], false)
            .0,
        2
    );
    assert_eq!(
        project
            .run(&["check", "--checks", "json,json", "--json"], false)
            .0,
        2
    );
    let capabilities = project.success(&["capabilities", "--json"], true);
    assert_eq!(capabilities["engine"], "rust");
}

#[test]
fn partial_comment_checks_do_not_run_style_rules_and_semantic_syntax_is_enforced() {
    let project = Project::new(&[(
        "sample.ts",
        "// Explains the public operation.\nlet choose = function () { return 1; }; export { choose };",
    )]);
    let report = project.success(&["check", "--checks", "comments", "--json"], true);
    assert_eq!(report["diagnostics"], json!([]));
    let project = Project::new(&[("duplicate.ts", "const value = 1; const value = 2;")]);
    let (status, report, _) = project.run(&["check", "--json"], false);
    assert_eq!(status, 1);
    assert!(diagnostic(&report, "syntax").is_some());
}

#[test]
fn overload_exceptions_are_scoped_and_relative_alias_checks_normalize_paths() {
    let project = Project::new(&[
        (
            "overload.ts",
            "function choose(value: string): string; function choose(value: string) { return value; } const outer = () => { function choose() { return 1; } return choose(); }; export { choose, outer };",
        ),
        ("src/same.ts", "import './nested/../peer';"),
        ("src/peer.ts", "const peer = 1; export { peer };"),
        (
            "src/nested/index.ts",
            "const nested = 1; export { nested };",
        ),
        ("src/child.ts", "import './nested';"),
    ]);
    let (status, report, stderr) = project.run(&["check", "--json"], false);
    assert_eq!(status, 1, "{stderr}");
    let findings = report["diagnostics"].as_array().unwrap();
    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding["rule"] == "func-style")
            .count(),
        1,
        "{report}"
    );
    assert!(
        findings
            .iter()
            .all(|finding| finding["file"] != "src/same.ts"),
        "{report}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding["file"] == "src/child.ts")
    );
}

#[test]
fn cargo_counts_include_target_specific_dependencies_and_deduplicate_names() {
    let project = Project::new(&[(
        "Cargo.toml",
        "[package]\nname='sample'\nversion='0.1.0'\n[dependencies]\na='1'\n[dev-dependencies]\na='1'\n[target.'cfg(windows)'.dependencies]\nb='1'\n",
    )]);
    let report = project.success(&["check", "--checks", "dependencies,toml", "--json"], true);
    assert!(
        report["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|metric| metric["name"] == "uniqueDeclaredDependencies" && metric["value"] == 2),
        "{report}"
    );
}

#[test]
fn else_if_chains_do_not_inflate_nesting_depth() {
    let project = Project::new(&[(
        "chain.ts",
        "const choose = (value: number) => { if (value === 1) return 1; else if (value === 2) return 2; else if (value === 3) return 3; return 0; }; export { choose };",
    )]);
    let report = project.success(&["check", "--json"], false);
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|finding| finding["rule"] != "max-depth"),
        "{report}"
    );
}

#[test]
fn malformed_cargo_dependency_groups_are_findings() {
    let project = Project::new(&[(
        "Cargo.toml",
        "dependencies='invalid'\n[package]\nname='sample'\nversion='0.1.0'\n",
    )]);
    let (status, report, _) = project.run(&["check", "--checks", "dependencies", "--json"], false);
    assert_eq!(status, 1, "{report}");
    assert!(diagnostic(&report, "dependencies/manifest").is_some());
}
