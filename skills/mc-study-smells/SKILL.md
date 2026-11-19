---
name: mc-study-smells
description: Distill candidate SkillFoundry checks and contextual review criteria from a conversation, review, or code example. Use when asked to study coding preferences or propose checker rules for approval.
---

# Study code smells

Keep the study read-only. Use the requested conversation, review, or examples. Use the current conversation when the user gives no other source.

1. Find explicit user corrections and decisions. Check the relevant code and callers when they determine whether a preference applies. Treat an accepted patch as evidence for its own scope.
2. Check the installed checker's capabilities and the existing `$maintainable-code` smell guidance. Point to existing coverage when it handles the example.
3. For each new candidate, show the evidence, scope, a case to flag, a case to allow, and the maintenance benefit. For a deterministic candidate, state the exact syntax, type, or dependency condition. When enough source is available, pilot the condition read-only and label the result as evidence for the proposal. For a contextual candidate, state what the reviewer must inspect.
4. Present the candidates in chat and wait for the user's decision on each one. Use `$mc-add-smell` when the user expressly asks to record a selected candidate. Change checker behavior after explicit approval to implement that candidate.
