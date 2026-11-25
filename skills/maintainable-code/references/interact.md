# Interact

`$maintainable-code /interact` starts one repository pass. The user's `@` notes in code and specs supply the work for that pass. The ordinary static check remains a read-only source-policy check. Run Interact again to resume work after the user edits a note or adds a reply.

## Discover and record work

1. Resolve the current Git worktree root. Read its repository instructions and `checkignore.json` (or `check.json` when absent). Inspect existing changes before starting; keep the user's edits.
2. Run the packaged recorder with Python 3.10 or newer and the packaged native checker. It scans the worktree using the directive check and returns the instruction list, a run file, and a read-only HTML activity page.

   ```text
   python <skill>/scripts/interact_trace.py start --root <repo> --checker <skill>/bin/maintainable-check
   ```

   Use `maintainable-check.exe` on Windows. Share the returned `view` as a local file link as soon as the run starts. Report discovery errors with their native diagnostics. An empty list ends the pass.

3. Read each directive in context before acting. Check nearby acceptance notes, the enclosing symbol, and any linked master spec. Execute ready work in dependency order. Update each item's activity before editing, before verification, and when a blocker or result is known.

   ```text
   python <skill>/scripts/interact_trace.py event --run <run> --item 1 --state Running --detail "Locating the state owner and callers."
   ```

   Replace the item number with the one returned from discovery. The view refreshes from an atomically replaced local HTML file. Run JSON and the HTML are stored under this worktree's Git metadata. Each new view includes previously cleared items. Run files belong to the agent invocation that created them and receive sequential updates from it.

## Source notes and master specs

Use one directive per line. Put inline directives in actual JS/TS comments near their target. Put shared decisions and acceptance criteria in a Markdown-formatted `.spec` file. A master spec can coordinate multiple source files using a repository-relative scope. The scanner reads directive lines in the `## Work` section, excluding fenced and indented code examples. Source files may use `// @spec ./floor-selection.spec` to link back to the master. Paths in that link are relative to the source file.

```text
# Floor selection

## Intent
The viewer reads the selected floor from the existing store.

## Scope
src/viewer/floor-selection/**

## Acceptance
AC-1: A response from a previous building cannot overwrite the selected floor.

## Work
@plan module FloorSelection -- Locate the state owner and its callers.
@do-type FloorSelectionState -- Represent loading, ready, and unavailable states.
@do-hook useFloorSelection -- Connect the viewer to the existing store.
@add-test AC-1 -- Switch buildings before the first request finishes.
@check AC-1
```

A project can limit discovery through `workDirectives.paths` and `workDirectives.tags` in its policy. Paths are positive repo-relative globs. Tag names omit the `@`. Omit either field to retain the normal file scope or full tag set. Include `spec`, `question`, and `reply` if that project limits tags and wants those annotations recognized. Setting `".spec": false` under `extensions` disables master spec selection.

```json
{
  "workDirectives": {
    "paths": ["src/**"],
    "tags": ["plan", "do-type", "do-hook", "add-test", "check", "review", "spec", "question", "reply"]
  }
}
```

## Handle each directive

| Directive | Deliverable and evidence |
| --- | --- |
| `@plan` | Identify ownership, public operations, callers, side effects, open decisions, and proposed checks. Write the plan in the spec's planning section or record it in the trace for an inline request. |
| `@do`, `@impl`, `@do-type`, `@do-hook` | Implement the requested behavior or unit through existing entry points. Inspect and verify affected consumers. |
| `@fix`, `@refactor` | Reproduce the problem or establish the existing contract, then verify the change against it. |
| `@add-test` | Add or strengthen a test in the project's existing suite. Run it and record its command and assertion. A test may intentionally fail when it reproduces an unresolved bug. Record the failed expectation as evidence for that test without marking the linked implementation or `@check` as verified. |
| `@check` | Run an existing check for the named condition. Record tool, scope, exit code, and result. When attached to a change, verify the changed behavior before clearing either note. |
| `@review` | Review the requested scope using the existing contextual review criteria. Attach findings with locations and separate them from tool results. |
| `@explain` | Record a walkthrough from the actual entry point to the relevant logic, with source references. |
| `@spec`, `@question`, `@reply` | Provide context for nearby work. They are discoverable annotations rather than work items. |

If a decision materially changes behavior or write scope, leave a short `@question` in the same file or master spec and record `Needs input`. The user can add `@reply` nearby. Continue independent ready work. A plan may clear when its requested output is available; begin implementation only for work directives that request it. A review finding requesting another change is evidence for that review and can become a separate work directive.

## Record evidence and clear

- For code changes and tests, run the repository's established verification tools. Use [verification.md](verification.md#run-and-report-checks) for evidence expectations. Include actual changed files and commands with results. Record verification failures as `Verification failed` and leave incomplete directives in place.
- Record a completed item's evidence with an event in state `Verified`. Re-read the original location and remove only the exact completed directive, plus any resolved question/reply specific to it. Preserve the spec's intent, scope, acceptance criteria, and outstanding work. Record `Cleared` after the source edit succeeds.
- If the note changed during execution, leave the new text in place and record why it is pending. If a previous run stopped between verification and cleanup, inspect its evidence and current source before repeating work.
- Give the user the local activity page and a short account of remaining directives. The page displays state, original source position, and the recorded sequence of decisions, edits, and checks.
