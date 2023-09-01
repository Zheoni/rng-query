use std::{fmt::Display, rc::Rc};

use crate::{
    coin::{self, CoinResult},
    dice::{Roll, RollParseError, RollResult},
    interval::{Interval, IntervalParseError, IntervalResult},
    Error, Pcg,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BufferedEntry {
    pub id: usize,
    pub data: EntryData,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EntryData {
    Text(Rc<str>),
    Coin,
    Dice(Roll),
    Interval(Interval),
}

impl BufferedEntry {
    pub fn eval(self, rng: &mut Pcg) -> Entry {
        match self.data {
            EntryData::Text(t) => Entry::Text(t),
            EntryData::Coin => Entry::Coin(coin::toss_coin(rng)),
            EntryData::Dice(r) => Entry::Dice(r.eval(rng)),
            EntryData::Interval(i) => Entry::Interval(i.eval(rng)),
        }
    }
}

/// Entry result
///
/// The display impl is transparent. And the [`Display`] [alternate
/// modifier](std::fmt#sign0) will only print the result and not the expression
/// itself.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// Just text
    Text(Rc<str>),
    /// A coin flip
    Coin(CoinResult),
    /// A dice roll
    Dice(RollResult),
    /// An interval sample
    Interval(IntervalResult),
}

impl Display for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Entry::Text(t) => t.fmt(f),
            Entry::Coin(r) => r.fmt(f),
            Entry::Dice(r) => r.fmt(f),
            Entry::Interval(i) => i.fmt(f),
        }
    }
}

pub(crate) fn parse_expr(expr: &Rc<str>) -> Result<EntryData, Error> {
    if expr.as_ref() == "coin" {
        return Ok(EntryData::Coin);
    }

    match expr.parse::<Roll>() {
        Err(RollParseError::NoMatch) => {}
        Ok(r) => return Ok(EntryData::Dice(r)),
        Err(e) => return Err(Error::Expr(e.to_string())),
    }

    match expr.parse::<Interval>() {
        Err(IntervalParseError::NoMatch) => {}
        Ok(i) => return Ok(EntryData::Interval(i)),
        Err(e) => return Err(Error::Expr(e.to_string())),
    }

    Ok(EntryData::Text(Rc::clone(expr)))
}
