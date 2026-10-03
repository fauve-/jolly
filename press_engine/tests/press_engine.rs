use game_engine::{
    GameState, Location::*, Order, Phase, Power, SupportStatus, adjudicate, is_legal_order,
};
use press_engine::*;

fn fixture() -> GameState {
    GameState::empty(Phase::Spring, 1)
        .with_unit(B2, Power::Red)
        .with_unit(C3, Power::Blue)
        .with_unit(A1, Power::Green)
        .with_unit(D2, Power::Yellow)
}

fn order(actor: Power, order: Order) -> Proposition {
    Proposition::Order { actor, order }
}

fn avoid(actor: Power, location: game_engine::Location) -> Proposition {
    Proposition::Avoid { actor, location }
}

fn r_move() -> Proposition {
    order(Power::Red, Order::Move { from: B2, to: C2 })
}

fn r_hold() -> Proposition {
    order(Power::Red, Order::Hold { at: B2 })
}

fn b_support() -> Proposition {
    order(
        Power::Blue,
        Order::SupportMove {
            from: C3,
            unit: B2,
            to: C2,
        },
    )
}

fn message(
    sender: Power,
    recipient: Power,
    visibility: Visibility,
    act: SpeechAct,
) -> PressMessage {
    PressMessage {
        sender,
        recipient,
        visibility,
        act,
    }
}

fn activate_and_respond(
    press: &mut PressPhase,
    power: Power,
    response: Option<PressMessage>,
) -> Result<PressActivation, PressError> {
    let activation = press.activate(power)?;
    press.respond(response)?;
    Ok(activation)
}

fn drain_and_close(press: &mut PressPhase) {
    while press.activate_next().unwrap().is_some() {
        press.respond(None).unwrap();
    }
    assert!(press.can_close());
    press.close().unwrap();
}

fn views(press: &PressPhase) -> Vec<PressView> {
    Power::ALL
        .into_iter()
        .map(|power| press.view_for(power))
        .collect()
}

fn assert_invalid_from_red(act: SpeechAct, expected: PressError) {
    let mut press = PressPhase::new(&fixture()).unwrap();
    press.activate(Power::Red).unwrap();
    let before = views(&press);
    assert_eq!(
        press.respond(Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            act,
        ))),
        Err(expected)
    );
    assert_eq!(views(&press), before);
    assert!(press.messages().is_empty());
    assert!(press.commitments().is_empty());
    assert_eq!(press.outstanding_proposal_count(), 0);
    assert_eq!(press.sent_count(Power::Red), 0);
}

#[test]
fn v01_to_v05_recipient_phase_and_actor_validation_is_atomic() {
    let mut self_recipient = PressPhase::new(&fixture()).unwrap();
    self_recipient.activate(Power::Red).unwrap();
    let before = views(&self_recipient);
    assert_eq!(
        self_recipient.respond(Some(message(
            Power::Red,
            Power::Red,
            Visibility::Private,
            SpeechAct::Promise(vec![r_move()]),
        ))),
        Err(PressError::SelfRecipient)
    );
    assert_eq!(views(&self_recipient), before);

    assert_eq!(
        PressPhase::new(&GameState::empty(Phase::Winter, 1)),
        Err(PressError::NotMovementPhase)
    );
    assert!(PressPhase::new(&GameState::empty(Phase::Autumn, 1)).is_ok());

    assert_invalid_from_red(
        SpeechAct::Promise(vec![b_support()]),
        PressError::InvalidActor,
    );
    assert_invalid_from_red(SpeechAct::Request(vec![r_move()]), PressError::InvalidActor);
    assert_invalid_from_red(
        SpeechAct::Propose(vec![order(Power::Green, Order::Hold { at: A1 })]),
        PressError::InvalidActor,
    );
}

#[test]
fn v06_to_v09_conflicts_avoidance_ownership_and_geometry_are_rejected() {
    assert_invalid_from_red(
        SpeechAct::Promise(vec![r_move(), r_hold()]),
        PressError::ContradictoryTerms,
    );
    assert_invalid_from_red(
        SpeechAct::Promise(vec![avoid(Power::Red, B2)]),
        PressError::InvalidAvoidance,
    );

    let mut valid_avoidance = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut valid_avoidance,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Promise(vec![avoid(Power::Red, C2)]),
        )),
    )
    .unwrap();
    assert_eq!(valid_avoidance.commitments().len(), 1);

    for invalid in [
        Order::Move { from: B2, to: D3 },
        Order::Hold { at: B3 },
        Order::Hold { at: C3 },
    ] {
        assert_invalid_from_red(
            SpeechAct::Promise(vec![order(Power::Red, invalid)]),
            PressError::InvalidOrder,
        );
    }

    assert_invalid_from_red(
        SpeechAct::Promise(vec![r_move(), avoid(Power::Red, C2)]),
        PressError::ContradictoryTerms,
    );

    let supporting_state = GameState::empty(Phase::Spring, 1)
        .with_unit(C3, Power::Red)
        .with_unit(B2, Power::Blue);
    let mut press = PressPhase::new(&supporting_state).unwrap();
    press.activate(Power::Red).unwrap();
    let before = views(&press);
    assert_eq!(
        press.respond(Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Promise(vec![
                order(
                    Power::Red,
                    Order::SupportMove {
                        from: C3,
                        unit: B2,
                        to: C2,
                    },
                ),
                avoid(Power::Red, C2),
            ]),
        ))),
        Err(PressError::ContradictoryTerms)
    );
    assert_eq!(views(&press), before);
}

#[test]
fn v10_accept_and_reject_require_an_outstanding_proposal() {
    for act in [SpeechAct::Accept, SpeechAct::Reject] {
        assert_invalid_from_red(act, PressError::NoOutstandingProposal);
    }
}

#[test]
fn v11_valid_multi_term_message_is_atomic_and_empty_bundles_are_invalid() {
    let mut press = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Promise(vec![avoid(Power::Red, D3), r_move()]),
        )),
    )
    .unwrap();
    assert_eq!(press.commitments().len(), 2);
    assert_eq!(press.sent_count(Power::Red), 1);

    for empty in [
        SpeechAct::Promise(vec![]),
        SpeechAct::Request(vec![]),
        SpeechAct::Propose(vec![]),
    ] {
        assert_invalid_from_red(empty, PressError::EmptyBundle);
    }
}

#[test]
fn t01_t03_requests_promises_and_proposals_have_distinct_transitions() {
    let mut request = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut request,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Request(vec![b_support(), avoid(Power::Blue, D3)]),
        )),
    )
    .unwrap();
    assert_eq!(request.messages().len(), 1);
    assert_eq!(request.unread_count(Power::Blue), 1);
    assert!(request.commitments().is_empty());
    assert_eq!(request.outstanding_proposal_count(), 0);

    let mut promise = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut promise,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Promise(vec![r_move(), avoid(Power::Red, D3)]),
        )),
    )
    .unwrap();
    assert_eq!(promise.commitments().len(), 2);
    for commitment in promise.commitments() {
        assert_eq!(commitment.actor, Power::Red);
        assert_eq!((commitment.phase, commitment.year), (Phase::Spring, 1));
        assert_eq!(commitment.counterparty, Power::Blue);
        assert_eq!(commitment.visibility, Visibility::Private);
        assert_eq!(commitment.origin, CommitmentOrigin::Promise);
    }

    let mut proposal = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut proposal,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![r_move(), b_support()]),
        )),
    )
    .unwrap();
    assert!(proposal.commitments().is_empty());
    assert_eq!(
        proposal
            .outstanding_proposal(Power::Red, Power::Blue)
            .unwrap()
            .terms,
        vec![r_move(), b_support()]
    );
}

#[test]
fn t04_t08_new_same_direction_proposal_supersedes_and_only_latest_can_be_accepted() {
    let mut press = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![r_move(), b_support()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Request(vec![r_hold()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![r_hold()]),
        )),
    )
    .unwrap();

    assert_eq!(press.outstanding_proposal_count(), 1);
    assert_eq!(
        press
            .outstanding_proposal(Power::Red, Power::Blue)
            .unwrap()
            .terms,
        vec![r_hold()]
    );
    assert_eq!(press.messages().len(), 3);

    activate_and_respond(
        &mut press,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Accept,
        )),
    )
    .unwrap();
    assert_eq!(press.outstanding_proposal_count(), 0);
    assert_eq!(press.commitments().len(), 1);
    assert_eq!(press.commitments()[0].proposition, r_hold());
}

fn opposite_proposals() -> PressPhase {
    let mut press = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![r_move(), b_support()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Propose(vec![b_support()]),
        )),
    )
    .unwrap();
    press
}

#[test]
fn t05_t08_opposite_proposals_are_independent_and_accept_or_reject_one_direction() {
    let coexist = opposite_proposals();
    assert!(
        coexist
            .outstanding_proposal(Power::Red, Power::Blue)
            .is_some()
    );
    assert!(
        coexist
            .outstanding_proposal(Power::Blue, Power::Red)
            .is_some()
    );

    let mut accepted = opposite_proposals();
    activate_and_respond(
        &mut accepted,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Request(vec![b_support()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut accepted,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Accept,
        )),
    )
    .unwrap();
    assert_eq!(accepted.commitments().len(), 2);
    assert!(
        accepted
            .outstanding_proposal(Power::Red, Power::Blue)
            .is_none()
    );
    assert!(
        accepted
            .outstanding_proposal(Power::Blue, Power::Red)
            .is_some()
    );

    let mut rejected = opposite_proposals();
    activate_and_respond(
        &mut rejected,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Request(vec![b_support()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut rejected,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Reject,
        )),
    )
    .unwrap();
    assert!(rejected.commitments().is_empty());
    assert!(
        rejected
            .outstanding_proposal(Power::Blue, Power::Red)
            .is_some()
    );
}

#[test]
fn t09_public_acceptance_does_not_publish_private_terms() {
    let mut press = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![r_move(), b_support()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Public,
            SpeechAct::Accept,
        )),
    )
    .unwrap();

    assert_eq!(press.commitments().len(), 2);
    assert_eq!(press.view_for(Power::Green).messages.len(), 1);
    assert_eq!(press.view_for(Power::Green).active_commitments.len(), 0);
    assert_eq!(press.view_for(Power::Yellow).active_commitments.len(), 0);
}

#[test]
fn t10_t12_reproposal_request_promise_and_bilateral_avoidance() {
    let mut press = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![r_move()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Reject,
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![r_move()]),
        )),
    )
    .unwrap();
    assert!(
        press
            .outstanding_proposal(Power::Red, Power::Blue)
            .is_some()
    );

    let mut request_then_promise = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut request_then_promise,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Request(vec![b_support()]),
        )),
    )
    .unwrap();
    assert!(request_then_promise.commitments().is_empty());
    activate_and_respond(
        &mut request_then_promise,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Promise(vec![b_support()]),
        )),
    )
    .unwrap();
    assert_eq!(request_then_promise.commitments().len(), 1);

    let mut bilateral = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut bilateral,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![avoid(Power::Red, C2), avoid(Power::Blue, C2)]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut bilateral,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Accept,
        )),
    )
    .unwrap();
    assert_eq!(bilateral.commitments().len(), 2);
    assert_eq!(bilateral.commitments()[0].actor, Power::Red);
    assert_eq!(bilateral.commitments()[1].actor, Power::Blue);
}

#[test]
fn d01_d02_private_and_public_history_are_filtered_but_only_recipient_gets_inbox() {
    let request = SpeechAct::Request(vec![b_support()]);
    for visibility in [Visibility::Private, Visibility::Public] {
        let mut press = PressPhase::new(&fixture()).unwrap();
        activate_and_respond(
            &mut press,
            Power::Red,
            Some(message(
                Power::Red,
                Power::Blue,
                visibility,
                request.clone(),
            )),
        )
        .unwrap();

        assert_eq!(press.view_for(Power::Red).messages.len(), 1);
        assert_eq!(press.view_for(Power::Blue).messages.len(), 1);
        assert_eq!(press.unread_count(Power::Blue), 1);
        assert_eq!(press.unread_count(Power::Green), 0);
        assert_eq!(press.unread_count(Power::Yellow), 0);
        let observer_count = usize::from(visibility == Visibility::Public);
        assert_eq!(press.view_for(Power::Green).messages.len(), observer_count);
        assert_eq!(press.view_for(Power::Yellow).messages.len(), observer_count);
    }
}

#[test]
fn d03_d04_multiple_unread_messages_make_one_nonrepeating_activation() {
    let mut press = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Request(vec![b_support()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Green,
        Some(message(
            Power::Green,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Request(vec![avoid(Power::Blue, D3)]),
        )),
    )
    .unwrap();

    let activation = press.activate(Power::Blue).unwrap();
    assert_eq!(activation.unread.len(), 2);
    assert_eq!(activation.unread[0].sender, Power::Red);
    assert_eq!(activation.unread[1].sender, Power::Green);
    press.respond(None).unwrap();
    assert_eq!(
        press.activate(Power::Blue),
        Err(PressError::PowerNotEligible)
    );
    assert_eq!(press.view_for(Power::Blue).messages.len(), 2);
}

#[test]
fn d05_private_conversations_do_not_leak_through_any_view_field() {
    let mut press = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![r_move(), b_support()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Green,
        Some(message(
            Power::Green,
            Power::Yellow,
            Visibility::Private,
            SpeechAct::Promise(vec![order(Power::Green, Order::Hold { at: A1 })]),
        )),
    )
    .unwrap();

    let blue = press.view_for(Power::Blue);
    assert_eq!(blue.messages.len(), 1);
    assert_eq!(blue.outstanding_proposals.len(), 1);
    assert!(blue.active_commitments.is_empty());
    let yellow = press.view_for(Power::Yellow);
    assert_eq!(yellow.messages.len(), 1);
    assert!(yellow.outstanding_proposals.is_empty());
    assert_eq!(yellow.active_commitments.len(), 1);
}

#[test]
fn s01_s02_each_power_has_one_opening_and_end_costs_nothing() {
    let mut press = PressPhase::new(&fixture()).unwrap();
    for power in Power::ALL {
        let activation = press.activate(power).unwrap();
        assert!(activation.is_opening);
        assert!(activation.unread.is_empty());
        press.respond(None).unwrap();
        assert_eq!(press.sent_count(power), 0);
        assert_eq!(press.activate(power), Err(PressError::PowerNotEligible));
    }
    assert!(press.can_close());
}

#[test]
fn s03_s04_budget_is_per_message_and_exhausted_power_can_still_read() {
    let config = PressConfig {
        max_press_messages_per_power: 2,
    };
    let mut press = PressPhase::with_config(&fixture(), config).unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Promise(vec![r_move(), avoid(Power::Red, D3)]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Request(vec![r_hold()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Promise(vec![r_hold()]),
        )),
    )
    .unwrap();
    assert_eq!(press.sent_count(Power::Red), 2);
    assert_eq!(press.sent_count(Power::Blue), 1);

    let one = PressConfig {
        max_press_messages_per_power: 1,
    };
    let mut exhausted = PressPhase::with_config(&fixture(), one).unwrap();
    activate_and_respond(
        &mut exhausted,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Request(vec![b_support()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut exhausted,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Request(vec![r_hold()]),
        )),
    )
    .unwrap();
    let activation = exhausted.activate(Power::Red).unwrap();
    assert_eq!(activation.unread.len(), 1);
    assert!(!activation.can_send);
    let before = views(&exhausted);
    assert_eq!(
        exhausted.respond(Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Promise(vec![r_hold()]),
        ))),
        Err(PressError::BudgetExhausted)
    );
    assert_eq!(views(&exhausted), before);
    assert_eq!(exhausted.sent_count(Power::Red), 1);
}

#[test]
fn s05_s07_zero_budget_still_has_openings_and_completion_waits_for_all_of_them() {
    let mut press = PressPhase::with_config(
        &fixture(),
        PressConfig {
            max_press_messages_per_power: 0,
        },
    )
    .unwrap();
    for (index, power) in Power::ALL.into_iter().enumerate() {
        let activation = press.activate(power).unwrap();
        assert!(!activation.can_send);
        press.respond(None).unwrap();
        assert_eq!(press.can_close(), index == Power::ALL.len() - 1);
    }
    assert!(press.messages().is_empty());
    press.close().unwrap();
}

#[test]
fn s06_repetition_is_allowed_and_finite_budgets_terminate_it() {
    let mut press = PressPhase::new(&fixture()).unwrap();
    let proposal = || {
        message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![r_move()]),
        )
    };
    let reject = || {
        message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Reject,
        )
    };

    activate_and_respond(&mut press, Power::Red, Some(proposal())).unwrap();
    activate_and_respond(&mut press, Power::Blue, Some(reject())).unwrap();
    for _ in 1..4 {
        activate_and_respond(&mut press, Power::Red, Some(proposal())).unwrap();
        activate_and_respond(&mut press, Power::Blue, Some(reject())).unwrap();
    }
    let final_red = press.activate(Power::Red).unwrap();
    assert_eq!(final_red.unread.len(), 1);
    assert!(!final_red.can_send);
    press.respond(None).unwrap();
    drain_and_close(&mut press);
    assert_eq!(press.messages().len(), 8);
    assert!(press.messages().len() <= Power::ALL.len() * 4);
}

#[test]
fn s08_s10_closure_expires_proposals_preserves_commitments_and_refuses_press() {
    let mut proposal = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut proposal,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![r_move()]),
        )),
    )
    .unwrap();
    drain_and_close(&mut proposal);
    assert_eq!(proposal.outstanding_proposal_count(), 0);
    assert_eq!(proposal.activate(Power::Blue), Err(PressError::PhaseClosed));
    assert_eq!(proposal.respond(None), Err(PressError::NoActivation));

    let mut promise = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut promise,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Promise(vec![r_move()]),
        )),
    )
    .unwrap();
    drain_and_close(&mut promise);
    assert_eq!(promise.commitments().len(), 1);
}

fn deterministic_script() -> PressPhase {
    let mut press = PressPhase::new(&fixture()).unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Propose(vec![b_support(), r_move()]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Green,
        Some(message(
            Power::Green,
            Power::Yellow,
            Visibility::Public,
            SpeechAct::Request(vec![avoid(Power::Yellow, F6)]),
        )),
    )
    .unwrap();
    activate_and_respond(
        &mut press,
        Power::Blue,
        Some(message(
            Power::Blue,
            Power::Red,
            Visibility::Private,
            SpeechAct::Accept,
        )),
    )
    .unwrap();
    drain_and_close(&mut press);
    press
}

#[test]
fn s11_fixed_policy_and_replies_are_deterministic() {
    assert_eq!(deterministic_script(), deterministic_script());
}

fn promised_phase(state: &GameState, terms: Vec<Proposition>) -> PressPhase {
    let mut press = PressPhase::new(state).unwrap();
    activate_and_respond(
        &mut press,
        Power::Red,
        Some(message(
            Power::Red,
            Power::Blue,
            Visibility::Private,
            SpeechAct::Promise(terms),
        )),
    )
    .unwrap();
    drain_and_close(&mut press);
    press
}

#[test]
fn c01_c02_positive_commitments_use_exact_submitted_order_not_outcome_or_fallback() {
    let state = GameState::empty(Phase::Spring, 1)
        .with_unit(B2, Power::Red)
        .with_unit(C2, Power::Blue);
    let promised = order(Power::Red, Order::Move { from: B2, to: C2 });
    let move_order = Order::Move { from: B2, to: C2 };
    let submitted = [move_order, Order::Hold { at: C2 }];
    assert!(!adjudicate(&state, &submitted).orders[&B2].succeeded);
    let mut fulfilled = promised_phase(&state, vec![promised]);
    assert_eq!(
        fulfilled.resolve_commitments(&submitted).unwrap()[0].status,
        CommitmentStatus::Fulfilled
    );

    for violating in [
        vec![Order::Hold { at: B2 }, Order::Hold { at: C2 }],
        vec![Order::Hold { at: C2 }],
        vec![Order::Move { from: B2, to: B3 }, Order::Hold { at: C2 }],
    ] {
        let mut phase = promised_phase(&state, vec![promised]);
        assert_eq!(
            phase.resolve_commitments(&violating).unwrap()[0].status,
            CommitmentStatus::Violated
        );
    }
}

#[test]
fn c03_c05_avoid_checks_moves_and_support_moves_but_not_nonentry_actions() {
    let state = fixture();
    assert_eq!(
        evaluate_proposition(
            Power::Red,
            avoid(Power::Red, C2),
            &state,
            &[Order::Move { from: B2, to: C2 }],
        ),
        CommitmentStatus::Violated
    );

    let support_state = GameState::empty(Phase::Spring, 1)
        .with_unit(C3, Power::Red)
        .with_unit(B2, Power::Blue);
    assert_eq!(
        evaluate_proposition(
            Power::Red,
            avoid(Power::Red, C2),
            &support_state,
            &[
                Order::SupportMove {
                    from: C3,
                    unit: B2,
                    to: C2,
                },
                Order::Hold { at: B2 },
            ],
        ),
        CommitmentStatus::Violated
    );
    assert_eq!(
        evaluate_proposition(
            Power::Red,
            avoid(Power::Red, C2),
            &state,
            &[Order::Hold { at: B2 }],
        ),
        CommitmentStatus::Fulfilled
    );

    let occupied_edge = GameState::empty(Phase::Spring, 1)
        .with_unit(C2, Power::Red)
        .with_unit(C3, Power::Blue);
    for nonentry in [
        Order::Hold { at: C2 },
        Order::SupportHold { from: C2, unit: C3 },
    ] {
        assert_eq!(
            evaluate_proposition(
                Power::Red,
                avoid(Power::Red, C2),
                &occupied_edge,
                &[nonentry],
            ),
            CommitmentStatus::Fulfilled
        );
    }
}

#[test]
fn c06_multiple_commitments_resolve_independently() {
    let state = fixture();
    let mut press = promised_phase(&state, vec![r_move(), avoid(Power::Red, D3)]);
    let results = press
        .resolve_commitments(&[Order::Move { from: B2, to: C2 }])
        .unwrap();
    assert_eq!(results.len(), 2);
    assert!(
        results
            .iter()
            .all(|result| result.status == CommitmentStatus::Fulfilled)
    );
    assert!(press.commitments().is_empty());
}

#[test]
fn c07_exact_support_is_fulfilled_even_when_cut() {
    let state = GameState::empty(Phase::Spring, 1)
        .with_unit(C3, Power::Red)
        .with_unit(B2, Power::Blue)
        .with_unit(C4, Power::Green);
    let support = Order::SupportMove {
        from: C3,
        unit: B2,
        to: C2,
    };
    let submitted = [
        support,
        Order::Move { from: B2, to: C2 },
        Order::Move { from: C4, to: C3 },
    ];
    assert_eq!(
        adjudicate(&state, &submitted).orders[&C3].support_status,
        Some(SupportStatus::Cut)
    );
    let mut press = promised_phase(&state, vec![order(Power::Red, support)]);
    assert_eq!(
        press.resolve_commitments(&submitted).unwrap()[0].status,
        CommitmentStatus::Fulfilled
    );
}

#[test]
fn c08_commitments_do_not_change_game_legality_or_block_violations() {
    let state = fixture();
    let violating = Order::Hold { at: B2 };
    let before = is_legal_order(&state, violating);
    let press = promised_phase(&state, vec![r_move()]);
    assert_eq!(is_legal_order(&state, violating), before);
    assert!(is_legal_order(&state, violating));
    assert_eq!(press.commitments().len(), 1);
    assert_eq!(
        adjudicate(&state, &[violating]).orders[&B2].submitted,
        Some(violating)
    );
}

#[test]
fn c09_resolution_archives_results_and_a_new_phase_starts_clean() {
    let spring = fixture();
    let mut press = promised_phase(&spring, vec![r_move()]);
    press
        .resolve_commitments(&[Order::Hold { at: B2 }])
        .unwrap();
    assert!(press.commitments().is_empty());
    assert_eq!(press.commitment_results().len(), 1);
    assert_eq!(
        press.commitment_results()[0].status,
        CommitmentStatus::Violated
    );

    let autumn = GameState::empty(Phase::Autumn, 1).with_unit(B2, Power::Red);
    let next = PressPhase::new(&autumn).unwrap();
    assert!(next.messages().is_empty());
    assert!(next.commitments().is_empty());
    assert!(next.commitment_results().is_empty());
    assert_eq!(next.sent_count(Power::Red), 0);
}
