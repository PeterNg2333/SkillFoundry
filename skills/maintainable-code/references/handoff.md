# Current implementation handoff

Read this when ending an implementation phase, leaving partial work or continuing a task across sessions.

Use the repository's established current handoff. Keep one evolving operational record and link to stable specs, architecture and tickets. When the user requests `handoff.md` and an equivalent record exists, consolidate or rename it and update its callers as part of the authorized documentation change.

Keep the record factual and short. Include enough evidence for another session to verify its starting point. Use the following shape, adapting headings to the repository.

```markdown
# Current handoff

Updated YYYY-MM-DD
Phase and review status
Branch/base revision and relevant working-tree changes

## Delivered behavior
Observable result and its scope.

## Current implementation
Changed files, public entry points, state/effect owners and important decisions.
Link to the current spec or ticket and stable architecture.

## Verification
Exact commands with passed, failed or not run results.
Substituted provider/device boundaries and remaining manual checks.

## Remaining work
Known defects, partial work and unresolved choices affecting the next step.

## Next action
One concrete next action, relevant files and any existing review gate.
```

On continuation, compare the record against the working tree, relevant code and recorded results. Preserve another contributor's edits and correct stale claims before relying on them. Resolve review status and operation grants from authoritative session/repository evidence.

At handoff, record only verification actually performed. Distinguish fixture results from live/provider/device evidence. Keep secrets out of the record. Update phase status according to the agreed review workflow and leave the precise next action available to the next session.
