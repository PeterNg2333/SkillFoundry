# Go ownership and organization

Use idiomatic Go declarations and gofmt. JavaScript const-arrow and export-list preferences belong to the JS/TS profile. Go uses named `func` declarations and capitalized identifiers for exported API.

The checker has two main responsibilities, selection policy and native inspection. CLI/configuration/report packages own the actual boundaries between them.

```text
maintainable-code/
|-- assets.go                    # Embedded default configuration
|-- checkignore.json             # Default ignore/type/style settings
|-- go.mod, go.sum               # Locked Go dependencies
|-- build.sh                     # Format/test/vet/build entry
|-- cmd/check/main.go            # CLI arguments, sequencing and exit codes
|-- internal/
|   |-- config/config.go         # Typed configuration and project overrides
|   |-- selection/
|   |   |-- select.go            # Scope, walk, exclusions and type routing
|   |   |-- ignore.go            # Git/Docker library integration
|   |   |-- select_test.go       # Ignore/scope behavior
|   |-- checks/
|   |   |-- checks.go            # Implemented check catalog and dispatch
|   |   |-- dependencies.go      # Manifest counts and limits
|   |   |-- json.go              # Strict JSON validation
|   |   |-- go.go                # Go AST, format and comment analysis
|   |   |-- checks_test.go       # Observable check results and coverage
|   |-- report/report.go         # Findings, metrics, coverage and rendering
|-- bin/maintainable-check.exe   # Built native program
|-- scripts/                    # Existing JS/TS AST profile and selector bridge
```

Keep each operation in its owner and types beside the implementation they describe. `selection` owns all path-policy decisions. Native checkers receive only selected files. `report` owns the current output contract, which also supplies the future model-review boundary. Entry points sequence those operations explicitly.

Use concrete structs for configuration/results and functions for stateless analysis. Introduce interfaces for actual interchangeable implementations or an existing standard-library boundary. Name files for their responsibility and keep related errors, transformations and types local. Add a dependency or package with its current consumer.

Wrap operational errors with the operation/path and `%w`. Expected absence of an optional ignore/config file is a normal branch. Unexpected read, parse and writer failures reach the CLI with a failing exit code. Keep roots, check selection and output writers explicit. The native inspection path uses local parsing libraries.

Use table-driven or focused tests at the public behavior boundary. Keep test cases beside their owner, including ignore negations, malformed settings, partial coverage and report write failures. Run gofmt, `go test ./...`, `go vet ./...` and the built executable before delivering a Go change. Verify standalone behavior when changing runtime dependencies or packaging.
