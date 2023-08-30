use std::{
    fs,
    io::{self, BufRead, IsTerminal},
    path::PathBuf,
};

use anyhow::{Context, Result};
use clap::Parser;
use rng_query::State;

/// CLI to use pseudorandomness the easy way
///
/// It works with lists of text, dice, coins and intervals.
///
/// If comibining multiple inputs, the order is always:
/// STDIN -> DATA_FILE(s) -> FILE(s) -> INLINE -> REPL?
#[derive(Debug, Parser)]
#[command(author, version)]
struct Cli {
    /// Input file(s)
    ///
    /// This can be specified multiple times. Evaluated AFTER input data files.
    #[arg(short, long)]
    file: Vec<PathBuf>,

    /// Input data file(s)
    ///
    /// Lines are always entries.
    /// This can be specified multiple times.
    /// Evaluated AFTER stdin.
    #[arg(short = 'F', short_alias = 'd', long)]
    data_file: Vec<PathBuf>,

    /// Output file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Inline input
    ///
    /// This can be specified multiple times, each time is equivalent
    /// to a line. evaluated AFTER input files
    exec: Vec<String>,

    /// Stdin lines are always entries
    #[arg(long)]
    stdin_data: bool,

    /// Force open the repl
    ///
    /// If stdin is detected as a TTY and no `--inline` nor `<FILE>`
    /// is given, the repl will be open by default.
    #[arg(short = 'i', long, alias = "interactive", conflicts_with = "output")]
    repl: bool,

    /// Only show the result for evaluated expressions
    ///
    /// By default this is true if stdout is NOT a TTY.
    #[arg(
        long,
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true
    )]
    plain: Option<bool>,

    /// Seed the pseudorandom generator
    #[arg(long)]
    seed: Option<u64>,

    /// Override color behaviour
    #[command(flatten)]
    color: colorchoice_clap::Color,

    /// Change the statement separator
    #[arg(long, default_value_t = ';', hide_short_help = true)]
    stmt_sep: char,
    /// Change the entry separator
    #[arg(long, default_value_t = ',', hide_short_help = true)]
    entry_sep: char,
    /// Change the options separator
    #[arg(long, default_value_t = '/', hide_short_help = true)]
    options_sep: char,
}

pub fn main() -> Result<()> {
    let args = Cli::parse();
    args.color.write_global();

    let mut state = if let Some(s) = args.seed {
        State::with_seed(s)
    } else {
        State::new()
    };
    state.separators.stmt = args.stmt_sep;
    state.separators.entry = args.entry_sep;
    state.separators.options = args.options_sep;

    let mut output = Vec::new();

    let plain = args.plain.unwrap_or_else(|| !io::stdout().is_terminal());
    let mut out: Box<dyn std::io::Write> = if let Some(path) = &args.output {
        let file = fs::File::create(path).context("Failed to create output file")?;
        let stream = anstream::StripStream::new(file);
        Box::new(stream)
    } else {
        Box::new(anstream::stdout().lock())
    };
    let mut print_stmt = |stmt| {
        if plain {
            write!(out, "{stmt:#}")
        } else {
            write!(out, "{stmt}")
        }
    };

    // Order always is STDIN -> DATA_FILE -> FILE -> INLINE -> REPL?
    let stdin = io::stdin();
    let in_is_tty = stdin.is_terminal();
    if !in_is_tty {
        for line in stdin.lock().lines() {
            let line = line?;
            if args.stdin_data {
                state.add_entry(&line);
            } else {
                let out = state.run_line(&line)?;
                output.extend(out);
            }
        }
    }
    for path in &args.data_file {
        let file = fs::File::open(path)?;
        let rdr = io::BufReader::new(file);
        for line in rdr.lines() {
            let line = line?;
            state.add_entry(&line);
        }
    }
    for path in &args.file {
        let file = fs::File::open(path)?;
        let rdr = io::BufReader::new(file);
        for line in rdr.lines() {
            let line = line?;
            let out = state.run_line(&line)?;
            output.extend(out);
        }
    }
    for line in &args.exec {
        let out = state.run_line(line)?;
        output.extend(out);
    }

    // Print stored output
    for stmt in &output {
        print_stmt(stmt)?;
    }

    // Open the repl if needed
    let no_input = args.exec.is_empty() && args.file.is_empty() && args.data_file.is_empty();
    if args.repl || (in_is_tty && no_input) {
        repl(&mut state)?;
    } else {
        // otherwise process remaining entries
        if let Some(stmt) = state.eof()? {
            print_stmt(&stmt)?;
        }
    }

    Ok(())
}

fn repl(state: &mut State) -> Result<()> {
    let mut rl = rustyline::DefaultEditor::new()?;
    while let Ok(line) = rl.readline(">> ") {
        let _ = rl.add_history_entry(line.as_str());
        let mut out = state.run_line(&line)?;
        if !line.ends_with('\\') {
            if let Some(tail_out) = state.eof()? {
                out.push(tail_out);
            }
        }
        for stmt in out {
            anstream::println!("{stmt}");
        }
    }
    Ok(())
}
