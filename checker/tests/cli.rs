mod support;

use serde_json::json;
use support::{Project, diagnostic};

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
