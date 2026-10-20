// SPDX-License-Identifier: MIT
//! SolidCraft's parameter expressions (`crates/doc/src/expr.rs`) in
//! Mitcad's expression language (`mitcad_model::expr`).
//!
//! The expression is read with SolidCraft's grammar and written again with
//! Mitcad's, keeping its value:
//!
//! - Precedence: `+ -` below `* / %` and juxtaposition (`2 width`,
//!   `10 mm`: a product) below unary signs below `^`; the parentheses
//!   written keep that reading in Mitcad, whose unit suffix binds tighter
//!   than any operator. A number with a unit stays `10 mm`; a unit alone
//!   is `(1 mm)`.
//! - Units are values (`mm`, `cm`, `m`, `um`, `µm`, `in`, `inch`, `ft`,
//!   `yd`, `km`, `mil`, `nm`, `deg`, `rad`, `°`, and `"` and `'` for inches
//!   and feet); `nm` becomes `0.000001 mm`.
//! - A decimal comma outside parentheses (`2,5 mm`), and mixed numbers
//!   (`2 1/8`) are numbers; inside a function's parentheses a comma
//!   separates arguments, as `;` does: Mitcad's printing uses `;`.
//! - `pi`/`PI` is `PI`. Functions: the same names where Mitcad has the
//!   function with the same meaning; `log10` is `log`, `hypot(a, b)` is
//!   `sqrt(a^2 + b^2)`, `mod(a, b)` is `a % b`, `trunc(x)` and SolidCraft's
//!   `sign(x)` (the signum; Mitcad's `sign` is 0 or 1) are written with
//!   `if`.
//! - Parameter names are written through a rename (names Mitcad reserves).

/// A token.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// A number as written (decimal comma made a point).
    Num(String),
    Id(String),
    Op(char),
}

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    let mut depth = 0usize;
    let digit = |k: usize| cs.get(k).is_some_and(|c| c.is_ascii_digit());
    while let Some(&c) = cs.get(i) {
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let st = i;
            while cs.get(i).is_some_and(|c| c.is_ascii_digit() || *c == '.') {
                i += 1;
            }
            let has_point = cs[st..i].contains(&'.');
            if depth == 0
                && !has_point
                && i > 0
                && digit(i - 1)
                && cs.get(i) == Some(&',')
                && digit(i + 1)
            {
                i += 1;
                while digit(i) {
                    i += 1;
                }
            }
            if cs.get(i).is_some_and(|c| *c == 'e' || *c == 'E') {
                let sign = cs.get(i + 1).is_some_and(|c| *c == '-' || *c == '+');
                let at = if sign { i + 2 } else { i + 1 };
                if digit(at) {
                    i = at;
                    while digit(i) {
                        i += 1;
                    }
                }
            }
            let text: String = cs[st..i].iter().collect::<String>().replace(',', ".");
            text.parse::<f64>()
                .map_err(|_| format!("bad number `{text}`"))?;
            if text.chars().all(|c| c.is_ascii_digit())
                && let Some((n, d, end)) = mixed_fraction(&cs, i, depth)
            {
                out.push(Tok::Op('('));
                out.push(Tok::Num(text));
                out.push(Tok::Op('+'));
                out.push(Tok::Num(n));
                out.push(Tok::Op('/'));
                out.push(Tok::Num(d));
                out.push(Tok::Op(')'));
                i = end;
                continue;
            }
            out.push(Tok::Num(text));
        } else if c.is_alphabetic() || c == '_' || c == 'µ' {
            let st = i;
            while cs.get(i).is_some_and(|c| c.is_alphanumeric() || *c == '_') {
                i += 1;
            }
            out.push(Tok::Id(cs[st..i].iter().collect()));
        } else if c == '°' {
            out.push(Tok::Id("deg".into()));
            i += 1;
        } else if c == '"' || c == '″' {
            out.push(Tok::Id("in".into()));
            i += 1;
        } else if c == '\'' || c == '′' {
            out.push(Tok::Id("ft".into()));
            i += 1;
        } else if c == '×' || c == '·' {
            out.push(Tok::Op('*'));
            i += 1;
        } else if c == '÷' {
            out.push(Tok::Op('/'));
            i += 1;
        } else if c == '−' {
            out.push(Tok::Op('-'));
            i += 1;
        } else if c == ';' {
            out.push(Tok::Op(','));
            i += 1;
        } else if "+-*/^(),%".contains(c) {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            out.push(Tok::Op(c));
            i += 1;
        } else {
            return Err(format!("unexpected `{c}`"));
        }
    }
    Ok(out)
}

/// The proper fraction of a mixed number whose whole part ends at `i`:
/// its numerator and denominator digits and where it ends.
fn mixed_fraction(cs: &[char], mut i: usize, depth: usize) -> Option<(String, String, usize)> {
    let digits = |from: usize| {
        cs.get(from..)
            .map_or(0, |r| r.iter().take_while(|c| c.is_ascii_digit()).count())
    };
    let space = i;
    while cs.get(i).is_some_and(|c| *c == ' ' || *c == '\t') {
        i += 1;
    }
    let numerator = digits(i);
    if i == space || numerator == 0 || cs.get(i + numerator) != Some(&'/') {
        return None;
    }
    let denominator = digits(i + numerator + 1);
    let end = i + numerator + 1 + denominator;
    let next = |k: usize| cs.get(end + k).copied();
    let continues = next(0) == Some('.')
        || (depth == 0 && next(0) == Some(',') && next(1).is_some_and(|c| c.is_ascii_digit()))
        || (matches!(next(0), Some('e' | 'E'))
            && next(1).is_some_and(|c| c.is_ascii_digit() || c == '-' || c == '+'));
    if denominator == 0 || continues {
        return None;
    }
    let n: String = cs[i..i + numerator].iter().collect();
    let d: String = cs[i + numerator + 1..end].iter().collect();
    let (nv, dv) = (n.parse::<f64>().ok()?, d.parse::<f64>().ok()?);
    (nv < dv && dv > 0.0).then_some((n, d, end))
}

/// A unit's spelling in Mitcad, as a value.
fn unit(name: &str) -> Option<&'static str> {
    Some(match name {
        "mm" => "mm",
        "cm" => "cm",
        "m" => "m",
        "um" | "µm" => "um",
        "in" | "inch" => "in",
        "ft" => "ft",
        "yd" => "yd",
        "km" => "km",
        "mil" => "mil",
        "nm" => "nm",
        "deg" => "deg",
        "rad" => "rad",
        _ => return None,
    })
}

/// The expression tree.
#[derive(Debug, Clone, PartialEq)]
enum Ast {
    Num(String),
    Unit(&'static str),
    Pi,
    Name(String),
    Neg(Box<Ast>),
    Bin(char, Box<Ast>, Box<Ast>),
    Call(String, Vec<Ast>),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > 64 {
            Err("expression nested too deeply".to_owned())
        } else {
            Ok(())
        }
    }

    fn expr(&mut self) -> Result<Ast, String> {
        self.enter()?;
        let mut a = self.term()?;
        while let Some(Tok::Op(c @ ('+' | '-'))) = self.peek().cloned() {
            self.pos += 1;
            let b = self.term()?;
            a = Ast::Bin(c, Box::new(a), Box::new(b));
        }
        self.depth -= 1;
        Ok(a)
    }

    fn term(&mut self) -> Result<Ast, String> {
        let mut a = self.unary()?;
        loop {
            match self.peek().cloned() {
                Some(Tok::Op(c @ ('*' | '/' | '%'))) => {
                    self.pos += 1;
                    let b = self.unary()?;
                    a = Ast::Bin(c, Box::new(a), Box::new(b));
                }
                Some(Tok::Id(_) | Tok::Num(_)) | Some(Tok::Op('(')) => {
                    let b = self.unary()?;
                    a = Ast::Bin('*', Box::new(a), Box::new(b));
                }
                _ => return Ok(a),
            }
        }
    }

    fn unary(&mut self) -> Result<Ast, String> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.pos += 1;
                self.enter()?;
                let v = self.unary()?;
                self.depth -= 1;
                Ok(Ast::Neg(Box::new(v)))
            }
            Some(Tok::Op('+')) => {
                self.pos += 1;
                self.enter()?;
                let v = self.unary()?;
                self.depth -= 1;
                Ok(v)
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<Ast, String> {
        let base = self.atom()?;
        if let Some(Tok::Op('^')) = self.peek() {
            self.pos += 1;
            self.enter()?;
            let e = self.unary()?;
            self.depth -= 1;
            return Ok(Ast::Bin('^', Box::new(base), Box::new(e)));
        }
        Ok(base)
    }

    fn atom(&mut self) -> Result<Ast, String> {
        match self.next() {
            Some(Tok::Num(v)) => Ok(Ast::Num(v)),
            Some(Tok::Op('(')) => {
                let v = self.expr()?;
                match self.next() {
                    Some(Tok::Op(')')) => Ok(v),
                    _ => Err("missing `)`".to_owned()),
                }
            }
            Some(Tok::Id(id)) => {
                if let Some(Tok::Op('(')) = self.peek() {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if matches!(self.peek(), Some(Tok::Op(')'))) {
                        self.pos += 1;
                    } else {
                        loop {
                            args.push(self.expr()?);
                            match self.next() {
                                Some(Tok::Op(',')) => continue,
                                Some(Tok::Op(')')) => break,
                                _ => {
                                    return Err(format!(
                                        "missing `)` after the arguments of `{id}`"
                                    ));
                                }
                            }
                        }
                    }
                    return Ok(Ast::Call(id, args));
                }
                if id == "pi" || id == "PI" {
                    return Ok(Ast::Pi);
                }
                if let Some(u) = unit(&id) {
                    return Ok(Ast::Unit(u));
                }
                Ok(Ast::Name(id))
            }
            Some(Tok::Op(c)) => Err(format!("unexpected `{c}`")),
            None => Err("unexpected end of expression".to_owned()),
        }
    }
}

/// Precedence levels of the printed forms.
const ADD: u8 = 1;
const MUL: u8 = 2;
const UNARY: u8 = 3;
const POW: u8 = 4;
const ATOM: u8 = 5;

struct Printer<'a> {
    rename: &'a dyn Fn(&str) -> String,
}

impl Printer<'_> {
    /// The text and its precedence.
    fn print(&self, a: &Ast) -> Result<(String, u8), String> {
        Ok(match a {
            Ast::Num(n) => (n.clone(), ATOM),
            Ast::Unit("nm") => ("(0.000001 mm)".to_owned(), ATOM),
            Ast::Unit(u) => (format!("(1 {u})"), ATOM),
            Ast::Pi => ("PI".to_owned(), ATOM),
            Ast::Name(n) => ((self.rename)(n), ATOM),
            Ast::Neg(x) => (format!("-{}", self.at(x, UNARY)?), UNARY),
            Ast::Bin('*', x, y) if matches!(**y, Ast::Unit(_)) && matches!(**x, Ast::Num(_)) => {
                let (Ast::Num(n), Ast::Unit(u)) = (&**x, &**y) else {
                    unreachable!()
                };
                if *u == "nm" {
                    (format!("{n} * 0.000001 mm"), MUL)
                } else {
                    (format!("{n} {u}"), MUL)
                }
            }
            Ast::Bin(op, x, y) => match op {
                '+' | '-' => (
                    format!("{} {op} {}", self.at(x, ADD)?, self.at(y, MUL)?),
                    ADD,
                ),
                '*' | '/' | '%' => (
                    format!("{} {op} {}", self.at(x, MUL)?, self.at(y, UNARY)?),
                    MUL,
                ),
                // Right-associative in both; the exponent is a unary.
                _ => (format!("{}^{}", self.at(x, ATOM)?, self.at(y, UNARY)?), POW),
            },
            Ast::Call(f, args) => self.call(f, args)?,
        })
    }

    /// `a` printed where precedence `min` is needed.
    fn at(&self, a: &Ast, min: u8) -> Result<String, String> {
        let (s, p) = self.print(a)?;
        Ok(if p < min { format!("({s})") } else { s })
    }

    fn args(&self, args: &[Ast]) -> Result<Vec<String>, String> {
        args.iter().map(|a| self.at(a, ADD)).collect()
    }

    fn call(&self, f: &str, args: &[Ast]) -> Result<(String, u8), String> {
        let a = self.args(args)?;
        let one = || -> Result<&String, String> {
            match a.as_slice() {
                [x] => Ok(x),
                _ => Err(format!("`{f}` takes one argument")),
            }
        };
        let two = || -> Result<(&String, &String), String> {
            match a.as_slice() {
                [x, y] => Ok((x, y)),
                _ => Err(format!("`{f}` takes two arguments")),
            }
        };
        let s = match f {
            "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "sqrt" | "abs" | "floor"
            | "ceil" | "round" | "exp" | "ln" | "log" => format!("{f}({})", one()?),
            "log10" => format!("log({})", one()?),
            "atan2" | "pow" => {
                let (x, y) = two()?;
                format!("{f}({x}; {y})")
            }
            "min" | "max" => {
                if a.is_empty() {
                    return Err(format!("`{f}` needs arguments"));
                }
                format!("{f}({})", a.join("; "))
            }
            "hypot" => {
                let (x, y) = two()?;
                format!("sqrt(({x})^2 + ({y})^2)")
            }
            "mod" => {
                let (x, y) = two()?;
                return Ok((format!("({x}) % ({y})"), MUL));
            }
            "trunc" => {
                let x = one()?;
                format!("if(({x}) < 0; ceil({x}); floor({x}))")
            }
            "sign" => {
                let x = one()?;
                format!("if(({x}) > 0; 1; if(({x}) < 0; -1; 0))")
            }
            _ => return Err(format!("unknown function `{f}`")),
        };
        Ok((s, ATOM))
    }
}

/// `s` in Mitcad's language; parameter names go through `rename`. An
/// error says what SolidCraft's grammar does not read.
pub fn translate(s: &str, rename: &dyn Fn(&str) -> String) -> Result<String, String> {
    let toks = lex(s)?;
    if toks.is_empty() {
        return Err("empty expression".to_owned());
    }
    let mut p = Parser {
        toks,
        pos: 0,
        depth: 0,
    };
    let ast = p.expr()?;
    if p.pos != p.toks.len() {
        return Err(format!("unexpected `{}`", s.trim()));
    }
    Ok(Printer { rename }.print(&ast)?.0)
}

/// The parameter names an expression refers to.
pub fn references(s: &str) -> Vec<String> {
    let Ok(toks) = lex(s) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if let Tok::Id(id) = t {
            let call = matches!(toks.get(i + 1), Some(Tok::Op('(')));
            if !call && unit(id).is_none() && id != "pi" && id != "PI" && !out.contains(id) {
                out.push(id.clone());
            }
        }
    }
    out
}

/// A number (with an optional unit) and nothing else: its value in mm or
/// radians (bare numbers taken as `bare`, the factor of a unit-less number:
/// 1 for mm, π/180 for degrees).
pub fn literal(s: &str, bare: f64) -> Option<f64> {
    let toks = lex(s).ok()?;
    let (sign, rest) = match toks.first() {
        Some(Tok::Op('-')) => (-1.0, &toks[1..]),
        Some(Tok::Op('+')) => (1.0, &toks[1..]),
        _ => (1.0, &toks[..]),
    };
    let factor = |u: &str| -> Option<f64> {
        Some(match u {
            "mm" => 1.0,
            "cm" => 10.0,
            "m" => 1000.0,
            "um" | "µm" => 0.001,
            "in" | "inch" => 25.4,
            "ft" => 304.8,
            "yd" => 914.4,
            "km" => 1e6,
            "mil" => 0.0254,
            "nm" => 1e-6,
            "deg" => std::f64::consts::PI / 180.0,
            "rad" => 1.0,
            _ => return None,
        })
    };
    match rest {
        [Tok::Num(n)] => Some(sign * n.parse::<f64>().ok()? * bare),
        [Tok::Num(n), Tok::Id(u)] => Some(sign * n.parse::<f64>().ok()? * factor(u)?),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> String {
        translate(s, &|n| n.to_owned()).unwrap()
    }

    #[test]
    fn expressions_keep_their_reading() {
        assert_eq!(t("40 mm"), "40 mm");
        assert_eq!(t("width / 2"), "width / 2");
        assert_eq!(t("2 * width + 5 mm"), "2 * width + 5 mm");
        assert_eq!(t("2 width"), "2 * width");
        assert_eq!(t("(a + 2) mm"), "(a + 2) * (1 mm)");
        assert_eq!(t("a - (b - c)"), "a - (b - c)");
        assert_eq!(t("a / (b * c)"), "a / (b * c)");
        assert_eq!(t("-2^2"), "-2^2");
        assert_eq!(t("(10 mm)^2"), "(10 mm)^2");
        assert_eq!(t("2,5 mm"), "2.5 mm");
        assert_eq!(t("max(1,2)"), "max(1; 2)");
        assert_eq!(t("atan2(h; w)"), "atan2(h; w)");
        assert_eq!(t("2*pi"), "2 * PI");
        assert_eq!(t("30°"), "30 deg");
        assert_eq!(t("2 1/8\""), "(2 + 1 / 8) * (1 in)");
        assert_eq!(t("hypot(a, b)"), "sqrt((a)^2 + (b)^2)");
        assert_eq!(t("mod(a, 3)"), "(a) % (3)");
        assert_eq!(t("log10(x)"), "log(x)");
        assert_eq!(t("5 nm"), "5 * 0.000001 mm");
        assert!(translate("foo(1)", &|n| n.to_owned()).is_err());
        assert!(translate("1 +", &|n| n.to_owned()).is_err());
        assert_eq!(translate("E * 2", &|n| format!("{n}_")).unwrap(), "E_ * 2");
    }

    #[test]
    fn references_and_literals() {
        assert_eq!(
            references("2*width + sin(ang) + 3 mm + pi"),
            ["width", "ang"]
        );
        assert_eq!(literal("40 mm", 1.0), Some(40.0));
        assert_eq!(literal("-3", 1.0), Some(-3.0));
        assert!((literal("90 deg", 1.0).unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(literal("width", 1.0), None);
    }
}
