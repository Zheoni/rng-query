use std::rc::Rc;

use rand::seq::SliceRandom;

use crate::{
    ast::{Amount, Choose, ChooseOptions, Entry, Query},
    Pcg,
};

/// A sample from a selected entry
///
/// This is an opaque type, hidden intentionally. It only expose the [`Display`]
/// implementation to access it. The
/// [`Display`] [alternate modifier](std::fmt#sign0) will only print the sampled
/// value and not the whole representation.
///
/// [`Display`]: std::fmt::Display
pub struct Sample(SampleData);

enum SampleData {
    Text(Rc<str>),
    Expr(Box<dyn std::fmt::Display>),
}

impl Sample {
    pub(crate) fn text(data: Rc<str>) -> Self {
        Self(SampleData::Text(data))
    }
    pub(crate) fn expr(data: Box<dyn std::fmt::Display>) -> Self {
        Self(SampleData::Expr(data))
    }
}

impl std::fmt::Display for Sample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            SampleData::Text(t) => t.fmt(f),
            SampleData::Expr(e) => e.fmt(f),
        }
    }
}

pub trait Eval {
    fn eval(&self, rng: &mut Pcg) -> Vec<Sample>;
}

impl<T> Eval for T
where
    T: Fn(&mut Pcg) -> Vec<Sample>,
{
    fn eval(&self, rng: &mut Pcg) -> Vec<Sample> {
        (self)(rng)
    }
}

impl Eval for Query {
    fn eval(&self, rng: &mut Pcg) -> Vec<Sample> {
        self.root.eval(rng)
    }
}

impl Eval for Choose {
    fn eval(&self, rng: &mut Pcg) -> Vec<Sample> {
        let Self { entries, options } = self;

        let selected = select(rng, entries, options);

        if selected.is_empty() {
            return vec![];
        }

        let mut it = selected.into_iter();
        let mut output = it.next().unwrap().1.eval(rng);
        for (_, entry) in it {
            output.append(&mut entry.eval(rng));
        }
        output
    }
}

impl Eval for Entry {
    fn eval(&self, rng: &mut Pcg) -> Vec<Sample> {
        match self {
            Entry::Text(t) => vec![Sample::text(t.clone())],
            Entry::Expr(e) => e.eval(rng),
        }
    }
}

fn select(
    rng: &mut Pcg,
    entries: &[(usize, Entry)],
    options: &ChooseOptions,
) -> Vec<(usize, Entry)> {
    if entries.is_empty() {
        return vec![];
    }

    let n = match options.amount {
        Amount::All => entries.len(),
        Amount::N(n) => n as usize,
    };

    // optimization for all
    if !options.repeating && n >= entries.len() {
        let mut entries = entries.to_vec();
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
