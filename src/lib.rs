pub mod coin;
pub mod dice;
pub mod entry;
pub mod interval;

use std::collections::HashMap;
use std::fmt::Display;
use std::rc::Rc;
use std::str::FromStr;

use entry::BufferedEntry;
use entry::Entry;
use entry::EntryData;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_pcg::Pcg64 as Pcg;

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}
pub(crate) use regex;

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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    stack: Vec<BufferedEntry>,
    rng: Pcg,
    entry_counter: usize,
    pub separators: Separators,
}

impl State {
    pub fn new() -> Self {
        Self::from_rng(Pcg::from_entropy())
    }
    pub fn with_seed(seed: u64) -> Self {
        Self::from_rng(Pcg::seed_from_u64(seed))
    }
    fn from_rng(rng: Pcg) -> Self {
        Self {
            stack: Vec::new(),
            rng,
            entry_counter: 0,
            separators: Separators::default(),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub fn run_line(&mut self, line: &str) -> Result<Vec<StmtOutput>, Error> {
        let sep = self.separators.stmt;
        let mut outputs = Vec::new();
        for part in line.split_inclusive(sep) {
            let options = self.run_stmt_part(part.trim_end_matches(sep))?;
            let is_end = options.is_some() || part.ends_with(sep);
            if is_end {
                let output = self.end_stmt(options.unwrap_or_default())?;
                outputs.push(output);
            }
        }
        Ok(outputs)
    }

    fn run_stmt_part(&mut self, stmt: &str) -> Result<Option<Options>, Error> {
        let (entries, options) = match stmt.split_once(self.separators.options) {
            Some((entries, options)) => (entries, Some(options)),
            None => (stmt, None),
        };
        for entry in entries.split(self.separators.entry) {
            self.add_entry(entry);
        }
        if let Some(options) = options {
            options.trim().parse::<Options>().map(Some)
        } else {
            Ok(None)
        }
    }

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

    let parse = |entry: &BufferedEntry| {
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Options error: {0}")]
    Options(String),
    #[error("Expression error: {0}")]
    Expr(String),
}
