//! `maintainable/single-use-helper`: private functions with exactly one direct call.
//!
//! Each language's binding analysis decides which functions are private and resolves their
//! references. This module owns the shared decision and diagnostic.

use std::ops::Range;

use crate::{catalog::SINGLE_USE_HELPER, report::Report};

/// A private, non-generator function binding declared once in its scope.
pub struct Helper {
    pub name: String,
    /// Offset of the bound name, where the diagnostic points.
    pub offset: usize,
    /// The whole declaration, including its body.
    pub declaration: Range<usize>,
    pub references: Vec<Reference>,
}

/// A resolved reference to a helper.
pub struct Reference {
    pub offset: usize,
    /// Whether the reference is the callee of a plain, non-optional call.
    pub direct_call: bool,
}

pub fn check(helpers: &[Helper], file: &str, source: &str, severity: &str, report: &mut Report) {
    for helper in helpers {
        let [reference] = helper.references.as_slice() else {
            continue;
        };
        // A recursive call inside the declaration is not a caller to inline into.
        if !reference.direct_call || helper.declaration.contains(&reference.offset) {
            continue;
        }
        report.finding(
            file,
            source,
            helper.offset,
            severity,
            SINGLE_USE_HELPER,
            format!(
                "{} has one direct call site. Inline it into the caller or document a local exception.",
                helper.name
            ),
        );
    }
}
