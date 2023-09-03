//! Small query language for pseudorandomness
//!
//! See <https://github.com/Zheoni/rng-query> for the syntax and CLI.
//!
//! ## Usage as a lib
//!
//! `rng-query` is mainly the CLI app, so this lib is not the main objective. I
//! will try to follow [semver](https://semver.org/) but for the lib it's not a
//! guarantee, so you may want to pin a specific version.
//!
//! Run a whole input with [`run`] or manage line by line with [`State`] and it
//! methods.
//!
//! All [`Display`] implementations of the crate *may* output ANSI color codes.
//! Use something like [anstream](https://docs.rs/anstream/) if you dont want
//! colors.

mod coin;
mod dice;
mod entry;
mod interval;
mod parse;

use std::fmt::Display;
use std::rc::Rc;
use std::str::FromStr;

use entry::SharedEntry;
use parse::{split_line_parts, QueryPart, SplitPartsError};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_pcg::Pcg64 as Pcg;

pub use coin::CoinResult;
pub use dice::RollResult;
pub use entry::Entry;
pub use interval::{Float, Int, IntervalSample, Num};

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
/// assert_eq!(sep.stmt, ';');
/// assert_eq!(sep.entry, ','); // And `\n` always.
/// assert_eq!(sep.options, '/');
/// ```
///
/// To use this, change the [`separators`](State::sep) field in [`State`].
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
    push: bool,
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
            push: false,
        }
    }
}

impl FromStr for Options {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let preset = match s {
            "shuffle" => Some(Options {
                amount: Amount::All,
                eval_expr: EvalExpr::Custom(false),
                ..Default::default()
            }),
            "list" => Some(Options {
                amount: Amount::All,
                eval_expr: EvalExpr::Custom(false),
                keep_order: true,
                ..Default::default()
            }),
            "eval" => Some(Options {
                amount: Amount::All,
                eval_expr: EvalExpr::Custom(true),
                keep_order: true,
                ..Default::default()
            }),
            _ => None,
        };
        if let Some(preset) = preset {
            return Ok(preset);
        }

        let re = regex!(r"\A(all\s|0|(?:[1-9][0-9]*))?\s*([\sreEop]*)\z");
        let cap = re
            .captures(s)
            .ok_or_else(|| Error::Options(format!("Bad options: {s}")))?;
        let amount = match cap.get(1).map(|m| m.as_str().trim_end()) {
            Some("all") => Amount::All,
            Some(n) => n
                .parse::<u32>()
                .map(Amount::N)
                .map_err(|e| Error::Options(format!("Bad amount: {e}")))?,
            None => Amount::N(1),
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
        let push = flags.contains(&'p');

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
            push,
        })
    }
}

/// Query interpreter
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    stack: Vec<Rc<str>>,
    rng: Pcg,
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
    /// # Panics
    /// (only in debug) if the input contains `\n`.
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
        self.stack.push(Rc::from(entry));
    }

    /// Signal the interpreter the end of the input
    ///
    /// This can also be used to force an statement end without getting an
    /// [`Separators::stmt`].
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

        // leave stack empty
        let entries = std::mem::take(&mut self.stack);

        // Make the entries shared to avoid copied data if repeated and add id
        // to sort them if needed. Also, if eval, parse everything so the same
        // query always fails/succeed no matter of the RNG state.
        let mut shared = Vec::with_capacity(entries.len());
        for (id, t) in entries.into_iter().enumerate() {
            let entry = SharedEntry::new(t, eval_expr)?;
            shared.push((id, entry));
        }

        let selected = select(&mut self.rng, shared, options);

        let output = selected
            .into_iter()
            .map(|(_, e)| e.eval(&mut self.rng))
            .collect::<Vec<_>>();

        if options.push {
            self.stack.reserve(output.len());
            for e in output {
                self.stack.push(e.into_raw())
            }
            return Ok(StmtOutput(vec![]));
        }

        Ok(StmtOutput(output))
    }
}

fn select(
    rng: &mut Pcg,
    mut entries: Vec<(usize, SharedEntry)>,
    options: Options,
) -> Vec<(usize, SharedEntry)> {
    if entries.is_empty() {
        return vec![];
    }

    let n = match options.amount {
        Amount::All => entries.len(),
        Amount::N(n) => n as usize,
    };

    // optimization for all
    if n == entries.len() {
        if !options.keep_order {
            entries.shuffle(rng);
        }
        return entries;
    }

    // general case
    let mut selected = if options.repeating {
        let mut selected = Vec::with_capacity(n);
        for _ in 0..n {
            let entry = entries.choose(rng).unwrap();
            selected.push(entry.clone());
        }
        selected
    } else {
        entries.choose_multiple(rng, n).cloned().collect()
    };

    if options.keep_order {
        selected.sort_unstable_by_key(|e| e.0);
    }
    selected
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
