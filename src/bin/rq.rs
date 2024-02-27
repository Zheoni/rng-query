use std::io::{self, BufRead, IsTerminal};

use anstream::println;
use clap::{arg, command};
use owo_colors::OwoColorize;
use rng_query::State;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = command!()
        .arg(arg!([query] "Query to evaluate"))
        .arg(
            arg!(-q --quiet "Quiet, only show the selected values")
                .visible_alias("only-values")
                .alias("hide-expr")
                .short_alias('E'),
        )
        .arg(arg!(-e --eval "Evaluate STDIN lines as expressions").alias("eval-stdin"))
        .arg(arg!(--seed <SEED> "Seed the pseudorandom generator"))
        .arg(
            arg!(--color <WHEN> "Controls when to use color")
                .default_value("auto")
                .value_parser(clap::builder::EnumValueParser::<clap::ColorChoice>::new()),
        )
        .get_matches();

    let color = match matches
        .get_one::<clap::ColorChoice>("color")
        .expect("default color value")
    {
        clap::ColorChoice::Auto => anstream::ColorChoice::Auto,
        clap::ColorChoice::Always => anstream::ColorChoice::Always,
        clap::ColorChoice::Never => anstream::ColorChoice::Never,
    };
    color.write_global();

    let seed = matches.get_one::<u64>("seed").copied();
    let query = matches.get_one::<String>("query");
    let eval_stdin = matches.get_flag("eval");
    let quiet = matches.get_flag("quiet");

    let mut state = if let Some(seed) = seed {
        State::with_seed(seed)
    } else {
        State::new()
    };

    let stdin = io::stdin();
    if query.is_none() || !stdin.is_terminal() {
        for line in stdin.lock().lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if eval_stdin {
                state.add_entry(line)?;
            } else {
                state.add_data(line);
            }
        }
    }

    let input = match query {
        Some(q) => q,
        None => "",
    };

    match state.run_query(input) {
        Ok(output) => {
            for sample in &output {
                if quiet {
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
