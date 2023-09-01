//! Small query language for pseudorandomness
//!
//! See <https://github.com/Zheoni/rng-query> for the syntax and CLI.
//!
//! Run a whole input with [`run`] or manage line by line with [`State`] and it
//! methods.
//!
//! All [`Display`] implementations of the crate *may* output ANSI color codes.
//! Use something like [anstream](https://docs.rs/anstream/) if you dont want
//! colors.

pub mod coin;
pub mod dice;
mod entry;
pub mod interval;
mod parse;

use std::collections::HashMap;
use std::fmt::Display;
use std::rc::Rc;
use std::str::FromStr;

use entry::BufferedEntry;
use entry::EntryData;
use parse::{split_line_parts, QueryPart, SplitPartsError};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_pcg::Pcg64 as Pcg;

pub use coin::CoinResult;
pub use dice::RollResult;
pub use entry::Entry;
pub use interval::IntervalResult;

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}
pub(crate) use regex;

/// Run a whole "program"
///
/// More than 1 query can be executed, so the result is a vec with a [`StmtOutput`]
/// for each query.
pub fn run(input: &str) -> Result<Vec<StmtOutput>, Error> {
    let mut state = State::new();
    let mut output = Vec::new();
    for line in input.lines() {
        output.extend(state.run_line(line)?);
    }
    if let Some(o) = state.eof()? {
        output.push(o);
    }
    Ok(output)
}

/// Customize the separators used
///
/// ```
/// use rng_query::Separators;
/// let sep = Separators::default();
/// assert_eq!(sep.stmt, ';'); // And `\n` always.
/// assert_eq!(sep.entry, ',');
/// assert_eq!(sep.options, '/');
/// ```
///
/// To use this, change the [`separators`](State::separators) field in [`State`].
///
/// Be careful, it can break expression parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Separators {
    pub stmt: char,
    pub entry: char,
    pub options: char,
}

impl Default for Separators {
    fn default() -> Self {
        Self {
            stmt: ';',
            entry: ',',
            options: '/',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Options {
    amount: Amount,
    repeating: bool,
    eval_expr: EvalExpr,
    keep_order: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Amount {
    All,
    N(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvalExpr {
    Auto,
    Custom(bool),
}

impl Default for Options {
    fn default() -> Self {
        Self {
            amount: Amount::N(1),
            repeating: false,
            eval_expr: EvalExpr::Auto,
            keep_order: false,
        }
    }
}

impl FromStr for Options {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let preset = match s {
            "shuffle" => Some(Options {
                amount: Amount::All,
                repeating: false,
                eval_expr: EvalExpr::Custom(false),
                keep_order: false,
            }),
            "list" => Some(Options {
                amount: Amount::All,
                repeating: false,
                eval_expr: EvalExpr::Custom(false),
                keep_order: true,
            }),
            "eval" => Some(Options {
                amount: Amount::All,
                repeating: false,
                eval_expr: EvalExpr::Custom(true),
                keep_order: true,
            }),
            _ => None,
        };
        if let Some(preset) = preset {
            return Ok(preset);
        }

        let re = regex!(r"\A(all\s|0|(?:[1-9][0-9]*))\s*([\sreEo]*)\z");
        let cap = re
            .captures(s)
            .ok_or_else(|| Error::Options(format!("Bad options: {s}")))?;
        let amount = match cap[1].trim_end() {
            "all" => Amount::All,
            n => n
                .parse::<u32>()
                .map(Amount::N)
                .map_err(|e| Error::Options(format!("Bad amount: {e}")))?,
        };

        let mut flags = cap[2]
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect::<Vec<_>>();
        flags.sort();
        let all_len = flags.len();
        flags.dedup();
        let unique_len = flags.len();
        if all_len != unique_len {
            return Err(Error::Options(format!(
                "Duplicate flags: {}",
                flags.iter().collect::<String>()
            )));
        }
        let repeating = flags.contains(&'r');
        let keep_order = flags.contains(&'o');

        let eval_expr = flags.contains(&'e');
        let not_eval_expr = flags.contains(&'E');
        if eval_expr && not_eval_expr {
            return Err(Error::Options(
                "Flags 'e' and 'E' are incompatible".to_string(),
            ));
        }
        let eval_expr = if eval_expr || not_eval_expr {
            EvalExpr::Custom(eval_expr)
        } else {
            EvalExpr::Auto
        };

        Ok(Options {
            amount,
            repeating,
            eval_expr,
            keep_order,
        })
    }
}

/// Query interpreter
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    stack: Vec<BufferedEntry>,
    rng: Pcg,
    entry_counter: usize,
    /// See [`Separators`]
    pub sep: Separators,
}

impl State {
    /// Create a new state
    ///
    /// Seed is autogenerated form entropy.
    pub fn new() -> Self {
        Self::from_rng(Pcg::from_entropy())
    }
    /// Create a new state with a seed
    pub fn with_seed(seed: u64) -> Self {
        Self::from_rng(Pcg::seed_from_u64(seed))
    }
    fn from_rng(rng: Pcg) -> Self {
        Self {
            stack: Vec::new(),
            rng,
            entry_counter: 0,
            sep: Separators::default(),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    /// Run a single line
    ///
    /// The line will be parsed, to add data, use [`State::add_entry`].
    ///
    /// The input should *NOT* include `\n`.
    pub fn run_line(&mut self, line: &str) -> Result<Vec<StmtOutput>, Error> {
        let mut outputs = Vec::new();

        let mut options = None;

        for part in split_line_parts(line, self.sep) {
            let part = part?;
            match part {
                QueryPart::Entry(e) => self.add_entry(e),
                QueryPart::Options(o) => {
                    assert!(options.is_none(), "more than one options in a query");
                    options = Some(o.parse()?);
                }
                QueryPart::EndStmt => {
                    let output = self.end_stmt(options.unwrap_or_default())?;
                    outputs.push(output);
                    options = None;
                }
            }
        }

        if let Some(options) = options {
            let output = self.end_stmt(options)?;
            outputs.push(output);
        }

        Ok(outputs)
    }

    /// Add an entry, without parsing it
    pub fn add_entry(&mut self, entry: &str) {
        let entry = entry.trim();
        if entry.is_empty() {
            return;
        }
        let data = EntryData::Text(Rc::from(entry));
        let id = self.entry_counter;
        self.entry_counter = self
            .entry_counter
            .checked_add(1)
            .expect("somehow you managed to get to the maximum number of entries. congrats.");
        self.stack.push(BufferedEntry { id, data });
    }

    /// Signal the interpreter the end of the input
    pub fn eof(&mut self) -> Result<Option<StmtOutput>, Error> {
        if self.stack.is_empty() {
            Ok(None)
        } else {
            self.end_stmt(Options::default()).map(Some)
        }
    }

    fn end_stmt(&mut self, options: Options) -> Result<StmtOutput, Error> {
        let eval_expr = match options.eval_expr {
            EvalExpr::Auto => self.stack.len() == 1,
            EvalExpr::Custom(r) => r,
        };
        let selected = select(&mut self.rng, &self.stack, options, eval_expr)?;

        let output = selected
            .into_iter()
            .map(|e| e.eval(&mut self.rng))
            .collect();

        self.stack.clear();
        Ok(StmtOutput(output))
    }
}

fn select(
    rng: &mut Pcg,
    entries: &[BufferedEntry],
    options: Options,
    eval_expr: bool,
) -> Result<Vec<BufferedEntry>, Error> {
    if entries.is_empty() {
        return Ok(vec![]);
    }

    let n = match options.amount {
        Amount::All => entries.len(),
        Amount::N(n) => n as usize,
    };

    let parse = |entry: &BufferedEntry| -> Result<BufferedEntry, Error> {
        if eval_expr {
            if let EntryData::Text(t) = &entry.data {
                let data = entry::parse_expr(t)?;
                return Ok(BufferedEntry { id: entry.id, data });
            }
        }
        Ok(entry.clone())
    };

    // optimization for all
    if n == entries.len() {
        let mut entries = entries.iter().map(parse).collect::<Result<Vec<_>, _>>()?;
        if !options.keep_order {
            entries.shuffle(rng);
        }
        return Ok(entries);
    }

    // general case
    let mut selected = if options.repeating {
        let mut cache = HashMap::<usize, BufferedEntry>::new();

        let mut selected = Vec::with_capacity(n);
        for _ in 0..n {
            let entry = entries.choose(rng).unwrap();
            let entry = if let Some(cached) = cache.get(&entry.id) {
                cached.clone()
            } else {
                let parsed = parse(entry)?;
                cache.insert(parsed.id, parsed.clone());
                parsed
            };
            selected.push(entry);
        }
        selected
    } else {
        entries
            .choose_multiple(rng, n)
            .map(parse)
            .collect::<Result<Vec<_>, _>>()?
    };

    if options.keep_order {
        selected.sort_unstable_by_key(|e| e.id);
    }
    Ok(selected)
}

/// Output of a query
///
/// This is a [`Vec`] of selected entries with a custom [`Display`] implementation
/// that prints each entry as a line.
///
/// Also, the [alternate modifier](std::fmt#sign0) will only print the expression output
/// and not the expression itself.
#[derive(Debug, Clone, PartialEq)]
pub struct StmtOutput(pub Vec<Entry>);

impl Display for StmtOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for entry in &self.0 {
            if f.alternate() {
                writeln!(f, "{entry:#}")?;
            } else {
                writeln!(f, "{entry}")?;
            }
        }
        Ok(())
    }
}

/// Query error
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Parsing options
    #[error("options: {0}")]
    Options(String),
    /// Parsing expressions
    #[error("expression: {0}")]
    Expr(String),
    #[error("inline entries: {0}")]
    SplitError(#[from] SplitPartsError),
}
