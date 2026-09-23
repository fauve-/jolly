use std::collections::{HashMap, HashSet};

pub mod types;

pub use types::*;

/// Resolves a movement phase without mutating the supplied state.
pub fn adjudicate(state: &GameState, submitted_orders: &[Order]) -> MovementResult {
    assert!(matches!(state.phase, Phase::Spring | Phase::Autumn));

    let (orders, mut resolutions) = normalize_orders(state, submitted_orders);
    let cut_supporters = cut_supporters(state, &orders);
    let effective_supports = effective_supports(&orders, &cut_supporters, &mut resolutions);

    let mut move_strength = HashMap::new();
    let mut hold_strength = HashMap::new();
    for (&source, &order) in &orders {
        match order {
            Order::Move { .. } => {
                move_strength.insert(
                    source,
                    1 + effective_supports.get(&source).copied().unwrap_or(0),
                );
            }
            Order::Hold { .. } => {
                hold_strength.insert(
                    source,
                    1 + effective_supports.get(&source).copied().unwrap_or(0),
                );
            }
            Order::SupportHold { .. } | Order::SupportMove { .. } => {}
        }
    }

    let mut incoming: HashMap<Location, Vec<Location>> = HashMap::new();
    for (&source, &order) in &orders {
        if let Order::Move { to, .. } = order {
            incoming.entry(to).or_default().push(source);
        }
    }
    let mut possible = HashSet::new();
    for contenders in incoming.values() {
        let strongest = contenders
            .iter()
            .map(|source| move_strength[source])
            .max()
            .expect("an incoming group cannot be empty");
        let winners: Vec<_> = contenders
            .iter()
            .filter(|source| move_strength[source] == strongest)
            .copied()
            .collect();
        if winners.len() == 1 {
            possible.insert(winners[0]);
        }
    }

    // Starting with all unique winners possible preserves closed swaps and
    // cycles. Removing blocked moves then propagates failure through chains.
    loop {
        let mut remove = Vec::new();
        for &source in &possible {
            let Order::Move { to, .. } = orders[&source] else {
                unreachable!()
            };
            let Some(&defender_power) = state.units().get(&to) else {
                continue;
            };
            let attacker_power = state.units()[&source];
            let defender_vacates =
                matches!(orders.get(&to), Some(Order::Move { .. })) && possible.contains(&to);

            if defender_vacates {
                continue;
            }
            if attacker_power == defender_power {
                remove.push(source);
                continue;
            }
            let defense = hold_strength.get(&to).copied().unwrap_or(1);
            if move_strength[&source] <= defense {
                remove.push(source);
            }
        }
        if remove.is_empty() {
            break;
        }
        for source in remove {
            possible.remove(&source);
        }
    }

    for (&source, resolution) in &mut resolutions {
        resolution.succeeded = match resolution.resolved_as {
            Order::Move { .. } => possible.contains(&source),
            Order::Hold { .. } => true,
            Order::SupportHold { .. } | Order::SupportMove { .. } => {
                resolution.support_status == Some(SupportStatus::Effective)
            }
        };
    }

    let mut units = HashMap::new();
    let mut dislodged = Vec::new();
    for (&location, &power) in state.units() {
        if possible.contains(&location) {
            let Order::Move { to, .. } = orders[&location] else {
                unreachable!()
            };
            units.insert(to, power);
            continue;
        }

        let enemy_entered = possible.iter().any(|source| {
            matches!(orders[source], Order::Move { to, .. } if to == location)
                && state.units()[source] != power
        });
        if enemy_entered {
            dislodged.push((location, power));
        } else {
            units.insert(location, power);
        }
    }
    dislodged.sort_by_key(|&(location, _)| location);

    MovementResult {
        units,
        orders: resolutions,
        dislodged,
    }
}

/// Applies an adjudicated movement result. Autumn occupation also transfers
/// supply-center ownership.
pub fn apply_movement(state: &mut GameState, result: MovementResult) {
    *state.units_mut() = result.units;
    if state.phase == Phase::Autumn {
        let captures: Vec<_> = GameState::SUPPLY_CENTERS
            .into_iter()
            .filter_map(|location| state.occupant(location).map(|power| (location, power)))
            .collect();
        for (location, power) in captures {
            state
                .centers_mut()
                .insert(location, CenterOwner::Power(power));
        }
    }
}

/// Applies legal Winter adjustments and reports rejected orders. A power with
/// a deficit must provide exactly that many valid, distinct disbands.
pub fn apply_adjustments(
    state: &mut GameState,
    adjustments: &[Adjustment],
) -> Vec<AdjustmentResolution> {
    assert_eq!(state.phase, Phase::Winter);
    let before = state.clone();
    let mut results: Vec<_> = adjustments
        .iter()
        .copied()
        .map(|adjustment| AdjustmentResolution {
            adjustment,
            applied: false,
        })
        .collect();

    for power in Power::ALL {
        let centers = before.center_count(power);
        let armies = before.army_count(power);
        if centers > armies {
            let capacity = centers - armies;
            let mut built = 0;
            for (index, adjustment) in adjustments.iter().copied().enumerate() {
                match adjustment {
                    Adjustment::Build {
                        at,
                        power: order_power,
                    } if order_power == power
                        && built < capacity
                        && at == power.home_center()
                        && before.center_owner(at) == Some(CenterOwner::Power(power))
                        && state.occupant(at).is_none() =>
                    {
                        state.units_mut().insert(at, power);
                        results[index].applied = true;
                        built += 1;
                    }
                    Adjustment::Waive { power: order_power } if order_power == power => {
                        results[index].applied = true;
                    }
                    _ => {}
                }
            }
        } else if armies > centers {
            let required = armies - centers;
            let candidates: Vec<_> = adjustments
                .iter()
                .enumerate()
                .filter_map(|(index, adjustment)| match *adjustment {
                    Adjustment::Disband {
                        at,
                        power: order_power,
                    } if order_power == power && before.occupant(at) == Some(power) => {
                        Some((index, at))
                    }
                    _ => None,
                })
                .collect();
            let unique: HashSet<_> = candidates.iter().map(|&(_, at)| at).collect();
            if candidates.len() == required && unique.len() == required {
                for (index, at) in candidates {
                    state.units_mut().remove(&at);
                    results[index].applied = true;
                }
            }
        }
    }

    results
}

fn normalize_orders(
    state: &GameState,
    submitted: &[Order],
) -> (HashMap<Location, Order>, HashMap<Location, OrderResolution>) {
    let mut by_source: HashMap<Location, Vec<Order>> = HashMap::new();
    for &order in submitted {
        by_source.entry(order.source()).or_default().push(order);
    }

    let mut orders = HashMap::new();
    let mut resolutions = HashMap::new();
    for &source in state.units().keys() {
        let choices = by_source.get(&source);
        let submitted_order = choices.and_then(|orders| (orders.len() == 1).then_some(orders[0]));
        let legal = submitted_order.is_some_and(|order| is_legal_order(state, order));
        let resolved_as = if legal {
            submitted_order.unwrap()
        } else {
            Order::Hold { at: source }
        };
        orders.insert(source, resolved_as);
        resolutions.insert(
            source,
            OrderResolution {
                submitted: submitted_order,
                resolved_as,
                legal,
                succeeded: false,
                support_status: None,
            },
        );
    }
    (orders, resolutions)
}

fn is_legal_order(state: &GameState, order: Order) -> bool {
    let source = order.source();
    if state.occupant(source).is_none() {
        return false;
    }
    match order {
        Order::Hold { .. } => true,
        Order::Move { from, to } => from.is_adjacent_to(to),
        Order::SupportHold { from, unit } => {
            state.occupant(unit).is_some() && from.is_adjacent_to(unit)
        }
        Order::SupportMove { from, unit, to } => {
            state.occupant(unit).is_some() && unit.is_adjacent_to(to) && from.is_adjacent_to(to)
        }
    }
}

fn cut_supporters(state: &GameState, orders: &HashMap<Location, Order>) -> HashSet<Location> {
    let mut cut = HashSet::new();
    for (&attacker, &order) in orders {
        let Order::Move { to, .. } = order else {
            continue;
        };
        let attacker_power = state.units()[&attacker];
        if state
            .occupant(to)
            .is_some_and(|power| power != attacker_power)
            && matches!(
                orders[&to],
                Order::SupportHold { .. } | Order::SupportMove { .. }
            )
        {
            cut.insert(to);
        }
    }
    cut
}

fn effective_supports(
    orders: &HashMap<Location, Order>,
    cut: &HashSet<Location>,
    resolutions: &mut HashMap<Location, OrderResolution>,
) -> HashMap<Location, usize> {
    let mut counts = HashMap::new();
    for (&supporter, &order) in orders {
        let target = match order {
            Order::SupportHold { unit, .. }
                if matches!(orders.get(&unit), Some(Order::Hold { .. })) =>
            {
                Some(unit)
            }
            Order::SupportMove { unit, to, .. } if matches!(orders.get(&unit), Some(Order::Move { to: actual, .. }) if *actual == to) => {
                Some(unit)
            }
            Order::SupportHold { .. } | Order::SupportMove { .. } => None,
            Order::Hold { .. } | Order::Move { .. } => continue,
        };

        let status = if cut.contains(&supporter) {
            SupportStatus::Cut
        } else if target.is_some() {
            SupportStatus::Effective
        } else {
            SupportStatus::Ineffective
        };
        resolutions.get_mut(&supporter).unwrap().support_status = Some(status);
        if status == SupportStatus::Effective {
            *counts.entry(target.unwrap()).or_insert(0) += 1;
        }
    }
    counts
}
