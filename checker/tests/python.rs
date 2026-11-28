mod support;

use serde_json::Value;
use support::{Project, diagnostic};

const SINGLE_USE: &str = "maintainable/single-use-helper";

fn findings<'a>(report: &'a Value, rule: &str) -> Vec<(&'a str, u64)> {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["rule"] == rule)
        .map(|finding| {
            (
                finding["file"].as_str().unwrap(),
                finding["line"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn coverage<'a>(report: &'a Value, check: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == check)
        .unwrap()
}

#[test]
fn python_is_checked_natively_and_reports_syntax_errors() {
    let project = Project::new(&[
        (
            "app/valid.py",
            "def total(values):\n    return sum(values)\n",
        ),
        ("app/invalid.py", "def broken(:\n    pass\n"),
        ("app/legacy.py", "print \"hello\"\n"),
        ("app/stub.pyi", "def total(values: list[int]) -> int: ...\n"),
    ]);
    let (status, report, stderr) = project.run(&["check", "--json"], true);
    assert_eq!(status, 1, "{stderr}\n{report}");
    assert_eq!(coverage(&report, "python")["status"], "checked");
    assert_eq!(coverage(&report, "python")["files"], 4);
    assert_eq!(report["summary"]["unchecked"], 0, "{report}");
    assert_eq!(
        findings(&report, "syntax"),
        [("app/invalid.py", 1), ("app/legacy.py", 1)]
    );
    assert!(
        diagnostic(&report, "syntax").unwrap()["message"]
            .as_str()
            .unwrap()
            .contains("Python")
    );
}

#[test]
fn python_single_use_helpers_follow_python_scopes() {
    let project = Project::new(&[(
        "scopes.py",
        r#"from functools import cache

__all__ = ["_exported"]


def _format(value):
    return str(value)


def render(values):
    _format = repr
    return [_format(item) for item in values] + [_format(values)]


print(_format(1))


def _label():
    return "module"


class View:
    _label = "class attribute"

    def show(self):
        return _label()


def _default():
    return 1


def configure(value=_default()):
    return value


def _reset():
    return 0


def rebind():
    global _reset
    _reset = lambda: 1


@cache
def _cached():
    return 1


def _items():
    yield 1


def _exported():
    return 1


def _walk(depth):
    return _walk(depth - 1) if depth else 0


def _handler():
    return 1


def _callback():
    return 1


def _twice():
    return 1


print(_cached(), list(_items()), _exported(), _walk(2), _handler())
handlers = [_handler, _callback]
_twice()
_twice()


def outer():
    def inner():
        return 1

    def shared():
        return 2

    return inner() + shared() + shared()


async def _fetch():
    return 1


async def main():
    return await _fetch()
"#,
    )]);
    let report = project
        .run(&["check", "--json", "--rules", SINGLE_USE], true)
        .1;
    let names: Vec<_> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["rule"] == SINGLE_USE)
        .map(|finding| {
            finding["message"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
        })
        .collect();
    assert_eq!(
        names,
        ["_format", "_label", "_default", "inner", "_fetch"],
        "{report}"
    );
}

#[test]
fn python_bypasses_and_work_directives_use_hash_comments() {
    let project = Project::new(&[(
        "orders.py",
        r#"# @impl Validate the order before saving.
# @bypass maintainable/single-use-helper -- Keeps the retry policy together.
def _retry():
    return 1


def _other():
    return 2


def outer():
    # @bypass maintainable/single-use-helper -- The inner function owns cleanup.
    def inner():
        return 1

    # @bypass maintainable/single-use-helper -- Nothing here is reported.
    value = _retry() + _other() + inner()
    return value


count = 1  # @bypass max-depth -- trailing comments do not attach to a statement.
# @bypass prefer-const -- Python has no const declarations.
total = count
"#,
    )]);
    let (status, report, stderr) = project.run(&["check", "--json", "--rules", SINGLE_USE], true);
    assert_eq!(status, 1, "{stderr}\n{report}");
    assert_eq!(
        findings(&report, SINGLE_USE),
        [("orders.py", 7)],
        "{report}"
    );
    let bypassed: Vec<_> = report["bypasses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|bypass| {
            (
                bypass["finding"]["line"].as_u64().unwrap(),
                bypass["directiveLine"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(bypassed, [(3, 2), (13, 12)], "{report}");
    assert_eq!(
        findings(&report, "maintainable/unused-bypass"),
        [("orders.py", 16)]
    );
    assert_eq!(
        findings(&report, "maintainable/invalid-bypass"),
        [("orders.py", 21), ("orders.py", 22)]
    );

    let work = project.success(&["check", "--json", "--checks", "directives"], true);
    let directives = work["directives"].as_array().unwrap();
    assert_eq!(directives.len(), 1, "{work}");
    assert_eq!(directives[0]["kind"], "impl");
    assert_eq!(directives[0]["line"], 1);
    assert_eq!(directives[0]["column"], 3);
}

#[test]
fn python_comment_groups_use_hash_comments() {
    let project = Project::new(&[
        ("checkignore.json", r#"{"limits":{"requireComments":true}}"#),
        (
            "documented.py",
            "# Totals.\n# Two lines, one group.\nx = 1\n\n# Second group.\ny = 2\n",
        ),
        ("bare.py", "x = 1\n"),
    ]);
    let report = project.success(&["check", "--json", "--checks", "comments"], true);
    assert_eq!(findings(&report, "comments/presence"), [("bare.py", 1)]);
    let groups: Vec<_> = report["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|metric| metric["name"] == "commentGroups")
        .map(|metric| {
            (
                metric["file"].as_str().unwrap(),
                metric["value"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(groups, [("bare.py", 0), ("documented.py", 2)]);
}

#[test]
fn python_similar_loops_report_absolute_import_paths_below_a_source_root() {
    let body = "    total = 0\n    for order in orders:\n        price = order.price * order.quantity\n        total += price - order.discount\n    return total\n";
    let project = Project::new(&[
        (
            "checkignore.json",
            r#"{"rules":{"maintainable/similar-loops":"warn"}}"#,
        ),
        (
            "src/shop/orders.py",
            &format!("def totals(orders):\n{body}"),
        ),
        (
            "src/shop/reports.py",
            &format!("from shop.orders import totals\n\n\ndef sums(orders):\n{body}"),
        ),
        (
            "web/orders.js",
            "const totals = (orders) => {\n  let total = 0;\n  for (const order of orders) {\n    const price = order.price * order.quantity;\n    total += price - order.discount;\n  }\n  return total;\n};\nexport { totals };\n",
        ),
    ]);
    let report = project.success(
        &["check", "--json", "--rules", "maintainable/similar-loops"],
        true,
    );
    let loops = report["diagnostics"].as_array().unwrap();
    assert_eq!(loops.len(), 1, "{report}");
    assert_eq!(loops[0]["file"], "src/shop/orders.py");
    assert_eq!(loops[0]["related"]["file"], "src/shop/reports.py");
    assert!(
        loops[0]["message"]
            .as_str()
            .unwrap()
            .ends_with("Import path: src/shop/orders.py <- src/shop/reports.py."),
        "{report}"
    );
}

#[test]
fn rules_must_name_a_selected_language_and_ruff_moves_to_python_lint() {
    let project = Project::new(&[("main.py", "x = 1\n")]);
    let (status, _, stderr) = project.run(
        &[
            "check",
            "--json",
            "--checks",
            "python",
            "--rules",
            "prefer-const",
        ],
        true,
    );
    assert_eq!(status, 2);
    assert!(stderr.contains("javascript, typescript"), "{stderr}");
    project.success(
        &[
            "check", "--json", "--checks", "python", "--rules", SINGLE_USE,
        ],
        true,
    );

    let legacy = Project::new(&[
        ("checkignore.json", r#"{"externalChecks":["python"]}"#),
        ("main.py", "x = 1\n"),
    ]);
    let (status, _, stderr) = legacy.run(&["check", "--json"], true);
    assert_eq!(status, 2);
    assert!(stderr.contains("python-lint"), "{stderr}");

    let capabilities = project.success(&["capabilities", "--json"], true);
    assert_eq!(
        capabilities["ruleLanguages"]["prefer-const"],
        serde_json::json!(["javascript", "typescript"])
    );
    assert_eq!(
        capabilities["ruleLanguages"][SINGLE_USE],
        serde_json::json!(["javascript", "typescript", "python"])
    );
    assert_eq!(capabilities["externalChecks"]["python-lint"], "ruff");
}
