use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Power {
    Green,
    Yellow,
    Red,
    Blue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Spring,
    Autumn,
    Winter,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Location {
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    B1,
    B2,
    B3,
    B4,
    B5,
    B6,
    C1,
    C2,
    C3,
    C4,
    C5,
    C6,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
}

impl Location {
    pub const ALL: [Self; 36] = [
        Self::A1,
        Self::A2,
        Self::A3,
        Self::A4,
        Self::A5,
        Self::A6,
        Self::B1,
        Self::B2,
        Self::B3,
        Self::B4,
        Self::B5,
        Self::B6,
        Self::C1,
        Self::C2,
        Self::C3,
        Self::C4,
        Self::C5,
        Self::C6,
        Self::D1,
        Self::D2,
        Self::D3,
        Self::D4,
        Self::D5,
        Self::D6,
        Self::E1,
        Self::E2,
        Self::E3,
        Self::E4,
        Self::E5,
        Self::E6,
        Self::F1,
        Self::F2,
        Self::F3,
        Self::F4,
        Self::F5,
        Self::F6,
    ];

    pub fn is_adjacent_to(self, other: Self) -> bool {
        let (row, column) = self.coordinates();
        let (other_row, other_column) = other.coordinates();
        row.abs_diff(other_row) + column.abs_diff(other_column) == 1
    }

    fn coordinates(self) -> (u8, u8) {
        let index = self as u8;
        (index / 6, index % 6)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CenterOwner {
    Power(Power),
    Neutral,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameState {
    pub phase: Phase,
    pub year: u8,
    units: HashMap<Location, Power>,
    centers: HashMap<Location, CenterOwner>,
}

impl GameState {
    pub const SUPPLY_CENTERS: [Location; 8] = [
        Location::A1,
        Location::B2,
        Location::B5,
        Location::C3,
        Location::C6,
        Location::D2,
        Location::E3,
        Location::E5,
    ];

    pub fn empty(phase: Phase, year: u8) -> Self {
        let centers = Self::SUPPLY_CENTERS
            .into_iter()
            .map(|location| (location, CenterOwner::Neutral))
            .collect();
        Self {
            phase,
            year,
            units: HashMap::new(),
            centers,
        }
    }

    pub fn initial() -> Self {
        let mut state = Self::empty(Phase::Spring, 1);
        for power in Power::ALL {
            let home = power.home_center();
            state.units.insert(home, power);
            state.centers.insert(home, CenterOwner::Power(power));
        }
        state
    }

    pub fn with_unit(mut self, location: Location, power: Power) -> Self {
        self.units.insert(location, power);
        self
    }

    pub fn with_center_owner(mut self, location: Location, owner: CenterOwner) -> Self {
        assert!(
            Self::SUPPLY_CENTERS.contains(&location),
            "{location:?} is not a supply center"
        );
        self.centers.insert(location, owner);
        self
    }

    pub fn occupant(&self, location: Location) -> Option<Power> {
        self.units.get(&location).copied()
    }

    pub fn center_owner(&self, location: Location) -> Option<CenterOwner> {
        self.centers.get(&location).copied()
    }

    pub fn army_count(&self, power: Power) -> usize {
        self.units
            .values()
            .filter(|&&occupant| occupant == power)
            .count()
    }

    pub fn center_count(&self, power: Power) -> usize {
        self.centers
            .values()
            .filter(|&&owner| owner == CenterOwner::Power(power))
            .count()
    }

    pub(crate) fn units(&self) -> &HashMap<Location, Power> {
        &self.units
    }

    pub(crate) fn units_mut(&mut self) -> &mut HashMap<Location, Power> {
        &mut self.units
    }

    pub(crate) fn centers_mut(&mut self) -> &mut HashMap<Location, CenterOwner> {
        &mut self.centers
    }
}

impl Power {
    pub const ALL: [Self; 4] = [Self::Green, Self::Yellow, Self::Red, Self::Blue];

    pub const fn home_center(self) -> Location {
        match self {
            Self::Green => Location::A1,
            Self::Yellow => Location::D2,
            Self::Red => Location::B5,
            Self::Blue => Location::E5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Order {
    Hold {
        at: Location,
    },
    Move {
        from: Location,
        to: Location,
    },
    SupportHold {
        from: Location,
        unit: Location,
    },
    SupportMove {
        from: Location,
        unit: Location,
        to: Location,
    },
}

impl Order {
    pub const fn source(self) -> Location {
        match self {
            Self::Hold { at } => at,
            Self::Move { from, .. }
            | Self::SupportHold { from, .. }
            | Self::SupportMove { from, .. } => from,
        }
    }
}

pub type OrderSet = Vec<Order>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportStatus {
    Effective,
    Ineffective,
    Cut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderResolution {
    pub submitted: Option<Order>,
    pub resolved_as: Order,
    pub legal: bool,
    pub succeeded: bool,
    pub support_status: Option<SupportStatus>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MovementResult {
    pub units: HashMap<Location, Power>,
    pub orders: HashMap<Location, OrderResolution>,
    pub dislodged: Vec<(Location, Power)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Adjustment {
    Build { at: Location, power: Power },
    Disband { at: Location, power: Power },
    Waive { power: Power },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdjustmentResolution {
    pub adjustment: Adjustment,
    pub applied: bool,
}
