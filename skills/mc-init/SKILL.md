---
name: mc-init
description: Initialize or update a project's SkillFoundry checkignore.json. Use when asked to declare code style, language coverage, ignores, or bypasses in automatic or questionnaire mode.
---

# Initialize project policy

`$mc-init` or `$mc-init auto` inspects the project, writes its policy, and verifies it. `$mc-init questionnaire` inspects the project and asks about policy choices before writing and verifying. A request for a draft stops before writing the project file.

## Inspect the project

Use the requested project root, or resolve it from the current working directory. Read the existing `checkignore.json`, repository instructions, language manifests and source extensions, lint and formatter settings, and Git and Docker ignore files that apply. Sample authored source when the configuration leaves an important convention unclear. Check supported rule IDs and language coverage using `maintainable-check capabilities --json` and the [checker reference](../maintainable-code/references/checks.md) when available. Prefer the current checkout's checker when initializing SkillFoundry itself.

## Choose policy

- **Language and scope.** Distinguish detected languages from what the checker can cover. Keep useful built-in mappings. Set an extension to `false` when the user wants it excluded. Add a custom mapping only for a supported source kind. Enable optional checks through `externalChecks` when the policy calls for them and their prerequisites are available. Use project commands as evidence for tool coverage outside the checker.
- **Style.** Put explicit project conventions and their rationale in `styleNotes`. Use `rules`, `overrides`, and tool selectors for enforceable settings. Carry forward existing severity and thresholds unless the task calls for changing them. Treat prevailing source style as evidence for proposing a convention in questionnaire mode.
- **Ignores and bypasses.** Reuse existing Git, Docker, and embedded exclusions. Add positive project-relative `ignore` globs for confirmed extra exclusions, such as generated or vendored paths. Use a rule-specific `bypasses` entry with the narrowest supported file scope and a reason for a confirmed exception. Preserve existing unrelated entries.

In automatic mode, use explicit user choices, existing configuration, and conventions established by the project. Write a conservative policy directly. Report any material choices supported only by inference. Leave uncertain exclusions, exceptions, and new enforcement choices for the user to decide.

In questionnaire mode, ask one question at a time on topics the user has yet to settle. Start with languages and scope, then cover style and severity, then ignores and bypasses. Present a recommendation from the inspection and allow a custom answer. An answer of `accept defaults` adopts the proposed choices. Wait for each answer, carry it forward, and produce the file after the requested choices are settled.

## Verify and write

Draft strict JSON using the supported checker fields. For an existing policy, merge the requested choices into the document and preserve unrelated settings. When the checker is available, run `maintainable-check select --root <project> --config <draft-path> --json` on the draft. Check that representative source remains selected and any new exclusions match only their intended paths. Correct invalid configuration before replacing the project file. When the checker is unavailable, verify JSON syntax and disclose the missing selection check.

Write `checkignore.json` at the target root and verify selection using that file. Report the path, language coverage, enforceable style, and new ignores or bypasses with their reasons. Include a focused read-only check when adopting new enforcement or measuring a baseline.
