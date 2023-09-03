use std::{fmt::Display, rc::Rc};

use crate::{
    coin::{self, CoinResult},
    dice::{Roll, RollParseError, RollResult},
    interval::{Interval, IntervalParseError, IntervalSample},
    Error, Pcg,
};

/// Entry that can be cheaply cloned and shared to support repeated selection
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SharedEntry(Rc<EntryData>);

#[derive(Debug, Clone, PartialEq)]
enum EntryData {
    Text(Rc<str>),
    /// Entry is an expression
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Coin,
    Dice(Roll),
    Interval(Interval),
}

impl SharedEntry {
    pub fn new(text: Rc<str>, eval_expr: bool) -> Result<Self, Error> {
        if eval_expr {
            Self::expr(text)
        } else {
            Ok(Self::text(text))
        }
    }

    pub fn text(text: Rc<str>) -> Self {
        Self(Rc::new(EntryData::Text(text)))
    }

    pub fn expr(text: Rc<str>) -> Result<Self, Error> {
        let data = if let Some(expr) = parse_expr(&text)? {
            EntryData::Expr(expr)
        } else {
            EntryData::Text(text)
        };
        Ok(Self(Rc::new(data)))
    }

    pub fn eval(&self, rng: &mut Pcg) -> Entry {
        match self.0.as_ref() {
            EntryData::Text(t) => Entry::Text(Rc::clone(t)),
            EntryData::Expr(e) => e.eval(rng),
        }
    }
}

impl Expr {
    fn eval(&self, rng: &mut Pcg) -> Entry {
        match self {
            Expr::Coin => Entry::Coin(coin::toss_coin(rng)),
            Expr::Dice(r) => Entry::Dice(r.eval(rng)),
            Expr::Interval(i) => Entry::Interval(i.eval(rng)),
        }
    }
}

fn parse_expr(expr: &str) -> Result<Option<Expr>, Error> {
    if expr == "coin" {
        return Ok(Some(Expr::Coin));
    }

    match expr.parse::<Roll>() {
        Err(RollParseError::NoMatch) => {}
        Ok(r) => return Ok(Some(Expr::Dice(r))),
        Err(e) => return Err(Error::Expr(e.to_string())),
    }

    match expr.parse::<Interval>() {
        Err(IntervalParseError::NoMatch) => {}
        Ok(i) => return Ok(Some(Expr::Interval(i))),
        Err(e) => return Err(Error::Expr(e.to_string())),
    }

    Ok(None)
}

/// Entry result
///
/// The display impl is transparent. And the [`Display`] [alternate
/// modifier](std::fmt#sign0) will only print the result and not the expression
/// itself.
///
/// This enum is non exhaustive because adding a new expression will not be a
/// breaking change.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// Just text
    Text(Rc<str>),
    /// A coin flip
    Coin(CoinResult),
    /// A dice roll
    Dice(RollResult),
    /// An interval sample
    Interval(IntervalSample),
}

impl Entry {
    pub(crate) fn into_raw(self) -> Rc<str> {
        match self {
            Entry::Text(t) => t,
            _ => Rc::from(format!("{:#}", self).as_str()),
        }
    }
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
