# Tiny Diplomacy: Press Engine Test Cases

## Task for Codex

Implement focused Rust tests for the **typed, deterministic press engine** in `fauve-/jolly`, and implement only the press behavior needed to make those tests pass. Use the three project specifications supplied with this document as the source of truth, especially the *Token, State, and Press Protocol* sections 5–14, 17–18, and 22. Keep the dependency direction `press_engine -> game_engine`; the semantic press engine must not depend on Candle or token generation.

Work in two passes: **A. semantic transitions, validation, visibility, and commitment resolution**; then **B. inbox scheduling, budgets, and phase closure**. The later token parser, serializer, and constrained decoder have a separate test group below. Tests should assert observable state, rather than private representation or particular method names. Keep existing game engine tests passing.

## Concrete fixture and conventions

Use this board unless a case supplies another one:

```rust
GameState::empty(Phase::Spring, 1)
    .with_unit(Location::B2, Power::Red)
    .with_unit(Location::C3, Power::Blue)
    .with_unit(Location::A1, Power::Green)
    .with_unit(Location::D2, Power::Yellow)
```

`R_MOVE = Red: Order::Move { from: B2, to: C2 }` and `B_SUPPORT = Blue: Order::SupportMove { from: C3, unit: B2, to: C2 }`. Both orders are geometrically legal in this fixture. `R_HOLD = Red: Order::Hold { at: B2 }`; `B_AVOID = Blue: AVOID D3` (Blue is not occupying D3). Other fixtures may use `GameState::empty(...).with_unit(...)`. The power on an order proposition must own the source unit; the `Order` type itself does not record ownership.

Assume four powers participate, even if a test uses an otherwise empty board. A *message* includes sender, one distinct recipient, visibility, and one speech act. `END` is the **absence of a sent message** in a press activation, not a delivered message. Use `max_press_messages_per_power = 4` for normal scheduling tests; explicitly override it for boundary tests.

For every rejected submission, assert **no changes** to messages, unread inboxes, outstanding proposals, active commitments, and send counts. If the API combines activation and submission, compare the phase snapshot at the correct transaction boundary: invalid output should not commit an outgoing message; a previously consumed activation should not be silently retried. A valid message consumes exactly one sender budget unit, regardless of its number of terms.

## A1. Message validation and atomicity

| ID | Setup and action | Expected result |
| --- | --- | --- |
| V01 | Red submits a private `PROMISE [R_MOVE]` to Red. | Reject self recipient; no message or commitment recorded. |
| V02 | Attempt to start press with a `Winter` game state. | Reject or expose no press phase; movement press is Spring/Autumn only. An equivalent Autumn fixture is accepted. |
| V03 | Red sends Blue `PROMISE [B_SUPPORT]`. | Reject: a promise may bind only its sender. |
| V04 | Red sends Blue `REQUEST [R_MOVE]`. | Reject: a request may name only its recipient. |
| V05 | Red sends Blue `PROPOSE [Green: HOLD A1]`. | Reject: neither party may bind a third power. |
| V06 | Red sends Blue a `PROMISE` with both `R_MOVE` and `R_HOLD`. | Reject the entire message: one unit cannot be given two orders; no partial commitment. |
| V07 | Red sends Blue `PROMISE [Red: AVOID B2]`. | Reject: Red already occupies B2. A promise to avoid unoccupied C2 is structurally valid. |
| V08 | Red sends Blue a promised `B2 MOVE D3` or an order from an empty square. | Reject: bad geometry or missing actor unit. Check source ownership too by trying `Red: HOLD C3`. |
| V09 | Red sends Blue `PROMISE [R_MOVE, Red: AVOID C2]`. | Reject as mechanically contradictory: the move enters the avoided square. Repeat with `Red: SUPPORT ... MOVE C2` and `Red: AVOID C2` in a fixture with valid support geometry. |
| V10 | Red sends Blue `ACCEPT`, then `REJECT`, when Blue has no outstanding proposal to Red. | Reject each; no counterparty commitment or budget charged. |
| V11 | Red sends Blue a valid multi-term message, including `R_MOVE` and `Red: AVOID D3`. | Accept both terms atomically. A syntactically empty `PROMISE`, `REQUEST`, or `PROPOSE` is rejected when parser validation is present. |

For V08, the current `game_engine::is_legal_order` is private. Codex may expose a general public legality query in `game_engine` or use an equivalent neutral adapter; it must not put press-specific rules in `game_engine`. Do not use the old illustrative `B3 SUPPORT B2 MOVE C2`: B3 is diagonal from C2.

## A2. Speech-act transitions

| ID | Setup and action | Expected result |
| --- | --- | --- |
| T01 | Red sends Blue `REQUEST [B_SUPPORT, B_AVOID]`. | Visible message and Blue inbox entry; **zero** new commitments and **zero** pending proposals. |
| T02 | Red sends Blue `PROMISE [R_MOVE, Red: AVOID D3]`. | Two active Red commitments appear immediately; no acceptance required. They are for Spring of year 1 and retain origin, counterparty, and visibility metadata. |
| T03 | Red sends Blue `PROPOSE [R_MOVE, B_SUPPORT]`. | Outstanding proposal keyed `(Red, Blue)` with both terms; no active commitments yet. |
| T04 | After T03, Red sends Blue another `PROPOSE [R_HOLD]`. | Exactly one `(Red, Blue)` proposal exists, containing only `R_HOLD`. The earlier proposal remains a history message but cannot be accepted. |
| T05 | With `(Red, Blue)` outstanding, Blue sends Red `PROPOSE [B_SUPPORT]`. | Both ordered-pair proposals exist independently. |
| T06 | After T05, Blue sends Red `ACCEPT`. | Only `(Red, Blue)` is consumed. Its full two-term bundle becomes active in one transition. `(Blue, Red)` stays outstanding. |
| T07 | After T05, Blue sends Red `REJECT`. | Only `(Red, Blue)` is removed; it creates no commitments. `(Blue, Red)` stays outstanding. |
| T08 | Create Red→Blue proposal A, supersede it with proposal B, then Blue accepts addressed to Red. | Commit only B's terms; A never becomes active. |
| T09 | Red sends Blue a private proposal; Blue sends Red a **public** `ACCEPT`. | Acceptance succeeds despite mismatched visibility. Everyone can observe the acceptance message; do not assume private proposal terms become public without an explicit context visibility policy. |
| T10 | After a rejection, Red proposes the same terms again. | Valid new proposal; no repetition filter. |
| T11 | Red sends Blue `REQUEST [B_SUPPORT]`, then Blue sends Red `PROMISE [B_SUPPORT]`. | Only Blue's promise creates the commitment. No implicit `ACCEPT` semantics for `REQUEST`. |
| T12 | Red sends Blue `PROPOSE [Red: AVOID C2, Blue: AVOID C2]`, then Blue accepts. | Two independently owned avoidance commitments; Blue occupies C3, not C2. |

Where metadata is exposed, check a private unilateral promise stays marked private, a public one stays marked public, and accepted commitments preserve their source proposal/acceptance information without claiming that secret proposal contents are visible to uninvolved powers.

## A3. Visibility and inbox delivery

| ID | Setup and action | Expected result |
| --- | --- | --- |
| D01 | Red sends Blue a private request. Query each power's visible history. | Red and Blue see it. Green and Yellow see **no message and no placeholder/count revealing it**. Only Blue has unread inbox work. |
| D02 | Red sends Blue a public request. Query history and inboxes. | All four powers see the same message and its sender/recipient/visibility. Only Blue receives an unread inbox item. Green and Yellow do not become eligible to act. |
| D03 | Red and Green send Blue separate messages before Blue's next activation. | Blue receives both in delivery order; exactly one activation consumes those two unread entries and may produce at most one reply. Both messages remain in visible history afterward. |
| D04 | After Blue consumes its unread messages and returns `END`, query Blue again without a new delivery. | No inbox-triggered activation; old history is not an unread event. |
| D05 | Red sends Blue a private message and Green sends Yellow a private message. | Blue's view does not contain Green→Yellow in any representation; Yellow's view does not contain Red→Blue. Red and Green each see their own sent message. |

Filter **proposals and commitments as well as transcripts** when exposing an agent view. D01 and D05 should assert the complete public view has no private-content leak, rather than checking only one history accessor. Internal state may retain everything for adjudication and diagnostics.

## A4. Scheduling, budgets, and closure

| ID | Setup and action | Expected result |
| --- | --- | --- |
| S01 | Fresh Spring phase, four empty inboxes. | Each power gets exactly one opening opportunity. Each may send one message or `END`; an empty inbox does not prevent initiation. No power gets a second opening. |
| S02 | Red replies `END` during opening with budget 4. | Sent count remains zero; no inbox entry or history message; Red may still reply to a later delivery. |
| S03 | Budget 2; Red sends two valid messages on separate eligible activations. | Red's count progresses 0→1→2; each message consumes exactly one unit, including multi-term messages. Blue's budget is unaffected. |
| S04 | Budget 1; Red sends once and later receives a Blue message. | Red gets an inbox-triggered activation, can read/consume it, but its only valid response is `END`; attempts to send a message are refused without a second send. |
| S05 | Budget 0; run all openings with `END`. | Every power gets the opening opportunity with only `END` available; phase closes with zero messages. |
| S06 | Red↔Blue alternate the identical `PROPOSE`/`REJECT` exchange with finite budget. | Repetition is accepted while send capacity remains. After all messages are consumed, phase ends; total sent messages ≤ `4 * budget`. No repeat-detection rule changes legality. |
| S07 | One opening complete, another power's opening pending, inboxes empty. | Phase is not complete yet. After all openings and all unread inbox entries are consumed, phase is complete. |
| S08 | Red→Blue proposal remains unaccepted when all openings and inbox activations finish. | Closing the press phase removes the pending proposal; no commitments are created. A later `ACCEPT` in this or a new phase cannot accept it. |
| S09 | Red promise or an accepted proposal exists when press closes. | Active commitments survive press closure through movement-order submission; resolution then archives results and expires those active commitments before the next movement phase. |
| S10 | After phase closure, attempt to submit a new message. | Refused; no state change or budget charge. |
| S11 | Run identical starting state, opening order, eligibility policy, and agent replies twice. | Message order, per-power activations, proposals, commitments, and outcome match exactly. If scheduler uses randomness, fix and record the seed. |

Opening versus inbox activation order is an environment scheduling choice: the protocol guarantees each opening, but does not require a particular interleaving of openings and replies. Tests should select an explicit scheduling policy and assert its results, rather than silently assuming one. A queue should snapshot the unread messages at activation start so arrivals during an activation remain unread until the next activation.

## A5. Commitment resolution against *submitted* orders

Use a separate Spring board as needed to make every order legal. Supply a complete order set for the relevant units and inspect the commitment results **before or independently of adjudication**.

| ID | Setup and action | Expected result |
| --- | --- | --- |
| C01 | Red promises `B2 MOVE C2` and submits exactly that order; a defending unit makes the move bounce. | Fulfilled, despite failed move. |
| C02 | Same promise; Red submits `B2 HOLD`, omits B2's order, or submits another B2 move (separate cases). | Violated in each case. The engine's fallback hold must not satisfy a promised submitted order. |
| C03 | Red promises `AVOID C2` while B2 is occupied, then submits `B2 MOVE C2`. | Violated whether move succeeds or bounces. |
| C04 | Fixture: Red at C3, Blue at B2, C2 empty. Red promises `AVOID C2` and submits `C3 SUPPORT B2 MOVE C2`. | Violated even if Blue holds instead or the support is cut/ineffective: Red submitted support for entry to C2. |
| C05 | Red promises `AVOID C2` and submits orders with no Red move/support-move into C2. | Fulfilled. A `HOLD` or `SUPPORT C2 HOLD` for a Red unit at C2 is a non-entry action, though the initial avoid proposition would have been invalid while Red occupied C2; test the resolution predicate separately for that edge case. |
| C06 | Red holds both a positive commitment to move B2→C2 and a separate avoid commitment for D3; submits the move. | Both classify fulfilled independently. |
| C07 | Red promises to support a specific move, submits that exact support order, and the support is cut or ineffective. | Positive support commitment fulfilled: outcome does not enter the check. |
| C08 | After any violated commitment, query available legal game orders / submit the violating order. | Legal order set and adjudication are unchanged; violation appears only in diagnostics. |
| C09 | Resolve Spring commitments and begin Autumn press. | Spring commitments no longer active; fulfillment records may remain in history; Autumn starts with fresh proposal/inbox/budget state. |

The game engine's `MovementResult` retains both `OrderResolution.submitted` and `resolved_as`; inspect **submitted** orders, not `resolved_as` or `succeeded`. Distinguish an order never submitted from the adjudicator's fallback `HOLD`. The current game engine does not associate an order with a power directly, so use the pre-movement board to identify its owner.

## B. Protocol adapter tests (implement after semantic press works)

These are test requirements for the later protocol crate; they should **not** force a tokenizer into `press_engine` now.

| ID | Input / property | Expected result |
| --- | --- | --- |
| P01 | `END` after `PRESS`; otherwise `PRIVATE BLUE PROMISE RED B2 MOVE C2 END`. | First parses as no message; second parses as a Red-authored typed promise when acting power is Red. Round trip through serialization. |
| P02 | `PUBLIC BLUE PROPOSE RED B2 MOVE C2 BLUE C3 SUPPORT B2 MOVE C2 END`. | Parses to ordered-pair proposal with two valid terms under the fixture, retaining visibility and recipient. |
| P03 | Serialize equal bundles whose terms were supplied in different orders. | Same canonical token sequence; parsing either yields the same semantic bundle. |
| P04 | Attempt malformed prefix, missing `END`, empty nonempty speech act, self recipient, nonexistent unit, wrong actor, and `ACCEPT` without matching proposal. | Parser or prefix mask rejects at the earliest decidable point; no partial press transition. |
| P05 | Generate movement orders while a conflicting promise exists. | Valid-next-token mask still offers otherwise legal promise-breaking orders. |
| P06 | Generate Blue's context after Red→Green private press and after Red→Green public press. | Private content absent without marker; public message present but not an unread Blue inbox trigger. |

## Decisions the tests must not accidentally invent

1. **Commitment disclosure after public acceptance of a private proposal:** the spec permits the public acceptance and says it reveals an agreement, but does not fully define how private proposal *terms* appear in other powers' active-commitment views. Preserve secrecy unless a separate policy explicitly publishes terms.
2. **Contradictions across distinct messages:** reject mechanically contradictory terms *within one message*. The spec does not say a later standalone promise rescinds an older one; keep each commitment and resolve it independently unless a later policy defines supersession.
3. **Invalid submission during an activation:** define an explicit API transaction boundary. The test must guarantee an invalid output does not create a message, mutate semantic state, or spend budget, without requiring the scheduler to roll back an already observed activation.
4. **Press normalization versus game adjudication:** press-time propositions should pass current unit ownership and geometry checks. Promise breaking stays possible at order submission, and resolution evaluates what was actually submitted. Do not silently adopt illustrative examples that violate the actual 6×6 board geometry.

## Acceptance gate

Run the Rust workspace tests. The semantic tests above should pass without any model or token protocol implementation. Later, parser round trips and malformed-input tests can run in the protocol crate. Keep fixtures and assertions small enough that a failed test identifies one rule rather than a whole scripted conversation.
