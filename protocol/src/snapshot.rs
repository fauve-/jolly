use game_engine::{GameState, Phase, Power};
use press_engine::{PressActivation, PressView};

use crate::{ProtocolError, ProtocolResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationMode {
    Movement,
    Adjustment,
    Press,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PressDecision {
    activation: PressActivation,
    view: PressView,
}

impl PressDecision {
    pub fn new(activation: PressActivation, view: PressView) -> Self {
        Self { activation, view }
    }

    pub const fn activation(&self) -> &PressActivation {
        &self.activation
    }

    pub const fn view(&self) -> &PressView {
        &self.view
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionSnapshot {
    state: GameState,
    acting_power: Power,
    mode: GenerationMode,
    press: Option<PressDecision>,
    diplomacy: Option<PressView>,
}

impl DecisionSnapshot {
    pub fn movement(state: &GameState, acting_power: Power) -> ProtocolResult<Self> {
        Self::new(state, acting_power, GenerationMode::Movement, None, None)
    }

    pub fn movement_with_diplomacy(
        state: &GameState,
        acting_power: Power,
        view: PressView,
    ) -> ProtocolResult<Self> {
        Self::new(
            state,
            acting_power,
            GenerationMode::Movement,
            None,
            Some(view),
        )
    }

    pub fn adjustment(state: &GameState, acting_power: Power) -> ProtocolResult<Self> {
        Self::new(state, acting_power, GenerationMode::Adjustment, None, None)
    }

    pub fn press(
        state: &GameState,
        activation: PressActivation,
        view: PressView,
    ) -> ProtocolResult<Self> {
        let acting_power = activation.power;
        let press = PressDecision::new(activation, view.clone());
        Self::new(
            state,
            acting_power,
            GenerationMode::Press,
            Some(press),
            Some(view),
        )
    }

    fn new(
        state: &GameState,
        acting_power: Power,
        mode: GenerationMode,
        press: Option<PressDecision>,
        diplomacy: Option<PressView>,
    ) -> ProtocolResult<Self> {
        if state.is_finished() {
            return Err(ProtocolError::TerminalGame);
        }
        let compatible = match mode {
            GenerationMode::Movement | GenerationMode::Press => {
                matches!(state.phase(), Phase::Spring | Phase::Autumn)
            }
            GenerationMode::Adjustment => state.phase() == Phase::Winter,
        };
        if !compatible {
            return Err(ProtocolError::IncompatibleMode { mode });
        }
        if mode == GenerationMode::Press && press.is_none() {
            return Err(ProtocolError::MissingPressDecision);
        }
        if press
            .as_ref()
            .is_some_and(|decision| decision.activation.power != acting_power)
        {
            return Err(ProtocolError::WrongPressActivation);
        }
        Ok(Self {
            state: state.clone(),
            acting_power,
            mode,
            press,
            diplomacy,
        })
    }

    pub const fn game_state(&self) -> &GameState {
        &self.state
    }

    pub const fn acting_power(&self) -> Power {
        self.acting_power
    }

    pub const fn mode(&self) -> GenerationMode {
        self.mode
    }

    pub const fn press_decision(&self) -> Option<&PressDecision> {
        self.press.as_ref()
    }

    pub const fn diplomacy(&self) -> Option<&PressView> {
        self.diplomacy.as_ref()
    }
}
