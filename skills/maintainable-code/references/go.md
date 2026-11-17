# Go project conventions and coverage

Use idiomatic named `func` declarations, capitalized exported identifiers and the project's gofmt/go vet/test commands for Go code. JS/TS const-arrow and export-list conventions apply to their own languages.

The skill's Rust checker embeds the Go grammar for syntax, import/function counts and optional AST comment groups. Comment-like strings remain strings. `--checks go,comments` selects those checks; dependency inspection counts direct and indirect `require` declarations in `go.mod` without resolving modules.

The Rust checker does not implement gofmt or Go type checking. Reports mark `go-format` unsupported when Go checks run. Preserve the project's real formatter, compiler and tests. Read [rust.md](rust.md) when changing the checker implementation itself.
