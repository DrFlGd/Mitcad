// SPDX-License-Identifier: MIT
//! Table lookups in expressions (mitcad#667): the columns a [`Lookup`]
//! gives for `Table.Column`, and `xlookup`, `match`, `index` and the
//! aggregates over columns (rules 31 to 37 of the module documentation).

use super::Span;
use super::ast::{BinaryOp, Function, Node, NodeKind};
use super::eval::{EvalError, Evaluator, Lookup, Value, compare, dimensionless};
use super::format::{DEFAULT_DECIMALS, format_number, text_literal};
use super::units::{Quantity, Unit};

/// One cell of a table column, as [`Lookup::column`] gives it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TableCell<'a> {
    /// No value (`null` in the design file).
    Empty,
    /// A number in the column's unit: `5.5` in an `mm` column is 5.5 mm,
    /// in an `in` column 5.5 inches.
    Number(f64),
    /// A text, in a [`Unit::TEXT`] column.
    Text(&'a str),
}

/// A table column, as [`Lookup::column`] gives it: its unit and its cells.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableColumn<'a> {
    /// The column's unit: any unit a parameter takes ([`Unit::MM`],
    /// [`Unit::IN`], [`Unit::DEG`], an area), [`Unit::NONE`] for a unitless
    /// column, or [`Unit::TEXT`] for a text column.
    pub unit: Unit,
    /// The cells, from the first row down.
    pub cells: Vec<TableCell<'a>>,
    /// The cells of the table's first column in the same rows, which name
    /// a row in messages (`Screws.dk is empty in row 3 (M5)`). May be left
    /// empty: the messages then give the row number only.
    pub labels: Vec<TableCell<'a>>,
}

impl<'a> TableColumn<'a> {
    /// A column of `unit` with `cells` and no row labels.
    pub fn new(unit: Unit, cells: Vec<TableCell<'a>>) -> Self {
        Self {
            unit,
            cells,
            labels: Vec::new(),
        }
    }

    /// The column with `labels`, the table's first column's cells, naming
    /// its rows in messages.
    pub fn with_labels(mut self, labels: Vec<TableCell<'a>>) -> Self {
        self.labels = labels;
        self
    }
}

/// Why [`Lookup::column`] has no column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnError {
    /// There is no table of that name.
    NoTable,
    /// The table has no column of that name.
    NoColumn,
}

/// How `xlookup` and `match` match a key (their modes have different
/// numbers, as in spreadsheets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchMode {
    /// The cell equal to the key: mode 0 of both.
    Exact,
    /// The cell equal to the key, else the largest cell below it: mode -1
    /// of `xlookup`, 1 of `match`.
    NextSmaller,
    /// The cell equal to the key, else the smallest cell above it: mode 1
    /// of `xlookup`, -1 of `match`.
    NextLarger,
}

/// A column argument with its cells.
struct ColumnArg<'n, 'l> {
    table: &'n str,
    column: &'n str,
    data: TableColumn<'l>,
    span: Span,
}

impl ColumnArg<'_, '_> {
    /// `Screws.dk`.
    fn name(&self) -> String {
        format!("{}.{}", self.table, self.column)
    }

    /// A value of the column's kind: a text, or one of its unit.
    fn kind(&self) -> Value {
        if self.data.unit.is_text() {
            Value::Text(String::new())
        } else {
            Value::Number(cell_number(self.data.unit, 1.0))
        }
    }
}

/// A number cell of a column in `unit` as a quantity.
fn cell_number(unit: Unit, number: f64) -> Quantity {
    if unit.is_text() {
        // A number in a text column, which a table refuses.
        Quantity::unitless(number)
    } else {
        Quantity::new(unit.to_canonical(number), unit.dims())
    }
}

/// A row's label, the text of its first cell.
fn label_text(cell: &TableCell<'_>) -> Option<String> {
    match cell {
        TableCell::Empty => None,
        TableCell::Number(number) => Some(format_number(*number, DEFAULT_DECIMALS)),
        TableCell::Text(text) => Some((*text).to_owned()),
    }
}

const XLOOKUP: &str = "xlookup";
const MATCH: &str = "match";
const INDEX: &str = "index";

impl<'a> Evaluator<'a> {
    /// `xlookup(key; keys; results; [if_not_found]; [mode])`: the cell of
    /// `results` in the first row where `keys` matches `key`.
    pub(crate) fn xlookup(&self, args: &[Node], span: Span) -> Result<Value, EvalError> {
        let key = self.value(&args[0])?;
        let keys = self.column_arg(XLOOKUP, &args[1])?;
        let results = self.column_arg(XLOOKUP, &args[2])?;
        if keys.table != results.table {
            return Err(EvalError::DifferentTables {
                function: XLOOKUP,
                span: results.span,
            });
        }
        if let Some(order) = args.get(5) {
            return Err(EvalError::LookupUnsupported {
                what: "the search order of 'xlookup' (its sixth argument)",
                span: order.span,
            });
        }
        let mode = match args.get(4) {
            Some(arg) => self.mode(Function::XLookup, arg)?,
            None => MatchMode::Exact,
        };
        // As with `if`, the fallback is evaluated anyway: a value error in
        // it counts only when it is used, and it must be of the results'
        // kind and dimension.
        let fallback = args.get(3).map(|arg| self.eval_lenient(arg)).transpose()?;
        if let Some(Ok(value)) = &fallback {
            self.unify_pair(XLOOKUP, value.clone(), results.kind(), span)?;
        }
        match self.find_row(XLOOKUP, &key, args[0].span, &keys, mode)? {
            Some(row) => {
                let value = self.cell(&results, row)?;
                match fallback {
                    Some(Ok(fallback)) => self.unify_pair(XLOOKUP, value, fallback, span),
                    _ => Ok(value),
                }
            }
            None => match fallback {
                Some(Ok(fallback)) => self.unify_pair(XLOOKUP, fallback, results.kind(), span),
                Some(Err(e)) => Err(e),
                None => Err(self.not_found(&key, &keys, mode, span)),
            },
        }
    }

    /// `match(key; keys; [mode])`: the first row, counting from 1, where
    /// `keys` matches `key`; unitless.
    pub(crate) fn match_row(&self, args: &[Node], span: Span) -> Result<Quantity, EvalError> {
        let key = self.value(&args[0])?;
        let keys = self.column_arg(MATCH, &args[1])?;
        let mode = match args.get(2) {
            Some(arg) => self.mode(Function::Match, arg)?,
            None => MatchMode::Exact,
        };
        match self.find_row(MATCH, &key, args[0].span, &keys, mode)? {
            Some(row) => Ok(Quantity::unitless((row + 1) as f64)),
            None => Err(self.not_found(&key, &keys, mode, span)),
        }
    }

    /// `index(column; row)`: the cell of `column` in row `row`, counting
    /// from 1; a fraction is cut off.
    pub(crate) fn index(&self, args: &[Node]) -> Result<Value, EvalError> {
        let column = self.column_arg(INDEX, &args[0])?;
        let q = self.eval(&args[1])?;
        let row = dimensionless(INDEX, q, args[1].span)?.trunc();
        let rows = column.data.cells.len();
        if row < 1.0 || row > rows as f64 {
            return Err(EvalError::RowOutOfRange {
                column: column.name(),
                row,
                rows,
                span: args[1].span,
            });
        }
        // A whole number from 1 to the number of rows.
        self.cell(&column, row as usize - 1)
    }

    /// The values of an aggregate (`min`, `max`, `sum`, `average`,
    /// `median`): its value arguments and the non-empty cells of its
    /// column arguments. Columns of only empty cells give `sum` zero and
    /// the others an error.
    pub(crate) fn aggregate_values(
        &self,
        function: &'static str,
        args: &[Node],
        span: Span,
    ) -> Result<Vec<Quantity>, EvalError> {
        let mut values = Vec::with_capacity(args.len());
        let mut column_dims = None;
        for arg in args {
            if !matches!(arg.kind, NodeKind::Column { .. }) {
                values.push(self.eval(arg)?);
                continue;
            }
            let column = self.column_arg(function, arg)?;
            let unit = column.data.unit;
            if unit.is_text() {
                return Err(EvalError::NotNumber { span: arg.span });
            }
            column_dims = column_dims.or(Some(unit.dims()));
            values.extend(column.data.cells.iter().filter_map(|cell| match cell {
                TableCell::Number(number) => Some(cell_number(unit, *number)),
                TableCell::Empty | TableCell::Text(_) => None,
            }));
        }
        if values.is_empty() {
            return match column_dims {
                Some(dims) if function == Function::Sum.name() => {
                    Ok(vec![Quantity::new(0.0, dims)])
                }
                _ => Err(EvalError::NoValues { function, span }),
            };
        }
        Ok(values)
    }

    /// The column that the argument `node` of `function` names, with its
    /// cells from the lookup.
    fn column_arg<'n>(
        &self,
        function: &'static str,
        node: &'n Node,
    ) -> Result<ColumnArg<'n, 'a>, EvalError> {
        let NodeKind::Column { table, column } = &node.kind else {
            return Err(EvalError::NotColumn {
                function,
                span: node.span,
            });
        };
        let span = node.span;
        let lookup: &'a dyn Lookup = self.lookup;
        let data = lookup.column(table, column).map_err(|e| match e {
            ColumnError::NoTable => EvalError::UnknownTable {
                name: table.clone(),
                span: Span::new(span.start, span.start + table.len()),
            },
            ColumnError::NoColumn => EvalError::UnknownColumn {
                table: table.clone(),
                column: column.clone(),
                span: Span::new(span.end.saturating_sub(column.len()), span.end),
            },
        })?;
        Ok(ColumnArg {
            table,
            column,
            data,
            span,
        })
    }

    /// The match mode of `function` that `node` gives.
    fn mode(&self, function: Function, node: &Node) -> Result<MatchMode, EvalError> {
        let name = function.name();
        let span = node.span;
        let mode = dimensionless(name, self.eval(node)?, span)?;
        let xlookup = function == Function::XLookup;
        // Only whole numbers are modes; a large one saturates.
        let whole = (mode.fract() == 0.0).then_some(mode as i64);
        match whole {
            Some(0) => Ok(MatchMode::Exact),
            Some(-1) if xlookup => Ok(MatchMode::NextSmaller),
            Some(1) if xlookup => Ok(MatchMode::NextLarger),
            Some(1) => Ok(MatchMode::NextSmaller),
            Some(-1) => Ok(MatchMode::NextLarger),
            Some(2) if xlookup => Err(EvalError::LookupUnsupported {
                what: "wildcard matching (mode 2)",
                span,
            }),
            _ => Err(EvalError::InvalidMode {
                function: name,
                mode,
                span,
            }),
        }
    }

    /// The first row (from 0) of `keys` that matches `key` by `mode`.
    /// Empty cells are skipped. A text key needs a text column and an
    /// exact match, a number key a number column of its dimension (a
    /// unitless key or column read in the default unit, rule 3); numbers
    /// are equal as `==` compares them.
    fn find_row(
        &self,
        function: &'static str,
        key: &Value,
        key_span: Span,
        keys: &ColumnArg<'_, '_>,
        mode: MatchMode,
    ) -> Result<Option<usize>, EvalError> {
        let unit = keys.data.unit;
        let key = match key {
            Value::Text(key) => {
                if !unit.is_text() {
                    return Err(EvalError::NotNumber { span: key_span });
                }
                if mode != MatchMode::Exact {
                    return Err(EvalError::ApproximateText {
                        function,
                        span: key_span,
                    });
                }
                // Texts are equal as `==` compares them, case included.
                let text = TableCell::Text(key);
                return Ok(keys.data.cells.iter().position(|cell| *cell == text));
            }
            Value::Number(key) => *key,
        };
        if unit.is_text() {
            return Err(EvalError::NotText { span: key_span });
        }
        // The dimensions agree even for a column of empty cells.
        self.unify(function, key, cell_number(unit, 1.0), key_span)?;
        let mut best: Option<(usize, f64)> = None;
        for (row, cell) in keys.data.cells.iter().enumerate() {
            let TableCell::Number(number) = cell else {
                continue;
            };
            let (key, cell) = self.unify(function, key, cell_number(unit, *number), key_span)?;
            if compare(BinaryOp::Eq, cell.value, key.value) {
                // An exact match wins; the rows before had none.
                return Ok(Some(row));
            }
            // The approximate modes look at every row: the column need not
            // be sorted, and of equal cells the first wins.
            let closer = match mode {
                MatchMode::Exact => false,
                MatchMode::NextSmaller => {
                    cell.value < key.value && best.is_none_or(|(_, best)| cell.value > best)
                }
                MatchMode::NextLarger => {
                    cell.value > key.value && best.is_none_or(|(_, best)| cell.value < best)
                }
            };
            if closer {
                best = Some((row, cell.value));
            }
        }
        Ok(best.map(|(row, _)| row))
    }

    /// The cell of `column` in `row` (from 0) as a value of the column's
    /// unit: `0.25` in an `in` column is 6.35 mm, a unitless column gives a
    /// number and a text column a text. An empty cell is an error.
    fn cell(&self, column: &ColumnArg<'_, '_>, row: usize) -> Result<Value, EvalError> {
        match column.data.cells.get(row) {
            Some(TableCell::Number(number)) => {
                Ok(Value::Number(cell_number(column.data.unit, *number)))
            }
            Some(TableCell::Text(text)) => Ok(Value::Text((*text).to_owned())),
            Some(TableCell::Empty) | None => Err(EvalError::EmptyCell {
                column: column.name(),
                row: row + 1,
                label: column.data.labels.get(row).and_then(label_text),
                span: column.span,
            }),
        }
    }

    /// The error for a key that `keys` does not have: the key written as
    /// an expression writes it, a unitless one in the column's unit.
    fn not_found(
        &self,
        key: &Value,
        keys: &ColumnArg<'_, '_>,
        mode: MatchMode,
        span: Span,
    ) -> EvalError {
        let unit = keys.data.unit;
        let key = match key {
            Value::Text(text) => text_literal(text),
            Value::Number(q) if q.is_dimensionless() && !unit.is_text() => {
                self.number_text(self.promote(*q, unit.dims()))
            }
            Value::Number(q) => self.number_text(*q),
        };
        EvalError::NotFound {
            column: keys.name(),
            key,
            mode,
            span,
        }
    }
}
