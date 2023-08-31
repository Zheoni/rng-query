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

#[derive(Debug, thiserror::Error)]
pub enum SplitEntriesError {
    #[error("missing trailing '\"' to close a string")]
    UnclosedString { start: usize },
    #[error("unbalanced {what}")]
    UnbalancedNesting { problem: usize, what: &'static str },
}

pub(crate) fn split_entries<'i>(
    line: &'i str,
    sep: char,
) -> impl Iterator<Item = Result<&'i str, SplitEntriesError>> {
    let mut split = SplitEntries::new(line, sep);
    let mut error = false;
    std::iter::from_fn(move || {
        if error {
            return None;
        }
        let val = split.next()?;
        if val.is_err() {
            error = true;
        }
        Some(val)
    })
    .fuse()
}

#[derive(Debug)]
struct SplitEntries<'i> {
    line: &'i str,
    sep: char,
    last_end: usize,
    chars: std::str::CharIndices<'i>,
}

impl<'i> SplitEntries<'i> {
    fn new(line: &'i str, sep: char) -> Self {
        Self {
            line,
            sep,
            last_end: 0,
            chars: line.char_indices(),
        }
    }

    fn trim_entry(&self, mut entry: &'i str) -> &'i str {
        entry = entry.trim_start_matches(self.sep).trim();
        if entry.starts_with('"')
            && entry.ends_with('"')
            && entry.chars().filter(|&c| c == '"').count() == 2
        {
            entry = entry.trim_matches('"').trim()
        }
        entry
    }
}

impl<'i> Iterator for SplitEntries<'i> {
    type Item = Result<&'i str, SplitEntriesError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.chars.as_str().is_empty() {
            return None;
        }

        // Stack can be local because an entry is never returned if the stack is
        // not empty
        let mut stack = Vec::new();

        while let Some((i, c)) = self.chars.next() {
            match c {
                '"' => {
                    let mut end = false;
                    while let Some((_, c)) = self.chars.next() {
                        if c == '"' {
                            end = true;
                            break;
                        }
                    }
                    if !end {
                        return Some(Err(SplitEntriesError::UnclosedString { start: i }));
                    }
                }
                '(' | '[' | '{' => stack.push(Nest::from_char(c)),
                ']' | ')' | '}' => match stack.last() {
                    Some(pending) if pending.matches(Nest::from_char(c)) => {
                        stack.pop();
                    }
                    Some(pending) => {
                        return Some(Err(SplitEntriesError::UnbalancedNesting {
                            problem: i,
                            what: pending.repr(),
                        }))
                    }
                    None => {
                        return Some(Err(SplitEntriesError::UnbalancedNesting {
                            problem: i,
                            what: Nest::from_char(c).repr(),
                        }));
                    }
                },
                c if c == self.sep && stack.is_empty() => {
                    let e = self.trim_entry(&self.line[self.last_end..i]);
                    self.last_end = i;
                    return Some(Ok(e));
                }
                _ => {}
            }
        }

        match stack.last() {
            None => {
                let e = self.trim_entry(&self.line[self.last_end..]);
                return Some(Ok(e));
            }
            Some(pending) => {
                return Some(Err(SplitEntriesError::UnbalancedNesting {
                    problem: self.line.len(),
                    what: pending.repr(),
                }))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Nest {
    Paren,
    Square,
    Curly,
}
impl Nest {
    fn from_char(c: char) -> Self {
        match c {
            '(' | ')' => Self::Paren,
            '[' | ']' => Self::Square,
            '{' | '}' => Self::Curly,
            _ => panic!("unknown nested symbol"),
        }
    }
    fn repr(self) -> &'static str {
        match self {
            Nest::Paren => "parenthesis",
            Nest::Square => "square brackets",
            Nest::Curly => "curly braces",
        }
    }
    fn matches(self, other: Self) -> bool {
        if self == other {
            return true;
        }
        // other matches
        match self {
            Nest::Paren | Nest::Square => matches!(other, Self::Paren | Self::Square),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("a, b, c" => vec!["a", "b", "c"]; "basic")]
    #[test_case("a,, c" => vec!["a", "", "c"]; "empty entries")]
    #[test_case("(a, b), c" => vec!["(a, b)", "c"]; "parens")]
    #[test_case("[a, b], c" => vec!["[a, b]", "c"]; "square")]
    #[test_case("{a, b}, c" => vec!["{a, b}", "c"]; "curly")]
    #[test_case("[a, b), c" => vec!["[a, b)", "c"]; "mixed1")]
    #[test_case("(a, b], c" => vec!["(a, b]", "c"]; "mixed2")]
    #[test_case("\"a, b \", c" => vec!["a, b", "c"]; "string only")]
    #[test_case("\"a,\" b, c" => vec!["\"a,\" b", "c"]; "string mixed")]
    #[test_case("\"s1\" out \"s2\"" => vec!["\"s1\" out \"s2\""]; "multiple strings")]
    #[test_case("({this)}" => panics "unbalanced curly braces"; "unbalanced")]
    #[test_case("({this}" => panics "unbalanced parenthesis"; "unclosed")]
    #[test_case("{this}]" => panics "unbalanced square brackets"; "unopened")]
    #[test_case("\"partial string" => panics; "unclosed string")]
    #[test_case("text \"partial string" => panics; "unclosed string mixed")]
    fn inline_split(s: &str) -> Vec<&str> {
        match split_entries(s, ',').collect::<Result<_, _>>() {
            Ok(v) => v,
            Err(e) => panic!("{}", e),
        }
    }
}
