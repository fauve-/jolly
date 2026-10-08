use std::str::FromStr;

use game_engine::{CenterOwner, GameState, Location::*, Order, Phase, Power};
use press_engine::{PressMessage, PressPhase, Proposition, SpeechAct, Visibility};
use protocol::*;

fn ids(tokens: &[Token]) -> Vec<TokenId> {
    tokens.iter().copied().map(Token::id).collect()
}

fn allowed(decoder: &Decoder) -> Vec<Token> {
    decoder.valid_next_tokens().tokens().collect()
}

fn single_red() -> GameState {
    GameState::empty(Phase::Spring, 1).with_unit(B2, Power::Red)
}

fn press_fixture() -> GameState {
    GameState::empty(Phase::Spring, 1)
        .with_unit(B2, Power::Red)
        .with_unit(C3, Power::Blue)
}

#[test]
fn vocabulary_is_the_frozen_84_token_bijection() {
    assert_eq!(TOKEN_COUNT, 84);
    assert_eq!(Token::ALL.len(), 84);
    assert_eq!(Token::SYMBOLS.len(), 84);
    assert_eq!(PROTOCOL_VERSION, "tiny-diplomacy/v0-press");
    assert_eq!(
        TOKEN_MANIFEST_SHA256,
        "b510b31198e270f1cff0f23825af861ea76675030c28677e6abaa5bc6a8c0902"
    );

    for (index, token) in Token::ALL.into_iter().enumerate() {
        assert_eq!(token.id().value(), index as u8);
        assert_eq!(Token::try_from(index as u8).unwrap(), token);
        assert_eq!(Token::from_str(token.symbol()).unwrap(), token);
    }
    assert!(Token::try_from(84).is_err());
    assert!(Token::from_str("EOS").is_err());
    assert!(Token::from_str("...").is_err());
}

#[test]
fn initial_observation_is_dense_canonical_and_95_tokens() {
    let snapshot = DecisionSnapshot::movement(&GameState::initial(), Power::Red).unwrap();
    let encoded = encode_observation(&snapshot).unwrap();
    let tokens: Vec<_> = encoded.iter().map(|id| id.token()).collect();

    assert_eq!(tokens.len(), 95);
    assert_eq!(
        &tokens[..5],
        &[
            Token::Bos,
            Token::Red,
            Token::Spring,
            Token::Year1,
            Token::Board
        ]
    );
    assert_eq!(
        &tokens[5..9],
        &[Token::A1, Token::Green, Token::A2, Token::Empty]
    );
    assert_eq!(tokens[77], Token::Centers);
    assert_eq!(
        &tokens[78..84],
        &[
            Token::A1,
            Token::Green,
            Token::B2,
            Token::Neutral,
            Token::B5,
            Token::Red
        ]
    );
    assert_eq!(tokens[94], Token::Orders);
}

#[test]
fn movement_round_trip_is_canonical_and_strict() {
    let state = GameState::empty(Phase::Spring, 1)
        .with_unit(B2, Power::Red)
        .with_unit(C2, Power::Red);
    let snapshot = DecisionSnapshot::movement(&state, Power::Red).unwrap();
    let response = TypedResponse::Movement(vec![
        Order::Move { from: C2, to: C1 },
        Order::Hold { at: B2 },
    ]);
    let encoded = encode_response(&snapshot, &response).unwrap();
    assert_eq!(
        encoded,
        ids(&[
            Token::B2,
            Token::Hold,
            Token::C2,
            Token::Move,
            Token::C1,
            Token::End
        ])
    );
    assert_eq!(
        parse_response(&snapshot, &encoded).unwrap(),
        TypedResponse::Movement(vec![
            Order::Hold { at: B2 },
            Order::Move { from: C2, to: C1 },
        ])
    );

    assert!(
        parse_response(
            &snapshot,
            &ids(&[Token::C2, Token::Hold, Token::B2, Token::Hold, Token::End])
        )
        .is_err()
    );
    assert!(
        parse_response(
            &snapshot,
            &ids(&[
                Token::B2,
                Token::Hold,
                Token::C2,
                Token::Hold,
                Token::End,
                Token::End
            ])
        )
        .is_err()
    );
}

#[test]
fn exact_single_army_masks_match_the_specification() {
    let snapshot = DecisionSnapshot::movement(&single_red(), Power::Red).unwrap();
    let mut decoder = Decoder::new(&snapshot, ResponseBudget::new(8)).unwrap();
    assert_eq!(allowed(&decoder), vec![Token::B2]);

    decoder.push(Token::B2.id()).unwrap();
    assert_eq!(allowed(&decoder), vec![Token::Hold, Token::Move]);

    decoder.push(Token::Move.id()).unwrap();
    assert_eq!(
        allowed(&decoder),
        vec![Token::A2, Token::B1, Token::B3, Token::C2]
    );

    decoder.push(Token::C2.id()).unwrap();
    assert_eq!(allowed(&decoder), vec![Token::End]);
    decoder.push(Token::End.id()).unwrap();
    assert!(allowed(&decoder).is_empty());
    assert_eq!(
        decoder.finish().unwrap(),
        TypedResponse::Movement(vec![Order::Move { from: B2, to: C2 }])
    );
}

#[test]
fn movement_budget_prunes_longer_branches_without_dead_ends() {
    let snapshot = DecisionSnapshot::movement(&single_red(), Power::Red).unwrap();
    assert!(Decoder::new(&snapshot, ResponseBudget::new(2)).is_err());

    let mut hold_only = Decoder::new(&snapshot, ResponseBudget::new(3)).unwrap();
    hold_only.push(Token::B2.id()).unwrap();
    assert_eq!(allowed(&hold_only), vec![Token::Hold]);

    let mut moves_fit = Decoder::new(&snapshot, ResponseBudget::new(4)).unwrap();
    moves_fit.push(Token::B2.id()).unwrap();
    assert_eq!(allowed(&moves_fit), vec![Token::Hold, Token::Move]);
}

#[test]
fn rejected_decoder_push_is_atomic() {
    let snapshot = DecisionSnapshot::movement(&single_red(), Power::Red).unwrap();
    let mut decoder = Decoder::new(&snapshot, ResponseBudget::new(4)).unwrap();
    let prefix = decoder.prefix().to_vec();
    let mask = decoder.valid_next_tokens();
    assert!(decoder.push(Token::End.id()).is_err());
    assert_eq!(decoder.prefix(), prefix);
    assert_eq!(decoder.valid_next_tokens(), mask);
    decoder.push(Token::B2.id()).unwrap();
}

#[test]
fn every_small_masked_movement_completion_strictly_parses() {
    fn walk(decoder: Decoder, completed: &mut Vec<TypedResponse>) {
        for token in decoder.valid_next_tokens().tokens() {
            let mut branch = decoder.clone();
            branch.push(token.id()).unwrap();
            if branch.is_terminal() {
                completed.push(branch.finish().unwrap());
            } else {
                walk(branch, completed);
            }
        }
    }

    let snapshot = DecisionSnapshot::movement(&single_red(), Power::Red).unwrap();
    let decoder = Decoder::new(&snapshot, ResponseBudget::new(4)).unwrap();
    let mut completed = Vec::new();
    walk(decoder, &mut completed);
    assert_eq!(completed.len(), 5); // HOLD plus four orthogonal moves.
}

#[test]
fn winter_forms_and_sorted_disband_lookahead_are_enforced() {
    let build_state =
        GameState::empty(Phase::Winter, 1).with_center_owner(B5, CenterOwner::Power(Power::Red));
    let build = DecisionSnapshot::adjustment(&build_state, Power::Red).unwrap();
    assert_eq!(
        parse_response(&build, &ids(&[Token::Build, Token::B5, Token::End])).unwrap(),
        TypedResponse::Adjustment(AdjustmentResponse::Build(B5))
    );
    assert_eq!(
        parse_response(&build, &ids(&[Token::Waive, Token::End])).unwrap(),
        TypedResponse::Adjustment(AdjustmentResponse::Waive)
    );

    let disband_state = GameState::empty(Phase::Winter, 1)
        .with_center_owner(B5, CenterOwner::Power(Power::Red))
        .with_unit(A2, Power::Red)
        .with_unit(B2, Power::Red)
        .with_unit(C2, Power::Red);
    let disband = DecisionSnapshot::adjustment(&disband_state, Power::Red).unwrap();
    let mut decoder = Decoder::new(&disband, ResponseBudget::new(5)).unwrap();
    assert_eq!(allowed(&decoder), vec![Token::Disband]);
    decoder.push(Token::Disband.id()).unwrap();
    assert_eq!(allowed(&decoder), vec![Token::A2, Token::B2]);

    assert!(
        parse_response(
            &disband,
            &ids(&[
                Token::Disband,
                Token::C2,
                Token::Disband,
                Token::B2,
                Token::End
            ])
        )
        .is_err()
    );
}

#[test]
fn exhausted_press_sender_can_only_emit_end() {
    let state = press_fixture();
    let mut phase = PressPhase::with_config(
        &state,
        press_engine::PressConfig {
            max_press_messages_per_power: 0,
        },
    )
    .unwrap();
    let activation = phase.activate(Power::Red).unwrap();
    let snapshot = DecisionSnapshot::press(&state, activation, phase.view_for(Power::Red)).unwrap();
    let decoder = Decoder::new(&snapshot, ResponseBudget::new(1)).unwrap();
    assert_eq!(allowed(&decoder), vec![Token::End]);
}

#[test]
fn press_parser_and_mask_preserve_direction_and_canonical_terms() {
    let state = press_fixture();
    let mut phase = PressPhase::new(&state).unwrap();
    phase.activate(Power::Red).unwrap();
    phase
        .respond(Some(PressMessage {
            sender: Power::Red,
            recipient: Power::Blue,
            visibility: Visibility::Private,
            act: SpeechAct::Propose(vec![Proposition::Order {
                actor: Power::Red,
                order: Order::Move { from: B2, to: C2 },
            }]),
        }))
        .unwrap();
    let activation = phase.activate(Power::Blue).unwrap();
    let snapshot =
        DecisionSnapshot::press(&state, activation, phase.view_for(Power::Blue)).unwrap();
    let mut decoder = Decoder::new(&snapshot, ResponseBudget::new(16)).unwrap();
    decoder.push(Token::Private.id()).unwrap();
    decoder.push(Token::Red.id()).unwrap();
    assert_eq!(
        allowed(&decoder),
        vec![
            Token::Propose,
            Token::Promise,
            Token::Request,
            Token::Accept,
            Token::Reject
        ]
    );
    decoder.push(Token::Accept.id()).unwrap();
    assert_eq!(allowed(&decoder), vec![Token::End]);
    decoder.push(Token::End.id()).unwrap();
    assert_eq!(
        decoder.finish().unwrap(),
        TypedResponse::Press(Some(PressMessage {
            sender: Power::Blue,
            recipient: Power::Red,
            visibility: Visibility::Private,
            act: SpeechAct::Accept,
        }))
    );
}

#[test]
fn press_term_serializer_sorts_by_complete_token_encoding() {
    let state = press_fixture();
    let mut phase = PressPhase::new(&state).unwrap();
    let activation = phase.activate(Power::Red).unwrap();
    let snapshot = DecisionSnapshot::press(&state, activation, phase.view_for(Power::Red)).unwrap();
    let response = TypedResponse::Press(Some(PressMessage {
        sender: Power::Red,
        recipient: Power::Blue,
        visibility: Visibility::Private,
        act: SpeechAct::Promise(vec![
            Proposition::Order {
                actor: Power::Red,
                order: Order::Move { from: B2, to: C2 },
            },
            Proposition::Avoid {
                actor: Power::Red,
                location: D3,
            },
        ]),
    }));
    let encoded = encode_response(&snapshot, &response).unwrap();
    assert_eq!(
        encoded,
        ids(&[
            Token::Private,
            Token::Blue,
            Token::Promise,
            Token::Red,
            Token::Avoid,
            Token::D3,
            Token::Red,
            Token::B2,
            Token::Move,
            Token::C2,
            Token::End,
        ])
    );

    let noncanonical = ids(&[
        Token::Private,
        Token::Blue,
        Token::Promise,
        Token::Red,
        Token::B2,
        Token::Move,
        Token::C2,
        Token::Red,
        Token::Avoid,
        Token::D3,
        Token::End,
    ]);
    assert!(parse_response(&snapshot, &noncanonical).is_err());
}

#[test]
fn diplomatic_context_fails_explicitly_until_record_grammar_is_available() {
    let state = press_fixture();
    let mut phase = PressPhase::new(&state).unwrap();
    let activation = phase.activate(Power::Red).unwrap();
    let snapshot = DecisionSnapshot::press(&state, activation, phase.view_for(Power::Red)).unwrap();
    assert_eq!(
        encode_observation(&snapshot),
        Err(ProtocolError::DiplomaticContextUnavailable)
    );
}
