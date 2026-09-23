use game_engine::{
    Adjustment, CenterOwner, GameState, Location::*, Order, Phase, Power, SupportStatus,
    adjudicate, apply_adjustments, apply_movement,
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

fn spring() -> GameState {
    GameState::empty(Phase::Spring, 1)
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
    assert_eq!(result.units.get(&A1), Some(&Power::Blue));
    assert_eq!(result.dislodged, vec![(A1, Power::Red)]);
}
