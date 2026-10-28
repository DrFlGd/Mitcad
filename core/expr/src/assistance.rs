// SPDX-License-Identifier: MIT
//! Formula editor assistance, using the expression tokenizer and syntax tree.

use super::ast::FUNCTIONS;
use super::lexer::{Tok, lex};
use super::{EvalContext, EvalError, Expr, Function, Lookup, Node, NodeKind, Quantity, Span};

/// Help for a supported function. Examples are ordinary editable formulas.
pub struct FunctionHelp {
    pub name: &'static str,
    pub arguments: &'static [&'static str],
    pub description: &'static str,
    pub example: &'static str,
}

pub fn function_help(function: Function) -> FunctionHelp {
    use Function::*;
    let (arguments, description, example): (&'static [&'static str], &'static str, &'static str) =
        match function {
            Sin => (&["angle"], "Sine of an angle", "SIN(30 deg)"),
            Cos => (&["angle"], "Cosine of an angle", "COS(60 deg)"),
            Tan => (&["angle"], "Tangent of an angle", "TAN(45 deg)"),
            Asin => (&["value"], "Inverse sine, as an angle", "ASIN(0.5)"),
            Acos => (&["value"], "Inverse cosine, as an angle", "ACOS(0.5)"),
            Atan => (&["value"], "Inverse tangent, as an angle", "ATAN(1)"),
            Atan2 => (&["y", "x"], "Angle from two coordinates", "ATAN2(3; 4)"),
            Sinh => (&["value"], "Hyperbolic sine", "SINH(1)"),
            Cosh => (&["value"], "Hyperbolic cosine", "COSH(1)"),
            Tanh => (&["value"], "Hyperbolic tangent", "TANH(1)"),
            Asinh => (&["value"], "Inverse hyperbolic sine", "ASINH(1)"),
            Acosh => (&["value"], "Inverse hyperbolic cosine", "ACOSH(2)"),
            Atanh => (&["value"], "Inverse hyperbolic tangent", "ATANH(0.5)"),
            Sqrt => (
                &["value"],
                "Square root, retaining compatible units",
                "SQRT(25 mm^2)",
            ),
            Abs => (&["value"], "Absolute value", "ABS(-5 mm)"),
            Sign => (&["value"], "Sign: minus one, zero or one", "SIGN(-5)"),
            Exp => (&["value"], "Exponential of a number", "EXP(1)"),
            Ln => (&["value"], "Natural logarithm", "LN(10)"),
            Log => (&["value"], "Base-ten logarithm", "LOG(100)"),
            Floor => (
                &["value", "[step]"],
                "Round down, optionally to a step",
                "FLOOR(23 mm; 5 mm)",
            ),
            Ceil | Ceiling => (
                &["value", "[step]"],
                "Round up, optionally to a step",
                "CEILING(23 mm; 5 mm)",
            ),
            Round => (
                &["value", "[digits]"],
                "Round to a number of decimal places",
                "ROUND(12.345; 2)",
            ),
            RoundUp => (
                &["value", "[digits]"],
                "Round away from zero",
                "ROUNDUP(12.345; 2)",
            ),
            RoundDown | Trunc => (
                &["value", "[digits]"],
                "Truncate toward zero",
                "TRUNC(12.345; 2)",
            ),
            MRound => (
                &["value", "step"],
                "Round to the nearest step",
                "MROUND(23 mm; 5 mm)",
            ),
            Int => (&["value"], "Round down to a whole number", "INT(4.8)"),
            Mod => (
                &["value", "divisor"],
                "Remainder with the divisor's sign",
                "MOD(11; 3)",
            ),
            Quotient => (
                &["value", "divisor"],
                "Whole-number part of a quotient",
                "QUOTIENT(11; 3)",
            ),
            Even => (
                &["value"],
                "Round away from zero to an even whole number",
                "EVEN(5)",
            ),
            Odd => (
                &["value"],
                "Round away from zero to an odd whole number",
                "ODD(4)",
            ),
            Min => (
                &["value1", "value2", "..."],
                "Smallest value, also of a table column",
                "MIN(12 mm; 8 mm)",
            ),
            Max => (
                &["value1", "value2", "..."],
                "Largest value, also of a table column",
                "MAX(12 mm; 8 mm)",
            ),
            Sum => (
                &["value1", "..."],
                "Sum of values, also of a table column",
                "SUM(2 mm; 3 mm; 4 mm)",
            ),
            Average => (
                &["value1", "..."],
                "Arithmetic mean of values, also of a table column",
                "AVERAGE(2 mm; 4 mm)",
            ),
            Product => (&["value1", "..."], "Product of values", "PRODUCT(2; 3; 4)"),
            Median => (
                &["value1", "..."],
                "Middle value after sorting, also of a table column",
                "MEDIAN(2; 9; 4)",
            ),
            Pow => (
                &["value", "exponent"],
                "Raise a value to a power",
                "POW(3; 2)",
            ),
            Clamp => (
                &["value", "minimum", "maximum"],
                "Keep a formula result within bounds",
                "CLAMP(12 mm; 5 mm; 10 mm)",
            ),
            If => (
                &["condition", "if_true", "if_false"],
                "Choose between two values",
                "IF(3 > 2; 10 mm; 5 mm)",
            ),
            And => (
                &["condition1", "..."],
                "True when every condition is true",
                "AND(3 > 2; 5 < 9)",
            ),
            Or => (
                &["condition1", "..."],
                "True when any condition is true",
                "OR(3 > 8; 5 < 9)",
            ),
            Not => (&["condition"], "Reverse a condition", "NOT(3 > 8)"),
            Ifs => (
                &["condition1", "value1", "..."],
                "Value of the first true condition",
                "IFS(3 < 2; 5; 1; 10)",
            ),
            Switch => (
                &["value", "case1", "result1", "...", "[default]"],
                "Choose a result by matching a value",
                "SWITCH(2; 1; 10; 2; 20; 0)",
            ),
            Choose => (
                &["index", "value1", "..."],
                "Choose a value by its one-based index",
                "CHOOSE(2; 10; 20; 30)",
            ),
            IfError => (
                &["value", "fallback"],
                "Use a fallback when a value fails",
                "IFERROR(1 / 0; 0)",
            ),
            Let => (
                &["name", "value", "...", "result"],
                "Name intermediate values inside one formula",
                "LET(pitch; 5 mm; width; 40 mm; FLOOR(width / pitch))",
            ),
            Value => (
                &["value", "unit"],
                "Read a quantity as a number in the given unit",
                "VALUE(25.4 mm; \"in\")",
            ),
            Radians => (&["degrees"], "Convert degrees to radians", "RADIANS(180)"),
            Degrees => (&["radians"], "Convert radians to degrees", "DEGREES(PI)"),
            Concat => (
                &["text", "text"],
                "Join texts and numbers as text",
                "CONCAT(\"Box \"; Width)",
            ),
            Text => (
                &["value", "format"],
                "Write a number as text, optionally in a format",
                "TEXT(Width; \"0.00\")",
            ),
            Left => (
                &["text", "count"],
                "The first characters of a text",
                "LEFT(\"Box\"; 2)",
            ),
            Right => (
                &["text", "count"],
                "The last characters of a text",
                "RIGHT(\"Box\"; 2)",
            ),
            Len => (
                &["text"],
                "The number of characters in a text",
                "LEN(\"Box\")",
            ),
            XLookup => (
                &["key", "keys", "results", "[if_not_found]", "[mode]"],
                "Look a key up in a table column and give the cell of another column in that \
                 row; mode 0 exact, -1 or the next smaller, 1 or the next larger",
                "XLOOKUP('M5'; Screws.Size; Screws.dk)",
            ),
            Match => (
                &["key", "keys", "[mode]"],
                "The row number of a key in a table column; mode 0 exact, 1 or the next \
                 smaller, -1 or the next larger",
                "MATCH('M5'; Screws.Size)",
            ),
            Index => (
                &["column", "row"],
                "The cell of a table column in a row, counting from one",
                "INDEX(Screws.dk; 3)",
            ),
        };
    FunctionHelp {
        name: function.name(),
        arguments,
        description,
        example,
    }
}

pub fn function_catalog() -> impl Iterator<Item = FunctionHelp> {
    FUNCTIONS
        .iter()
        .map(|(function, _, _)| function_help(*function))
}

/// Innermost unfinished function call at a byte cursor, even before it parses.
pub fn signature_at(text: &str, cursor: usize) -> Option<(Function, usize)> {
    let cursor = cursor.min(text.len());
    if !text.is_char_boundary(cursor) {
        return None;
    }
    let tokens = match lex(&text[..cursor]) {
        Ok(tokens) => tokens,
        // Keep the known call stack while a unit string or number is being
        // typed. A lexical error carries the start of that unfinished token.
        Err(error) => lex(&text[..error.span.start]).ok()?,
    };
    let mut calls: Vec<(Option<Function>, usize)> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.tok {
            Tok::LParen => {
                let function = index.checked_sub(1).and_then(|previous| {
                    let t = tokens[previous];
                    (t.tok == Tok::Ident)
                        .then(|| Function::from_name(&text[t.span.start..t.span.end]))
                        .flatten()
                });
                calls.push((function, 0));
            }
            Tok::RParen => {
                calls.pop();
            }
            Tok::Semicolon | Tok::Comma => {
                if let Some((_, argument)) = calls.last_mut() {
                    *argument += 1;
                }
            }
            _ => {}
        }
    }
    calls
        .into_iter()
        .rev()
        .find_map(|(function, argument)| function.map(|f| (f, argument)))
}

/// Indent LET bindings without rewriting text inside quoted tokens.
pub fn indent_let(text: &str) -> String {
    let Ok(tokens) = lex(text) else {
        return text.to_owned();
    };
    let mut result = String::new();
    let mut previous = 0;
    let mut stack: Vec<(bool, usize)> = Vec::new();
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if token.tok == Tok::RParen && stack.last().is_some_and(|(is_let, _)| *is_let) {
            depth = depth.saturating_sub(1);
            while result.ends_with(char::is_whitespace) {
                result.pop();
            }
            result.push('\n');
            result.push_str(&"    ".repeat(depth));
            previous = token.span.start;
        }
        result.push_str(&text[previous..token.span.end]);
        previous = token.span.end;
        let newline = match token.tok {
            Tok::LParen => {
                let is_let = index.checked_sub(1).is_some_and(|previous| {
                    let name = tokens[previous];
                    name.tok == Tok::Ident
                        && text[name.span.start..name.span.end].eq_ignore_ascii_case("let")
                });
                stack.push((is_let, 0));
                if is_let {
                    depth += 1;
                }
                is_let
            }
            Tok::RParen => {
                stack.pop();
                false
            }
            Tok::Semicolon | Tok::Comma => {
                if let Some((is_let, argument)) = stack.last_mut() {
                    *argument += 1;
                    *is_let && *argument % 2 == 0
                } else {
                    false
                }
            }
            _ => false,
        };
        if newline {
            result.push('\n');
            result.push_str(&"    ".repeat(depth));
            previous = tokens
                .get(index + 1)
                .map_or(text.len(), |next| next.span.start);
        }
    }
    result.push_str(&text[previous..]);
    result
}

/// The smallest syntax node containing a cursor or selected byte range.
/// Enclosing LET bindings remain in scope when evaluating that node.
pub fn inspect(
    expression: &Expr,
    selected: Span,
    lookup: &dyn Lookup,
    context: &EvalContext,
) -> (Span, Result<Quantity, EvalError>) {
    fn visit(
        node: &Node,
        selected: Span,
        scopes: &[Vec<Node>],
        best: &mut Option<(Node, Vec<Vec<Node>>)>,
    ) {
        if selected.start < node.span.start || selected.end > node.span.end {
            return;
        }
        *best = Some((node.clone(), scopes.to_vec()));
        match &node.kind {
            NodeKind::Neg(child) | NodeKind::WithUnit { value: child, .. } => {
                visit(child, selected, scopes, best)
            }
            NodeKind::Binary { lhs, rhs, .. } => {
                visit(lhs, selected, scopes, best);
                visit(rhs, selected, scopes, best);
            }
            NodeKind::Call { function, args } => {
                for (index, child) in args.iter().enumerate() {
                    let mut nested = scopes.to_vec();
                    if *function == Function::Let {
                        let pairs = if index == args.len() - 1 {
                            index
                        } else {
                            index / 2 * 2
                        };
                        if pairs > 0 {
                            nested.push(args[..pairs].to_vec());
                        }
                        if index != args.len() - 1 && index % 2 == 0 {
                            if selected.start >= child.span.start && selected.end <= child.span.end
                            {
                                // A declaration's name describes its bound value,
                                // with preceding bindings in scope, not the LET result.
                                let mut value = args[index + 1].clone();
                                value.span = child.span;
                                *best = Some((value, nested));
                            }
                            continue;
                        }
                    }
                    visit(child, selected, &nested, best);
                }
            }
            _ => {}
        }
    }
    let mut best = None;
    visit(expression.root(), selected, &[], &mut best);
    let (mut node, scopes) = best.unwrap_or_else(|| (expression.root().clone(), Vec::new()));
    let span = node.span;
    for mut bindings in scopes.into_iter().rev() {
        bindings.push(node);
        node = Node::new(
            NodeKind::Call {
                function: Function::Let,
                args: bindings,
            },
            span,
        );
    }
    (span, Expr::from_node(node).eval(lookup, context))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_nested_calls_and_decimal_commas_use_real_tokens() {
        assert_eq!(signature_at("FLOOR(", 6), Some((Function::Floor, 0)));
        assert_eq!(
            signature_at("FLOOR(2; MAX(3; 4", 18),
            Some((Function::Max, 1))
        );
        assert_eq!(signature_at("FLOOR(2,5; ", 12), Some((Function::Floor, 1)));
        assert_eq!(signature_at("VALUE(2 mm; \"mm\")", 18), None);
        assert_eq!(
            signature_at("VALUE(2 mm; \"m", 15),
            Some((Function::Value, 1))
        );
    }
    #[test]
    fn inspect_keeps_units_and_lexical_let_bindings() {
        let source = "LET(pitch; 5 mm; width; 40 mm; FLOOR(width / pitch))";
        let expr = Expr::parse(source).unwrap();
        let start = source.find("width / pitch").unwrap();
        let (span, value) = inspect(
            &expr,
            Span::new(start, start + "width / pitch".len()),
            &(),
            &EvalContext::default(),
        );
        assert_eq!(&source[span.start..span.end], "width / pitch");
        assert_eq!(value.unwrap().value, 8.0);
    }
    #[test]
    fn every_supported_function_has_help() {
        assert_eq!(function_catalog().count(), FUNCTIONS.len());
        for help in function_catalog() {
            assert!(
                !help.description.is_empty()
                    && !help.example.is_empty()
                    && !help.arguments.is_empty()
            );
            assert!(Expr::parse(help.example).is_ok(), "{}", help.name);
        }
    }
    #[test]
    fn indentation_preserves_tokens_and_formula_meaning() {
        let text = "LET(pitch; 5 mm; width; 40 mm; FLOOR(width / pitch))";
        let formatted = indent_let(text);
        assert!(formatted.contains("\n    pitch; 5 mm;\n    width; 40 mm;\n"));
        assert_eq!(Expr::parse(text), Expr::parse(&formatted));
        assert_eq!(indent_let("VALUE(5 mm; \"mm\")"), "VALUE(5 mm; \"mm\")");
    }
    #[test]
    fn let_declarations_show_their_binding_values() {
        let text = "LET(a; 1; b; a + 1; a + b)";
        let expression = Expr::parse(text).unwrap();
        for (name, expected) in [("a", 1.0), ("b", 2.0)] {
            let start = text.find(name).unwrap();
            let (span, value) = inspect(
                &expression,
                Span::new(start, start + name.len()),
                &(),
                &EvalContext::default(),
            );
            assert_eq!(&text[span.start..span.end], name);
            assert_eq!(value.unwrap().value, expected);
        }
    }
}
