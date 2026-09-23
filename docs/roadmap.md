# Tiny Diplomacy GPT: Implementation Roadmap

## Project purpose

The purpose of this project is not initially to build a strong or faithful full-Diplomacy AI. It is to learn, from first principles, how a small GPT-style model becomes an agent and how increasingly sophisticated behaviors emerge when that agent is given strategy, memory, communication, and other cognitive machinery.

The first long-lived experimental environment will be **Tiny Diplomacy**, not standard seven-player Diplomacy.

The environment should remain deliberately small while the model and agent architecture become more sophisticated. The project should therefore prioritize experiments such as:

- next-token prediction and autoregressive action generation;
- constrained decoding;
- imitation learning;
- self-play reinforcement learning;
- structured negotiation;
- public and private communication;
- commitments and reputation;
- context and memory;
- policy populations and behavioral variation;
- LoRA-based strategic or communicative variation;
- multiple cooperating models;
- explicit planning and search.

Graduating to standard Diplomacy is a **far-future branch**, not the next major milestone after learning competent No-Press play.

The central design rule remains:

> Introduce only one major source of complexity at a time, and require a measurable success condition before adding the next one.

This keeps failures interpretable.

If a model cannot overfit a tiny deterministic sequence, the problem is in the model or training loop.

If it imitates legal orders but fails in self-play, the problem is in the reinforcement-learning layer.

If it plays No-Press Tiny Diplomacy well but cannot use promises, the problem is in communication, context, or training rather than basic adjudication.

If it can use structured press but cannot maintain reputation across phases, the problem is likely in memory or context representation.

The goal is not to hide these boundaries behind a large end-to-end model. The goal is to expose them so they can be studied.

---

## Current environment

The authoritative board-game rules are defined in **Tiny Diplomacy v0 Specification**.

The authoritative model-facing representation is defined in **Tiny Diplomacy v0 Token, State, and Press Protocol**.

The core game remains deliberately small:

- Four powers on a fixed 6×6 orthogonal grid
- Eight fixed supply centers: four asymmetric home centers and four neutral centers
- One starting army per power
- Armies only
- Spring and Autumn simultaneous movement phases, followed by Winter adjustments
- `HOLD`, `MOVE`, and cross-power `SUPPORT`
- Simple support-cutting and strength rules rather than full Diplomacy adjudication
- Builds and mandatory disbands based on supply-center ownership
- Builds only on a power's original, currently owned, empty home center
- Dislodged armies are destroyed
- No fleets, convoys, or retreats
- Victory at five supply centers
- Maximum game length of 10 game years
- Complete board state supplied directly to the model
- Legal-action masking for model-facing order generation
- Engine fallback of illegal movement orders to `HOLD`

The **core game rules** remain independent of negotiation.

Structured press is an additional agent/environment layer. This is intentional: the same game engine should support both No-Press and Press experiments.

### Structured press extension

The current press protocol adds:

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

The combined protocol currently contains **77 model-visible tokens**.

Press is structured and game-grounded rather than natural language.

The protocol supports:

- public and private messages;
- requests;
- unilateral promises;
- multi-term proposals;
- acceptance and rejection;
- positive commitments to issue specific movement orders;
- negative `AVOID <location>` commitments;
- mechanically observable commitment fulfillment and violation.

It deliberately does **not** encode concepts such as:

```text
ALLY
TRUST
BETRAY
LIE
FRIEND
ENEMY
```

Those are intended to emerge, or be inferred later from behavior, rather than being declared as primitive semantics.

### Press turn structure

Press is event-driven.

Each Spring or Autumn movement phase begins with one opening press opportunity per power.

After the opening opportunities, additional press inferences occur only when a power has unread inbox messages.

A model press inference emits either:

- one structured message; or
- `END`, meaning no message.

Actual messages are bounded by an environment configuration such as:

```text
max_press_messages_per_power = 4
```

This limit is not a token and is not fixed into the protocol. It is an experimental parameter.

A public message is visible to every power but creates an inbox event only for its named recipient.

A private message is visible only to sender and recipient.

Repeated requests or proposals remain legal. The finite message budget turns conversational loops into a bounded strategic cost rather than a special-cased protocol error.

Press ends when:

- all powers have received their opening opportunity; and
- all unread inboxes are empty.

Unaccepted proposals expire at the end of the press phase.

Accepted commitments survive into movement-order generation, but **do not constrain legal action masks**. Agents remain mechanically free to violate promises.

---

## System boundary: model, agent, game, and press environment

The **model** remains a decoder-only next-token predictor.

It accepts a token sequence and returns logits for the next token.

The **game engine** owns:

- board state;
- phase/year state;
- units;
- supply-center ownership;
- legal game orders;
- adjudication;
- victory and timeout.

The **press subsystem** owns:

- message delivery;
- public/private visibility;
- unread inboxes;
- outstanding proposals;
- active commitments;
- message budgets;
- commitment expiration;
- fulfillment/violation diagnostics.

The **agent** is the surrounding system that:

1. Serializes current game state.
2. Adds the appropriate visible negotiation state and history.
3. Invokes the model repeatedly to produce an order or press sequence.
4. Masks syntactically or semantically invalid token continuations.
5. Parses generated sequences into typed actions or messages.
6. Applies game actions or press-state transitions.
7. During training, converts demonstrations or rewards into losses.

This distinction remains important.

Tool use, acting, negotiation, and reinforcement learning do not require a fundamentally different neural network. They require protocols and environment state that give generated tokens operational meaning.

---

## Technology decision: Rust and Candle

The initial implementation will use **Rust with Hugging Face Candle**.

Candle will provide:

- CPU and CUDA tensors
- Matrix multiplication and primitive tensor operations
- Automatic differentiation
- Parameter storage and neural-network building blocks
- Optimizers such as SGD and AdamW
- Device and dtype handling
- Model serialization, preferably with Safetensors

We will implement the educationally important transformer components ourselves using Candle tensor operations:

- Token embeddings
- Positional embeddings or another explicit positional representation
- Causal attention masks
- Multi-head self-attention
- MLP blocks
- Residual connections
- Layer normalization or RMS normalization
- Output projection
- Cross-entropy loss
- Autoregressive sampling
- Supervised training
- REINFORCE
- Later value-baseline logic

We will avoid using a ready-made GPT implementation from `candle-transformers` as the primary model.

Existing implementations may still be read as references or used as test oracles.

### Why Candle is sufficient

For the proposed small model, Candle provides the essential substrate:

- tensor operations;
- GPU execution;
- autograd;
- trainable variables;
- optimizers;
- custom-operation support.

Nothing about causal attention, cross-entropy training, behavior cloning, REINFORCE, structured press, or a learned value head intrinsically requires PyTorch.

Candle's smaller surface area also fits the educational purpose of the project: tensor shapes, ownership, graph construction, and training logic remain relatively explicit.

### What PyTorch offers beyond Candle

PyTorch's advantage is maturity and breadth rather than fundamentally different modeling capability.

| Area | Candle | PyTorch advantage |
| --- | --- | --- |
| Core training | Tensors, autograd, layers, SGD/AdamW | Broader operator coverage and more battle-tested gradients |
| GPU performance | CUDA and custom kernels | More fused kernels and widely tuned training paths |
| Mixed precision | Lower-level dtype/device facilities | Mature AMP and gradient-scaling workflows |
| Debugging | Rust errors and tensor inspection | Rich hooks, profiler support, anomaly detection |
| Distributed training | Multi-GPU support exists | Mature DDP/FSDP ecosystem |
| Ecosystem | Lean and growing | Far more examples and research implementations |
| Experiment speed | Explicit compiled Rust | Very fast Python instrumentation and iteration |

Those differences matter much more for large-scale research workloads than for Tiny Diplomacy.

### Re-evaluation rule

Stay on Candle unless one of these concrete blockers appears:

- a required differentiable operation is unavailable or incorrect;
- a needed CUDA path is missing and implementing it overwhelms the learning objective;
- training performance prevents useful iteration after basic optimization;
- numerical instability cannot reasonably be diagnosed;
- later experiments genuinely require mature distributed-training infrastructure.

If one occurs, first create a small PyTorch reference implementation for differential testing.

A complete rewrite should remain a last resort.

---

## Proposed initial model

The initial target remains intentionally small:

- Decoder-only transformer
- 4 transformer blocks
- `d_model` around 128
- 4 attention heads
- MLP hidden width around 512
- Initial context target around **512 tokens**
- Current vocabulary: **77 tokens**
- Roughly 0.5–1 million parameters, adjusted after implementation

The previous 256-token target is no longer the default because structured press, active commitments, inbox contents, and visible history now consume meaningful context.

The 512-token target is still an experimental choice rather than a sacred architectural constant.

Later experiments should deliberately compare shorter and longer contexts.

---

## Token and state protocol

The project uses a purpose-built symbolic protocol rather than a natural-language tokenizer.

Each token is an enum value with a stable integer ID.

The current protocol provides:

- dense 36-cell board serialization;
- explicit supply-center ownership;
- phase and year;
- acting power;
- movement order grammar;
- Winter adjustment grammar;
- press-generation mode;
- structured negotiation acts;
- game-grounded commitments.

The canonical no-history state prefix through `ORDERS` or `ADJUST` is currently **95 tokens**.

Press history and negotiation state are additional context.

Requirements:

- every state has a canonical serialization;
- equivalent game-order sets serialize identically;
- press messages have canonical semantic representations;
- active commitments and proposals serialize deterministically;
- private messages are completely absent from unauthorized contexts;
- every generated sequence can be parsed without ambiguity;
- syntax errors remain distinct from illegal game actions;
- prefix-aware legal-token enumeration is available;
- protocol versions are explicit and recorded in training data.

---

# Build sequence and success gates

## Phase 1: Specify Tiny Diplomacy v0 — complete

The base game rules are captured in **Tiny Diplomacy v0 Specification**.

Specified:

- fixed 6×6 map;
- four asymmetric starting positions;
- eight supply centers;
- Spring/Autumn/Winter calendar;
- `HOLD`, `MOVE`, and `SUPPORT`;
- support validity and cutting;
- attack and defensive strength;
- ties and failed moves;
- simultaneous movement chains;
- swaps and longer cycles;
- vacated provinces;
- self-dislodgement prevention;
- destruction of dislodged armies;
- persistent supply-center ownership;
- builds, waives, and disbands;
- victory and timeout;
- illegal-order fallback;
- model-facing legal-action masking.

Success gate:

- **Met at the game-design level.**

Remaining work is implementation and testing.

---

## Phase 2: Build the authoritative game engine

Build:

- `GameState`
- fixed map representation
- seat assignment
- units
- supply-center ownership
- phase/year state
- movement-order types
- Winter-adjustment types
- legal-order enumeration
- simultaneous order submission
- deterministic movement adjudication
- dependency chains and cycles
- Spring/Autumn/Winter transitions
- supply-center capture
- build/disband application
- victory, timeout, and draw detection
- seeded seat rotation
- random-policy game generation
- telemetry hooks separate from game reward

Test heavily:

- uncontested moves;
- equal-strength bounces;
- supported attacks;
- supported holds;
- support cutting;
- legal-but-ineffective support;
- failed moves defending origin;
- moves into vacated provinces;
- friendly occupancy;
- self-dislodgement prevention;
- successful movement chains;
- blocked movement chains;
- direct swaps;
- longer cycles;
- multi-party contests;
- dislodgement and destruction;
- Autumn center transfer;
- legal and illegal builds;
- mandatory disbands;
- illegal movement fallback to `HOLD`;
- victory at five centers;
- timeout/draw handling;
- deterministic replay.

Success gate:

- unit/property tests cover every rule;
- thousands of randomly generated legal turns execute without invariant violations;
- serialized/replayed games reproduce identical states.

---

## Phase 3: Implement the token, state, and press protocol

The semantic design is now substantially specified in **Tiny Diplomacy v0 Token, State, and Press Protocol**.

Implementation work:

### Core protocol

- stable 77-token enum and integer IDs;
- canonical board serializer;
- supply-center serializer;
- order serializer/parser;
- Winter adjustment serializer/parser;
- deterministic canonical ordering;
- protocol-version identifier.

### Press types

Define authoritative typed representations for:

- `Visibility`
- `PressMessage`
- `SpeechAct`
- `Request`
- `Promise`
- `Proposal`
- `Commitment`
- `AvoidanceCommitment`
- proposal status
- commitment fulfillment status

### Negotiation state

Implement:

- public/private visibility;
- per-power unread inboxes;
- visible press history;
- outstanding proposals keyed by `(sender, recipient)`;
- proposal supersession;
- proposal acceptance/rejection;
- active commitments;
- commitment expiration;
- commitment fulfillment/violation checks.

### Event-driven press loop

Implement environment-configured:

```text
max_press_messages_per_power
```

with the initial suggested value:

```text
4
```

Semantics:

1. one opening press opportunity for every power;
2. later inferences triggered by unread inbox messages;
3. multiple unread messages may be consumed in one activation;
4. one generated message maximum per press inference;
5. `END` sends nothing and consumes no message budget;
6. public messages trigger only their named recipient;
7. repeated proposals are permitted;
8. exhausted senders may still read messages but can only emit `END`;
9. press ends when opening opportunities are complete and all inboxes are empty;
10. unaccepted proposals expire when press closes.

### Press tests

Test at minimum:

- private message invisible to uninvolved powers;
- public message visible to all;
- public message triggers only its recipient;
- opening opportunity permits initiation from an empty inbox;
- `END` consumes no send budget;
- sent message consumes exactly one budget unit;
- exhausted sender cannot emit another message;
- exhausted sender can still consume inbox messages;
- multiple inbox messages trigger one activation;
- repeated identical proposals remain legal;
- newer same-direction proposal supersedes older proposal;
- opposite-direction proposals coexist;
- `ACCEPT` creates the expected commitments;
- `REJECT` removes the expected outstanding proposal;
- unaccepted proposal expires at press close;
- positive commitment judged by submitted order, not outcome;
- `AVOID` fulfillment/violation judged by submitted orders;
- commitments never change movement-order legality.

Success gate:

- serializer/parser round trips hold;
- fuzzed sequences cannot crash parsers;
- hidden private information never leaks into unauthorized contexts;
- all press-state transitions are deterministic for a fixed event order;
- bounded press always terminates under finite message budgets.

---

## Phase 4: Build the tiny GPT

Build from Candle primitives:

- embedding lookup;
- positional representation;
- causal mask;
- multi-head self-attention;
- MLP;
- residual and normalization paths;
- stacked transformer blocks;
- vocabulary projection;
- cross-entropy loss;
- AdamW;
- checkpoint save/load;
- greedy sampling;
- stochastic sampling.

Begin with artificial datasets, not Diplomacy:

- repeating cycles such as `A B C A B C`;
- copy tasks;
- delayed-copy tasks;
- small deterministic grammars;
- tiny datasets intended to be deliberately overfit.

Success gate:

- the model overfits a tiny deterministic dataset to near-zero loss;
- expected continuations survive checkpoint save/reload;
- CPU and CUDA implementations agree within acceptable numerical tolerance.

---

## Phase 5: Generate supervised No-Press game data

Begin with the simplest behavioral environment.

Build:

- random-but-legal policy;
- simple heuristic policy;
- dataset generator producing `(state prefix, target orders)` examples;
- deterministic train/validation split;
- reproducibility metadata.

The heuristic policy may prefer:

- nearby supply-center capture;
- threatened-center defense;
- favorable supported attacks;
- avoidance of obviously futile moves.

No external training data is required.

Synthetic engine-generated data is preferable initially because it is:

- unlimited;
- perfectly parseable;
- tied to known legal actions;
- easy to reproduce.

Success gate:

- trained model produces syntactically valid orders;
- imitation of the heuristic policy is measurably better than random on held-out positions.

---

## Phase 6: Add constrained decoding

At every generation step:

1. inspect current game/negotiation state;
2. inspect the generated prefix;
3. enumerate semantically valid next tokens;
4. set all other logits to negative infinity;
5. sample from the remaining distribution.

Use the same general mechanism for:

- `ORDERS`;
- `ADJUST`;
- `PRESS`.

Track both:

- raw unmasked legality;
- executed legality after masking.

For game orders, executed legality should be 100%.

For press, generated messages should always satisfy both grammar and state-dependent semantics.

Success gate:

- complete legal order sequences are always produced;
- valid press messages are always produced;
- raw model legality improves with training;
- constrained decoding never creates impossible parser states.

---

## Phase 7: Build the evaluation harness

Build this before self-play RL.

### Fixed opponents

Include:

- random legal policy;
- fixed heuristic policy;
- frozen historical neural checkpoints;
- later, policy mixtures or populations.

### Game metrics

Record:

- win/draw/survival rates;
- supply-center score;
- seat/power performance;
- raw and constrained legality;
- policy entropy;
- average game length;
- results against each fixed opponent;
- pairwise or Elo-like ratings, interpreted cautiously.

### Press metrics

When press is enabled, additionally record:

- messages sent;
- public/private ratio;
- press inferences;
- inbox depth;
- requests;
- promises;
- proposals;
- acceptances;
- rejections;
- superseded proposals;
- commitments created;
- commitments fulfilled;
- commitments violated;
- repeated requests/proposals;
- unused message budget;
- performance after fulfilled versus violated agreements.

Evaluation must support both:

```text
max_press_messages_per_power = 0
```

and press-enabled configurations.

This provides a clean No-Press control.

Success gate:

- every training run produces reproducible reports against fixed external anchors;
- Press and No-Press policies can be compared under otherwise matched conditions.

---

## Phase 8: Introduce No-Press self-play reinforcement learning

Start with plain REINFORCE because it exposes the direct relationship:

```text
token log-probabilities
    -> action-sequence log-probability
    -> trajectory return
    -> policy-gradient update
```

Build:

- self-play episode runner;
- per-token and per-action log-probability accounting;
- reward/return calculation;
- REINFORCE loss;
- entropy bonus;
- advantage normalization if useful;
- checkpoint league of historical policies.

Then consider:

1. learned value-head baseline;
2. advantage estimates;
3. improved credit assignment;
4. reward shaping only if measurements justify it;
5. PPO-like clipping only if plain policy gradients are too unstable.

The purpose of this phase is to establish strategic competence **before** communication becomes another source of variation.

Success gate:

- the policy improves against fixed random and heuristic baselines;
- improvement is visible across seats rather than only against contemporaneous self-play copies.

---

## Phase 9: Add structured press to Tiny Diplomacy

Do **not** graduate to a larger game yet.

Keep the board, movement rules, victory condition, and model architecture as stable as possible while adding communication.

Use the structured press protocol already defined:

```text
PUBLIC
PRIVATE
REQUEST
PROMISE
PROPOSE
ACCEPT
REJECT
AVOID
```

The initial environment should expose:

- current board state;
- current negotiation state;
- unread inbox messages;
- enough visible history to interpret ongoing negotiation;
- the press-generation marker.

Important experimental controls:

```text
max_press_messages_per_power = 0
1
2
4
8
```

The protocol remains unchanged across these conditions.

Train and evaluate whether agents learn to:

- request useful actions;
- promise strategically relevant actions;
- propose reciprocal cooperation;
- accept advantageous proposals;
- reject poor proposals;
- use `AVOID` as a non-entry agreement;
- keep or violate commitments depending on learned incentives;
- condition later behavior on communication.

Do not label behavior as deception merely because a commitment was violated.

The environment should report only mechanically observable facts.

Success gate:

- agents measurably condition movement orders on received press;
- press-enabled play differs from matched No-Press play;
- proposal/commitment behavior can be measured mechanically;
- communication can improve performance in at least some controlled conditions.

---

## Phase 10: Study memory, reputation, and partial observability

Once structured press works, manipulate what survives into later contexts.

The board remains fully observable.

Private communication introduces the project's first intentional information asymmetry.

Compare context configurations such as:

1. current board only;
2. current board + active commitments;
3. active commitments + current inbox;
4. previous phase's visible press;
5. several phases of visible press;
6. previous fulfillment/violation records;
7. full available history up to the context limit;
8. compressed summaries or explicit memory.

Evaluate context lengths such as:

```text
128
256
512
1024
```

when useful.

Questions:

- Does remembering promises improve cooperation?
- Do agents learn to behave differently toward reliable and unreliable counterparts?
- Does longer history improve opponent modeling?
- Does irrelevant history degrade tactical play?
- How much history is required before reputation-like behavior appears?
- Does private communication create materially different strategies from public-only press?
- Do agents infer hidden alliances from public actions without seeing private messages?
- Can compact learned or algorithmic summaries preserve useful history?

Avoid implementing a hand-authored token such as:

```text
TRUST BLUE 0.27
```

unless the explicit experiment is to compare engineered reputation state against learned reputation.

Success gate:

- context ablations produce reproducible behavioral differences;
- the model can use at least some historical information that is not reconstructible from the current board alone.

---

## Phase 11: Population and adaptation experiments

Once a competent base policy exists, explore whether strategic styles can diverge.

### Full-model populations

Maintain multiple independently trained policies.

Compare:

- homogeneous self-play;
- historical checkpoint leagues;
- heterogeneous policy populations;
- evolutionary or tournament-based selection.

### LoRA experiments

LoRA is not initially required for memory efficiency because the base model is tiny.

Use it as an experimental mechanism.

Possible setup:

```text
shared base model
    + LoRA A
    + LoRA B
    + LoRA C
    ...
```

Questions:

- Can low-rank adaptations create distinct strategic styles?
- Can different adapters develop different rates of cooperation, aggression, or promise fulfillment?
- How much behavioral diversity is possible while the base world/game representation stays shared?
- How does LoRA diversity compare with fully independent models?

Potential experiments:

- RL-train adapters independently;
- clone successful adapters;
- perturb or mutate adapters;
- tournament selection;
- freeze base weights and evolve only adapters;
- compare lifetime gradient learning with population-level selection.

Avoid assigning psychological labels in advance unless the experiment explicitly requires a controlled behavioral prior.

Success gate:

- separately trained policies or adapters show stable, measurable behavioral differences across repeated evaluation games.

---

## Phase 12: Modular and multi-model agents

After the single-model structured protocol is understood, experiment with division of cognitive labor.

The symbolic Tiny Diplomacy protocol can act as an intermediate representation between components.

Possible architectures:

```text
board/history
    -> strategy model
    -> symbolic intent/actions
```

or:

```text
board/history
    -> planner
    -> policy
    -> orders
```

or:

```text
board/history
    -> opponent model
    -> strategy model
    -> orders
```

Potential modules:

- strategic policy;
- opponent-action predictor;
- value critic;
- planner;
- memory summarizer;
- negotiation policy;
- later, natural-language translator.

Possible experiments:

- one model doing everything;
- strategy and opponent-model separation;
- strategy and planning separation;
- multiple proposal generators with a selecting critic;
- fixed strategy model with swappable communication-style adapters;
- fixed communication component with swappable strategic policies.

The initial goal is not natural language.

The important question is whether modularity changes:

- sample efficiency;
- strategic performance;
- interpretability;
- behavioral diversity;
- failure modes.

Success gate:

- at least one modular architecture can be compared fairly with a single-model baseline under controlled parameter or compute budgets.

---

## Phase 13: Add explicit planning and search

Reinforcement learning can place strategic regularities into policy weights, but autoregressive generation is not itself tree search.

Later experiments may add:

- learned value function;
- engine rollouts;
- candidate-order sampling;
- shallow tree search;
- policy/value-guided search;
- opponent-policy models.

This creates a clean comparison among:

- knowledge stored in weights;
- inference-time computation;
- explicit simulation;
- combinations of learned policy and lookahead.

With press enabled, later planning experiments may also ask whether agents plan around:

- likely responses to proposals;
- commitment credibility;
- future reputation effects.

Those should be added only after simpler press behavior is measurable.

---

## Phase 14: Far-future graduation toward standard Diplomacy

Standard Diplomacy is no longer a near-term roadmap milestone.

Only revisit it after the Tiny Diplomacy environment has served the desired behavioral experiments.

Possible migration sequence:

1. larger map;
2. more powers;
3. fleets and coasts;
4. standard home-center semantics;
5. retreats;
6. convoys;
7. standard map;
8. adjudication edge cases;
9. standard victory condition;
10. adaptation of the existing press protocol to the larger action space.

Use an established adjudicator as a test oracle even if our own representation remains custom.

The objective should be to preserve already-understood agent mechanisms while increasing the world's strategic complexity.

Do not simultaneously redesign:

- the board;
- the movement engine;
- negotiation semantics;
- memory architecture;
- training algorithm.

Success gate:

- defined only when the project actually chooses to pursue full Diplomacy.

---

# Suggested Rust workspace structure

```text
tiny-diplomacy-gpt/
  Cargo.toml
  crates/
    game/          # Rules, state, legal actions, adjudication
    protocol/      # Tokens, serialization, parsing, legal-prefix masks
    press/         # Inboxes, visibility, proposals, commitments, press loop
    model/         # Candle transformer and checkpoint format
    agents/        # Random, heuristic, neural, population, search agents
    data/          # Dataset generation and loading
    train/         # Supervised and RL training binaries
    eval/          # Tournaments, ablations, metrics, reports
  configs/         # Versioned experiment/environment configurations
  tests/           # Cross-crate scenarios and golden cases
  docs/            # Rules, protocol, experiments, design decisions
```

The exact crate split may remain coarse initially.

Important dependency boundaries:

- `game` must not depend on Candle;
- `press` must not depend on Candle;
- `protocol` must not depend on Candle;
- model code should interact with the environment through ordinary typed state/action structures;
- No-Press games should remain runnable without instantiating the press subsystem.

This keeps behavioral machinery independently testable.

---

# Experimental discipline

Every run should record:

- Git revision;
- game-rules version;
- protocol version;
- press configuration;
- `max_press_messages_per_power`;
- visibility configuration if varied;
- context length;
- model hyperparameters;
- optimizer;
- learning rate;
- random seeds;
- training-data generator;
- policy versions;
- hardware;
- Candle version;
- checkpoint identity;
- evaluation opponent set;
- evaluation results.

Press-enabled experiments should additionally record enough event data to deterministically reconstruct:

- message order;
- inbox delivery;
- visibility;
- proposal supersession;
- acceptance/rejection;
- commitment creation;
- fulfillment/violation.

Prefer deterministic tests and seeded scheduling.

If multiple powers are simultaneously eligible for a press inference, the scheduler must use a deterministic or seeded selection procedure and record it.

GPU training may still contain nondeterministic numerical paths, so reproducibility means statistically consistent results rather than necessarily bit-for-bit identical weights.

---

# Immediate next steps

The game rules and the first structured press protocol are now specified at the design level.

The next implementation work should proceed approximately in this order:

1. **Freeze protocol IDs for the current 77-token draft.**
2. Define authoritative Rust types for:
   - game state;
   - movement orders;
   - Winter adjustments;
   - press messages;
   - proposals;
   - commitments;
   - inbox state.
3. Implement and heavily test the base game engine without model code.
4. Implement state/order serializers and parsers.
5. Implement legal game-order enumeration.
6. Implement the prefix-aware constrained-decoding state machine.
7. Implement the press subsystem independently of the model:
   - visibility;
   - inboxes;
   - proposal state;
   - commitments;
   - configurable message budget;
   - event-driven termination.
8. Add press-specific deterministic and privacy tests.
9. Create the Rust workspace and a minimal Candle tensor/autograd smoke test.
10. Implement the transformer.
11. Overfit artificial sequence tasks.
12. Train a supervised No-Press policy.
13. Establish No-Press self-play competence.
14. Enable structured press on the same Tiny Diplomacy environment.

The next concrete engineering deliverable is therefore no longer another game-design document.

It is an **authoritative Rust representation of the already-specified game and protocol**, with enough deterministic tests that the neural model can be added without ambiguity about what its tokens mean.

---

## References

- Hugging Face Candle repository and feature overview
- Candle core API documentation
- Candle neural-network API documentation
- Diplomacy research environment and datasets
- *DipNet: Human-Level Play in the Game of Diplomacy by Combining Strategic Reasoning with Natural Language Processing*
- Cicero research code
