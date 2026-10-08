use game_engine::{
    Adjustment, CenterOwner, GameOutcome, GameState, Location::*, Order, OrderRejectionReason,
    Phase, Power, RejectedOrder, SupportStatus, adjudicate, apply_adjustments, apply_movement,
};

fn hold(at: game_engine::Location) -> Order {
    Order::Hold { at }
}

fn move_to(from: game_engine::Location, to: game_engine::Location) -> Order {
    Order::Move { from, to }
}

fn support_hold(from: game_engine::Location, unit: game_engine::Location) -> Order {
    Order::SupportHold { from, unit }
}

fn support_move(
    from: game_engine::Location,
    unit: game_engine::Location,
    to: game_engine::Location,
) -> Order {
    Order::SupportMove { from, unit, to }
}

#[test]
fn an_army_cannot_support_itself() {
    let state = spring().with_unit(B2, Power::Red);

    assert!(!game_engine::is_legal_order(&state, support_hold(B2, B2)));
    assert!(!game_engine::is_legal_order(
        &state,
        support_move(B2, B2, C2)
    ));
}

fn spring() -> GameState {
    GameState::empty(Phase::Spring, 1)
}

fn red_near_victory(phase: Phase, year: u8) -> GameState {
    GameState::empty(phase, year)
        .with_center_owner(A1, CenterOwner::Power(Power::Red))
        .with_center_owner(B2, CenterOwner::Power(Power::Red))
        .with_center_owner(B5, CenterOwner::Power(Power::Red))
        .with_center_owner(C3, CenterOwner::Power(Power::Red))
        .with_unit(B6, Power::Red)
}

#[test]
fn uncontested_move() {
    let state = spring().with_unit(A1, Power::Red);

    let result = adjudicate(&state, &[move_to(A1, A2)]);

    assert_eq!(result.units.get(&A1), None);
    assert_eq!(result.units.get(&A2), Some(&Power::Red));
    assert!(result.orders[&A1].succeeded);
}

#[test]
fn equal_strength_bounce() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(B2, Power::Blue);

    let result = adjudicate(&state, &[move_to(A1, A2), move_to(B2, A2)]);

    assert_eq!(result.units.get(&A1), Some(&Power::Red));
    assert_eq!(result.units.get(&B2), Some(&Power::Blue));
    assert_eq!(result.units.get(&A2), None);
}

#[test]
fn supported_attack() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(B2, Power::Green)
        .with_unit(A2, Power::Blue);

    let result = adjudicate(
        &state,
        &[move_to(A1, A2), support_move(B2, A1, A2), hold(A2)],
    );

    assert_eq!(result.units.get(&A2), Some(&Power::Red));
    assert_eq!(result.dislodged, vec![(A2, Power::Blue)]);
    assert_eq!(
        result.orders[&B2].support_status,
        Some(SupportStatus::Effective)
    );
}

#[test]
fn supported_defense_stops_an_equal_attack() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(B2, Power::Red)
        .with_unit(A2, Power::Blue)
        .with_unit(A3, Power::Blue);

    let result = adjudicate(
        &state,
        &[
            move_to(A1, A2),
            support_move(B2, A1, A2),
            hold(A2),
            support_hold(A3, A2),
        ],
    );

    assert_eq!(result.units.get(&A1), Some(&Power::Red));
    assert_eq!(result.units.get(&A2), Some(&Power::Blue));
    assert!(result.dislodged.is_empty());
}

#[test]
fn an_unsuccessful_attack_still_cuts_support() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(B2, Power::Green)
        .with_unit(A2, Power::Blue)
        .with_unit(C2, Power::Yellow);

    let result = adjudicate(
        &state,
        &[
            move_to(A1, A2),
            support_move(B2, A1, A2),
            hold(A2),
            move_to(C2, B2),
        ],
    );

    assert_eq!(result.orders[&B2].support_status, Some(SupportStatus::Cut));
    assert_eq!(result.units.get(&A1), Some(&Power::Red));
    assert_eq!(result.units.get(&A2), Some(&Power::Blue));
    assert_eq!(result.units.get(&C2), Some(&Power::Yellow));
}

#[test]
fn legal_support_for_an_action_not_issued_is_ineffective() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(B1, Power::Green);

    let result = adjudicate(&state, &[move_to(A1, A2), support_hold(B1, A1)]);

    assert!(result.orders[&B1].legal);
    assert_eq!(
        result.orders[&B1].support_status,
        Some(SupportStatus::Ineffective)
    );
    assert_eq!(result.units.get(&A2), Some(&Power::Red));
}

#[test]
fn a_failed_move_defends_its_origin_with_strength_one() {
    let state = spring()
        .with_unit(B2, Power::Red)
        .with_unit(B3, Power::Blue)
        .with_unit(B1, Power::Green)
        .with_unit(C2, Power::Green)
        .with_unit(A2, Power::Yellow);

    let result = adjudicate(
        &state,
        &[
            move_to(B2, B3),
            hold(B3),
            move_to(B1, B2),
            support_move(C2, B1, B2),
            support_hold(A2, B2),
        ],
    );

    assert_eq!(
        result.orders[&A2].support_status,
        Some(SupportStatus::Ineffective)
    );
    assert_eq!(result.units.get(&B2), Some(&Power::Green));
    assert_eq!(result.units.get(&B3), Some(&Power::Blue));
    assert_eq!(result.dislodged, vec![(B2, Power::Red)]);
}

#[test]
fn movement_into_a_successfully_vacated_province() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(A2, Power::Blue);

    let result = adjudicate(&state, &[move_to(A1, A2), move_to(A2, A3)]);

    assert_eq!(result.units.get(&A2), Some(&Power::Red));
    assert_eq!(result.units.get(&A3), Some(&Power::Blue));
}

#[test]
fn a_blocked_friendly_movement_chain_fails_backward() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(A2, Power::Red)
        .with_unit(A3, Power::Red)
        .with_unit(A4, Power::Blue);

    let result = adjudicate(
        &state,
        &[move_to(A1, A2), move_to(A2, A3), move_to(A3, A4), hold(A4)],
    );

    assert_eq!(result.units.get(&A1), Some(&Power::Red));
    assert_eq!(result.units.get(&A2), Some(&Power::Red));
    assert_eq!(result.units.get(&A3), Some(&Power::Red));
    assert_eq!(result.units.get(&A4), Some(&Power::Blue));
}

#[test]
fn a_movement_chain_succeeds_when_its_end_is_open() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(A2, Power::Red)
        .with_unit(A3, Power::Red);

    let result = adjudicate(&state, &[move_to(A1, A2), move_to(A2, A3), move_to(A3, A4)]);

    assert_eq!(result.units.get(&A1), None);
    assert_eq!(result.units.get(&A2), Some(&Power::Red));
    assert_eq!(result.units.get(&A3), Some(&Power::Red));
    assert_eq!(result.units.get(&A4), Some(&Power::Red));
}

#[test]
fn direct_swap_succeeds() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(A2, Power::Blue);

    let result = adjudicate(&state, &[move_to(A1, A2), move_to(A2, A1)]);

    assert_eq!(result.units.get(&A1), Some(&Power::Blue));
    assert_eq!(result.units.get(&A2), Some(&Power::Red));
    assert!(result.dislodged.is_empty());
}

#[test]
fn longer_movement_cycle_succeeds() {
    let state = spring()
        .with_unit(A1, Power::Green)
        .with_unit(A2, Power::Red)
        .with_unit(B2, Power::Blue)
        .with_unit(B1, Power::Yellow);

    let result = adjudicate(
        &state,
        &[
            move_to(A1, A2),
            move_to(A2, B2),
            move_to(B2, B1),
            move_to(B1, A1),
        ],
    );

    assert_eq!(result.units.get(&A1), Some(&Power::Yellow));
    assert_eq!(result.units.get(&A2), Some(&Power::Green));
    assert_eq!(result.units.get(&B2), Some(&Power::Red));
    assert_eq!(result.units.get(&B1), Some(&Power::Blue));
}

#[test]
fn three_way_equal_strength_contest_bounces_everyone() {
    let state = spring()
        .with_unit(A1, Power::Green)
        .with_unit(A3, Power::Red)
        .with_unit(B2, Power::Blue);

    let result = adjudicate(&state, &[move_to(A1, A2), move_to(A3, A2), move_to(B2, A2)]);

    assert_eq!(result.units.get(&A1), Some(&Power::Green));
    assert_eq!(result.units.get(&A3), Some(&Power::Red));
    assert_eq!(result.units.get(&B2), Some(&Power::Blue));
    assert_eq!(result.units.get(&A2), None);
}

#[test]
fn superior_friendly_strength_cannot_self_dislodge() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(B2, Power::Red)
        .with_unit(A2, Power::Red);

    let result = adjudicate(
        &state,
        &[move_to(A1, A2), support_move(B2, A1, A2), hold(A2)],
    );

    assert_eq!(result.units.get(&A1), Some(&Power::Red));
    assert_eq!(result.units.get(&A2), Some(&Power::Red));
    assert!(result.dislodged.is_empty());
}

#[test]
fn dislodged_army_is_destroyed_without_a_retreat() {
    let state = spring()
        .with_unit(B1, Power::Red)
        .with_unit(A2, Power::Green)
        .with_unit(B2, Power::Blue);

    let result = adjudicate(
        &state,
        &[move_to(B1, B2), support_move(A2, B1, B2), hold(B2)],
    );

    assert_eq!(result.units.get(&B2), Some(&Power::Red));
    assert_eq!(
        result.units.values().filter(|&&p| p == Power::Blue).count(),
        0
    );
    assert_eq!(result.dislodged, vec![(B2, Power::Blue)]);
}

#[test]
fn autumn_occupation_transfers_supply_center_ownership() {
    let mut state = GameState::empty(Phase::Autumn, 1)
        .with_unit(B1, Power::Red)
        .with_center_owner(B2, CenterOwner::Neutral);
    let result = adjudicate(&state, &[move_to(B1, B2)]);

    apply_movement(&mut state, result);

    assert_eq!(state.occupant(B2), Some(Power::Red));
    assert_eq!(state.center_owner(B2), Some(CenterOwner::Power(Power::Red)));
}

#[test]
fn legal_winter_build_uses_owned_empty_original_home_center() {
    let mut state = GameState::empty(Phase::Winter, 1)
        .with_center_owner(A1, CenterOwner::Power(Power::Green))
        .with_center_owner(B2, CenterOwner::Power(Power::Green));
    let build = Adjustment::Build {
        at: A1,
        power: Power::Green,
    };

    let result = apply_adjustments(&mut state, &[build]);

    assert!(result[0].applied);
    assert_eq!(state.occupant(A1), Some(Power::Green));
}

#[test]
fn winter_build_on_a_captured_enemy_home_center_is_illegal() {
    let mut state = GameState::empty(Phase::Winter, 1)
        .with_center_owner(A1, CenterOwner::Power(Power::Red))
        .with_center_owner(B2, CenterOwner::Power(Power::Red));
    let build = Adjustment::Build {
        at: A1,
        power: Power::Red,
    };

    let result = apply_adjustments(&mut state, &[build]);

    assert!(!result[0].applied);
    assert_eq!(state.occupant(A1), None);
}

#[test]
fn mandatory_winter_disband_removes_the_selected_army() {
    let mut state = GameState::empty(Phase::Winter, 1)
        .with_center_owner(A1, CenterOwner::Power(Power::Green))
        .with_unit(A1, Power::Green)
        .with_unit(A2, Power::Green);
    let disband = Adjustment::Disband {
        at: A2,
        power: Power::Green,
    };

    let result = apply_adjustments(&mut state, &[disband]);

    assert!(result[0].applied);
    assert_eq!(state.army_count(Power::Green), 1);
    assert_eq!(state.occupant(A2), None);
}

#[test]
fn illegal_movement_order_falls_back_to_hold() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(A2, Power::Blue)
        .with_unit(B1, Power::Blue);

    let result = adjudicate(
        &state,
        &[move_to(A1, C1), move_to(A2, A1), support_move(B1, A2, A1)],
    );

    assert!(!result.orders[&A1].legal);
    assert_eq!(result.orders[&A1].resolved_as, hold(A1));
    assert!(!result.orders[&A1].succeeded);
    assert_eq!(result.units.get(&A1), Some(&Power::Blue));
    assert_eq!(result.dislodged, vec![(A1, Power::Red)]);
}

#[test]
fn spring_autumn_and_winter_advance_the_calendar() {
    let mut state = spring().with_unit(A1, Power::Red);
    let spring_result = adjudicate(&state, &[hold(A1)]);

    apply_movement(&mut state, spring_result);

    assert_eq!(state.phase(), Phase::Autumn);
    assert_eq!(state.year(), 1);
    assert!(!state.is_finished());

    let autumn_result = adjudicate(&state, &[hold(A1)]);
    apply_movement(&mut state, autumn_result);

    assert_eq!(state.phase(), Phase::Winter);
    assert_eq!(state.year(), 1);

    apply_adjustments(&mut state, &[]);

    assert_eq!(state.phase(), Phase::Spring);
    assert_eq!(state.year(), 2);
}

#[test]
fn autumn_victory_ends_the_game_before_winter() {
    let mut state = GameState::empty(Phase::Autumn, 4)
        .with_center_owner(A1, CenterOwner::Power(Power::Red))
        .with_center_owner(B2, CenterOwner::Power(Power::Red))
        .with_center_owner(B5, CenterOwner::Power(Power::Red))
        .with_center_owner(C3, CenterOwner::Power(Power::Red))
        .with_center_owner(C6, CenterOwner::Power(Power::Red));

    let result = adjudicate(&state, &[]);
    apply_movement(&mut state, result);

    assert_eq!(state.phase(), Phase::Autumn);
    assert_eq!(
        state.outcome(),
        Some(&GameOutcome::Victory { winner: Power::Red })
    );
}

#[test]
fn autumn_ten_records_all_high_score_leaders() {
    let mut state = GameState::empty(Phase::Autumn, 10)
        .with_center_owner(A1, CenterOwner::Power(Power::Green))
        .with_center_owner(B2, CenterOwner::Power(Power::Green))
        .with_center_owner(B5, CenterOwner::Power(Power::Red))
        .with_center_owner(C3, CenterOwner::Power(Power::Red));

    let result = adjudicate(&state, &[]);
    apply_movement(&mut state, result);

    assert_eq!(
        state.outcome(),
        Some(&GameOutcome::TimeLimit {
            leaders: vec![Power::Green, Power::Red],
        })
    );
}

#[test]
fn a_dislodged_hold_is_not_reported_as_successful() {
    let state = spring()
        .with_unit(B1, Power::Red)
        .with_unit(A2, Power::Green)
        .with_unit(B2, Power::Blue);

    let result = adjudicate(
        &state,
        &[move_to(B1, B2), support_move(A2, B1, B2), hold(B2)],
    );

    assert_eq!(result.dislodged, vec![(B2, Power::Blue)]);
    assert!(!result.orders[&B2].succeeded);
}

#[test]
fn build_and_waive_cannot_both_apply() {
    let mut state = GameState::empty(Phase::Winter, 1)
        .with_center_owner(A1, CenterOwner::Power(Power::Green))
        .with_center_owner(B2, CenterOwner::Power(Power::Green))
        .with_unit(C1, Power::Green);
    let adjustments = [
        Adjustment::Build {
            at: A1,
            power: Power::Green,
        },
        Adjustment::Waive {
            power: Power::Green,
        },
    ];

    let results = apply_adjustments(&mut state, &adjustments);

    assert!(results.iter().all(|result| !result.applied));
    assert_eq!(state.occupant(A1), None);
    assert_eq!(state.phase(), Phase::Spring);
}

#[test]
fn duplicate_waivers_are_rejected_but_one_waiver_is_valid() {
    let base =
        GameState::empty(Phase::Winter, 1).with_center_owner(A1, CenterOwner::Power(Power::Green));
    let waive = Adjustment::Waive {
        power: Power::Green,
    };

    let mut duplicate = base.clone();
    let duplicate_results = apply_adjustments(&mut duplicate, &[waive, waive]);
    assert!(duplicate_results.iter().all(|result| !result.applied));

    let mut single = base;
    let single_results = apply_adjustments(&mut single, &[waive]);
    assert!(single_results[0].applied);
}

#[test]
fn malformed_mandatory_disbands_are_atomic_and_do_not_end_winter() {
    let mut state = GameState::empty(Phase::Winter, 1)
        .with_center_owner(A1, CenterOwner::Power(Power::Green))
        .with_unit(A1, Power::Green)
        .with_unit(A2, Power::Green);
    let disband = Adjustment::Disband {
        at: A2,
        power: Power::Green,
    };
    let mixed = [
        disband,
        Adjustment::Waive {
            power: Power::Green,
        },
    ];

    let rejected = apply_adjustments(&mut state, &mixed);

    assert!(rejected.iter().all(|result| !result.applied));
    assert_eq!(state.army_count(Power::Green), 2);
    assert_eq!(state.phase(), Phase::Winter);

    let accepted = apply_adjustments(&mut state, &[disband]);
    assert!(accepted[0].applied);
    assert_eq!(state.army_count(Power::Green), 1);
    assert_eq!(state.phase(), Phase::Spring);
}

#[test]
fn every_rejected_raw_order_is_retained_with_a_reason() {
    let state = spring()
        .with_unit(A1, Power::Red)
        .with_unit(B2, Power::Blue);
    let nonexistent = hold(B3);
    let duplicate_hold = hold(A1);
    let duplicate_move = move_to(A1, A2);
    let bad_geometry = move_to(B2, D2);

    let result = adjudicate(
        &state,
        &[nonexistent, duplicate_hold, duplicate_move, bad_geometry],
    );

    assert_eq!(
        result.rejected_orders,
        vec![
            RejectedOrder {
                order: nonexistent,
                reason: OrderRejectionReason::NoUnitAtSource,
            },
            RejectedOrder {
                order: duplicate_hold,
                reason: OrderRejectionReason::MultipleOrdersForSource,
            },
            RejectedOrder {
                order: duplicate_move,
                reason: OrderRejectionReason::MultipleOrdersForSource,
            },
            RejectedOrder {
                order: bad_geometry,
                reason: OrderRejectionReason::IllegalOrder,
            },
        ]
    );
    assert_eq!(result.orders[&A1].submitted, None);
    assert_eq!(result.orders[&A1].resolved_as, hold(A1));
    assert_eq!(result.orders[&B2].submitted, Some(bad_geometry));
    assert!(!result.orders[&B2].legal);
}

#[test]
fn autumn_capture_updates_ownership_before_checking_victory() {
    let mut state = red_near_victory(Phase::Autumn, 4);
    let result = adjudicate(&state, &[move_to(B6, C6)]);

    apply_movement(&mut state, result);

    assert_eq!(state.center_owner(C6), Some(CenterOwner::Power(Power::Red)));
    assert_eq!(state.center_count(Power::Red), 5);
    assert_eq!(
        state.outcome(),
        Some(&GameOutcome::Victory { winner: Power::Red })
    );
}

#[test]
fn victory_takes_precedence_over_the_year_ten_time_limit() {
    let mut state = red_near_victory(Phase::Autumn, 10);
    let result = adjudicate(&state, &[move_to(B6, C6)]);

    apply_movement(&mut state, result);

    assert_eq!(
        state.outcome(),
        Some(&GameOutcome::Victory { winner: Power::Red })
    );
}

#[test]
fn year_ten_with_one_highest_score_records_one_leader() {
    let mut state = GameState::empty(Phase::Autumn, 10)
        .with_center_owner(A1, CenterOwner::Power(Power::Green))
        .with_center_owner(B2, CenterOwner::Power(Power::Green))
        .with_center_owner(B5, CenterOwner::Power(Power::Red));
    let result = adjudicate(&state, &[]);

    apply_movement(&mut state, result);

    assert_eq!(
        state.outcome(),
        Some(&GameOutcome::TimeLimit {
            leaders: vec![Power::Green],
        })
    );
}

#[test]
fn spring_occupation_neither_captures_a_center_nor_triggers_victory() {
    let mut state = red_near_victory(Phase::Spring, 4);
    let result = adjudicate(&state, &[move_to(B6, C6)]);

    apply_movement(&mut state, result);

    assert_eq!(state.occupant(C6), Some(Power::Red));
    assert_eq!(state.center_owner(C6), Some(CenterOwner::Neutral));
    assert_eq!(state.center_count(Power::Red), 4);
    assert_eq!(state.phase(), Phase::Autumn);
    assert_eq!(state.outcome(), None);
}

#[test]
fn unoccupied_supply_center_retains_its_owner_after_autumn() {
    let mut state =
        GameState::empty(Phase::Autumn, 3).with_center_owner(B2, CenterOwner::Power(Power::Blue));
    let result = adjudicate(&state, &[]);

    apply_movement(&mut state, result);

    assert_eq!(
        state.center_owner(B2),
        Some(CenterOwner::Power(Power::Blue))
    );
    assert_eq!(state.phase(), Phase::Winter);
}

#[test]
fn two_required_disbands_apply_as_one_complete_submission() {
    let mut state = GameState::empty(Phase::Winter, 2)
        .with_center_owner(A1, CenterOwner::Power(Power::Green))
        .with_unit(A1, Power::Green)
        .with_unit(A2, Power::Green)
        .with_unit(A3, Power::Green);
    let adjustments = [
        Adjustment::Disband {
            at: A2,
            power: Power::Green,
        },
        Adjustment::Disband {
            at: A3,
            power: Power::Green,
        },
    ];

    let results = apply_adjustments(&mut state, &adjustments);

    assert!(results.iter().all(|result| result.applied));
    assert_eq!(state.army_count(Power::Green), 1);
    assert_eq!(state.occupant(A1), Some(Power::Green));
    assert_eq!(state.phase(), Phase::Spring);
    assert_eq!(state.year(), 3);
}

#[test]
fn malformed_two_unit_disband_submissions_are_atomic() {
    let base = GameState::empty(Phase::Winter, 2)
        .with_center_owner(A1, CenterOwner::Power(Power::Green))
        .with_unit(A1, Power::Green)
        .with_unit(A2, Power::Green)
        .with_unit(A3, Power::Green);
    let green_disband = |at| Adjustment::Disband {
        at,
        power: Power::Green,
    };
    let cases = [
        vec![green_disband(A2)],
        vec![green_disband(A1), green_disband(A2), green_disband(A3)],
        vec![green_disband(A2), green_disband(A2)],
        vec![
            green_disband(A2),
            Adjustment::Disband {
                at: A3,
                power: Power::Blue,
            },
        ],
    ];

    for adjustments in cases {
        let mut state = base.clone();
        let results = apply_adjustments(&mut state, &adjustments);

        assert!(results.iter().all(|result| !result.applied));
        assert_eq!(state.army_count(Power::Green), 3);
        assert_eq!(state.phase(), Phase::Winter);
    }
}

#[test]
fn omitted_optional_build_advances_without_building() {
    let mut state =
        GameState::empty(Phase::Winter, 2).with_center_owner(A1, CenterOwner::Power(Power::Green));

    let results = apply_adjustments(&mut state, &[]);

    assert!(results.is_empty());
    assert_eq!(state.occupant(A1), None);
    assert_eq!(state.phase(), Phase::Spring);
    assert_eq!(state.year(), 3);
}

#[test]
fn one_powers_applied_build_survives_another_powers_unresolved_disband() {
    let mut state = GameState::empty(Phase::Winter, 2)
        .with_center_owner(A1, CenterOwner::Power(Power::Green))
        .with_center_owner(B5, CenterOwner::Power(Power::Red))
        .with_unit(B5, Power::Red)
        .with_unit(B4, Power::Red);
    let build = Adjustment::Build {
        at: A1,
        power: Power::Green,
    };
    let invalid_red = Adjustment::Waive { power: Power::Red };

    let first_results = apply_adjustments(&mut state, &[build, invalid_red]);

    assert!(first_results[0].applied);
    assert!(!first_results[1].applied);
    assert_eq!(state.occupant(A1), Some(Power::Green));
    assert_eq!(state.phase(), Phase::Winter);

    let red_disband = Adjustment::Disband {
        at: B4,
        power: Power::Red,
    };
    let retry_results = apply_adjustments(&mut state, &[red_disband]);

    assert!(retry_results[0].applied);
    assert_eq!(state.occupant(A1), Some(Power::Green));
    assert_eq!(state.army_count(Power::Red), 1);
    assert_eq!(state.phase(), Phase::Spring);
}

#[test]
fn rejected_optional_build_is_lost_when_winter_advances() {
    let mut state =
        GameState::empty(Phase::Winter, 2).with_center_owner(A1, CenterOwner::Power(Power::Green));
    let invalid_build = Adjustment::Build {
        at: B2,
        power: Power::Green,
    };

    let results = apply_adjustments(&mut state, &[invalid_build]);

    assert!(!results[0].applied);
    assert_eq!(state.army_count(Power::Green), 0);
    assert_eq!(state.phase(), Phase::Spring);
    assert_eq!(state.year(), 3);
}

#[test]
#[should_panic(expected = "cannot adjudicate a finished game")]
fn finished_game_rejects_further_adjudication() {
    let mut state = red_near_victory(Phase::Autumn, 4);
    let result = adjudicate(&state, &[move_to(B6, C6)]);
    apply_movement(&mut state, result);

    let _ = adjudicate(&state, &[]);
}

#[test]
#[should_panic(expected = "cannot move a finished game")]
fn finished_game_rejects_further_movement_application() {
    let mut state = red_near_victory(Phase::Autumn, 4);
    let result = adjudicate(&state, &[move_to(B6, C6)]);
    let duplicate_result = result.clone();
    apply_movement(&mut state, result);

    apply_movement(&mut state, duplicate_result);
}

#[test]
fn game_state_rejects_years_outside_the_calendar() {
    assert!(std::panic::catch_unwind(|| GameState::empty(Phase::Spring, 0)).is_err());
    assert!(std::panic::catch_unwind(|| GameState::empty(Phase::Spring, 11)).is_err());
}

#[test]
fn initial_state_matches_the_authoritative_setup() {
    let state = GameState::initial();

    assert_eq!(state.phase(), Phase::Spring);
    assert_eq!(state.year(), 1);
    assert_eq!(state.outcome(), None);
    for (power, home) in [
        (Power::Green, A1),
        (Power::Yellow, D2),
        (Power::Red, B5),
        (Power::Blue, E5),
    ] {
        assert_eq!(state.occupant(home), Some(power));
        assert_eq!(state.center_owner(home), Some(CenterOwner::Power(power)));
        assert_eq!(state.army_count(power), 1);
        assert_eq!(state.center_count(power), 1);
    }
}
