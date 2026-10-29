// SPDX-License-Identifier: MIT
//! How the rank analysis ([`crate::analysis`]) factors a component's rows
//! (mitcad#883): the order the rows are factored in, and the columns
//! every row leaves out because an earlier row fixes them.
//!
//! The analysis asks of each row in creation order how far it lies from
//! the span of the rows before it. Factoring the Gram matrix in creation
//! order answers that, but fixed geometry made that matrix nearly dense:
//! the implicit equations of hundreds of fixed arcs (created first) share
//! their centres and end points with each other and with the rows that
//! fix them, and eliminating them first joined all those rows. A sketch
//! of projected edges factored a matrix of 1 174 rows with 640 000 entries
//! in its factor, 0.15 s per analysis, for geometry that is fixed anyway.
//! Two exact rewrites keep the answers and the matrix sparse:
//!
//! - **Known columns.** A row with a single column `v` (fixing an unknown:
//!   `Fixed`, a radius dimension) puts the unit vector `e_v` into the span.
//!   Every later row may then leave column `v` out: the span of the rows up
//!   to it, and its distance from the span of the rows before it, are the
//!   same. Rows keep their full length as the unit of that distance, so the
//!   pivots are the same numbers. A dependent row's combination of the
//!   reduced rows gives its combination of the full rows by adding the
//!   left-out parts, each `e_v` as the row that fixed `v` (or that row's
//!   own combination when it was dependent).
//! - **Implicit rows later.** The implicit equations of an entity (an arc's
//!   end on its circle) are factored, instead of first, just before the
//!   first row that fixes a column they could no longer do without (their
//!   free columns could not be matched one to each row), or that fixes
//!   nothing and shares a free column with them. Factoring rows after rows
//!   created after them changes none of the answers if each of them is
//!   independent where it is factored: were a row's dependence in creation
//!   order to need a moved row, that moved row would be a combination of
//!   rows factored before it. So the analysis checks that every moved row
//!   was independent, and factors again in creation order when one was
//!   not (an arc ending where its equation does not change with that end).
//!
//! The pivots and combinations are those of creation order up to rounding;
//! the analysis's tolerances are far above it.

use std::sync::{Arc, OnceLock};

use crate::sparse::{GramPlan, Rows};

/// The order and the left-out columns of a component's rank analysis.
#[derive(Debug)]
pub(crate) struct AnalysisPlan {
    /// The component row factored at each position.
    pub order: Vec<usize>,
    /// The row at this position comes after a row created after it, so it
    /// must be independent there for the plan to hold.
    pub moved: Vec<bool>,
    /// For each column, the position of the first row with that column
    /// alone (`usize::MAX`: none); rows after it leave the column out.
    pub pin: Vec<usize>,
    /// For each position, the Jacobian entries (indices into the row values
    /// of the component's Jacobian) the row keeps, and their columns.
    pub kept_ptr: Vec<usize>,
    pub kept_at: Vec<usize>,
    pub kept_col: Vec<usize>,
    /// The Gram matrix of the kept entries, factored in position order.
    pub gram: Arc<GramPlan>,
    /// The Gram matrix of every row's columns that no row fixes, in a
    /// minimum degree order (for the fully constrained unknowns), made
    /// on first use.
    pub free_gram: OnceLock<Arc<GramPlan>>,
}

/// Which rewrites a plan makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Variant {
    /// Creation order, every column (the analysis as it was).
    Plain,
    /// Creation order, known columns left out.
    Reduced,
    /// Implicit rows later, known columns left out.
    Ordered,
}

/// The plans of one component, each made on first use.
#[derive(Clone, Debug, Default)]
pub(crate) struct AnalysisPlans {
    plans: [OnceLock<Arc<AnalysisPlan>>; 3],
}

impl AnalysisPlans {
    pub fn get(&self, jac: &Rows, n: usize, implicit: &[usize], v: Variant) -> Arc<AnalysisPlan> {
        let k = match v {
            Variant::Plain => 0,
            Variant::Reduced => 1,
            Variant::Ordered => 2,
        };
        Arc::clone(self.plans[k].get_or_init(|| Arc::new(AnalysisPlan::new(jac, n, implicit, v))))
    }
}

impl AnalysisPlan {
    /// `implicit[i]` is the entity of the component's leading row `i` (its
    /// implicit equations come first, an entity's rows together).
    fn new(jac: &Rows, n: usize, implicit: &[usize], variant: Variant) -> AnalysisPlan {
        let m = jac.n_rows();
        let order = if variant == Variant::Ordered {
            later_implicit(jac, n, implicit)
        } else {
            (0..m).collect()
        };
        let mut moved = vec![false; m];
        let mut latest = 0;
        for (k, &r) in order.iter().enumerate() {
            moved[k] = k > 0 && latest > r;
            latest = latest.max(r);
        }
        let mut pin = vec![usize::MAX; n];
        if variant != Variant::Plain {
            for (k, &r) in order.iter().enumerate() {
                if let [c] = jac.row(r).0
                    && pin[*c] == usize::MAX
                {
                    pin[*c] = k;
                }
            }
        }
        let mut kept_ptr = Vec::with_capacity(m + 1);
        let mut kept_at = Vec::new();
        let mut kept_col = Vec::new();
        kept_ptr.push(0);
        let mut row_cols: Vec<Vec<usize>> = Vec::with_capacity(m);
        for (k, &r) in order.iter().enumerate() {
            let start = kept_col.len();
            for (t, &c) in jac.row(r).0.iter().enumerate() {
                // A pin keeps its own column (pin[c] == k).
                if pin[c] == usize::MAX || pin[c] >= k {
                    kept_at.push(jac.ptr[r] + t);
                    kept_col.push(c);
                }
            }
            row_cols.push(kept_col[start..].to_vec());
            kept_ptr.push(kept_col.len());
        }
        let identity: Vec<usize> = (0..m).collect();
        let gram = GramPlan::shared(&row_cols, n, Some(&identity));
        AnalysisPlan {
            order,
            moved,
            pin,
            kept_ptr,
            kept_at,
            kept_col,
            gram,
            free_gram: OnceLock::new(),
        }
    }

    /// Whether any row fixes a column.
    pub fn has_pins(&self) -> bool {
        self.pin.iter().any(|&p| p != usize::MAX)
    }

    /// Whether every row that fixes a column does so at the Jacobian's
    /// current values: its one derivative is not zero.
    pub fn pins_hold(&self, jac: &Rows) -> bool {
        self.pin.iter().all(|&k| {
            k == usize::MAX || {
                let v = jac.val[self.kept_at[self.kept_ptr[k]]];
                v.is_finite() && v != 0.0
            }
        })
    }

    /// The kept entries with the Jacobian's current values, one row per
    /// position.
    pub fn kept_rows(&self, jac: &Rows) -> Rows {
        Rows {
            ptr: self.kept_ptr.clone(),
            col: self.kept_col.clone(),
            val: self.kept_at.iter().map(|&a| jac.val[a]).collect(),
        }
    }

    /// The Gram plan of every component row's columns that no row fixes
    /// (rows indexed as in the component).
    pub fn free_gram(&self, jac: &Rows, n: usize) -> Arc<GramPlan> {
        Arc::clone(self.free_gram.get_or_init(|| {
            let row_cols: Vec<Vec<usize>> = (0..jac.n_rows())
                .map(|r| {
                    jac.row(r)
                        .0
                        .iter()
                        .copied()
                        .filter(|&c| self.pin[c] == usize::MAX)
                        .collect()
                })
                .collect();
            GramPlan::shared(&row_cols, n, None)
        }))
    }
}

/// An entity with more implicit rows than this keeps them first (its
/// test below would cost too much per fixed column).
const MOVED_ROWS: usize = 32;

/// The order with the implicit rows (the first `implicit.len()` rows, an
/// entity's rows together) moved later. An entity's rows go just before
/// the first other row that fixes one of their columns when, without it,
/// their free columns can no longer be matched one to each row (they
/// would depend on each other), or that fixes nothing and shares a free
/// column with them; the rest at the end. The other rows keep their order.
/// Entities with many rows, or whose rows cannot be matched to columns
/// even with every column free, keep theirs first.
fn later_implicit(jac: &Rows, n: usize, implicit: &[usize]) -> Vec<usize> {
    let m = jac.n_rows();
    let k0 = implicit.len();
    // The entities' row ranges.
    let mut groups: Vec<(usize, usize)> = Vec::new();
    for i in 0..k0 {
        match groups.last_mut() {
            Some(g) if implicit[i] == implicit[g.0] => g.1 = i + 1,
            _ => groups.push((i, i + 1)),
        }
    }
    let rows_of =
        |g: usize| -> Vec<&[usize]> { (groups[g].0..groups[g].1).map(|i| jac.row(i).0).collect() };
    let mut pinned = vec![false; n];
    let mut placed = vec![false; groups.len()];
    let mut order = Vec::with_capacity(m);
    let mut col_groups: Vec<Vec<usize>> = vec![Vec::new(); n];
    for g in 0..groups.len() {
        let rows = rows_of(g);
        if rows.len() > MOVED_ROWS || !matched(&rows, |_| true) {
            placed[g] = true;
            order.extend(groups[g].0..groups[g].1);
            continue;
        }
        let mut cols: Vec<usize> = rows.iter().flat_map(|r| r.iter().copied()).collect();
        cols.sort_unstable();
        cols.dedup();
        for c in cols {
            col_groups[c].push(g);
        }
    }
    let mut due: Vec<usize> = Vec::new();
    for r in k0..m {
        due.clear();
        match jac.row(r).0 {
            [v] => {
                if !pinned[*v] {
                    for &g in &col_groups[*v] {
                        if !placed[g] && !matched(&rows_of(g), |c| !pinned[c] && c != *v) {
                            due.push(g);
                        }
                    }
                    pinned[*v] = true;
                }
            }
            cols => {
                for &v in cols {
                    if !pinned[v] {
                        due.extend(col_groups[v].iter().copied().filter(|&g| !placed[g]));
                    }
                }
            }
        }
        due.sort_unstable();
        due.dedup();
        for &g in &due {
            placed[g] = true;
            order.extend(groups[g].0..groups[g].1);
        }
        order.push(r);
    }
    for (g, &(a, b)) in groups.iter().enumerate() {
        if !placed[g] {
            order.extend(a..b);
        }
    }
    order
}

/// Whether every row can have a column of its own among the `free` ones
/// (a matching, by augmenting paths; the rows are few).
fn matched(rows: &[&[usize]], free: impl Fn(usize) -> bool) -> bool {
    // (column, row) pairs of the matching.
    let mut owner: Vec<(usize, usize)> = Vec::new();
    let mut seen: Vec<usize> = Vec::new();
    (0..rows.len()).all(|r| {
        seen.clear();
        augment(r, rows, &free, &mut owner, &mut seen)
    })
}

fn augment(
    r: usize,
    rows: &[&[usize]],
    free: &impl Fn(usize) -> bool,
    owner: &mut Vec<(usize, usize)>,
    seen: &mut Vec<usize>,
) -> bool {
    for &c in rows[r] {
        if !free(c) || seen.contains(&c) {
            continue;
        }
        seen.push(c);
        match owner.iter().position(|&(oc, _)| oc == c) {
            None => {
                owner.push((c, r));
                return true;
            }
            Some(i) => {
                let other = owner[i].1;
                if augment(other, rows, free, owner, seen) {
                    owner[i].1 = r;
                    return true;
                }
            }
        }
    }
    false
}
