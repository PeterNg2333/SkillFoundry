---
name: maintainable-code
description: "Plan and deliver maintainability-focused changes in small reviewable phases. Use for architecture and phase planning, implementation, refactoring, handoffs, or Go CLI and AST/style/dependency checks selected by checkignore.json, Git/Docker ignores and file extensions."
---

# Maintainable Code

Make each agreed phase easy to understand, verify and continue. Apply the user's code organization preferences while respecting repository and framework requirements.

## Maintain this skill

The source of truth is `skills/maintainable-code/` in the `SkillFoundry` repository. The installed skill directory is a versioned runtime copy. Make reusable changes in the source checkout, run `sh build.sh`, package the skill with the repository's `scripts/package.sh`, then update the installed copy from that ZIP using `scripts/install.sh --replace`. The repository README owns distribution commands. Check `distribution.json` and `.installation.json` in an installed copy for its version, source revision and installation receipt.

## Establish the current task

Read the repository entry instructions, architecture source and current handoff when present. Inspect the working tree, affected code and actual callers before proposing changes. Resolve existing authorization and phase status from the session and repository workflow.

Scale the process to the task. A focused fix can use a short plan and the existing handoff. Planning and research requests produce their requested artifacts. Add workflow documents or tooling when requested or when they are a concrete dependency of the authorized work.

## Plan one working phase

Present a short plan before editing with the following information.

- One observable outcome and the checks that demonstrate it.
- Owning modules, state, side effects, public operations and affected callers.
- Files to change or add, each with its responsibility.
- Failure behavior and any compatibility or permission implications.
- A locality, reuse and refactor decision based on the existing code.

Aim for one or two main modules per phase. Count cohesive responsibilities with an interface. Treat this as a review-size guideline. Include the actual consumer when a contract changes across layers and explain any larger scope. Separate unrelated cleanup and substantial preparatory refactoring into reviewable work.

## Keep implementation local

Keep related types, transformations, state and error handling together. Split at an actual ownership, lifecycle or reuse seam. Give public operations precise names and keep their surface small. Use functions for transformations and concrete classes where state, lifecycle or the framework makes them clearer.

For each new helper, wrapper or shared abstraction, identify the responsibility it hides and its current callers. Introduce dependencies and folders with the slice that uses them. Keep a short linear flow together and comments focused on reasons or constraints.

For TypeScript/JavaScript code or check configuration, read [typescript.md](references/typescript.md). It owns the required import-alias convention, const-arrow/export organization and type locality. For Go code and native checker changes, read [go.md](references/go.md) for explicit package/file ownership and idiomatic Go declarations. Other languages use their repository conventions.

## Verify the result

Run the existing checks relevant to the changed behavior and inspect the actual diff. Tests should exercise public outcomes, meaningful edge cases and affected failure paths. Match verification to the changed boundary, including the actual entry point when integration behavior changes.

For code changes, read [checks.md](references/checks.md) and choose the supplied checks for the affected scope. The Go CLI provides standalone native checks and shared selection. The JS/TS profile supplies its existing AST rules and uses that same selection. Apply `checkignore.json`, scoped `.gitignore`, root `.dockerignore` and extension routing before analysis. Report actual coverage, skipped reasons, metrics and remaining warnings. Keep reusable implementation, rules, dependencies and fixtures in this skill, with project settings in the target repository. Explain tooling cost, false positives or custom-rule maintenance when a check may be excessive. Review exclusion and rule changes as policy changes.

Before handoff, trace one normal and one failure path. Locate their state owner and side effects. Consider one likely next change and where its edits would concentrate. Simplify flows that a maintainer would struggle to follow.

## Leave a current handoff

For a phase handoff, leaving partial implementation or resuming implementation across sessions, read [handoff.md](references/handoff.md). Update the repository's existing current record when implementation progress or review status changes. Status and explanation requests use a read-only comparison with that record. A request to establish this workflow can create one concise `handoff.md` when no current record exists.

Report the delivered behavior, changed responsibilities, verification results and remaining work. Follow the user's and repository's review cadence before advancing to another phase. Use existing matching operation grants. A skill supplies a workflow and leaves execution authorization with the session and repository rules.
