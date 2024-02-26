use std::rc::Rc;

use crate::{ast, regex, Error};

#[derive(Debug)]
struct Query<'a> {
    entries: Vec<Entry<'a>>,
    options: Option<&'a str>,
}

#[derive(Debug)]
enum Entry<'a> {
    Query(Box<Query<'a>>),
    Entry(&'a str),
}

struct Cursor<'a> {
    input: &'a str,
    chars: std::str::Chars<'a>,
    slice_start: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a str) -> Cursor<'a> {
        Cursor {
            input,
            chars: input.chars(),
            slice_start: 0,
        }
    }

    fn first(&self) -> Option<char> {
        self.chars.clone().next()
    }

    fn eat(&mut self) -> Option<char> {
        self.chars.next()
    }

    fn current_pos(&self) -> usize {
        self.input.len() - self.chars.as_str().len()
    }

    fn set_start(&mut self) {
        self.slice_start = self.current_pos();
    }

    fn take_slice(&mut self) -> &'a str {
        let cur = self.current_pos();
        let start = self.slice_start;
        self.slice_start = cur;
        &self.input[start..cur]
    }

    fn eat_until(&mut self, f: impl Fn(char) -> bool) -> bool {
        let mut last = '\0';
        while let Some(c) = self.first() {
            if last != '\\' && f(c) {
                return true;
            }
            last = c;
            self.eat();
        }
        false
    }
}

fn parse_query_rec<'a>(cursor: &mut Cursor<'a>, is_root: bool) -> Result<Query<'a>, String> {
    let mut entries = Vec::new();
    let mut options = None;

    cursor.set_start(); // mark start

    fn take_entry<'a>(cursor: &mut Cursor<'a>, trim_last: bool) -> Entry<'a> {
        let mut s = cursor.take_slice();
        if trim_last && !s.is_empty() {
            s = &s[..s.len() - 1]; // this may be a problem with utf8 codepoints
        }
        s = s.trim();
        Entry::Entry(s)
    }

    let mut end_found = false;
    while let Some(c) = cursor.eat() {
        match c {
            '{' => {
                let q = parse_query_rec(cursor, false)?;
                entries.push(Entry::Query(Box::new(q)));
            }
            '}' => {
                end_found = true;
                if is_root {
                    return Err("unexpected '}'".to_string());
                }
                if options.is_none() {
                    entries.push(take_entry(cursor, true)); // push last entry
                }
                cursor.set_start(); // skip '}' for next slice
                break;
            }
            '[' | '(' => {
                let found = cursor.eat_until(|c| c == ']' || c == ')');
                if !found {
                    return Err("unbalanced parenthesis/square brackets".to_string());
                }
                cursor.eat();
            }
            '"' | '\'' => {
                let found = cursor.eat_until(|cc| cc == c);
                if !found {
                    return Err("unclosed string".to_string());
                }
                cursor.eat();
            }
            ',' | '\n' => {
                entries.push(take_entry(cursor, true));
            }
            '/' => {
                entries.push(take_entry(cursor, true)); // push last entry

                cursor.eat_until(|c| c == '}');
                let s = cursor.take_slice().trim();
                if s.is_empty() {
                    return Err("empty options".to_string());
                }
                if options.is_some() {
                    return Err("multiple options".to_string());
                }
                options = Some(s);
            }
            _ => {}
        }
    }
    if !is_root && !end_found {
        return Err("missing '}'".to_string());
    }
    if is_root && options.is_none() {
        entries.push(take_entry(cursor, false));
    }
    entries.retain(|e| {
        if let Entry::Entry(s) = e {
            !s.is_empty()
        } else {
            true
        }
    });

    Ok(Query { entries, options })
}

fn build_ast(q: &Query) -> Result<ast::Query, Error> {
    let root = ast_choose(q)?;
    Ok(ast::Query { root })
}

fn ast_choose(q: &Query) -> Result<ast::Choose, Error> {
    let mut entries = Vec::with_capacity(q.entries.len());
    for (id, entry) in q.entries.iter().enumerate() {
        let e = ast_entry(entry)?;
        entries.push((id, e));
    }

    let options = if let Some(options) = q.options {
        ast_options(options)?
    } else {
        ast::ChooseOptions::default()
    };

    Ok(ast::Choose { entries, options })
}

fn ast_entry(entry: &Entry) -> Result<ast::Entry, Error> {
    let e = match entry {
        Entry::Query(q) => ast::Entry::Expr(Rc::new(ast_choose(q)?)),
        Entry::Entry(e) => ast::Entry::parse(e)?,
    };
    Ok(e)
}

fn ast_options(s: &str) -> Result<ast::ChooseOptions, Error> {
    match s {
        "shuffle" => return Ok(ast::ChooseOptions::shuffle()),
        "list" => return Ok(ast::ChooseOptions::list()),
        _ => {}
    };

    let re = regex!(r"\A(all\b|(?:[0-9]+))?([ ro]*)\z");
    let cap = re
        .captures(s)
        .ok_or_else(|| Error::Options(format!("Bad options: {s:?}")))?;
    let amount = match cap.get(1).map(|m| m.as_str().trim_end()) {
        Some("all") => ast::Amount::All,
        Some(n) => n
            .parse::<u32>()
            .map(ast::Amount::N)
            .map_err(|e| Error::Options(format!("Bad amount: {e}")))?,
        None => ast::Amount::N(1),
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

    Ok(ast::ChooseOptions {
        amount,
        repeating,
        keep_order,
    })
}

pub fn parse_query(input: &str) -> Result<ast::Query, Error> {
    let mut cursor = Cursor::new(input);
    let q = parse_query_rec(&mut cursor, true).map_err(Error::ParseQuery)?;
    build_ast(&q)
}
