use std::rc::Rc;

use crate::{eval::Eval, Error};

#[derive(Debug, Clone)]
pub struct Query {
    pub root: Choose,
}

#[derive(Debug, Clone)]
pub struct Choose {
    pub entries: Vec<(usize, Entry)>,
    pub options: ChooseOptions,
}

#[derive(Debug, Clone, Copy)]
pub struct ChooseOptions {
    pub repeating: bool,
    pub keep_order: bool,
    pub amount: Amount,
}

impl Default for ChooseOptions {
    fn default() -> Self {
        Self {
            repeating: false,
            keep_order: false,
            amount: Amount::N(1),
        }
    }
}

impl ChooseOptions {
    pub fn shuffle() -> Self {
        ChooseOptions {
            amount: Amount::All,
            ..Default::default()
        }
    }
    pub fn list() -> Self {
        ChooseOptions {
            amount: Amount::All,
            keep_order: true,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Amount {
    All,
    N(u32),
}

#[derive(Clone)]
pub enum Entry {
    Text(Rc<str>),
    Expr(Rc<dyn Eval>),
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => f.debug_tuple("Text").field(text).finish(),
            Self::Expr(_) => f.write_str("Expr(..)"),
        }
    }
}

impl Entry {
    pub fn parse(entry: &str) -> Result<Self, Error> {
        let e = if let Some(expr) = crate::expr::parse_expr(entry)? {
            Self::Expr(expr)
        } else {
            let s = clean_string(entry);
            Self::data(s)
        };
        Ok(e)
    }

    pub fn data(entry: &str) -> Self {
        Self::Text(Rc::from(entry))
    }
}

fn clean_string(s: &str) -> &str {
    if !s.starts_with(['\'', '"']) {
        return s;
    }
    let delim = s.chars().next().unwrap();
    if !s.ends_with(delim) || s.chars().nth_back(1).is_some_and(|c| c == '\\') {
        return s;
    }
    let content = &s[1..s.len() - 1];
    if content.contains(delim) {
        return s;
    }
    content
}
