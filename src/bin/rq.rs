use std::io::{self, BufRead, IsTerminal};

use anstream::println;
use clap::Parser;
use owo_colors::OwoColorize;
use rng_query::State;

#[derive(clap::Parser)]
struct Args {
    /// Query to evaluate in line, skip to read from stdin
    query: Option<String>,
    /// Hide expression in the output sample
    #[arg(short = 'E', long)]
    hide_expr: bool,
    /// Eval STDIN
    ///
    /// By default it will be treated just as data, one entry per line. With
    /// this enabled, STDIN will be a regular entry expression PER LINE.
    #[arg(short = 'e', long)]
    eval_stdin: bool,
    /// Seed the pseudorandom generator
    #[arg(long)]
    seed: Option<u64>,
    /// Enable or disable color
    #[command(flatten)]
    color: colorchoice_clap::Color,
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    args.color.write_global();

    let mut state = if let Some(seed) = args.seed {
        State::with_seed(seed)
    } else {
        State::new()
    };

    let stdin = io::stdin();
    if args.query.is_none() || !stdin.is_terminal() {
        for line in stdin.lock().lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if args.eval_stdin {
                state.add_entry(line)?;
            } else {
                state.add_data(line);
            }
        }
    }

    let input = match &args.query {
        Some(q) => q,
        None => "",
    };

    match state.run_query(input) {
        Ok(output) => {
            for sample in &output {
                if args.hide_expr {
                    println!("{sample:#}");
                } else {
                    println!("{sample}");
                }
            }
        }
        Err(err) => println!("{}: {err}", "error".red()),
    }

    Ok(())
}
