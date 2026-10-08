use std::{fmt, str::FromStr};

use crate::{ProtocolError, ProtocolResult};
use game_engine::{Location, Phase, Power};
use press_engine::Visibility;

pub const PROTOCOL_VERSION: &str = "tiny-diplomacy/v0-press";
pub const TOKEN_COUNT: usize = 82;
pub const TOKEN_MANIFEST_SHA256: &str =
    "1f21b70055bc131316ea17152c7be4928e5d0e82fef018b88ad950eee0ae83e7";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Token {
    Bos = 0,
    Board = 1,
    Centers = 2,
    Orders = 3,
    Adjust = 4,
    Press = 5,
    Diplomacy = 6,
    Inbox = 7,
    Proposals = 8,
    Commitments = 9,
    History = 10,
    End = 11,
    Green = 12,
    Yellow = 13,
    Red = 14,
    Blue = 15,
    Neutral = 16,
    Empty = 17,
    Spring = 18,
    Autumn = 19,
    Winter = 20,
    Year1 = 21,
    Year2 = 22,
    Year3 = 23,
    Year4 = 24,
    Year5 = 25,
    Year6 = 26,
    Year7 = 27,
    Year8 = 28,
    Year9 = 29,
    Year10 = 30,
    Hold = 31,
    Move = 32,
    Support = 33,
    Build = 34,
    Waive = 35,
    Disband = 36,
    None = 37,
    Public = 38,
    Private = 39,
    Propose = 40,
    Promise = 41,
    Request = 42,
    Accept = 43,
    Reject = 44,
    Avoid = 45,
    A1 = 46,
    A2 = 47,
    A3 = 48,
    A4 = 49,
    A5 = 50,
    A6 = 51,
    B1 = 52,
    B2 = 53,
    B3 = 54,
    B4 = 55,
    B5 = 56,
    B6 = 57,
    C1 = 58,
    C2 = 59,
    C3 = 60,
    C4 = 61,
    C5 = 62,
    C6 = 63,
    D1 = 64,
    D2 = 65,
    D3 = 66,
    D4 = 67,
    D5 = 68,
    D6 = 69,
    E1 = 70,
    E2 = 71,
    E3 = 72,
    E4 = 73,
    E5 = 74,
    E6 = 75,
    F1 = 76,
    F2 = 77,
    F3 = 78,
    F4 = 79,
    F5 = 80,
    F6 = 81,
}

impl Token {
    pub const ALL: [Self; TOKEN_COUNT] = [
        Self::Bos,
        Self::Board,
        Self::Centers,
        Self::Orders,
        Self::Adjust,
        Self::Press,
        Self::Diplomacy,
        Self::Inbox,
        Self::Proposals,
        Self::Commitments,
        Self::History,
        Self::End,
        Self::Green,
        Self::Yellow,
        Self::Red,
        Self::Blue,
        Self::Neutral,
        Self::Empty,
        Self::Spring,
        Self::Autumn,
        Self::Winter,
        Self::Year1,
        Self::Year2,
        Self::Year3,
        Self::Year4,
        Self::Year5,
        Self::Year6,
        Self::Year7,
        Self::Year8,
        Self::Year9,
        Self::Year10,
        Self::Hold,
        Self::Move,
        Self::Support,
        Self::Build,
        Self::Waive,
        Self::Disband,
        Self::None,
        Self::Public,
        Self::Private,
        Self::Propose,
        Self::Promise,
        Self::Request,
        Self::Accept,
        Self::Reject,
        Self::Avoid,
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

    pub const SYMBOLS: [&'static str; TOKEN_COUNT] = [
        "BOS",
        "BOARD",
        "CENTERS",
        "ORDERS",
        "ADJUST",
        "PRESS",
        "DIPLOMACY",
        "INBOX",
        "PROPOSALS",
        "COMMITMENTS",
        "HISTORY",
        "END",
        "GREEN",
        "YELLOW",
        "RED",
        "BLUE",
        "NEUTRAL",
        "EMPTY",
        "SPRING",
        "AUTUMN",
        "WINTER",
        "YEAR_1",
        "YEAR_2",
        "YEAR_3",
        "YEAR_4",
        "YEAR_5",
        "YEAR_6",
        "YEAR_7",
        "YEAR_8",
        "YEAR_9",
        "YEAR_10",
        "HOLD",
        "MOVE",
        "SUPPORT",
        "BUILD",
        "WAIVE",
        "DISBAND",
        "NONE",
        "PUBLIC",
        "PRIVATE",
        "PROPOSE",
        "PROMISE",
        "REQUEST",
        "ACCEPT",
        "REJECT",
        "AVOID",
        "A1",
        "A2",
        "A3",
        "A4",
        "A5",
        "A6",
        "B1",
        "B2",
        "B3",
        "B4",
        "B5",
        "B6",
        "C1",
        "C2",
        "C3",
        "C4",
        "C5",
        "C6",
        "D1",
        "D2",
        "D3",
        "D4",
        "D5",
        "D6",
        "E1",
        "E2",
        "E3",
        "E4",
        "E5",
        "E6",
        "F1",
        "F2",
        "F3",
        "F4",
        "F5",
        "F6",
    ];

    pub const fn id(self) -> TokenId {
        TokenId(self as u8)
    }

    pub const fn symbol(self) -> &'static str {
        Self::SYMBOLS[self as usize]
    }
}

impl fmt::Display for Token {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.symbol())
    }
}

impl TryFrom<u8> for Token {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::ALL
            .get(value as usize)
            .copied()
            .ok_or(ProtocolError::UnknownTokenId(value))
    }
}

impl FromStr for Token {
    type Err = ProtocolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::SYMBOLS
            .iter()
            .position(|symbol| *symbol == value)
            .map(|index| Self::ALL[index])
            .ok_or_else(|| ProtocolError::UnknownToken(value.to_owned()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TokenId(u8);

impl TokenId {
    pub const fn value(self) -> u8 {
        self.0
    }

    pub fn token(self) -> Token {
        // TokenId can only be constructed through validated conversions.
        Token::ALL[self.0 as usize]
    }
}

impl TryFrom<u8> for TokenId {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Token::try_from(value).map(Token::id)
    }
}

impl From<Token> for TokenId {
    fn from(token: Token) -> Self {
        token.id()
    }
}

pub fn tokens_from_ids(ids: &[u8]) -> ProtocolResult<Vec<TokenId>> {
    ids.iter().copied().map(TokenId::try_from).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenMask([bool; TOKEN_COUNT]);

impl TokenMask {
    pub const fn empty() -> Self {
        Self([false; TOKEN_COUNT])
    }

    pub fn insert(&mut self, token: Token) {
        self.0[token as usize] = true;
    }

    pub fn contains(&self, token: Token) -> bool {
        self.0[token as usize]
    }

    pub fn is_empty(&self) -> bool {
        !self.0.iter().any(|allowed| *allowed)
    }

    pub fn tokens(&self) -> impl Iterator<Item = Token> + '_ {
        Token::ALL.into_iter().filter(|token| self.contains(*token))
    }

    pub fn as_bools(&self) -> &[bool; TOKEN_COUNT] {
        &self.0
    }
}

impl Default for TokenMask {
    fn default() -> Self {
        Self::empty()
    }
}

pub(crate) const fn power_token(power: Power) -> Token {
    match power {
        Power::Green => Token::Green,
        Power::Yellow => Token::Yellow,
        Power::Red => Token::Red,
        Power::Blue => Token::Blue,
    }
}

pub(crate) const fn token_power(token: Token) -> Option<Power> {
    match token {
        Token::Green => Some(Power::Green),
        Token::Yellow => Some(Power::Yellow),
        Token::Red => Some(Power::Red),
        Token::Blue => Some(Power::Blue),
        _ => None,
    }
}

pub(crate) const fn phase_token(phase: Phase) -> Token {
    match phase {
        Phase::Spring => Token::Spring,
        Phase::Autumn => Token::Autumn,
        Phase::Winter => Token::Winter,
    }
}

pub(crate) fn year_token(year: u8) -> ProtocolResult<Token> {
    if !(1..=10).contains(&year) {
        return Err(ProtocolError::IllegalResponse(format!(
            "year {year} is not representable"
        )));
    }
    Ok(Token::ALL[(Token::Year1 as usize) + usize::from(year - 1)])
}

pub(crate) fn location_token(location: Location) -> Token {
    let index = Location::ALL
        .iter()
        .position(|candidate| *candidate == location)
        .expect("Location::ALL must contain every location");
    Token::ALL[Token::A1 as usize + index]
}

pub(crate) fn token_location(token: Token) -> Option<Location> {
    let id = token as usize;
    let first = Token::A1 as usize;
    let offset = id.checked_sub(first)?;
    Location::ALL.get(offset).copied()
}

pub(crate) const fn visibility_token(visibility: Visibility) -> Token {
    match visibility {
        Visibility::Public => Token::Public,
        Visibility::Private => Token::Private,
    }
}

pub(crate) const fn token_visibility(token: Token) -> Option<Visibility> {
    match token {
        Token::Public => Some(Visibility::Public),
        Token::Private => Some(Visibility::Private),
        _ => None,
    }
}
