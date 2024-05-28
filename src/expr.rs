use std::rc::Rc;

use crate::{eval::Eval, Error};

mod coin;
mod color;
mod dice;
mod interval;

pub fn parse_expr(expr: &str) -> Result<Option<Rc<dyn Eval>>, Error> {
    // one word specials
    let thing: Option<Rc<dyn Eval>> = match expr {
        "coin" => Some(Rc::new(coin::toss_coin)),
        "color" => Some(Rc::new(color::gen_color)),
        _ => None,
    };
    if thing.is_some() {
        return Ok(thing);
    }

    // more complex ones, maybe add a precheck match in the future
    match expr.parse::<dice::Roll>() {
        Err(dice::RollParseError::NoMatch) => {}
        Ok(r) => return Ok(Some(Rc::new(r))),
        Err(e) => return Err(Error::Expr(e.to_string())),
    }

    match expr.parse::<interval::Interval>() {
        Err(interval::IntervalParseError::NoMatch) => {}
        Ok(i) => return Ok(Some(Rc::new(i))),
        Err(e) => return Err(Error::Expr(e.to_string())),
    }

    Ok(None)
}
