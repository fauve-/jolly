# Tiny Diplomacy Protocol Crate: Baseline Requirements

- **Status:** Implementation baseline
- **Date:** 2026-10-02
- **Language protocol:** `tiny-diplomacy/v0-press`
- **Scope:** The model-facing boundary between the game/press engines and a future neural policy.

## 1. Intention

The protocol crate turns authoritative game and negotiation state into a language the model can consume, and turns the model's generated language back into typed actions the engines can execute.

The model sees token IDs and produces logits over token IDs. It does not manipulate Rust game objects directly. The protocol provides the meaning of those integers, the structure of a response, and the constraints on its continuation.

This boundary lets us test the environment before introducing a transformer. A scripted or random token generator should be able to exercise exactly the same interface as the eventual model.

The central guarantee is:

> For a fixed valid decision snapshot and a sufficient response budget, every response completed by following the protocol's allowed-token masks must parse successfully and produce a semantically legal action for that snapshot.

This guarantee concerns legal submission, not strategic quality or adjudication success. A legal move may bounce; legal support may be ineffective; a legal order may break a promise.

## 2. Authority and dependencies

| Component | Owns |
| --- | --- |
| `game_engine` | Board state, movement legality, Winter eligibility, simultaneous adjudication, calendar, ownership, terminal outcomes |
| `press_engine` | Activations, message budgets, visibility, proposal lifecycle, commitments, fulfillment diagnostics |
| `protocol` | Token identities, canonical encodings, response parsing, incremental decoding, model-context construction |
| Future model/training code | Logits, sampling, learned weights, optimization, trajectories and reward |

`protocol` depends on both engines. `press_engine` depends on `game_engine`. Neither engine should depend on the protocol or model.

Protocol code must reuse authoritative engine legality and validation wherever available. If the engine lacks a suitable read-only validator, expose one rather than maintain a second independent rules implementation.

Parsing and mask calculation are read-only. The caller submits a completed typed action to the appropriate engine separately. Parsing a promise does not itself create a commitment.

This document complements the existing token/state/press specification and implementation roadmap. It does not change their game rules or press semantics. Negotiation remains outside the core game engine; the structured-press specification adds it as a separate environment layer.

## 3. Deliverables

The baseline implementation must provide:

1. A stable vocabulary and explicit token-ID mapping.
2. Deterministic encoding of the game observation and generated actions.
3. Strict parsing and state-dependent validation of complete responses.
4. Incremental decoding and an allowed-token mask for every generation mode.
5. A visibility-safe interface for diplomatic context.
6. Structured errors, useful diagnostics, and tests of the boundary invariants.

Candle, a transformer, training algorithms, rewards, and natural-language translation are outside this crate's baseline scope.

## 4. Vocabulary and compatibility

The vocabulary contains **84 model-visible tokens**:

| Category | Count | Contents |
| --- | ---: | --- |
| Control | 12 | `BOS BOARD CENTERS ORDERS ADJUST PRESS DIPLOMACY INBOX PROPOSALS COMMITMENTS HISTORY END` |
| Powers and cell values | 6 | `GREEN YELLOW RED BLUE NEUTRAL EMPTY` |
| Calendar | 13 | `SPRING AUTUMN WINTER`, `YEAR_1` through `YEAR_10` |
| Game actions | 7 | `HOLD MOVE SUPPORT BUILD WAIVE DISBAND NONE` |
| Press/status | 10 | `PUBLIC PRIVATE PROPOSE PROMISE REQUEST ACCEPT REJECT AVOID FULFILLED VIOLATED` |
| Locations | 36 | `A1` through `F6`, in row-major order |

Every symbolic token maps to exactly one integer ID. The frozen assignment is defined in Section 1.8 of `../../docs/protocol.md`: controls are IDs 0–11, powers/cell values 12–17, phases 18–20, years 21–30, game actions 31–37, press/status tokens 38–47, and row-major locations 48–83. Code must implement that table explicitly rather than derive persistence from Rust enum declaration order.

A `u8` can hold this vocabulary, but the public model adapter may use another integer width. Record the protocol version and token-table fingerprint with persisted datasets and checkpoints. Incompatible mappings must be rejected rather than silently interpreted.

There is no `EOS`, `PAD`, `UNK`, punctuation, or natural-language token. Whitespace in debug text is formatting only. If batching needs padding, handle it outside the semantic vocabulary with explicit lengths/masks unless a later protocol revision introduces a token.

Unknown integer IDs and unknown textual spellings produce errors.

## 5. Decision snapshots and generation modes

A decoding session is bound to an immutable snapshot containing the acting power, game phase/year, board and center state, generation mode, and applicable negotiation state or activation metadata.

| Mode | Valid phase | Generated response |
| --- | --- | --- |
| Movement | Spring or Autumn | One order per owned army, then `END` |
| Adjustment | Winter | The required adjustment form, then `END` |
| Press | Before Spring or Autumn movement | Silence (`END`) or one message followed by `END` |

Reject incompatible modes and terminal-game decisions explicitly. Press sessions must correspond to an eligible activation under the press engine's rules.

Other powers' unsubmitted or privately submitted movement orders must not enter a movement decision's context. Simultaneous decisions use the same pre-adjudication board snapshot. Actual other-power orders must not be required to decide whether support can be submitted legally.

If authoritative state changes before submission, the caller must revalidate or reject the stale action. The protocol's guarantee applies to its bound snapshot.

## 6. Canonical game observation

The base observation is:

```text
BOS <acting-power> <phase> <year>
BOARD <location> <occupant> ...
CENTERS <center> <owner> ...
<generation-marker>
```

Requirements:

- Emit all 36 board cells in row-major order: `A1 A2 ... A6 B1 ... F6`.
- Board values are a power or `EMPTY`.
- Emit the eight centers in exactly this order: `A1 B2 B5 C3 C6 D2 E3 E5`.
- Center values are a power or `NEUTRAL`.
- End the supplied prompt with `ORDERS`, `ADJUST`, or `PRESS`, as appropriate.

Without diplomatic context, the prefix is **95 tokens**: four header tokens, 73 board tokens, 17 center tokens, and one generation marker. Diagnostic examples may use `...`; actual encodings must never omit cells or introduce an ellipsis token.

Equivalent visible snapshots must serialize identically regardless of hash-map insertion order. The encoder must validate that input state is representable by this protocol version.

## 7. Complete response encoding and parsing

The response parser consumes the generated suffix; the environment-supplied generation marker is not part of that suffix. Debug renderings may show both together, but APIs must make the distinction explicit.

### Movement

```text
<source> HOLD
<source> MOVE <destination>
<source> SUPPORT <supported-source> HOLD
<source> SUPPORT <supported-source> MOVE <destination>
```

Emit exactly one order for each acting-power army, in ascending source-location order, then `END`. With no armies, the response is `END`.

Validate ownership, completeness, uniqueness, and current movement/support geometry. Preserve legal support even when it might contribute no strength. Allow moves into friendly-occupied cells when the engine allows their submission; eventual vacating is an adjudication question.

### Winter

| Situation | Response |
| --- | --- |
| Zero adjustment delta | `NONE END` |
| Positive delta, eligible empty owned home | `BUILD <home> END` or `WAIVE END` |
| Positive delta, no eligible build location | `WAIVE END` |
| Negative delta | Exactly `abs(delta)` distinct `DISBAND <location>` entries, sorted by location, then `END` |

V0 permits at most one build because each power has one original home center. Do not emit one `WAIVE` per unused capacity slot or permit build/disband mixtures.

### Press

```text
END
<visibility> <recipient> ACCEPT END
<visibility> <recipient> REJECT END
<visibility> <recipient> <PROMISE|REQUEST|PROPOSE> <term> ... END
```

A term is either `<power> <movement-order>` or `<power> AVOID <location>`. Term-bearing acts require at least one term. The sender comes from the decision snapshot.

Enforce sender/recipient restrictions, referenced-unit legality, geometry, bundle consistency, occupied-location restrictions on `AVOID`, and the appropriate outstanding proposal for `ACCEPT`/`REJECT`. Reuse press-engine validation for any additional engine preconditions.

`PROMISE` terms name the sender; `REQUEST` terms name the recipient; `PROPOSE` terms name either participant. A request creates no proposal or commitment. A proposal becomes commitments only through acceptance. Proposal direction matters: an acceptance addressed to RED by BLUE refers to the outstanding RED-to-BLUE proposal.

Press terms are sorted lexicographically by their complete token-ID encoding. Exact duplicate terms are rejected rather than collapsed. Serializer, strict parser, mask, and press-engine semantic normalization must share this rule. Do not silently reorder a noncanonical generated suffix in the strict parser.

Complete parsing must reject missing `END`, incomplete operands, invalid IDs, extra tokens after `END`, incorrect mode tokens, and noncanonical ordering. Do not silently repair malformed responses or convert illegal movement to HOLD; that fallback belongs to the game engine's separate robustness path.

## 8. Incremental constrained decoding

At each step, compute which tokens can extend the generated prefix to a complete legal response. Grammar validity alone is insufficient.

The decoder tracks the current operand position plus semantic state: pending source/action, already completed orders, remaining adjustment count, visibility, recipient, speech act, existing bundle terms, and relevant proposal preconditions.

For a hypothetical position with RED at B2:

| Prefix, shown with prompt marker | Decoder knowledge | Possible continuation |
| --- | --- | --- |
| `ORDERS` | Next required source | The first RED army location |
| `ORDERS B2` | Choosing this army's action | `HOLD`, `MOVE`, or a support action with a legal completion |
| `ORDERS B2 MOVE` | Choosing destination | Legal orthogonal neighbors of B2 |
| `ORDERS B2 HOLD` | First order complete | Next required source, or `END` if all armies are covered |

For hypothetical RED at B2 and BLUE at C3, both able to interact with C2:

```text
PRESS PRIVATE BLUE PROPOSE RED B2 MOVE C2
```

The decoder has a complete first proposal term. It may allow `END` if the engine accepts that bundle, or a permitted next actor that can begin another canonical, consistent term. It must prevent a second contradictory B2 order. It must also prevent an `AVOID C2` term for RED if that contradicts RED's proposed move into C2.

Use an incremental grammar/state machine with local engine legality checks. Do not implement the production mask by enumerating every complete legal response and searching for prefixes. Small bounded enumerations are useful as independent test oracles.

Required properties:

- **Soundness:** Every allowed token preserves at least one legal complete continuation.
- **Completeness:** Every continuation allowed by the defined canonical language and decision constraints remains reachable.
- **Termination:** `END` is allowed exactly when the response is complete; after it, the decoder is terminal and accepts no further tokens.
- **No reachable dead ends:** Every nonterminal prefix reached by valid steps has a nonempty mask and a completion within its remaining budget.
- **Determinism:** Identical snapshots and prefixes yield identical masks.
- **Safe failure:** Arbitrary invalid prefixes return a structured error, never a panic or fabricated fallback mask.

If a response limit is enforced, the decoder must reserve enough remaining tokens to finish any allowed branch. It must not cut off an incomplete order or press term and append `END`. Reject budgets that cannot hold any legal response. Press message-count limits and token-response limits are separate controls.

## 9. How the mask interacts with the model

The future inference loop is:

1. Encode the acting power's observation into token IDs.
2. Run the model to obtain logits for the next token.
3. Obtain the protocol mask for the current generated prefix.
4. Set disallowed logits to negative infinity, then sample over allowed tokens.
5. Advance the decoder with the sampled token; repeat until `END`.
6. Parse/validate the completed suffix and submit its typed action to the engine.

The model chooses among legal continuations; the protocol supplies legality. Temperature and sampling are model-layer concerns. Protocol errors must remain distinguishable from numerical/sampler failures.

Retain enough information for training/debugging to measure raw probability mass on illegal tokens before masking. The protocol provides masks and error categories; it does not define an RL objective.

Commitments **must not restrict the movement mask**. For example, a recorded promise to move B2 to C2 cannot remove another legal B2 destination. Fulfillment is assessed by the press engine against raw submitted orders, independently of whether adjudication makes the move succeed.

## 10. Diplomatic context and privacy

The environment preserves negotiation state between inferences. Context construction must consume a power-filtered press view or provide equivalent tested filtering. Include public messages and private messages sent or received by the acting power; omit other powers' private messages entirely, including hidden-message placeholders or global sequence gaps that disclose their occurrence.

Current commitments and outstanding proposals must be distinguishable from historical messages. The model should not need to recover active state from a long transcript. Preserve sender, recipient, visibility, direction, and applicability to the upcoming phase.

When present, the diplomatic block appears after `CENTERS` and before `ORDERS` or `PRESS`:

```text
DIPLOMACY
INBOX <zero or more records>
PROPOSALS <zero or more records>
COMMITMENTS <zero or more records>
HISTORY <zero or more records>
<generation-marker>
```

Each record begins with `BOS`, includes sender or committing power plus phase/year, visibility, counterparty or recipient, and its act/status, and ends with `END`. The exact record grammar is authoritative in Section 18 of `../../docs/protocol.md`. `INBOX` uses delivery order, proposals use `(sender, recipient)` order, commitments use lexicographic token order, and history uses per-observer visible chronology.

Mixed visibility must not leak private agreement terms. A public acceptance of a private proposal reveals the acceptance but not its terms. An accepted proposal's commitment details are public only if both proposal and acceptance were public.

The target total capacity is **512 tokens**, including the supplied prompt and generated suffix. Reserve the complete response budget before encoding context. Inbox records, outstanding proposals, active commitments, and section markers are required; overflow returns an explicit error. Optional history is the longest contiguous suffix of whole eligible records that fits and is emitted chronologically. Never truncate a record or silently lose required state.

## 11. Suggested Rust interface

Names and concrete engine types are illustrative:

```rust
encode_observation(&DecisionView, &ContextPolicy)
    -> Result<Vec<TokenId>, ProtocolError>;

encode_response(&TypedResponse)
    -> Result<Vec<TokenId>, ProtocolError>;

parse_response(&DecisionSnapshot, &[TokenId])
    -> Result<TypedResponse, ProtocolError>;

Decoder::new(&DecisionSnapshot, ResponseBudget)
    -> Result<Decoder, ProtocolError>;

Decoder::valid_next_tokens(&self) -> TokenMask;
Decoder::push(&mut self, TokenId) -> Result<(), ProtocolError>;
Decoder::finish(self) -> Result<TypedResponse, ProtocolError>;
```

Separate generated-response parsing from observation decoding. If observation decoding is exposed, it reconstructs only encoded visible information, not hidden negotiation state or every internal engine field.

Errors should identify token position, generation mode, and a useful cause such as incomplete operand, invalid ownership, illegal geometry, missing proposal, conflicting terms, noncanonical order, insufficient budget, or context overflow. A rejected decoder step should leave the decoder unchanged.

## 12. Acceptance criteria

| Area | Required evidence |
| --- | --- |
| Vocabulary | Exactly 84 unique symbols/IDs; bijective conversion; unknown-ID rejection; frozen mapping fixture |
| Observation | Golden initial-state encodings for each power/mode; fixed cell/center order; 95-token base length |
| Responses | Typed action round trips for every movement form, Winter case, and speech act; semantic equality after documented canonicalization |
| Strictness | Malformed, incomplete, trailing, wrong-mode, duplicate/conflicting, and noncanonical sequences fail clearly |
| Movement legality | Ownership, adjacency, all support forms, zero armies, completeness; legal ineffective support remains reachable |
| Winter legality | Unavailable build, waived capacity, exact sorted disband count, no mixed adjustment forms |
| Press legality | Actor restrictions, directional proposals, acceptance/rejection, `AVOID`, conflicting bundles, eligible activation/budget |
| Mask agreement | Completed masked responses pass strict parsing and authoritative engine validation |
| Reachability | Bounded exhaustive tests of small states/prefixes; randomized reachable states; no legal canonical branches unintentionally removed |
| Promise breaking | Existing commitments leave otherwise legal movement choices available |
| Privacy | Unauthorized private state is absent; hidden-only changes do not alter the unauthorized power's visible encoding |
| Budget and purity | No truncated actions; impossible budgets fail; parse/mask operations do not mutate either engine |
| Determinism | Encodings and masks independent of collection insertion order |

Exhaustive testing is practical for bounded fixtures, not every possible game state and press transcript. Combine independent small-state oracles, regression cases, and property-based generation. Test raw invalid-input behavior separately from constrained generation.

## 13. Implementation order and completion gate

1. Implement the frozen token IDs and symbolic/debug conversion.
2. Encode canonical base observations and movement/Winter responses.
3. Implement strict response parsing with engine validation.
4. Add incremental movement and Winter masks.
5. Implement the press subsystem validators needed by protocol code.
6. Add press response encoding, parsing, and masks.
7. Implement the specified diplomatic-context encoding and budget/privacy policy.
8. Exercise the full boundary with a random masked token generator, without a neural model.

The milestone is complete when a valid decision can be encoded, a response generated incrementally through masks, and that response submitted as the intended typed legal action—with deterministic encoding, visibility preserved, and failures explicit.

The numeric ID table, press-term canonicalization and duplicate policy, diplomatic-context grammar, context-overflow policy, and invalid raw press behavior are defined by the current protocol. Changes to any of them require an explicit protocol revision and reviewed fixture updates. Parser and decoder operations remain read-only; the runner consumes an invalid raw press activation without delivering a message or charging send budget, as specified by the press protocol.

## Source documents

- `../../docs/game-spec.md`: authoritative v0 game rules.
- `../../docs/protocol.md`: token inventory and IDs, grammars, press semantics, diplomatic context, canonicalization, and constrained-decoding boundary.
- `../../docs/roadmap.md`: implementation sequencing and success gates.

This document is based on those specifications and the proposed protocol-crate direction. It does not claim to verify the current repository implementation.
