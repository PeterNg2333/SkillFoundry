# TypeScript organization and checks

Read this for TypeScript/JavaScript implementation, refactoring or static-check setup. Apply these user preferences to ordinary authored modules, with explicit repository and framework requirements taking precedence.

## File organization

- Put imports first and keep related local types beside the implementation they describe.
- Require project path aliases for cross-directory source imports and re-exports, including types and styles. Same-directory imports may use `./`. Reuse configured aliases. When the affected app needs an alias, configure its source-root mapping in the compiler and build/runtime resolver with the current slice, then migrate affected callers.
- Use named `const` arrow bindings for ordinary functions and React components. Keep class methods in their framework or lifecycle form.
- Collect public value exports and type exports in separate lists at the end of the file. Export only what real consumers use.
- Keep a cohesive type in its owning feature. Extract a shared definition when actual consumers share the same meaning. Derive process-boundary types from their existing validation schemas.
- Keep straightforward types and inferred local values. State explicit types where they clarify an operation's contract.

```typescript
type Reply =
  | { status: 'completed'; text: string }
  | { status: 'failed'; message: string }

const getReplyText = (reply: Reply): string => {
  if (reply.status === 'failed') {
    return reply.message
  }

  return reply.text
}

export { getReplyText }
export type { Reply }
```

Const bindings must be initialized before invocation. A function body can refer to a later binding when it runs after initialization. Preserve class, overload, generator and framework entry forms when those semantics are required. A Next page can keep its const component and end with `export default Page`. Nest decorated classes and runtime DI tokens keep their required runtime forms.

## Executable checks

Use the native Rust CLI described in [checks.md](checks.md). The SkillFoundry checkout owns implementation and dependencies under `checker/`; the installed skill includes the executable. It applies `checkignore.json`, scoped Git ignores, root Docker ignore and extension mappings before parsing. Run selected checks on affected files or the project root. Dependency checks measure declared package counts.

| Intent | Owner |
|---|---|
| Const arrow bindings and bottom export lists | Embedded ast-grep YAML patterns and native export-order inspection. |
| Type-only imports and separate type export lists | Bundled TypeScript syntax checks, with decorator metadata handling. |
| Cross-directory aliases | Bundled rule covers static/dynamic imports and re-exports, including child-directory paths. Compiler and runtime resolution use the project configuration. |
| Depth, complexity and file length | Bundled review warnings with thresholds in the skill's default configuration. |
| Whitespace, wrapping and punctuation | Project formatter and its existing configuration. |
| Strict types, indexed values and optional-property semantics | Project TypeScript compiler. |
| Unsafe values, Promise outcomes and exhaustive switches | Project type-aware linting against its actual compiler projects. |
| Process boundaries and feature-private imports | Project architecture-specific rules and boundary tests. |

Maintain shared syntax/style checks and regression fixtures in SkillFoundry's `checker/` when preferences evolve. Use the project's `checkignore.json` for scope and explicit exceptions. Keep compiler, formatter, ownership and behavior checks in their existing commands. Scope AST rules to ordinary syntax and verify class methods, overloads and framework entries when extending coverage.

Report every remaining bundled warning with its file/line, metric and disposition. Explain expensive rules, false positives and custom-rule maintenance when they affect adoption. Review exclusion, threshold and suppression changes as policy changes. Evaluate cohesion and ownership against the actual code when a metric raises attention.

## Review beyond syntax

Check state ownership, effect ordering, cancellation, duplicate operations and recovery through the real public flow. Inspect check-configuration and suppression changes alongside production code. Record exactly which checks ran and which external boundaries were substituted.
