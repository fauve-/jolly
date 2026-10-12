use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use game_engine::{GameState, Phase};
use rand::{Rng, SeedableRng, rngs::StdRng, seq::IteratorRandom};
use serde::Serialize;

use protocol::{
    ContextPolicy, DecisionSnapshot, Decoder, PROTOCOL_VERSION, ResponseBudget,
    TOKEN_MANIFEST_SHA256, TokenId, TypedResponse, encode_observation_with_policy, encode_response,
};

const RESPONSE_TOKENS: usize = 64;
const CONTEXT_TOKENS: usize = 512;

#[derive(Debug, Serialize)]
struct RunTrace {
    schema_version: u32,
    seed: u64,
    protocol_version: String,
    token_manifest_sha256: String,
    context_capacity: usize,
    response_budget: usize,
    failure: Option<String>,
    decisions: Vec<DecisionTrace>,
}

impl RunTrace {
    fn new(seed: u64) -> Self {
        Self {
            schema_version: 1,
            seed,
            protocol_version: PROTOCOL_VERSION.to_owned(),
            token_manifest_sha256: TOKEN_MANIFEST_SHA256.to_owned(),
            context_capacity: CONTEXT_TOKENS,
            response_budget: RESPONSE_TOKENS,
            failure: None,
            decisions: Vec::new(),
        }
    }

    fn record_decision(&mut self, snapshot: &DecisionSnapshot) -> usize {
        let index = self.decisions.len();
        self.decisions.push(DecisionTrace {
            index,
            year: snapshot.game_state().year(),
            phase: format!("{:?}", snapshot.game_state().phase()),
            acting_power: format!("{:?}", snapshot.acting_power()),
            mode: format!("{:?}", snapshot.mode()),
            board: snapshot.game_state().board_state_string(),
            prompt_token_ids: Vec::new(),
            response_token_ids: Vec::new(),
            typed_response: None,
        });
        index
    }

    fn write_failure(&self) -> Result<PathBuf> {
        let path = PathBuf::from(format!("failure-trace-{}.json", self.seed));
        let json =
            serde_json::to_string_pretty(self).context("couldn't serialize failure trace")?;
        std::fs::write(&path, json)
            .with_context(|| format!("couldn't write failure trace to {}", path.display()))?;
        Ok(path)
    }
}

#[derive(Debug, Serialize)]
struct DecisionTrace {
    index: usize,
    year: u8,
    phase: String,
    acting_power: String,
    mode: String,
    board: String,
    prompt_token_ids: Vec<u8>,
    response_token_ids: Vec<u8>,
    typed_response: Option<String>,
}

fn token_values(tokens: &[TokenId]) -> Vec<u8> {
    tokens.iter().map(|token| token.value()).collect()
}

fn report_failure(trace: &RunTrace) {
    match trace.write_failure() {
        Ok(path) => eprintln!("failure trace written to {}", path.display()),
        Err(error) => eprintln!("couldn't write failure trace: {error:#}"),
    }
}

fn random_response(
    snapshot: &DecisionSnapshot,
    rng: &mut impl Rng,
    trace: &mut RunTrace,
) -> Result<(Vec<TokenId>, TypedResponse)> {
    let decision_index = trace.record_decision(snapshot);

    // no model but whatever.
    let policy = ContextPolicy::new(CONTEXT_TOKENS, RESPONSE_TOKENS);
    let prompt = encode_observation_with_policy(snapshot, &policy)?;
    trace.decisions[decision_index].prompt_token_ids = token_values(&prompt);

    let mut decoder = Decoder::new(snapshot, ResponseBudget::new(RESPONSE_TOKENS))?;

    while !decoder.is_terminal() {
        let token = decoder
            .valid_next_tokens()
            .tokens()
            .choose(rng)
            .context("nonterminal decoder had no legal continuation")?;

        trace.decisions[decision_index]
            .response_token_ids
            .push(token.id().value());
        decoder.push(token.id())?;
    }

    let suffix = decoder.prefix().to_vec();
    let response = decoder.finish()?;
    trace.decisions[decision_index].typed_response = Some(format!("{response:#?}"));

    ensure!(
        encode_response(snapshot, &response)? == suffix,
        "decision {decision_index} did not round-trip through response encoding"
    );
    Ok((suffix, response))
}

fn play_press_and_movement(
    state: &mut GameState,
    rng: &mut impl Rng,
    trace: &mut RunTrace,
) -> Result<()> {
    let mut press = press_engine::PressPhase::new(state)?;

    while let Some(activation) = press.activate_next()? {
        let power = activation.power;
        let view = press.view_for(power);
        let snapshot = DecisionSnapshot::press(state, activation, view)?;

        let (_, response) = random_response(&snapshot, rng, trace)?;
        let TypedResponse::Press(message) = response else {
            bail!("press snapshot produced a non-press response")
        };

        press.respond(message)?;
    }

    press.close().context("Couldn't close press")?;

    let mut submitted = Vec::new();

    for power in game_engine::Power::ALL {
        let snapshot =
            DecisionSnapshot::movement_with_diplomacy(state, power, press.view_for(power))?;

        let (_, response) = random_response(&snapshot, rng, trace)?;
        let TypedResponse::Movement(orders) = response else {
            bail!("movement snapshot produced a non-movement response")
        };

        submitted.extend(orders);
    }

    press.resolve_commitments(&submitted)?;

    let result = game_engine::adjudicate(state, &submitted);
    ensure!(
        result.rejected_orders.is_empty(),
        "protocol-generated orders were rejected: {:?}",
        result.rejected_orders
    );
    game_engine::apply_movement(state, result);
    Ok(())
}

fn play_adjustments(state: &mut GameState, rng: &mut impl Rng, trace: &mut RunTrace) -> Result<()> {
    let mut adjustments = Vec::new();

    for power in game_engine::Power::ALL {
        let snapshot = DecisionSnapshot::adjustment(state, power)?;
        let (_, response) = random_response(&snapshot, rng, trace)?;

        let TypedResponse::Adjustment(response) = response else {
            bail!("adjustment snapshot produced a non-adjustment response")
        };

        adjustments.extend(response.adjustments(power));
    }

    let results = game_engine::apply_adjustments(state, &adjustments);
    ensure!(
        results.iter().all(|result| result.applied),
        "protocol-generated adjustments were rejected: {results:?}"
    );
    Ok(())
}

fn run_game(seed: u64, trace: &mut RunTrace) -> Result<()> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut state = GameState::initial();

    while !state.is_finished() {
        let phase = state.phase();
        let start = std::time::Instant::now();
        println!("starting phase: {phase:?}\n{}", state.board_state_string());
        match phase {
            Phase::Spring | Phase::Autumn => {
                play_press_and_movement(&mut state, &mut rng, trace)?;
            }
            Phase::Winter => {
                play_adjustments(&mut state, &mut rng, trace)?;
            }
        }

        println!(
            "phase {phase:?} took {:?}",
            std::time::Instant::now().duration_since(start)
        );
    }

    let outcome = state.outcome().context("Couldn't retrieve game outcome")?;
    println!("{}", state.board_state_string());
    println!("{outcome:?}");

    Ok(())
}

fn main() -> Result<()> {
    let seed: u64 = std::env::args()
        .nth(1)
        .context("required arg not present")?
        .parse()
        .context("arg not parsable number")?;
    // we don't care about success or failure, we're simply trying to walk through the game state.
    // This is basically a dry-run of how we're going to do this with candle later on.

    // 1. we start every game with a fixed rng seed
    // 2. for each decision (movement, press), serialize the
    // 3. generate tokens by uniform sampling from the legal-token mask until we hit END (basically a random walk).
    // 4. Parse the result and plop it into the engine.
    // 5. Keep going through the press, movement, winter phases until the game ends for whatever reason.
    // 6. keep track of enough stuff to reproduce failures (we generate invalid moves or other suchlike stuff)

    let mut trace = RunTrace::new(seed);
    match catch_unwind(AssertUnwindSafe(|| run_game(seed, &mut trace))) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            trace.failure = Some(format!("{error:#}"));
            report_failure(&trace);
            return Err(error);
        }
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("non-string panic payload");
            trace.failure = Some(format!("panic: {message}"));
            report_failure(&trace);
            resume_unwind(payload);
        }
    }
    println!("we did it bros");
    Ok(())
}
