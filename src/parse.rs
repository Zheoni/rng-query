use crate::Separators;

pub(crate) fn split_line_parts(
    line: &str,
    sep: Separators,
) -> impl Iterator<Item = Result<QueryPart<'_>, SplitPartsError>> {
    debug_assert!(!line.contains('\n'), "unexpected newline. this is a bug");
    let mut split = SplitParts::new(line, sep);
    let mut error = false;
    std::iter::from_fn(move || {
        if error {
            return None;
        }
        let val = split.next()?;
        if val.is_err() {
            error = true;
        }
        Some(val)
    })
    .fuse()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryPart<'i> {
    Entry(&'i str),
    Options(&'i str),
    EndStmt,
}

#[derive(Debug, thiserror::Error)]
pub enum SplitPartsError {
    #[error("missing trailing '\"' to close a string")]
    UnclosedString { start: usize },
    #[error("unbalanced {what}")]
    UnbalancedNesting { problem: usize, what: &'static str },
}

#[derive(Debug)]
struct SplitParts<'i> {
    line: &'i str,
    sep: Separators,
    last_end: usize,
    chars: std::str::Chars<'i>,
    mode: Mode,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Mode {
    Entry,
    Options,
    EndStmt,
}

impl<'i> SplitParts<'i> {
    fn new(line: &'i str, sep: Separators) -> Self {
        Self {
            line,
            sep,
            last_end: 0,
            chars: line.chars(),
            mode: Mode::Entry,
        }
    }

    fn current_offset(&self) -> usize {
        let remaining = self.chars.as_str().len();
        self.line.len() - remaining
    }

    fn take_slice(&mut self) -> &'i str {
        let current = self.current_offset();
        let s = &self.line[self.last_end..current];
        self.last_end = current;
        s
    }

    fn consume_string(&mut self) -> Result<(), SplitPartsError> {
        let mut end = false;
        #[allow(clippy::while_let_on_iterator)] // i like this syntax more here
        while let Some(c) = self.chars.next() {
            if c == '"' {
                end = true;
                break;
            }
        }
        if !end {
            return Err(SplitPartsError::UnclosedString {
                start: self.current_offset(),
            });
        }
        Ok(())
    }

    fn consume_options(&mut self) -> Option<Result<QueryPart<'i>, SplitPartsError>> {
        // consume until end stmt is found or line ends
        let end_found = self.chars.any(|c| c == self.sep.stmt);
        if end_found {
            self.mode = Mode::EndStmt;
        } else {
            self.mode = Mode::Entry;
        }
        let options = self.take_slice().trim_end_matches(self.sep.stmt).trim();
        return Some(Ok(QueryPart::Options(options)));
    }

    fn next_entry(&mut self) -> Option<Result<QueryPart<'i>, SplitPartsError>> {
        if self.chars.as_str().is_empty() {
            return None;
        }

        // Stack can be local because an entry is never returned if the stack is
        // not empty
        let mut stack = Vec::new();

        while let Some(c) = self.chars.next() {
            match c {
                '"' => {
                    if let Err(e) = self.consume_string() {
                        return Some(Err(e));
                    }
                }
                '(' | '[' | '{' => stack.push(Nest::from_char(c)),
                ']' | ')' | '}' => match stack.last() {
                    Some(pending) if pending.matches(Nest::from_char(c)) => {
                        stack.pop();
                    }
                    Some(pending) => {
                        return Some(Err(SplitPartsError::UnbalancedNesting {
                            problem: self.current_offset(),
                            what: pending.repr(),
                        }))
                    }
                    None => {
                        return Some(Err(SplitPartsError::UnbalancedNesting {
                            problem: self.current_offset(),
                            what: Nest::from_char(c).repr(),
                        }));
                    }
                },
                c if c == self.sep.entry && stack.is_empty() => {
                    let e = trim_entry(self.take_slice(), c);
                    return Some(Ok(QueryPart::Entry(e)));
                }
                c if c == self.sep.options && stack.is_empty() => {
                    let e = trim_entry(self.take_slice(), c);
                    self.mode = Mode::Options;
                    if e.is_empty() {
                        return self.next();
                    } else {
                        return Some(Ok(QueryPart::Entry(e)));
                    }
                }
                c if c == self.sep.stmt && stack.is_empty() => {
                    let e = trim_entry(self.take_slice(), c);
                    self.mode = Mode::EndStmt;
                    if e.is_empty() {
                        return self.next();
                    } else {
                        return Some(Ok(QueryPart::Entry(e)));
                    }
                }
                _ => {}
            }
        }

        // if here, it means no more special chars so consume everything
        match stack.last() {
            None => {
                let e = trim_entry(&self.line[self.last_end..], self.sep.entry);
                self.last_end = self.line.len();
                Some(Ok(QueryPart::Entry(e)))
            }
            Some(pending) => Some(Err(SplitPartsError::UnbalancedNesting {
                problem: self.line.len(),
                what: pending.repr(),
            })),
        }
    }
}

impl<'i> Iterator for SplitParts<'i> {
    type Item = Result<QueryPart<'i>, SplitPartsError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.mode {
            Mode::Options => self.consume_options(),
            Mode::EndStmt => {
                self.mode = Mode::Entry;
                Some(Ok(QueryPart::EndStmt))
            }
            Mode::Entry => self.next_entry(),
        }
    }
}

fn trim_entry(mut entry: &str, sep: char) -> &str {
    entry = entry.trim_end_matches(sep).trim();
    if entry.starts_with('"')
        && entry.ends_with('"')
        && entry.chars().filter(|&c| c == '"').count() == 2
    {
        entry = entry.trim_matches('"').trim()
    }
    entry
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Nest {
    Paren,
    Square,
    Curly,
}
impl Nest {
    fn from_char(c: char) -> Self {
        match c {
            '(' | ')' => Self::Paren,
            '[' | ']' => Self::Square,
            '{' | '}' => Self::Curly,
            _ => panic!("unknown nested symbol"),
        }
    }
    fn repr(self) -> &'static str {
        match self {
            Nest::Paren => "parenthesis",
            Nest::Square => "square brackets",
            Nest::Curly => "curly braces",
        }
    }
    fn matches(self, other: Self) -> bool {
        if self == other {
            return true;
        }
        // other matches
        match self {
            Nest::Paren | Nest::Square => matches!(other, Self::Paren | Self::Square),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;
    use QueryPart::Entry as E;
    use QueryPart::*;

    #[test_case("a, b, c" => vec![E("a"), E("b"), E("c")]; "basic")]
    #[test_case("a,, c" => vec![E("a"), E(""), E("c")]; "empty entries")]
    #[test_case("(a, b), c" => vec![E("(a, b)"), E("c")]; "parens")]
    #[test_case("[a, b], c" => vec![E("[a, b]"), E("c")]; "square")]
    #[test_case("{a, b}, c" => vec![E("{a, b}"), E("c")]; "curly")]
    #[test_case("[a, b), c" => vec![E("[a, b)"), E("c")]; "mixed1")]
    #[test_case("(a, b], c" => vec![E("(a, b]"), E("c")]; "mixed2")]
    #[test_case("\"a, b \", c" => vec![E("a, b"), E("c")]; "string only")]
    #[test_case("\"a,\" b, c" => vec![E("\"a,\" b"), E("c")]; "string mixed")]
    #[test_case("\"s1\" out \"s2\"" => vec![E("\"s1\" out \"s2\"")]; "multiple strings")]
    #[test_case("({this)}" => panics "unbalanced curly braces"; "unbalanced")]
    #[test_case("({this}" => panics "unbalanced parenthesis"; "unclosed")]
    #[test_case("{this}]" => panics "unbalanced square brackets"; "unopened")]
    #[test_case("\"partial string" => panics; "unclosed string")]
    #[test_case("text \"partial string" => panics; "unclosed string mixed")]
    #[test_case("a, b / options" => vec![E("a"), E("b"), Options("options")]; "basic options")]
    #[test_case("a; b" => vec![E("a"), EndStmt, E("b")]; "2 stmt")]
    #[test_case("a / opt; b / opt 2" => vec![E("a"), Options("opt"), EndStmt, E("b"), Options("opt 2")]; "2 stmt with options")]
    #[test_case("a / opt;" => vec![E("a"), Options("opt"), EndStmt]; "options trailing")]
    #[test_case("/ opt" => vec![Options("opt")]; "only options")]
    #[test_case("a /" => vec![E("a"), Options("")]; "empty options")]
    #[test_case("/" => vec![Options("")]; "only empty options")]
    #[test_case("(a;b);c" => vec![E("(a;b)"), EndStmt, E("c")]; "escaped nested stmt sep")]
    #[test_case("\"a;b\";c" => vec![E("a;b"), EndStmt, E("c")]; "escaped string stmt sep")]
    #[test_case("(a/b)/c" => vec![E("(a/b)"), Options("c")]; "escaped nested opts sep")]
    #[test_case("\"a/b\"/c" => vec![E("a/b"), Options("c")]; "escaped string opts sep")]
    fn split_parts(s: &str) -> Vec<QueryPart> {
        match split_line_parts(s, Separators::default()).collect::<Result<_, _>>() {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }
}
