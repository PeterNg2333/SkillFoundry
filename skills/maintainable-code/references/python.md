# Python project conventions and coverage

Use idiomatic `def` functions, classes where state or lifecycle needs an owner, and the project's formatter, linter, type checker and tests. JS/TS const-arrow and export-list conventions apply to their own languages. Repository and framework requirements take precedence.

- Mark module-private functions with a leading underscore, or list the public surface in `__all__`. Keep a private helper when it owns a distinct responsibility such as retry cleanup. Otherwise inline a helper that has one direct caller.
- Use absolute imports for modules outside the current directory. `from . import name` and `from .module import name` can stay relative.
- Put each entry on its own line in a dict literal that has a `**` unpacking and at least three other entries.
- Choose a fallback in priority order instead of repeating `if mode is None and ...` guards. For example, keep an explicit value with `explicit if explicit is not None else infer_mode()`, and let `infer_mode` return early for each choice.

The skill's Rust checker runs the shared native rules on `.py` and `.pyi` files through the default `python` check. These include syntax, single-use private helpers, cross-directory relative imports, nesting, complexity and file length, and the opt-in decision, literal and similar-loop rules. Comment groups and Interact directives read `#` comments. A standalone `# @bypass <rule-id> -- <reason>` comment applies to the next statement or declaration. [Rule languages and Python specifics](checks.md#rule-languages)

The native checker does not implement Ruff, a formatter or type checking. Enable `python-lint` to run Ruff through the checker, and preserve the project's own formatter, type checker and tests. Read [rust.md](rust.md) when changing the checker implementation itself.
