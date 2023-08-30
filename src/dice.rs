//! Dice expression

use owo_colors::OwoColorize;
use rand::Rng;

use crate::regex;
use crate::Pcg;
use std::{fmt::Display, str::FromStr};

/// A description of a dice roll
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Roll {
    /// Number of dice
    pub amount: u16,
    /// Number of sides
    pub sides: u16,
}

/// Error from [`Roll::from_str`]
#[derive(Debug, thiserror::Error)]
pub enum RollParseError {
    #[error("The input is not a dice roll")]
    NoMatch,
    #[error("Invalid dice roll: {0}")]
    Invalid(String),
}

impl FromStr for Roll {
    type Err = RollParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = regex!(r"(\d+)?d(\d+|%)");

        let caps = re.captures(s).ok_or(RollParseError::NoMatch)?;

        let amount = caps.get(1).map_or(Ok(1), |m| {
            m.as_str()
                .parse::<u16>()
                .map_err(|e| RollParseError::Invalid(format!("bad amount: {e}")))
                .and_then(|a| {
                    if a == 0 {
                        Err(RollParseError::Invalid("amount can't be 0".to_string()))
                    } else {
                        Ok(a)
                    }
                })
        })?;
        let sides = match &caps[2] {
            "%" => 100,
            num => num
                .parse::<u16>()
                .map_err(|e| RollParseError::Invalid(format!("bad number of sides: {e}")))
                .and_then(|s| {
                    if s == 0 {
                        Err(RollParseError::Invalid(
                            "number of sides can't be 0".to_string(),
                        ))
                    } else {
                        Ok(s)
                    }
                })?,
        };
        Ok(Roll { amount, sides })
    }
}

impl Display for Roll {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use owo_colors::AnsiColors::*;
        let color = match self.sides {
            1 => BrightBlack,
            4 => BrightGreen,
            6 => BrightBlue,
            8 => BrightRed,
            10 => BrightCyan,
            12 => BrightYellow,
            20 => BrightMagenta,
            _ => BrightWhite,
        };

        if self.amount > 1 {
            write!(f, "{}", self.amount.color(color).italic())?;
        }
        write!(f, "{}{}", "d".color(color), self.sides.color(color))
    }
}

/// Result of a [`Roll`] evaluation
///
/// The [`Display`] [alternate modifier](std::fmt#sign0) will only print
/// [`RollResult::total`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollResult {
    /// Original roll description
    pub roll: Roll,
    dice: Vec<u16>,
    total: u32,
}

impl Roll {
    pub(crate) fn eval(&self, rng: &mut Pcg) -> RollResult {
        let dice: Vec<_> = (0..self.amount)
            .map(|_| rng.gen_range(1..=self.sides))
            .collect();
        RollResult {
            roll: *self,
            total: dice.iter().map(|&v| v as u32).sum(),
            dice,
        }
    }
}

impl RollResult {
    /// Results obtained
    pub fn dice(&self) -> &[u16] {
        &self.dice
    }

    /// Total value
    pub fn total(&self) -> u32 {
        self.total
    }
}

impl Display for RollResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            return self.total.fmt(f);
        }

        write!(f, "{}: ", self.roll)?;
        if self.dice.len() == 1 {
            write!(f, "{}", self.dice[0])
        } else {
            write!(f, "[{}", self.dice[0])?;
            for val in &self.dice[1..] {
                write!(f, "{}{val}", "+".dimmed())?;
            }
            write!(f, "] = {}", self.total)
        }
    }
}
