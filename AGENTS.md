# Tiny Diplomacy

The authoritative game rules are in:
- docs/game-spec.md

The model protocol is in:
- docs/protocol.md

The implementation roadmap is in:
- docs/roadmap.md

For game-engine behavior, game-spec.md takes precedence over assumptions
about standard Diplomacy. Tiny Diplomacy intentionally differs from
standard Diplomacy in several rules.

Do not implement behavior based on standard Diplomacy unless explicitly
specified in game-spec.md.

The authoritative specifications for Tiny Diplomacy live in the various docs/ directories. Changes to protocol grammar, token IDs, game semantics, or press semantics must update the corresponding specification and regression tests. If implementation and documentation disagree, flag the conflict rather than silently treating either one as correct.