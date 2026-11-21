mod support;

use support::{Project, diagnostic};

const CLASS_NAME: &str = "maintainable/long-jsx-class-name";
const STYLE_ASSERTION: &str = "maintainable/playwright-style-assertion";
const MAPPED_BUTTON: &str = "maintainable/complex-mapped-button";

const ENABLED: &str = r#"{"rules":{
    "maintainable/long-jsx-class-name":"warn",
    "maintainable/playwright-style-assertion":"warn",
    "maintainable/complex-mapped-button":"warn"
}}"#;

#[test]
fn class_name_warns_on_long_static_fragments_and_allows_grouped_lines() {
    let long = "a b c d e f g h i j k l m n o p q r s t";
    let source = format!(
        r#"const a = <button className="{long}" />;
const b = <button className={{`{long} ${{pressed ? "pressed" : ""}}`}} />;
const c = <button className={{cn("short", pressed && "{long}")}} />;
const d = <button className={{cn(
  "a b c d e f g h i j",
  "k l m n o p q r s t",
)}} />;
const e = <button className="a b c d e f g h i j k l m n o p q r s" />;
const f = <button className={{chooseLabel("{long}")}} />;"#
    );
    let args = ["check", "--json", "--rules", CLASS_NAME];
    let default = Project::new(&[("buttons.tsx", &source)]).success(&args, true);
    assert!(diagnostic(&default, CLASS_NAME).is_none());

    let report = Project::new(&[("checkignore.json", ENABLED), ("buttons.tsx", &source)])
        .success(&args, true);
    let findings = report["diagnostics"].as_array().unwrap();
    assert_eq!(findings.len(), 3);
    for (index, finding) in findings.iter().enumerate() {
        assert_eq!(finding["rule"], CLASS_NAME);
        assert_eq!(finding["severity"], "warning");
        assert_eq!(finding["line"], index + 1);
    }
}

#[test]
fn playwright_style_assertions_warn_in_behavior_specs_only() {
    let source = r#"import { expect, test } from "@playwright/test";
test("opens the floor", async ({ page }) => {
  const hint = page.getByRole("button");
  await expect(hint).not.toHaveCSS("transition-duration", "0.18s");
  await expect.poll(() => hint.evaluate(node => getComputedStyle(node).clipPath)).toMatch(/shape/);
  const before = (await hint.boundingBox())!;
  expect(before.height).toBe(42);
  expect(await hint.boundingBox()).toEqual(before);
  await expect.poll(() => hint.evaluate(node => node.scrollWidth - node.clientWidth)).toBe(0);
  const target = await hint.boundingBox();
  await page.mouse.click(target!.x, target!.y);
  await expect(hint).toHaveAttribute("aria-pressed", "true");
  await expect(hint).toBeInViewport();
});"#;
    let args = ["check", "--json", "--rules", STYLE_ASSERTION];
    let default = Project::new(&[("flow.spec.ts", source)]).success(&args, true);
    assert_eq!(default["summary"]["warnings"], 0);

    let report = Project::new(&[
        ("checkignore.json", ENABLED),
        ("e2e/flow.spec.ts", source),
        ("e2e/map.visual.spec.ts", source),
        ("e2e/visual/map.spec.ts", source),
        ("e2e/helpers.ts", source),
        ("unit.spec.ts", "expect(hint).toHaveCSS('color', 'red');"),
    ])
    .success(&args, true);
    let findings = report["diagnostics"].as_array().unwrap();
    assert_eq!(findings.len(), 5);
    for (finding, line) in findings.iter().zip([4, 5, 6, 8, 9]) {
        assert_eq!(finding["rule"], STYLE_ASSERTION);
        assert_eq!(finding["severity"], "warning");
        assert_eq!(finding["file"], "e2e/flow.spec.ts");
        assert_eq!(finding["line"], line);
    }
}

#[test]
fn complex_mapped_buttons_warn_without_flagging_simple_markup() {
    let source = r#"const hints = rows.map(item => <button onClick={() => open(item.id)} aria-pressed={item.active} className={cn("base", item.active && "selected")}><span>{item.label}</span></button>);
const levels = rows.map(function (item) { return <div><button onClick={() => open(item.id)} aria-current={item.active ? "page" : undefined} className={item.active ? "active" : "normal"}>{item.label}</button></div>; });
const directions = rows.map(item => <button onClick={() => open(item.id)} className={`base ${item.direction}`}><Arrow /></button>);
const simple = rows.map(item => <button onClick={() => open(item.id)} className="base" aria-pressed={item.active}>{item.label}</button>);
const text = rows.map(item => <button onClick={() => open(item.id)} className={item.active ? "active" : "normal"}>{item.label}</button>);
const component = rows.map(item => <HintButton onClick={() => open(item.id)} className={item.active ? "active" : "normal"} aria-pressed={item.active} />);
const insideOtherCallback = rows.map(item => <Widget render={() => <button onClick={() => open(item.id)} className={item.active ? "active" : "normal"}><Icon /></button>} />);
const standalone = <button onClick={open} className={active ? "active" : "normal"}><Icon /></button>;"#;
    let args = ["check", "--json", "--rules", MAPPED_BUTTON];
    let default = Project::new(&[("buttons.jsx", source)]).success(&args, true);
    assert_eq!(default["summary"]["warnings"], 0);

    let report = Project::new(&[("checkignore.json", ENABLED), ("buttons.jsx", source)])
        .success(&args, true);
    let findings = report["diagnostics"].as_array().unwrap();
    assert_eq!(findings.len(), 3);
    for (index, finding) in findings.iter().enumerate() {
        assert_eq!(finding["rule"], MAPPED_BUTTON);
        assert_eq!(finding["severity"], "warning");
        assert_eq!(finding["line"], index + 1);
    }
}
