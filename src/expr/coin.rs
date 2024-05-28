//! Coin expression

use owo_colors::OwoColorize;
use rand::Rng;

use crate::{Pcg, Sample};

pub fn toss_coin(rng: &mut Pcg) -> Sample {
    const HEADS: &str = "heads";
    const TAILS: &str = "tails";
    let res = match rng.gen::<bool>() {
        true => HEADS.green().bold().to_string(),
        false => TAILS.purple().bold().to_string(),
    };
    Sample::text(res.into())
}
