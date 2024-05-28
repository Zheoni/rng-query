use owo_colors::OwoColorize;
use rand::Rng;

use crate::{Pcg, Sample};

pub fn gen_color(rng: &mut Pcg) -> Vec<Sample> {
    let r: u8 = rng.gen();
    let g: u8 = rng.gen();
    let b: u8 = rng.gen();

    let hex = format!(" {r:02X}{g:02X}{b:02X} ");
    let color = owo_colors::DynColors::Rgb(r, g, b);
    vec![Sample::text(hex.bold().on_color(color).to_string().into())]
}
