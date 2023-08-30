use std::fmt::Display;

use owo_colors::OwoColorize;
use rand::Rng;

use crate::Pcg;

pub fn toss_coin(rng: &mut Pcg) -> CoinResult {
    match rng.gen::<bool>() {
        true => CoinResult::Heads,
        false => CoinResult::Tails,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinResult {
    Heads,
    Tails,
}

impl Display for CoinResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const HEADS: &str = "heads";
        const TAILS: &str = "tails";
        match self {
            CoinResult::Heads => write!(f, "{}", HEADS.green().bold()),
            CoinResult::Tails => write!(f, "{}", TAILS.purple().bold()),
        }
    }
}
