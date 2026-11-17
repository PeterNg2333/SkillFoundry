package maintainablecheck

import _ "embed"

// Defaults travel with the executable, including when it is copied out of the skill.
//
//go:embed checkignore.json
var Defaults []byte
