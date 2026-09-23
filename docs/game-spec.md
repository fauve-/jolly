# Tiny Diplomacy v0 Specification

## 1. Purpose

Tiny Diplomacy v0 is a deliberately simplified, Diplomacy-inspired simultaneous-action strategy game.

Its purpose is to provide the first environment for the Tiny Diplomacy GPT project. The rules should preserve the strategically interesting parts of Diplomacy—simultaneous movement, contested territory, support, positional play, expansion, and asymmetric openings—while avoiding the adjudication complexity of full Diplomacy.

The rules are intended to be simple enough that:

- the game engine can be implemented and tested independently of the model;
- every position has an unambiguous result;
- the model can focus on choosing strategically useful legal actions rather than learning the mechanics of a large ruleset;
- later versions can introduce more of standard Diplomacy incrementally.

The v0 game specification is intentionally allowed to diverge from standard Diplomacy where simpler rules make the environment easier to reason about.

---

## 2. Board

The game is played on a fixed 6×6 grid.

Movement is orthogonal only:

- north;
- south;
- east;
- west.

There are no blocked cells.

Coordinates use rows `A` through `F` and columns `1` through `6`.

The map is:

```text
    1 2 3 4 5 6
A   G x x x x x
B   x S x x R x
C   x x S x x S
D   x Y x x x x
E   x x S x B x
F   x x x x x x
```

Legend:

- `G` — Green home supply center and starting position
- `Y` — Yellow home supply center and starting position
- `R` — Red home supply center and starting position
- `B` — Blue home supply center and starting position
- `S` — neutral supply center
- `x` — ordinary territory

### 2.1 Home supply centers

The four home supply centers are:

- `A1`
- `B5`
- `D2`
- `E5`

Each seat begins with one army on its home supply center.

### 2.2 Neutral supply centers

The four neutral supply centers are:

- `B2`
- `C3`
- `C6`
- `E3`

There are therefore eight supply centers total.

### 2.3 Static geography

The map and supply-center locations are fixed between games.

The asymmetry of the starting positions is intentional. Different seats are expected to have different opening and expansion strategies.

Agents or player identities may be rotated among seats between games so that a policy experiences the game from multiple asymmetric starting positions.

---

## 3. Powers and Units

There are four powers:

- Green
- Yellow
- Red
- Blue

Tiny Diplomacy v0 uses armies only.

There are no fleets.

Each power begins with:

- one home supply center;
- one army occupying that center.

At most one army may occupy a province at the end of a movement phase.

---

## 4. Game Calendar

A game year consists of:

1. Spring
2. Autumn
3. Winter

Spring and Autumn are movement phases.

Winter is an adjustment phase.

The game lasts at most 10 game years, giving a maximum of 20 movement phases.

The maximum sequence is therefore:

```text
Spring 1
Autumn 1
Winter 1
...
Spring 10
Autumn 10
Winter 10
```

If a victory condition is reached after an Autumn ownership update, the game ends immediately and the Winter phase for that year is not played.

---

## 5. Supply Centers and Ownership

Supply-center ownership is persistent.

A power continues to own a supply center after moving away from it until another power captures it.

Supply-center ownership changes only after an Autumn movement phase.

After Autumn resolves:

- if a supply center is occupied by an army, ownership transfers to that army's power;
- if a supply center is unoccupied, its ownership does not change.

Capturing another power's original home center gives normal ownership of that supply center.

However, home-center identity is permanent. A captured home center does not become a build location for the capturing power.

Ordinary non-supply-center territory has no persistent ownership and contributes no score.

---

## 6. Victory and Game Score

A power wins immediately when it controls at least five of the eight supply centers.

Victory is checked:

1. after an Autumn movement phase resolves;
2. after supply-center ownership is updated;
3. before Winter adjustments.

If no power reaches five supply centers by the end of Autumn in game year 10, the game ends.

The game score is simply:

```text
score = number of supply centers owned
```

No score is awarded for:

- ordinary territory;
- battles won;
- units destroyed;
- surviving armies;
- supports;
- movement activity.

If the game reaches the time limit, the power with the most supply centers has the highest game score.

If multiple powers are tied for the highest supply-center count, the result is a draw among those powers.

Training reward is deliberately not specified by this document. Game score and reinforcement-learning reward are separate design questions.

---

## 7. Movement-Phase Orders

During Spring and Autumn, every army receives exactly one order.

The available order types are:

- `HOLD`
- `MOVE`
- `SUPPORT`

All movement-phase orders are submitted before any are resolved.

They then resolve simultaneously.

---

## 8. HOLD

A `HOLD` order instructs the army to remain in its current province.

Example:

```text
A2 HOLD
```

A holding army has base defensive strength 1.

Valid, uncut support-to-hold orders increase its defensive strength.

---

## 9. MOVE

A `MOVE` order attempts to move an army to an orthogonally adjacent province.

Example:

```text
A2 MOVE A3
```

A move either:

- succeeds, in which case the army occupies the destination; or
- fails, in which case the army remains in its original province.

An army never disappears merely because one of its moves fails.

Incoming attacks do not pin an army in place. If its outgoing move succeeds, it vacates its original province regardless of attacks directed at that province.

---

## 10. SUPPORT

Support is attached to a specific intended action.

There are two forms:

```text
A1 SUPPORT B1 HOLD
```

and

```text
A1 SUPPORT B1 MOVE B2
```

Powers may support armies belonging to other powers.

No coordination, alliance, or permission is required.

### 10.1 Support-to-hold validity

`SUPPORT <unit> HOLD` is valid when:

- the named unit exists;
- the supporting army is orthogonally adjacent to the named unit's current province.

The support contributes only if the named unit actually issued `HOLD`.

If the named unit issued another order, the support is legal but ineffective.

### 10.2 Support-to-move validity

`SUPPORT <unit> MOVE <destination>` is valid when:

- the named unit exists;
- the named unit could legally move to the named destination;
- the supporter could legally move to that same destination;
- the named unit actually issued that exact move.

If the named unit issues a different order or moves to a different destination, the support contributes no strength.

### 10.3 Support across powers

Support may be given to another power's army.

For example:

```text
RED A2 MOVE B2
GREEN B3 SUPPORT A2 MOVE B2
```

The Green support contributes normally if it remains uncut.

### 10.4 Legal but ineffective support

A support order may be legal yet contribute no strength.

Examples include:

- supporting HOLD when the named army moved;
- supporting a move that the named army did not issue;
- support that is later cut.

These cases are distinct from an illegal support order.

---

## 11. Support Cutting

A valid support is cut if any army belonging to another power issues a valid `MOVE` into the supporter's current province.

The attack does not need to succeed.

Attack strength, bouncing, and the eventual fate of the attacking army do not affect whether support is cut.

Example:

```text
RED   B2 SUPPORT C2 MOVE C3
BLUE  B3 MOVE B2
```

The attack on `B2` cuts RED's support even if BLUE later fails to enter `B2`.

Friendly movement does not cut support.

There are no standard-Diplomacy exceptions based on the province from which the attack originated.

Support cutting is therefore determined from valid submitted move orders rather than the eventual result of those moves.

---

## 12. Strength

### 12.1 Move strength

A move has strength:

```text
1 + number of valid, uncut supports for that exact move
```

### 12.2 Defensive strength

An army that issued `HOLD` has defensive strength:

```text
1 + number of valid, uncut supports for its HOLD
```

### 12.3 Failed moves

An army whose `MOVE` fails remains in its original province.

It defends that province with strength 1.

A failed move does not retroactively become a HOLD order and therefore does not receive support-to-hold strength.

---

## 13. Contested Destinations

For each destination, compare all incoming moves.

Only a unique strongest incoming move can succeed.

If two or more incoming moves tie for the highest attack strength, all tied moves fail.

Example:

```text
RED    B2 MOVE C2       strength 2
YELLOW C1 MOVE C2       strength 2
```

Both moves fail.

If an incumbent army remains in the destination, the unique strongest incoming move must also strictly exceed the incumbent's defensive strength.

Equal attack and defense results in a failed attack.

Example:

```text
BLUE C2 HOLD
BLUE C3 SUPPORT C2 HOLD

RED  B2 MOVE C2
RED  B3 SUPPORT B2 MOVE C2
```

Attack strength is 2 and defense strength is 2.

The attack fails and BLUE remains in `C2`.

---

## 14. Vacated Provinces

A province whose occupant successfully moves away is treated as empty for incoming movement.

The former occupant contributes no defense to that province.

For example:

```text
BLUE C2 MOVE D2
RED  B2 MOVE C2
```

If BLUE successfully reaches `D2`, then `C2` is empty for purposes of resolving RED's move.

If multiple armies move into the vacated province, they are resolved exactly as they would be for any other empty province.

No special rule exists for "moving defenders."

---

## 15. Friendly Occupancy and Self-Dislodgement

A power may not destroy or dislodge one of its own armies.

An army may issue a move into a province currently occupied by a friendly army, but the move succeeds only if the friendly army successfully vacates that province.

Example:

```text
RED A2 MOVE A3
RED B2 MOVE A2
```

If `A2 MOVE A3` succeeds, `B2 MOVE A2` may succeed.

If `A2 MOVE A3` fails, the army remains at `A2`, and `B2 MOVE A2` also fails.

Superior friendly attack strength does not remove the friendly occupant.

---

## 16. Movement Chains, Swaps, and Cycles

Movement is simultaneous.

Chains of movement may succeed when all relevant moves can resolve successfully.

Example:

```text
A1 MOVE A2
A2 MOVE A3
A3 MOVE A4
```

If each destination can be successfully entered, all three moves succeed.

If the last move is blocked, the resulting failure may propagate backward because the blocked army no longer vacates its province.

### 16.1 Swaps

Direct swaps are allowed.

Example:

```text
A1 MOVE A2
A2 MOVE A1
```

If both moves otherwise succeed, the armies exchange provinces.

### 16.2 Longer cycles

Longer movement cycles are also allowed.

Example:

```text
A1 MOVE A2
A2 MOVE B2
B2 MOVE B1
B1 MOVE A1
```

If every move in the cycle otherwise succeeds, the complete cycle succeeds.

Incoming attack strength does not prevent a unit from leaving its origin if its own move succeeds.

---

## 17. Dislodgement

If an enemy move successfully enters a province whose incumbent army failed to vacate, the incumbent is dislodged.

In Tiny Diplomacy v0, a dislodged army is destroyed immediately.

There is no retreat phase.

Armies are otherwise removed only by mandatory Winter disbanding.

---

## 18. Winter Adjustments

After Autumn supply-center ownership is updated, calculate for each power:

```text
delta = supply centers owned - armies owned
```

The sign of `delta` determines the adjustment type.

### 18.1 No adjustment

If:

```text
delta == 0
```

the power makes no adjustment.

Canonical intent:

```text
<ADJUST>
NONE
<END>
```

### 18.2 Builds

If:

```text
delta > 0
```

the power may build up to `delta` armies, subject to build-location restrictions.

Builds are optional.

A player may waive an available build.

Canonical intent:

```text
<ADJUST>
BUILD A1
<END>
```

or:

```text
<ADJUST>
WAIVE
<END>
```

A build is legal only when the province:

- is the power's original home supply center;
- is currently owned by that power;
- is empty.

A power may never build on a captured enemy home center.

Because each v0 power has only one original home center, at most one army can be built by a power in a single Winter.

Unused build capacity is lost.

### 18.3 Disbands

If:

```text
delta < 0
```

the power must disband exactly `abs(delta)` armies.

Example:

```text
<ADJUST>
DISBAND C3
DISBAND E4
<END>
```

The player chooses which armies to remove.

Disbands are mandatory.

Voluntary extra disbands are not allowed.

Build and disband orders are never mixed in one Winter adjustment.

---

## 19. Illegal Orders

The authoritative game engine must define behavior for illegal orders even though the model-facing agent will normally use legal-action masking.

For movement phases:

> An illegal order is replaced with `HOLD` for that army.

Other armies' orders remain unaffected.

Examples of illegal orders include:

- moving to a non-adjacent province;
- malformed support geometry;
- referring to a nonexistent unit.

Legal-but-ineffective support orders are not converted to HOLD. They remain support orders but contribute no strength.

This distinction should be represented explicitly in engine diagnostics.

---

## 20. Legal-Action Masking

The neural agent will use constrained decoding.

At each generation step, tokens that cannot lead to a legal order sequence are masked out before sampling.

Conceptually:

```text
illegal token logits -> negative infinity
legal token logits   -> unchanged
```

The executed agent therefore chooses only among legal continuations.

The engine's illegal-order fallback still exists for robustness and independent testing.

Raw, unmasked model behavior should be measurable separately so that training can track whether the model learns the game's syntax and legality even though illegal actions are prevented from executing.

---

## 21. Observation Model

Tiny Diplomacy v0 is fully observable.

The complete current game state is passed to the model as context.

The model does not need to query the board using an action or tool.

Board querying, partial observability, information gathering, and tool-like interaction are explicitly deferred to later experiments.

The v0 agent interface is conceptually:

```text
game state -> model context
model output -> game orders
```

---

## 22. Seat Rotation

The geography is fixed, but player or agent assignment to starting seats may vary between games.

This is intended to expose the same learned policy to different asymmetric opening positions.

Evaluation should record performance by seat so that any inherent positional advantages can be measured.

Power identity should not be assumed to imply one permanent board location at the model-training level.

---

## 23. Telemetry

Telemetry is separate from game score and future RL reward.

Useful measurements include:

- supply centers owned;
- armies owned;
- centers captured;
- centers lost;
- battles won;
- battles lost;
- units destroyed;
- units lost;
- supports issued;
- supports received;
- supports cut;
- legal-but-ineffective supports;
- illegal raw model orders;
- probability mass assigned to legal actions before masking;
- game length;
- performance by seat.

These metrics may be used for debugging and analysis but do not affect v0 scoring unless explicitly introduced in a later version.

---

## 24. Explicitly Out of Scope for v0

Tiny Diplomacy v0 does not include:

- fleets;
- coast rules;
- convoys;
- retreats;
- negotiation;
- natural-language communication;
- hidden information;
- fog of war;
- board-query tools;
- persistent ownership of ordinary territory;
- score for battles or territory;
- standard Diplomacy's special support-cutting exceptions;
- standard Diplomacy's full head-to-head movement rules;
- standard Diplomacy's convoy or paradox adjudication;
- a reinforcement-learning reward specification.

These may be introduced deliberately in later versions.

---

## 25. Remaining Implementation Work

The game-design rules are considered sufficiently specified for v0.

The next specification layer should define:

1. authoritative Rust data structures for board state, units, centers, orders, phases, and ownership;
2. the deterministic simultaneous-movement adjudication algorithm;
3. canonical state serialization;
4. canonical order serialization;
5. token inventory and stable token IDs;
6. constrained-decoding state machine;
7. exact seat-randomization procedure;
8. engine test cases for all major adjudication patterns.

Important canonical adjudication tests should include:

- uncontested move;
- equal-strength bounce;
- supported attack;
- supported defense;
- cut support;
- legal-but-ineffective support;
- failed move defending its origin;
- movement into a successfully vacated province;
- blocked friendly movement chain;
- successful movement chain;
- direct swap;
- longer movement cycle;
- three-way contest;
- self-dislodgement prevention;
- dislodgement and destruction;
- Autumn supply-center transfer;
- legal and illegal Winter builds;
- mandatory disband;
- illegal-order fallback to HOLD.

