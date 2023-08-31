//! Interval expression

use std::{
    fmt::{Display, Write},
    str::FromStr,
};

use owo_colors::OwoColorize;
use rand::{
    distributions::{Open01, OpenClosed01},
    Rng,
};

use crate::regex;
use crate::Pcg;

/// Int type used in the interval
pub type Int = i32;
/// Float type used in the interval
pub type Float = f32;

/// Description of an interval
#[derive(Debug, Clone, PartialEq)]
pub struct Interval {
    low_inc: bool,
    high_inc: bool,
    kind: IntervalKind,
}

#[derive(Debug, Clone, PartialEq)]
enum IntervalKind {
    Int(std::ops::Range<Int>),
    Float(std::ops::Range<Float>),
}

/// Error from [`Interval::from_str`]
#[derive(Debug, thiserror::Error)]
pub enum IntervalParseError {
    #[error("the input is not an interval")]
    NoMatch,
    #[error("invalid interval: {0}")]
    Invalid(String),
}

impl FromStr for Interval {
    type Err = IntervalParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match parse_range(s) {
            Err(IntervalParseError::NoMatch) => {}
            other => return other,
        }
        parse_interval(s)
    }
}

const START: &str = "start";
const END: &str = "end";
const TOO_BIG: &str = "value is too big";
const EMPTY_INTERVAL: &str = "the interval is empty";

fn parse_int(num: &str, part: &str) -> Result<Int, IntervalParseError> {
    num.parse::<Int>()
        .map_err(|e| IntervalParseError::Invalid(format!("{part}: {e}")))
}

fn parse_float(num: &str, part: &str) -> Result<Float, IntervalParseError> {
    num.parse::<Float>()
        .map_err(|e| IntervalParseError::Invalid(format!("{part}: {e}")))
}

fn build_int_range(
    mut start: Int,
    mut end: Int,
    low_inc: bool,
    high_inc: bool,
) -> Result<std::ops::Range<Int>, IntervalParseError> {
    if !low_inc {
        start = start
            .checked_add(1)
            .ok_or_else(|| IntervalParseError::Invalid(format!("{START} {TOO_BIG}")))?;
    }
    if high_inc {
        end = end
            .checked_add(1)
            .ok_or_else(|| IntervalParseError::Invalid(format!("{END} {TOO_BIG}")))?;
    }
    let range = start..end;
    if range.is_empty() {
        return Err(IntervalParseError::Invalid(EMPTY_INTERVAL.to_string()));
    }
    Ok(range)
}

fn parse_interval(s: &str) -> Result<Interval, IntervalParseError> {
    let re = regex!(
        r"\A([\[\(])\s*((?:\+|-)?(?:\d*\.)?\d+)\s*(,|\.{2})\s*((?:\+|-)?(?:\d*\.)?\d+)\s*([\]\)])\z"
    );

    let caps = re.captures(s).ok_or(IntervalParseError::NoMatch)?;

    let low_inc = &caps[1] == "[";
    let high_inc = &caps[5] == "]";
    let start = &caps[2];
    let end = &caps[4];
    let is_float = &caps[3] == "," || start.contains('.') || end.contains('.');

    let kind = if is_float {
        let start = parse_float(start, START)?;
        let end = parse_float(end, END)?;
        let range = start..end;
        if range.is_empty() {
            return Err(IntervalParseError::Invalid(EMPTY_INTERVAL.to_string()));
        }
        IntervalKind::Float(start..end)
    } else {
        let start = parse_int(start, START)?;
        let end = parse_int(end, END)?;
        let range = build_int_range(start, end, low_inc, high_inc)?;
        IntervalKind::Int(range)
    };
    Ok(Interval {
        low_inc,
        high_inc,
        kind,
    })
}

fn parse_range(s: &str) -> Result<Interval, IntervalParseError> {
    let re = regex!(r"\A((?:\+|-)?\d+)..(=)?((?:\+|-)?\d+)\z");

    let caps = re.captures(s).ok_or(IntervalParseError::NoMatch)?;

    let start = parse_int(&caps[1], START)?;
    let end = parse_int(&caps[3], END)?;
    let inclusive = caps.get(2).is_some();

    let range = build_int_range(start, end, true, inclusive)?;

    Ok(Interval {
        low_inc: true,
        high_inc: inclusive,
        kind: IntervalKind::Int(range),
    })
}

impl Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.low_inc {
            true => f.write_char('[')?,
            false => f.write_char('(')?,
        }

        match &self.kind {
            IntervalKind::Int(r) => {
                let mut start = r.start;
                if !self.low_inc {
                    start = start.checked_sub(1).unwrap(); // checked in creation
                }
                let mut end = r.end;
                if self.high_inc {
                    end = end.checked_sub(1).unwrap(); // checked in creation
                }
                write!(f, "{start}..{end}")?;
            }
            IntervalKind::Float(r) => {
                let start = r.start;
                let end = r.end;
                write!(f, "{start}, {end}")?;
            }
        }

        match self.low_inc {
            true => f.write_char(']'),
            false => f.write_char(')'),
        }
    }
}

/// Sample from an interval
///
/// The [`Display`] [alternate modifier](std::fmt#sign0) will only print
/// the sampled value.
#[derive(Debug, Clone, PartialEq)]
pub struct IntervalResult {
    /// Original interval
    pub interval: Interval,
    /// Value obtained
    pub value: Num,
}

/// Either an [`Int`] or a [`Float`].
#[derive(Debug, Clone, PartialEq)]
pub enum Num {
    Int(Int),
    Float(Float),
}

impl Interval {
    pub(crate) fn eval(&self, rng: &mut Pcg) -> IntervalResult {
        let Interval {
            low_inc,
            high_inc,
            kind,
        } = self;
        let value = match kind {
            IntervalKind::Int(r) => Num::Int(rng.gen_range(r.clone())),
            IntervalKind::Float(r) => {
                let f = match (low_inc, high_inc) {
                    (true, true) => rng.gen_range(r.start..=r.end),
                    (true, false) => rng.gen_range(r.start..r.end),
                    (false, true) => {
                        let val: Float = rng.sample(OpenClosed01);
                        let scale = r.end - r.start;
                        val * scale + r.start
                    }
                    (false, false) => {
                        let val: Float = rng.sample(Open01);
                        let scale = r.end - r.start;
                        val * scale + r.start
                    }
                };
                Num::Float(f)
            }
        };
        IntervalResult {
            interval: self.clone(),
            value,
        }
    }
}

impl Display for IntervalResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            self.value.fmt(f)
        } else {
            write!(f, "{}: {}", self.interval.bold().yellow(), self.value)
        }
    }
}

impl Display for Num {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Num::Int(n) => n.fmt(f),
            Num::Float(n) => n.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("[1..10]" => 1..11 ; "inclusive")]
    #[test_case("[1..10)" => 1..10 ; "end exclusive")]
    #[test_case("(1..10]" => 2..11 ; "start exclusive")]
    #[test_case("(1..10)" => 2..10 ; "exclusive")]
    #[test_case("1..10" => 1..10 ; "alt exclusive")]
    #[test_case("1..=10" => 1..11 ; "alt inclusive")]
    #[test_case("[-5..-3)" => -5..-3 ; "neg")]
    #[test_case("[-5..-3]" => -5..-2 ; "neg inclusive")]
    #[test_case("-5..-3" => -5..-3 ; "alt neg")]
    #[test_case("-5..=-3" => -5..-2 ; "alt neg inclusive")]
    fn parse_int(s: &str) -> std::ops::Range<Int> {
        let interval = s.parse::<Interval>().expect("failed to parse");
        match interval.kind {
            IntervalKind::Int(r) => r,
            IntervalKind::Float(_) => panic!("not int"),
        }
    }

    #[test_case("[1,10]" => (1.0..10.0, true, true) ; "inclusive")]
    #[test_case("[1,10)" => (1.0..10.0, true, false) ; "end exclusive")]
    #[test_case("(1,10]" => (1.0..10.0, false, true) ; "start exclusive")]
    #[test_case("(1,10)" => (1.0..10.0, false, false) ; "exclusive")]
    #[test_case("(1.0,10.0)" => (1.0..10.0, false, false) ; "full decimal")]
    #[test_case("(1.0,10)" => (1.0..10.0, false, false) ; "only first decimal")]
    #[test_case("(1,10.0)" => (1.0..10.0, false, false) ; "only second decimal")]
    #[test_case("(1.,10)" => panics "failed to parse" ; "bad partial decimal start")] // no float with trailing .
    #[test_case("(0,.9)" => (0.0..0.9, false, false) ; "bad partial decimal end")]
    #[test_case("(1.0,10.0)" => (1.0..10.0, false, false) ; "partial decimal")]
    #[test_case("(1.0..10.0)" => (1.0..10.0, false, false) ; "decimal on int")]
    #[test_case("(.5..1)" => (0.5..1.0, false, false) ; "one decimal on int")]
    #[test_case("(1..10)" => panics "not float" ; "int")]
    #[test_case("(-1, 1)" => (-1.0..1.0, false, false) ; "neg start")]
    #[test_case("(2, -1)" => panics "failed to parse" ; "neg end")] // start > end
    #[test_case("(-2, -1)" => (-2.0..-1.0, false, false) ; "neg")]
    fn parse_float(s: &str) -> (std::ops::Range<Float>, bool, bool) {
        let interval = s.parse::<Interval>().expect("failed to parse");
        match interval.kind {
            IntervalKind::Int(_) => panic!("not float"),
            IntervalKind::Float(r) => (r, interval.low_inc, interval.high_inc),
        }
    }
}
