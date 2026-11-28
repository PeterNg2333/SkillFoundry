//! Native policy rules and the source languages that implement each one.
//!
//! This table is the single declaration of rule coverage. Configuration validation,
//! per-file policy, `capabilities --json` and the parity corpus in `tests/parity/` read it.
//! A rule runs for a file only when the file's language is listed here.

pub const JAVASCRIPT: &str = "javascript";
pub const TYPESCRIPT: &str = "typescript";
pub const PYTHON: &str = "python";
pub const LANGUAGES: &[&str] = &[JAVASCRIPT, TYPESCRIPT, PYTHON];

const ECMASCRIPT: &[&str] = &[JAVASCRIPT, TYPESCRIPT];
const SHARED: &[&str] = LANGUAGES;

pub const CROSS_DIRECTORY_IMPORTS: &str = "maintainable/cross-directory-imports";
pub const SINGLE_USE_HELPER: &str = "maintainable/single-use-helper";
pub const OBJECT_SPREAD_NEWLINE: &str = "maintainable/object-spread-newline";
pub const LONG_CONDITION: &str = "maintainable/long-condition";
pub const FALLBACK_ORDER: &str = "maintainable/fallback-order";
pub const LONG_JSX_CLASS_NAME: &str = "maintainable/long-jsx-class-name";
pub const PLAYWRIGHT_STYLE_ASSERTION: &str = "maintainable/playwright-style-assertion";
pub const COMPLEX_MAPPED_BUTTON: &str = "maintainable/complex-mapped-button";
pub const SIMILAR_LOOPS: &str = "maintainable/similar-loops";
pub const TYPE_IMPORTS: &str = "@typescript-eslint/consistent-type-imports";

pub struct Rule {
    pub id: &'static str,
    pub languages: &'static [&'static str],
}

pub const RULES: &[Rule] = &[
    Rule {
        id: "prefer-const",
        languages: ECMASCRIPT,
    },
    Rule {
        id: "no-var",
        languages: ECMASCRIPT,
    },
    Rule {
        id: "func-style",
        languages: ECMASCRIPT,
    },
    Rule {
        id: "no-restricted-syntax",
        languages: ECMASCRIPT,
    },
    Rule {
        id: CROSS_DIRECTORY_IMPORTS,
        languages: SHARED,
    },
    Rule {
        id: TYPE_IMPORTS,
        languages: &[TYPESCRIPT],
    },
    Rule {
        id: "max-depth",
        languages: SHARED,
    },
    Rule {
        id: "complexity",
        languages: SHARED,
    },
    Rule {
        id: "max-lines",
        languages: SHARED,
    },
    Rule {
        id: SINGLE_USE_HELPER,
        languages: SHARED,
    },
    Rule {
        id: OBJECT_SPREAD_NEWLINE,
        languages: SHARED,
    },
    Rule {
        id: LONG_CONDITION,
        languages: SHARED,
    },
    Rule {
        id: FALLBACK_ORDER,
        languages: SHARED,
    },
    Rule {
        id: LONG_JSX_CLASS_NAME,
        languages: ECMASCRIPT,
    },
    Rule {
        id: PLAYWRIGHT_STYLE_ASSERTION,
        languages: ECMASCRIPT,
    },
    Rule {
        id: COMPLEX_MAPPED_BUTTON,
        languages: ECMASCRIPT,
    },
    Rule {
        id: SIMILAR_LOOPS,
        languages: SHARED,
    },
];

pub fn rule(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|rule| rule.id == id)
}

pub fn known(id: &str) -> bool {
    rule(id).is_some()
}

pub fn supports(id: &str, language: &str) -> bool {
    rule(id).is_some_and(|rule| rule.languages.contains(&language))
}

pub fn ids() -> impl Iterator<Item = &'static str> {
    RULES.iter().map(|rule| rule.id)
}
