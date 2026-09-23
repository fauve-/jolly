# Tiny Diplomacy v0 Token, State, and Press Protocol

**Status:** Draft  
**Protocol version:** `tiny-diplomacy/v0-press`  
**Target maximum context:** 512 tokens

This document defines the model-visible token vocabulary, canonical game-state serialization, movement and Winter order grammars, and the first structured press protocol for Tiny Diplomacy v0.

The authoritative Tiny Diplomacy v0 game engine remains responsible for board rules, action legality, adjudication, supply-center ownership, and victory conditions. The press subsystem is authoritative for message visibility, outstanding proposals, active commitments, and commitment fulfillment diagnostics.

Legal-token masking constrains both order generation and structured press generation to syntactically and semantically valid continuations.

The design goal of the press language is deliberately narrow:

> Represent observable communicative acts and game-grounded propositions precisely, while leaving concepts such as trust, alliance, betrayal, sincerity, and deception to emerge from agent behavior.

---

## 1. Vocabulary

The original no-press protocol contains 68 tokens. Structured press adds nine tokens:

```text
PRESS
PUBLIC
PRIVATE
PROPOSE
PROMISE
REQUEST
ACCEPT
REJECT
AVOID
```

The resulting protocol contains **77 model-visible tokens**.

### 1.1 Control tokens — 7

```text
BOS
BOARD
CENTERS
ORDERS
ADJUST
PRESS
END
```

- `BOS` begins every model-visible decision or press-generation sequence.
- `BOARD` begins the dense army-occupancy section.
- `CENTERS` begins the supply-center ownership section.
- `ORDERS` begins Spring or Autumn movement-order generation.
- `ADJUST` begins Winter adjustment generation.
- `PRESS` begins structured press generation.
- `END` terminates the generated response.

`END` remains the sole terminator. There is no separate `EOS` token.

### 1.2 Powers and cell values — 6

```text
GREEN
YELLOW
RED
BLUE
NEUTRAL
EMPTY
```

The four power tokens may represent:

- the acting power;
- board occupancy;
- supply-center ownership;
- press sender or recipient identity;
- the actor named in a press proposition.

`EMPTY` is valid only as a board-cell value.

`NEUTRAL` is valid only as a supply-center owner.

### 1.3 Calendar tokens — 13

```text
SPRING
AUTUMN
WINTER
YEAR_1
YEAR_2
YEAR_3
YEAR_4
YEAR_5
YEAR_6
YEAR_7
YEAR_8
YEAR_9
YEAR_10
```

The initial press protocol refers only to the current upcoming movement phase. It therefore does not introduce additional temporal tokens.

### 1.4 Game-action tokens — 7

```text
HOLD
MOVE
SUPPORT
BUILD
WAIVE
DISBAND
NONE
```

- `HOLD`, `MOVE`, and `SUPPORT` are movement-phase actions.
- `BUILD`, `WAIVE`, `DISBAND`, and `NONE` are Winter adjustment actions.

### 1.5 Press tokens — 8

```text
PUBLIC
PRIVATE
PROPOSE
PROMISE
REQUEST
ACCEPT
REJECT
AVOID
```

`PRESS` is classified as a control token because it marks the start of press generation.

The remaining press tokens have these broad roles:

- `PUBLIC` and `PRIVATE` specify message visibility.
- `PROPOSE`, `PROMISE`, `REQUEST`, `ACCEPT`, and `REJECT` are speech acts.
- `AVOID` expresses a negative, location-grounded commitment.

### 1.6 Location tokens — 36

```text
A1 A2 A3 A4 A5 A6
B1 B2 B3 B4 B5 B6
C1 C2 C3 C4 C5 C6
D1 D2 D3 D4 D5 D6
E1 E2 E3 E4 E5 E6
F1 F2 F3 F4 F5 F6
```

### 1.7 Token count

| Category | Count |
| --- | ---: |
| Control | 7 |
| Powers and cell values | 6 |
| Calendar | 13 |
| Game actions | 7 |
| Press tokens | 8 |
| Locations | 36 |
| **Total** | **77** |

The protocol does not define `EOS`, `PAD`, `UNK`, punctuation, unit-type, natural-language, alliance, trust, betrayal, lie, threat, or friendship tokens.

Whitespace and line breaks may appear in human-readable renderings but are not tokens.

---

## 2. Canonical Game-State Serialization

Every order-generation decision begins with:

```text
BOS <acting-power> <phase> <year>
BOARD
<location> <occupant> ...
CENTERS
<supply-center> <owner> ...
<generation-marker>
```

where:

- `<acting-power>` is `GREEN`, `YELLOW`, `RED`, or `BLUE`;
- `<phase>` is `SPRING`, `AUTUMN`, or `WINTER`;
- `<year>` is `YEAR_1` through `YEAR_10`;
- `<generation-marker>` is `ORDERS` in Spring and Autumn, and `ADJUST` in Winter.

There is no `STATE` token.

### 2.1 Dense board section

The `BOARD` section contains all 36 board locations, including empty ones.

```text
<occupant> ::= GREEN | YELLOW | RED | BLUE | EMPTY
```

Locations are serialized in row-major order:

```text
A1 A2 A3 A4 A5 A6
B1 B2 B3 B4 B5 B6
C1 C2 C3 C4 C5 C6
D1 D2 D3 D4 D5 D6
E1 E2 E3 E4 E5 E6
F1 F2 F3 F4 F5 F6
```

The board section therefore always contains 73 tokens:

```text
BOARD + 36 location/value pairs
```

### 2.2 Supply-center section

Only the eight fixed supply centers appear:

```text
A1 B2 B5 C3 C6 D2 E3 E5
```

Each is followed by:

```text
GREEN | YELLOW | RED | BLUE | NEUTRAL
```

The center section therefore always contains 17 tokens.

### 2.3 Base prompt length

The canonical no-history game-state prefix through `ORDERS` or `ADJUST` remains **95 tokens**.

Press context, commitments, proposals, and selected history are additional context supplied around this canonical state rather than changes to the board encoding itself.

---

## 3. Movement-Order Grammar

During Spring and Autumn the acting power generates exactly one order for each of its armies, followed by `END`.

```ebnf
movement_response = { movement_order }, "END" ;

movement_order =
    location, "HOLD"
  | location, "MOVE", location
  | location, "SUPPORT", location, "HOLD"
  | location, "SUPPORT", location, "MOVE", location ;
```

Semantic constraints remain unchanged from the no-press protocol:

- each source must contain an army belonging to the acting power;
- every acting-power army receives exactly one order;
- orders are emitted in ascending source-location order;
- movement and support geometry must be legal;
- `END` is valid only once every required order has been emitted.

Press commitments do **not** constrain legal order generation.

An agent is always mechanically permitted to break a promise.

This is intentional.

---

## 4. Winter Adjustment Grammar

Winter behavior remains unchanged.

### 4.1 No adjustment

```text
NONE END
```

### 4.2 Build

```text
BUILD <home-center> END
```

or:

```text
WAIVE END
```

### 4.3 Mandatory disband

```text
DISBAND <location>
...
END
```

Exactly `abs(delta)` armies must be disbanded when:

```text
delta = supply centers owned - armies owned < 0
```

Structured press v0 does not apply to Winter adjustments.

---

## 5. Press Model

Structured press occurs before a Spring or Autumn movement phase.

A press message is not a game-board action.

Instead, it modifies or records **negotiation state**.

Negotiation state may contain:

- visible messages;
- outstanding proposals;
- active commitments;
- commitment ownership;
- proposal sender and recipient;
- public/private visibility metadata;
- post-phase fulfillment diagnostics.

The press system does not force agents to honor commitments.

### 5.1 Press generation prefix

A press-generation inference uses the current board state and acting power, but terminates its environment-supplied prefix with `PRESS` instead of `ORDERS`.

Conceptually:

```text
BOS RED SPRING YEAR_3
BOARD ...
CENTERS ...
... visible diplomatic context ...
PRESS
```

The model then generates either:

```text
END
```

to say nothing, or one structured press message followed by `END`.

The exact serialization of diplomatic history and active commitments is intentionally left as a separate context-formatting layer. The semantic forms defined in this document are authoritative.

---

## 6. Message Visibility

Every non-empty press message names one recipient and one visibility mode.

### 6.1 Private press

```text
PRIVATE BLUE ...
```

A private message from RED to BLUE is visible only to:

- RED;
- BLUE.

GREEN and YELLOW receive no indication that the message occurred.

### 6.2 Public press

```text
PUBLIC BLUE ...
```

The message is addressed to BLUE but visible to all powers.

This intentionally distinguishes:

- **recipient** — who is being spoken to;
- **visibility** — who can observe the message.

### 6.3 Sender identity

The sender is implicit in the `BOS <acting-power>` prefix.

There is no `FROM` token.

### 6.4 Recipient constraints

The recipient must be a power other than the sender.

Initial press v0 does not support multi-recipient private messages.

---

## 7. Grounded Press Propositions

Press content refers to the upcoming movement phase.

A positive action proposition is:

```text
<power> <movement-order>
```

Examples:

```text
RED B2 MOVE C2
BLUE B3 SUPPORT B2 MOVE C2
GREEN A1 HOLD
```

The proposition means:

> The named power will issue the named order in the upcoming movement phase.

The proposition refers to the **submitted order**, not to whether the order eventually succeeds.

Thus:

```text
RED B2 MOVE C2
```

is fulfilled if RED actually submits `B2 MOVE C2`, even if that move later bounces.

---

## 8. AVOID

`AVOID` introduces a negative location-grounded proposition.

```text
<power> AVOID <location>
```

Example:

```text
RED AVOID C3
```

means:

> During the upcoming movement phase, RED will issue no order that attempts to enter C3 or helps another unit enter C3.

RED violates `AVOID C3` if any RED unit submits either:

```text
<source> MOVE C3
```

or:

```text
<source> SUPPORT <unit> MOVE C3
```

The outcome of that move or support does not matter.

### 8.1 What AVOID does not prohibit

`AVOID C3` does not prohibit:

```text
C3 HOLD
```

or:

```text
<source> SUPPORT C3 HOLD
```

because neither action attempts to enter C3 or assist entry into C3.

### 8.2 Occupied locations

An `AVOID <location>` proposition is valid only if the committing power does not currently occupy that location.

This keeps `AVOID` semantically equivalent to a non-entry/non-interference commitment rather than an evacuation order.

### 8.3 Bilateral demilitarized zones

No `DMZ` token is required.

A bilateral DMZ is represented compositionally:

```text
RED AVOID C3
BLUE AVOID C3
```

---

## 9. REQUEST

`REQUEST` asks the recipient to make one or more commitments but creates no commitment by itself.

Example:

```text
PRIVATE BLUE
REQUEST
BLUE B3 SUPPORT B2 MOVE C2
END
```

Meaning:

> RED asks BLUE to submit `B3 SUPPORT B2 MOVE C2`.

Another example:

```text
PRIVATE BLUE
REQUEST
BLUE AVOID C3
END
```

Meaning:

> RED asks BLUE not to enter or support entry into C3.

### 9.1 REQUEST constraints

All requested propositions must name the recipient as their actor.

A request may contain multiple non-conflicting propositions.

No unit may be assigned contradictory requested orders.

A request does not create an outstanding proposal and does not require `ACCEPT` or `REJECT`.

---

## 10. PROMISE

`PROMISE` creates a unilateral commitment by the sender immediately upon successful message parsing.

Example:

```text
PRIVATE BLUE
PROMISE
RED B2 MOVE C2
END
```

The environment records an active RED commitment to issue that order.

Another example:

```text
PUBLIC BLUE
PROMISE
RED AVOID C3
END
```

The commitment is addressed to BLUE but publicly observable.

### 10.1 PROMISE constraints

Every proposition in a `PROMISE` must name the sender as its actor.

A promise may contain multiple non-conflicting commitments.

A unilateral promise does not require acceptance.

---

## 11. PROPOSE

`PROPOSE` creates a pending bundle of commitments involving the sender and recipient.

Example:

```text
PRIVATE BLUE
PROPOSE
RED B2 MOVE C2
BLUE B3 SUPPORT B2 MOVE C2
END
```

This means:

> RED proposes that RED commit to moving B2 to C2 and BLUE commit to supporting that move.

No commitment exists merely because the proposal was sent.

Another example:

```text
PRIVATE BLUE
PROPOSE
RED AVOID C3
BLUE AVOID C3
END
```

This proposes a bilateral non-entry agreement for C3.

### 11.1 PROPOSE constraints

Every proposal term must name either:

- the sender; or
- the recipient.

No third party may be committed by a proposal to which it is not a participant.

A proposal may contain multiple terms.

No single army may be assigned two contradictory movement orders within one proposal.

Semantically contradictory terms must be rejected by the constrained decoder or parser where they can be determined mechanically.

### 11.2 Outstanding-proposal rule

For each ordered pair of powers, there may be at most one outstanding proposal:

```text
sender -> recipient
```

A new proposal from the same sender to the same recipient supersedes the earlier unaccepted proposal.

Example:

```text
RED -> BLUE proposal A
RED -> BLUE proposal B
```

After proposal B is delivered, proposal A is no longer outstanding.

The opposite direction is independent:

```text
BLUE -> RED proposal C
```

may coexist with RED's outstanding proposal to BLUE.

This rule avoids proposal-ID tokens in press v0.

---

## 12. ACCEPT

`ACCEPT` accepts the currently outstanding proposal sent by the recipient of the acceptance message to the accepting power.

Example sequence:

```text
RED -> BLUE:
PRIVATE BLUE
PROPOSE
RED B2 MOVE C2
BLUE B3 SUPPORT B2 MOVE C2
END
```

BLUE may respond:

```text
PRIVATE RED
ACCEPT
END
```

The environment then converts every term in RED's outstanding proposal to BLUE into an active commitment for the relevant power.

### 12.1 ACCEPT constraints

`ACCEPT` is legal only if an outstanding proposal exists from the addressed recipient to the acting power.

Its visibility mode need not match the visibility mode of the original proposal unless later experiments choose to impose that restriction.

The acceptance itself has its own visibility.

Thus a power may, in principle, publicly accept a privately received proposal. That reveals the existence of the agreement.

---

## 13. REJECT

`REJECT` rejects the currently outstanding proposal sent by the recipient of the rejection message to the rejecting power.

Example:

```text
PRIVATE RED
REJECT
END
```

The referenced proposal is removed from outstanding negotiation state and creates no commitments.

`REJECT` is legal only when such an outstanding proposal exists.

---

## 14. Commitment Semantics

A commitment records:

- committing power;
- committed proposition;
- movement phase to which it applies;
- originating speech act;
- counterparty or recipient;
- visibility;
- fulfillment status after orders are submitted.

Commitments expire after the movement phase to which they refer.

### 14.1 Fulfilled positive action commitment

A positive action commitment is fulfilled when the power submits exactly the committed order.

Whether that order succeeds is irrelevant.

### 14.2 Violated positive action commitment

A positive action commitment is violated when:

- the power submits a different order for the referenced unit; or
- the committed order is no longer submitted for any other reason.

### 14.3 Fulfilled AVOID commitment

`<power> AVOID <location>` is fulfilled if that power submits neither:

```text
MOVE <location>
```

nor:

```text
SUPPORT <unit> MOVE <location>
```

during the relevant phase.

### 14.4 Violated AVOID commitment

The commitment is violated if at least one such order is submitted.

### 14.5 Commitment legality does not constrain orders

Active commitments are diagnostic state, not game rules.

The order decoder must not mask out actions merely because they violate a commitment.

This is essential for studying:

- promise keeping;
- promise breaking;
- strategic defection;
- reputation;
- misleading communication.

---

## 15. No Built-In Psychological Semantics

The protocol intentionally does not define tokens such as:

```text
ALLY
FRIEND
ENEMY
TRUST
BETRAY
LIE
DECEIVE
THREAT
COOPERATE
```

These are behavioral interpretations, not primitive game-grounded acts.

The environment may measure objective facts such as:

- a promise was made;
- a proposal was accepted;
- a commitment was fulfilled;
- a commitment was violated;
- two powers repeatedly supported each other;
- a public and private statement differed.

It should not automatically conclude from those facts that an agent:

- trusted another;
- formed an alliance;
- betrayed another;
- lied.

Those higher-level concepts should remain subjects of later analysis.

---

## 16. Press Grammar

### 16.1 Complete press response

```ebnf
press_response =
    "END"
  | visibility, recipient, speech_act, "END" ;

visibility =
    "PUBLIC"
  | "PRIVATE" ;

recipient =
    power ;
```

State-dependent masking prohibits selecting the acting power as its own recipient.

### 16.2 Speech acts

```ebnf
speech_act =
    "ACCEPT"
  | "REJECT"
  | "PROMISE", commitment, { commitment }
  | "REQUEST", commitment, { commitment }
  | "PROPOSE", commitment, { commitment } ;
```

The context-free grammar intentionally permits a general `commitment`. State-dependent semantic constraints restrict which powers may appear under each speech act.

### 16.3 Commitments

```ebnf
commitment =
    positive_commitment
  | avoidance_commitment ;

positive_commitment =
    power, movement_order ;

avoidance_commitment =
    power, "AVOID", location ;
```

### 16.4 Movement order embedded in press

```ebnf
movement_order =
    location, "HOLD"
  | location, "MOVE", location
  | location, "SUPPORT", location, "HOLD"
  | location, "SUPPORT", location, "MOVE", location ;
```

### 16.5 Powers

```ebnf
power =
    "GREEN"
  | "YELLOW"
  | "RED"
  | "BLUE" ;
```

### 16.6 Locations

```ebnf
location =
    "A1" | "A2" | "A3" | "A4" | "A5" | "A6"
  | "B1" | "B2" | "B3" | "B4" | "B5" | "B6"
  | "C1" | "C2" | "C3" | "C4" | "C5" | "C6"
  | "D1" | "D2" | "D3" | "D4" | "D5" | "D6"
  | "E1" | "E2" | "E3" | "E4" | "E5" | "E6"
  | "F1" | "F2" | "F3" | "F4" | "F5" | "F6" ;
```

---

## 17. Press Semantic Constraints

The EBNF defines structural validity only.

The constrained decoder must also enforce:

### 17.1 General constraints

- recipient is not the sender;
- press occurs only for Spring or Autumn movement phases;
- all referenced units and locations are meaningful in the current game state;
- embedded movement orders must describe actions that are meaningful under current geometry;
- contradictory terms within one message are not permitted;
- `AVOID <location>` is valid only when the named actor does not currently occupy that location.

### 17.2 PROMISE

- every commitment actor must equal the sender.

### 17.3 REQUEST

- every commitment actor must equal the recipient.

### 17.4 PROPOSE

- every commitment actor must be either sender or recipient.

### 17.5 ACCEPT and REJECT

- an outstanding proposal from recipient to sender must exist.

### 17.6 Order legality versus future possibility

Press propositions describe intended orders for the upcoming phase.

The initial implementation should prefer propositions that correspond to orders legal in the current state.

However, press legality and final order legality are conceptually distinct. If later negotiation is allowed before all state-changing events are fixed, this rule may need revision.

---

## 18. Negotiation State and Context

The model itself has no persistent memory across inference calls.

Therefore, the environment must preserve negotiation state between calls.

At minimum it should retain:

- outstanding proposals;
- active commitments;
- relevant public messages;
- private messages visible to the acting power;
- prior fulfillment or violation records if the experiment exposes history.

The model should not be required to reconstruct still-active commitments solely from an arbitrarily long transcript.

### 18.1 Visibility filtering

When constructing context for a power:

- all public press may be included;
- private press sent by that power may be included;
- private press received by that power may be included;
- private press between other powers must not be included;
- by default, there should be no marker revealing that hidden press occurred.

This introduces communication-level partial observability while leaving the board fully observable.

### 18.2 Current diplomatic state versus history

Implementations should distinguish:

- **active negotiation state** — outstanding proposals and current commitments;
- **historical press** — previous messages;
- **behavior history** — whether prior commitments were fulfilled or violated.

These may be serialized separately so context-memory experiments can vary them independently.

---

## 19. Context-Length Policy

The original no-press protocol targeted a 256-token context.

Structured press increases the target maximum to approximately **512 tokens**.

This is a design target rather than a permanent architectural requirement.

The base board and center state consumes 95 tokens before generated orders. Remaining context may be allocated among:

- active commitments;
- outstanding proposals;
- recent visible press;
- prior fulfillment/violation history;
- generated press or movement orders.

The implementation should measure actual serialized lengths before fixing the model's final context size.

Later experiments may compare 128, 256, 512, and 1024-token contexts.

---

## 20. Constrained Decoding Boundary

The environment supplies authoritative context.

The model generates either:

- movement orders after `ORDERS`;
- Winter adjustments after `ADJUST`;
- a structured press message after `PRESS`.

At each generation step, the environment computes:

```text
valid_next_tokens(
    game_state,
    negotiation_state,
    acting_power,
    generation_mode,
    generated_prefix
) -> token mask
```

The mask enforces:

- context-free grammar;
- current game-state constraints;
- current negotiation-state constraints;
- sender/recipient restrictions;
- speech-act semantics;
- proposal existence for acceptance/rejection;
- syntactic non-contradiction where mechanically determinable.

The mask must **not** enforce commitment fulfillment during later order generation.

---

## 21. Canonicalization Requirements

The game-state rules from the no-press protocol remain unchanged:

1. board locations appear in fixed row-major order;
2. supply centers appear in fixed canonical order;
3. movement orders are sorted by source location;
4. disbands are sorted by location;
5. human-readable whitespace is irrelevant;
6. equivalent game-order sets serialize identically.

Structured press adds:

7. every parsed press message has one canonical semantic representation;
8. commitment terms within a message should be serialized in a deterministic order;
9. outstanding proposals are stored canonically by `(sender, recipient)`;
10. active commitments are serialized deterministically;
11. public/private visibility metadata is preserved exactly;
12. hidden private messages are omitted entirely from unauthorized contexts.

---

## 22. Press Turn Structure

Structured press is **event-driven**.

The environment does not run a fixed number of conversational rounds. Instead, press continues while agents have unread messages that can trigger additional press inferences, subject to environment-configured communication limits.

The press turn structure is deliberately separate from the token vocabulary and semantic grammar.

### 22.1 Environment configuration

The environment defines a configuration value such as:

```text
max_press_messages_per_power = 4
```

This value:

- is not a model-visible token;
- is not encoded into the protocol vocabulary;
- may vary between training or evaluation experiments;
- limits the number of actual messages each power may send during one movement phase.

A value of `0` produces a No-Press control condition without changing the protocol.

The initial recommended value is:

```text
max_press_messages_per_power = 4
```

This is a starting experimental parameter, not a permanent game rule.

### 22.2 Opening press opportunity

At the beginning of every Spring or Autumn press phase, each power receives one opening press inference even if its inbox is empty.

This guarantees that every power has an opportunity to initiate negotiation.

An opening inference may generate:

- one structured press message; or
- `END`, meaning that the power sends no message.

Only actual messages count against the sender's configured message budget.

An `END` response does not consume message budget.

### 22.3 Inbox

Each power has an inbox containing **unread messages that have not yet triggered a press inference**.

The inbox is not the complete diplomatic transcript.

Older messages may remain available through the model's visible press history or negotiation-state serialization, but they do not continue to trigger new press turns merely because they exist.

When one or more unread messages are delivered to a power, that power becomes eligible for another press inference.

### 22.4 Processing multiple unread messages

A power may have multiple unread messages when it is activated.

For example:

```text
RED -> BLUE message A
GREEN -> BLUE message B
```

BLUE may then receive a context containing both messages.

The environment performs **one press inference** for BLUE, not necessarily one inference per unread message.

After that inference completes:

- all messages that were present in BLUE's inbox for that activation are marked read;
- BLUE may respond with one structured message or with `END`;
- BLUE is not required to answer every received message individually.

Receiving a message therefore creates an **opportunity to act**, not an obligation to reply.

### 22.5 Message delivery

A generated message is delivered to its named recipient.

For private press:

```text
PRIVATE BLUE ...
```

the message:

- enters BLUE's inbox;
- remains visible only to sender and recipient.

For public press:

```text
PUBLIC BLUE ...
```

the message:

- enters BLUE's inbox;
- is visible to all powers;
- does **not** enter GREEN's or YELLOW's inbox merely because they can observe it.

Only the addressed recipient receives an immediate inbox-triggered press opportunity.

This prevents a single public message from automatically creating multiple new press inferences.

Other powers may react to that public message later if some independent event gives them another press opportunity.

### 22.6 Message budget

Each power may send at most:

```text
max_press_messages_per_power
```

actual messages during one Spring or Autumn press phase.

The budget is tracked independently per power.

For example, with:

```text
max_press_messages_per_power = 4
```

the theoretical maximum is 16 sent messages in a four-power movement phase.

A per-power budget is preferred over a single global message cap because it prevents two powers from consuming the entire communication allowance before the others can participate.

### 22.7 Exhausted send budget

A power whose send budget is exhausted may still:

- receive messages;
- have unread messages delivered to its inbox;
- observe newly visible public messages;
- receive a final press inference triggered by its unread inbox.

However, once the power has exhausted its send budget, its valid press output is constrained to:

```text
END
```

This allows messages to be consumed and marked read without permitting the power to exceed its communication allowance.

### 22.8 Repetition is allowed

The environment does not prohibit an agent from repeating a previously rejected request or proposal.

For example:

```text
RED -> BLUE: PROPOSE X
BLUE -> RED: REJECT
RED -> BLUE: PROPOSE X
BLUE -> RED: REJECT
```

is mechanically valid until one of the participants exhausts its configured send budget or chooses to stop.

The protocol does not attempt to classify repeated negotiation as irrational, annoying, or redundant.

Repeated messages are behavior that training and evaluation may measure.

The configured message budget converts otherwise unbounded conversational loops into a bounded strategic cost.

### 22.9 Press termination

The press phase ends when all of the following are true:

1. every power has received its opening press opportunity;
2. every power's inbox is empty;
3. there are no unread messages remaining that could trigger another press inference.

The phase may also terminate naturally because every power has exhausted its send budget and all pending inboxes have been consumed.

Once press terminates:

1. no additional press messages may be sent for that movement phase;
2. outstanding accepted commitments remain active;
3. outstanding unaccepted proposals remain recorded only according to the environment's proposal-expiration policy;
4. the environment begins movement-order generation;
5. commitments do not restrict legal movement actions;
6. after orders are submitted and adjudicated, commitment fulfillment or violation is classified.

For Press v0, outstanding unaccepted proposals should expire when the press phase ends.

### 22.10 Derived computational bound

The semantic press rules do not require a separate model-visible inference limit.

Because:

- every power receives exactly one opening opportunity;
- every additional press activation is caused by unread messages;
- every sent message is charged against a finite per-power budget;

the number of productive press exchanges is already bounded by the environment configuration.

An implementation may still enforce an internal defensive maximum on press inference count to guard against software bugs. Such a guard is an implementation safety check, not part of the game or protocol semantics.

### 22.11 Suggested initial event loop

Conceptually:

```text
initialize each inbox as empty
initialize sent_count[power] = 0

for each power:
    run opening press inference
    if a message is generated:
        deliver it
        sent_count[power] += 1

while any inbox contains unread messages:
    choose an eligible power
    collect that power's unread inbox messages

    run one press inference using:
        current game state
        current negotiation state
        visible press history
        unread inbox messages

    mark those inbox messages read

    if sent_count[power] < max_press_messages_per_power:
        if a message is generated:
            deliver it
            sent_count[power] += 1
    else:
        only END is permitted

end press phase
begin movement-order generation
```

The order used to select among simultaneously eligible powers must be deterministic or seeded and recorded as part of the experiment configuration.

### 22.12 Communication bandwidth as an experimental variable

Because the message limit belongs to environment configuration rather than the protocol, experiments may compare conditions such as:

```text
0 messages per power
1 message per power
2 messages per power
4 messages per power
8 messages per power
```

without changing the vocabulary or retraining the tokenizer.

This provides a clean experimental variable for studying the effect of communication bandwidth on:

- coordination;
- bargaining;
- promise formation;
- strategic defection;
- reputation;
- coalition behavior;
- game performance.

## 23. Telemetry for Press Experiments

In addition to existing game telemetry, record:

- messages sent;
- public versus private messages;
- requests sent and received;
- promises made;
- proposals made;
- proposals accepted;
- proposals rejected;
- proposals superseded;
- active commitments;
- commitments fulfilled;
- commitments violated;
- `AVOID` commitments made;
- `AVOID` commitments fulfilled or violated;
- support promised versus support issued;
- move promised versus move issued;
- performance following fulfilled agreements;
- performance following broken agreements;
- amount of visible press context per inference;
- probability mass assigned to legal press tokens before masking;
- messages sent per power per movement phase;
- press inferences per power per movement phase;
- repeated requests or proposals;
- unused message budget at press termination;
- inbox depth at each press activation.

These metrics are observational and do not themselves define RL reward.

---

## 24. Explicitly Out of Scope for Press v0

The initial structured press language does not include:

- unrestricted natural language;
- explicit alliance objects;
- trust scores;
- lies or deception labels;
- threats;
- conditional `IF/THEN` agreements;
- OR clauses;
- multi-phase commitments;
- commitments extending across game years;
- Winter negotiation;
- multi-recipient private messages;
- proposal IDs;
- explicit cancellation or withdrawal tokens;
- explicit DMZ tokens;
- beliefs about hidden state;
- claims about what another power intends;
- natural-language translation;
- model-controlled changes to the press message budget;
- unbounded press phases.

These may be added only when a specific experiment requires them.

---

## 25. Example Press Exchanges

### 25.1 Simple request

RED asks BLUE for support:

```text
PRIVATE BLUE
REQUEST
BLUE B3 SUPPORT B2 MOVE C2
END
```

No commitment is created.

### 25.2 Unilateral promise

RED promises BLUE not to contest C3:

```text
PRIVATE BLUE
PROMISE
RED AVOID C3
END
```

The commitment exists immediately.

### 25.3 Bilateral agreement

RED proposes:

```text
PRIVATE BLUE
PROPOSE
RED B2 MOVE C2
BLUE B3 SUPPORT B2 MOVE C2
END
```

BLUE accepts:

```text
PRIVATE RED
ACCEPT
END
```

Both commitments become active.

### 25.4 Public promise

RED publicly promises BLUE not to enter C3:

```text
PUBLIC BLUE
PROMISE
RED AVOID C3
END
```

All four powers may observe the commitment.

### 25.5 Bilateral non-entry agreement

RED proposes:

```text
PRIVATE BLUE
PROPOSE
RED AVOID C3
BLUE AVOID C3
END
```

BLUE accepts:

```text
PRIVATE RED
ACCEPT
END
```

Both powers now have active `AVOID C3` commitments.

### 25.6 Promise breaking

RED has previously promised:

```text
RED AVOID C3
```

but submits:

```text
B3 MOVE C3
```

The move may succeed or fail.

In either case, the commitment is classified as violated because RED attempted to enter C3.

---

## 26. Design Principle

The game engine defines what actions **do**.

The press protocol defines what communicative acts **mean operationally**.

The environment can therefore establish objective facts such as:

```text
proposal made
proposal accepted
commitment created
order submitted
commitment fulfilled
commitment violated
```

while deliberately avoiding claims about internal motive.

This creates a small, machine-checkable language in which cooperation, reputation, strategic defection, coalition behavior, and potentially deceptive behavior can emerge without those concepts being directly encoded as tokens.
