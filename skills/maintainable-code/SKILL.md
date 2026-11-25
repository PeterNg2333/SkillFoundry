---
name: maintainable-code
description: "Plan and deliver maintainability-focused changes in small reviewable phases. Use for architecture, implementation, refactoring, verification and test design, handoffs, scoped static checks, or recording and reviewing code smells."
---

# Maintainable Code

Make each agreed phase easy to understand, verify and continue. Apply the user's code organization preferences while respecting repository and framework requirements.

## Quality philosophy

Use **20% coding, 70% checking, and 10% human review** as a default allocation of effort. Scale the depth to the change and its risk.

- **Coding.** Implement the requested change, including ordinary feature work, fixes, and refactoring.
- **Checking.** Start with deterministic checks. Verify readability, correctness, performance, and business logic wherever the change affects them. Use the [verification guide](references/verification.md#what-to-check) to choose evidence.
- **Human review.** Present the key decisions, evidence, and remaining uncertainty. Focus the review on product intent, tradeoffs, and material risk.

## Shortcuts

- `/help` shows the shortcut syntax, scope examples, and supported checks. Read [checks.md](references/checks.md) and [smells.md](references/smells.md) and answer without scanning the project.
- `/init [auto|questionnaire]` sets up project policy. Use the [mc-init workflow](../mc-init/SKILL.md) for project inspection, policy choices, and verification.
- `/check [description]` runs static checks. Read [checks.md](references/checks.md) to translate the description into project-relative paths, check IDs and rule IDs. With no description, run the project's default native check scope.
- `/interact` discovers work directives in the current Git repository, carries out ready items, and records an activity page. Read [interact.md](references/interact.md) before starting.
- `/add-smell [example or description]` records or updates a reusable smell in the source skill. Read [smells.md](references/smells.md) to choose a deterministic check or contextual review rule and record the evidence.
- `/review-smells [scope]` explicitly requests read-only contextual review. Apply the criteria in [smells.md](references/smells.md) to the named files or feature.

Prefix a shortcut with `$maintainable-code` to select this skill explicitly. Treat `/check` as a read-only request. Carry out source work directives when the user asks to implement or refactor them.

## Maintain this skill

The source of truth is the `SkillFoundry` repository: `checker/` owns the Rust program, defaults, rules and fixtures; `skills/maintainable-code/` owns these instructions and references. The installed directory is a versioned runtime copy. Make reusable changes in the source checkout, run `sh scripts/build.sh` from its root, package with `sh scripts/package.sh maintainable-code`, then update the installed copy from that ZIP using `sh scripts/install.sh <archive> --replace`. The repository README owns distribution commands. Check `distribution.json` and `.installation.json` for version, source revision and installation receipt.

## Establish the current task

Read the repository entry instructions, architecture source and current handoff when present. Inspect the working tree, affected code and actual callers before proposing changes. Resolve existing authorization and phase status from the session and repository workflow.

Before editing a target project, read `checkignore.json` or, if absent, `check.json`. Follow the declared `styleNotes` and `fileStructure`, enforce the applicable `rules`, and check the scope and reasons of existing `bypasses`. Explain changes to project policy.

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

For TypeScript/JavaScript code or check configuration, read [typescript.md](references/typescript.md) for import aliases, const-arrow/export organization and type locality. For native checker changes, read [rust.md](references/rust.md) for file ownership and Rust conventions. For Go project code, read [go.md](references/go.md). Other languages use repository conventions.

## Verify the result

Use the [verification workflow](references/verification.md#run-and-report-checks) when implementing a code change. Choose applicable checks from the project's scripts or CI configuration, run them on the affected scope, and inspect the diff. For behavior changes, verify an observable outcome through the relevant entry point.

Include each verification command or manual action, its scope, and its actual result in the handoff. If a relevant check could not run, name the blocker and the behavior or policy left unverified.

For SkillFoundry checks, use [checks.md](references/checks.md) to resolve the applicable rule IDs and paths. Source checks use the project policy, scoped `.gitignore`, root `.dockerignore` and extension routing. Use `--checks file-structure` to focus on the declared directories and file types. Check the report for coverage, unsupported checks, skipped paths, and warnings. Reusable rules and fixtures belong to `checker/`. The target project owns its policy. Explain changes to exclusions, severity, thresholds, or bypasses.

Before handoff, trace one normal and one failure path. Locate their state owner and side effects. Consider one likely next change and where its edits would concentrate. Simplify flows that a maintainer would struggle to follow.

## Leave a current handoff

For a phase handoff, leaving partial implementation or resuming implementation across sessions, read [handoff.md](references/handoff.md). Update the repository's existing current record when implementation progress or review status changes. Status and explanation requests use a read-only comparison with that record. A request to establish this workflow can create one concise `handoff.md` when no current record exists.

Report the delivered behavior, changed responsibilities, verification results and remaining work. Follow the user's and repository's review cadence before advancing to another phase. Use existing matching operation grants. A skill supplies a workflow and leaves execution authorization with the session and repository rules.
