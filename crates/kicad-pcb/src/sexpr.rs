//! The s-expression text KiCad writes: nested lists of atoms and quoted strings.
//!
//! From KiCad's file-format documentation ("S-Expression Format"): tokens in parentheses, strings
//! in double quotes (UTF-8), numbers in plain decimal notation. The parser is iterative and
//! bounded, so a hostile file returns an error and cannot overflow the stack or the memory.

/// The deepest nesting a file may have. Real boards go about 8 deep.
pub const MAX_DEPTH: usize = 64;
/// The most nodes (atoms, strings and lists) a file may have.
pub const MAX_NODES: usize = 12_000_000;

/// One element of the text.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// An unquoted word or number: `kicad_pcb`, `1.6`, `yes`.
    Atom(String),
    /// A quoted string, without its quotes.
    Str(String),
    /// `( ... )`: the first element is usually the token's name.
    List(Vec<Node>),
}

impl Node {
    /// The name of a list: its first element when that is an atom.
    pub fn head(&self) -> Option<&str> {
        match self {
            Node::List(items) => match items.first() {
                Some(Node::Atom(name)) => Some(name),
                _ => None,
            },
            _ => None,
        }
    }

    /// Everything in a list after its name. Empty for an atom or a string.
    pub fn args(&self) -> &[Node] {
        match self {
            Node::List(items) => items.get(1..).unwrap_or(&[]),
            _ => &[],
        }
    }

    /// The text of an atom or a string.
    pub fn text(&self) -> Option<&str> {
        match self {
            Node::Atom(s) | Node::Str(s) => Some(s),
            Node::List(_) => None,
        }
    }

    /// The value of an atom that is a finite number.
    pub fn number(&self) -> Option<f64> {
        match self {
            Node::Atom(s) => s.parse::<f64>().ok().filter(|v| v.is_finite()),
            _ => None,
        }
    }

    /// The first list among the arguments with this name.
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.args().iter().find(|n| n.head() == Some(name))
    }

    /// Every list among the arguments with this name.
    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.args().iter().filter(move |n| n.head() == Some(name))
    }

    /// The `i`th argument as a number.
    pub fn num(&self, i: usize) -> Option<f64> {
        self.args().get(i).and_then(Node::number)
    }

    /// The `i`th argument as text.
    pub fn str_at(&self, i: usize) -> Option<&str> {
        self.args().get(i).and_then(Node::text)
    }

    /// The two numbers of `(name X Y)`.
    pub fn pair(&self) -> Option<(f64, f64)> {
        Some((self.num(0)?, self.num(1)?))
    }

    /// The three numbers of `(name X Y Z)`, or of `(name (xyz X Y Z))`.
    pub fn triple(&self) -> Option<[f64; 3]> {
        let node = self.child("xyz").unwrap_or(self);
        Some([node.num(0)?, node.num(1)?, node.num(2)?])
    }

    /// True when the arguments hold this bare word (`hide`, `locked`).
    pub fn has_flag(&self, word: &str) -> bool {
        self.args()
            .iter()
            .any(|n| matches!(n, Node::Atom(a) if a == word))
    }

    /// True when `(name yes)` is among the arguments, or the bare word `name` is.
    pub fn is_yes(&self, name: &str) -> bool {
        self.has_flag(name)
            || self
                .child(name)
                .is_some_and(|n| matches!(n.str_at(0), Some("yes") | Some("true")) || n.args().is_empty())
    }
}

fn is_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// Parses a whole file: one top-level list.
pub fn parse(text: &str) -> Result<Node, String> {
    let mut stack: Vec<Vec<Node>> = Vec::new();
    let mut done: Option<Node> = None;
    let mut nodes = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if is_space(c) {
            continue;
        }
        nodes += 1;
        if nodes > MAX_NODES {
            return Err("the file has too many items".into());
        }
        let node = match c {
            '(' => {
                if done.is_some() {
                    break;
                }
                if stack.len() >= MAX_DEPTH {
                    return Err("the file nests too deeply".into());
                }
                stack.push(Vec::new());
                continue;
            }
            ')' => {
                let items = stack.pop().ok_or("a ) with no matching (")?;
                Node::List(items)
            }
            '"' => {
                let mut s = String::new();
                loop {
                    match chars.next() {
                        None => return Err("a string is never closed".into()),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some('r') => s.push('\r'),
                            Some(other) => s.push(other),
                            None => return Err("a string is never closed".into()),
                        },
                        Some(other) => s.push(other),
                    }
                }
                Node::Str(s)
            }
            first => {
                let mut s = String::new();
                s.push(first);
                while let Some(&next) = chars.peek() {
                    if is_space(next) || next == '(' || next == ')' || next == '"' {
                        break;
                    }
                    s.push(next);
                    chars.next();
                }
                Node::Atom(s)
            }
        };
        match stack.last_mut() {
            Some(parent) => parent.push(node),
            None => {
                if done.is_none() && matches!(node, Node::List(_)) {
                    done = Some(node);
                } else if done.is_none() {
                    return Err("text outside the top-level list".into());
                }
            }
        }
    }
    if !stack.is_empty() {
        return Err("a ( is never closed".into());
    }
    done.ok_or_else(|| "no s-expression found".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_atoms_strings_and_lists() {
        let n = parse("(a 1.5 \"b c\" (d -2) yes)").unwrap();
        assert_eq!(n.head(), Some("a"));
        assert_eq!(n.num(0), Some(1.5));
        assert_eq!(n.str_at(1), Some("b c"));
        assert_eq!(n.child("d").and_then(|d| d.num(0)), Some(-2.0));
        assert!(n.has_flag("yes"));
    }

    #[test]
    fn string_escapes_are_read() {
        let n = parse(r#"(t "a\"b\\c\nd")"#).unwrap();
        assert_eq!(n.str_at(0), Some("a\"b\\c\nd"));
    }

    #[test]
    fn hostile_text_is_an_error_not_a_panic() {
        assert!(parse("").is_err());
        assert!(parse(")").is_err());
        assert!(parse("(a").is_err());
        assert!(parse("(a \"b").is_err());
        assert!(parse("text").is_err());
        let deep = "(".repeat(100_000);
        assert!(parse(&deep).is_err());
        let deep_closed = format!("{}{}", "(".repeat(MAX_DEPTH + 1), ")".repeat(MAX_DEPTH + 1));
        assert!(parse(&deep_closed).is_err());
        let ok = format!("{}{}", "(".repeat(MAX_DEPTH), ")".repeat(MAX_DEPTH));
        assert!(parse(&ok).is_ok());
    }

    #[test]
    fn a_number_must_be_finite() {
        let n = parse("(a inf NaN 1e400 3)").unwrap();
        assert_eq!(n.num(0), None);
        assert_eq!(n.num(1), None);
        assert_eq!(n.num(2), None);
        assert_eq!(n.num(3), Some(3.0));
    }
}
