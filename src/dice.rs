//! Dice expression

use owo_colors::OwoColorize;
use rand::Rng;

use crate::regex;
use crate::Pcg;
use std::fmt::Write;
use std::{fmt::Display, str::FromStr};

/// A description of a dice roll
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Roll {
    /// Number of dice
    amount: u16,
    /// Number of sides
    sides: u16,
    /// Use exploding dice
    ///
    /// If a die results in it's maximum value (number of sides) an extra die
    /// is rolled.
    exploding: bool,
    /// See [`SelectDice`]
    select: Option<SelectDice>,
    /// Amount to add/subtract to the sum of the rolls
    modifier: i32,
}

/// Select a subset of the total dice rolled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectDice {
    /// Number of dice to select
    amount: u16,
    /// What to do with the selected dice
    action: SelectAction,
    /// Which dice to select
    which: SelectWhich,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectAction {
    Keep,
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectWhich {
    High,
    Low,
}

/// Error from [`Roll::from_str`]
#[derive(Debug, thiserror::Error)]
pub enum RollParseError {
    #[error("the input is not a dice roll")]
    NoMatch,
    #[error("invalid dice roll: {0}")]
    Invalid(String),
}

impl FromStr for Roll {
    type Err = RollParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = regex!(r"\A(\d+)?d(\d+|%)(!)?(([kd][hl]?)(\d+)?)?((?:[+-]\d+)+)?\z");

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

        let exploding = caps.get(3).is_some();

        let select = if caps.get(4).is_some() {
            let (action, which) = match &caps[5] {
                "k" | "kh" => (SelectAction::Keep, SelectWhich::High),
                "kl" => (SelectAction::Keep, SelectWhich::Low),
                "d" | "dl" => (SelectAction::Drop, SelectWhich::Low),
                "dh" => (SelectAction::Drop, SelectWhich::High),
                _ => panic!("unknown select kind"),
            };
            let amount = caps.get(6).map_or(Ok(1), |m| {
                m.as_str()
                    .parse::<u16>()
                    .map_err(|e| RollParseError::Invalid(format!("bad select amount: {e}")))
                    .and_then(|a| {
                        if a == 0 {
                            Err(RollParseError::Invalid(
                                "select amount can't be 0".to_string(),
                            ))
                        } else {
                            Ok(a)
                        }
                    })
            })?;
            Some(SelectDice {
                action,
                which,
                amount,
            })
        } else {
            None
        };

        let modifier = caps.get(7).map_or(Ok(0), |m| {
            let re = regex!(r"[+-]\d+");
            re.find_iter(m.as_str())
                .map(|m| {
                    m.as_str()
                        .parse::<i32>()
                        .map_err(|e| RollParseError::Invalid(format!("bad modifier: {e}")))
                })
                .sum::<Result<i32, _>>()
        })?;

        Ok(Roll {
            amount,
            sides,
            exploding,
            select,
            modifier,
        })
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
        write!(f, "{}{}", "d".color(color), self.sides.color(color))?;
        if self.exploding {
            f.write_char('!')?;
        }
        if let Some(select) = self.select {
            let s = match (select.action, select.which) {
                (SelectAction::Keep, SelectWhich::High) => "k",
                (SelectAction::Keep, SelectWhich::Low) => "kl",
                (SelectAction::Drop, SelectWhich::High) => "dh",
                (SelectAction::Drop, SelectWhich::Low) => "d",
            };
            f.write_str(s)?;
            if select.amount > 1 {
                write!(f, "{}", select.amount)?;
            }
        }
        print_modifier(f, self.modifier)?;

        Ok(())
    }
}

/// Result of a [`Roll`] evaluation
///
/// The [`Display`] [alternate modifier](std::fmt#sign0) will only print
/// [`RollResult::total`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollResult {
    /// Original roll description
    roll: Roll,
    dice: Vec<Die>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Die {
    pub val: u16,
    pub drop: bool,
}

impl Roll {
    pub(crate) fn eval(&self, rng: &mut Pcg) -> RollResult {
        let mut dice = Vec::new();

        for _ in 0..self.amount {
            loop {
                let val = rng.gen_range(1..=self.sides);
                dice.push(Die { val, drop: false });
                if !(self.exploding && val == self.sides) {
                    break;
                }
            }
        }

        if let Some(select) = &self.select {
            let n = select.amount as usize;
            dice.sort_unstable();
            let drop_die = |d: &mut Die| d.drop = true;
            match (select.action, select.which) {
                (SelectAction::Keep, SelectWhich::High) => {
                    dice.iter_mut().rev().skip(n).for_each(drop_die);
                }
                (SelectAction::Keep, SelectWhich::Low) => {
                    dice.iter_mut().skip(n).for_each(drop_die)
                }
                (SelectAction::Drop, SelectWhich::High) => {
                    dice.iter_mut().rev().take(n).for_each(drop_die)
                }
                (SelectAction::Drop, SelectWhich::Low) => {
                    dice.iter_mut().take(n).for_each(drop_die)
                }
            }
        }

        RollResult { roll: *self, dice }
    }
}

impl RollResult {
    pub fn roll(&self) -> &Roll {
        &self.roll
    }

    /// Results obtained
    pub fn rolled_dice(&self) -> &[Die] {
        &self.dice
    }

    pub fn taken_dice(&self) -> impl Iterator<Item = u16> + '_ {
        self.dice.iter().filter_map(|d| (!d.drop).then_some(d.val))
    }

    /// Total value
    pub fn total(&self) -> i32 {
        self.taken_dice().map(|v| v as i32).sum::<i32>() + self.roll.modifier
    }
}

impl Display for RollResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            return self.total().fmt(f);
        }

        write!(f, "{}: ", self.roll)?;

        if self.roll.exploding || self.roll.select.is_some() || self.roll.modifier != 0 {
            write!(f, "[{}", self.dice[0])?;
            for val in &self.dice[1..] {
                write!(f, "{}{val}", "+".dimmed())?;
            }
            write!(f, "]")?;
            print_modifier(f, self.roll.modifier)?;
            write!(f, " = ")?;
        }

        write!(f, "{}", self.total())
    }
}

impl Display for Die {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.drop {
            write!(f, "{}{}", self.val.dimmed().red(), "d".dimmed().red())
        } else {
            self.val.fmt(f)
        }
    }
}

fn print_modifier(f: &mut std::fmt::Formatter<'_>, modifier: i32) -> std::fmt::Result {
    match modifier {
        0 => Ok(()),
        1.. => {
            write!(f, "{:+}", modifier.green())
        }
        ..=-1 => {
            write!(f, "{:+}", modifier.red())
        }
    }
}
