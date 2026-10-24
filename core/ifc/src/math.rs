// SPDX-License-Identifier: MIT
//! Points, directions and affine transforms of placements (millimetres).

/// An affine transform: `p' = M p + t` as three rows `[m0 m1 m2 t]`.
pub type Matrix = [[f64; 4]; 3];

pub const IDENTITY: Matrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

// The vector helpers, shared with the model and the other readers; `length`
// and `normalize` (the unit vector, None for a zero or not finite one) are
// this module's names for `norm` and `unit`.
pub use mitcad_vec3::{add, cross, dot, norm as length, scale, sub, unit as normalize};

/// A frame from an origin, a z direction and a direction the x axis is
/// taken from (projected at right angles to z), as IFC's axis placements
/// do. A missing x direction, or one along z, takes the model's x axis
/// (its y axis when z is along x).
pub fn frame(origin: [f64; 3], z: Option<[f64; 3]>, x: Option<[f64; 3]>) -> Matrix {
    let z = z.and_then(normalize).unwrap_or([0.0, 0.0, 1.0]);
    let along = |v: [f64; 3]| normalize(sub(v, scale(z, dot(v, z))));
    let x = x
        .and_then(along)
        .or_else(|| along([1.0, 0.0, 0.0]))
        .or_else(|| along([0.0, 1.0, 0.0]))
        .unwrap_or([1.0, 0.0, 0.0]);
    let y = cross(z, x);
    from_axes(origin, x, y, z)
}

/// The matrix whose columns are the axes, placed at `origin`.
pub fn from_axes(origin: [f64; 3], x: [f64; 3], y: [f64; 3], z: [f64; 3]) -> Matrix {
    [
        [x[0], y[0], z[0], origin[0]],
        [x[1], y[1], z[1], origin[1]],
        [x[2], y[2], z[2], origin[2]],
    ]
}

pub fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 3];
    for (r, row) in out.iter_mut().enumerate() {
        for c in 0..4 {
            let mut v = (0..3).map(|k| a[r][k] * b[k][c]).sum::<f64>();
            if c == 3 {
                v += a[r][3];
            }
            row[c] = v;
        }
    }
    out
}

pub fn apply(m: &Matrix, p: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
}

/// The linear part applied to a direction (not normalized).
pub fn apply_vector(m: &Matrix, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2])
}

pub fn column(m: &Matrix, c: usize) -> [f64; 3] {
    [m[0][c], m[1][c], m[2][c]]
}

pub fn origin(m: &Matrix) -> [f64; 3] {
    column(m, 3)
}

fn determinant3(m: &Matrix) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

pub fn determinant(m: &Matrix) -> f64 {
    determinant3(m)
}

/// The inverse of an affine transform, or None when it is singular.
pub fn inverse(m: &Matrix) -> Option<Matrix> {
    let d = determinant3(m);
    if d.abs() < 1e-300 || !d.is_finite() {
        return None;
    }
    let mut inv = [[0.0; 4]; 3];
    let a = |r: usize, c: usize| m[r][c];
    inv[0][0] = (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1)) / d;
    inv[0][1] = (a(0, 2) * a(2, 1) - a(0, 1) * a(2, 2)) / d;
    inv[0][2] = (a(0, 1) * a(1, 2) - a(0, 2) * a(1, 1)) / d;
    inv[1][0] = (a(1, 2) * a(2, 0) - a(1, 0) * a(2, 2)) / d;
    inv[1][1] = (a(0, 0) * a(2, 2) - a(0, 2) * a(2, 0)) / d;
    inv[1][2] = (a(0, 2) * a(1, 0) - a(0, 0) * a(1, 2)) / d;
    inv[2][0] = (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0)) / d;
    inv[2][1] = (a(0, 1) * a(2, 0) - a(0, 0) * a(2, 1)) / d;
    inv[2][2] = (a(0, 0) * a(1, 1) - a(0, 1) * a(1, 0)) / d;
    let t = origin(m);
    for row in &mut inv {
        row[3] = -(row[0] * t[0] + row[1] * t[1] + row[2] * t[2]);
    }
    Some(inv)
}

/// Whether the linear part is a rotation (orthonormal, right-handed)
/// within `tolerance`.
pub fn is_rigid(m: &Matrix, tolerance: f64) -> bool {
    let cols = [column(m, 0), column(m, 1), column(m, 2)];
    for i in 0..3 {
        if (dot(cols[i], cols[i]) - 1.0).abs() > tolerance {
            return false;
        }
        for j in i + 1..3 {
            if dot(cols[i], cols[j]).abs() > tolerance {
                return false;
            }
        }
    }
    determinant3(m) > 0.0
}

/// The uniform scale of a transform whose linear part is a rotation times
/// a scale (mirrors included), or None for another (a non-uniform scale,
/// a shear).
pub fn uniform_scale(m: &Matrix, tolerance: f64) -> Option<f64> {
    let cols = [column(m, 0), column(m, 1), column(m, 2)];
    let s = length(cols[0]);
    if s < 1e-12 {
        return None;
    }
    for i in 0..3 {
        if (length(cols[i]) - s).abs() > tolerance * s {
            return None;
        }
        for j in i + 1..3 {
            if dot(cols[i], cols[j]).abs() > tolerance * s * s {
                return None;
            }
        }
    }
    Some(s)
}

/// Rows `[[r00 r01 r02 tx], ...]` as JSON-friendly vectors.
pub fn rows(m: &Matrix) -> Vec<Vec<f64>> {
    m.iter().map(|r| r.to_vec()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Matrix, b: &Matrix) -> bool {
        (0..3).all(|r| (0..4).all(|c| (a[r][c] - b[r][c]).abs() < 1e-12))
    }

    #[test]
    fn frames_and_inverses() {
        let f = frame(
            [1.0, 2.0, 3.0],
            Some([0.0, 0.0, 2.0]),
            Some([0.0, 1.0, 0.0]),
        );
        assert_eq!(apply(&f, [1.0, 0.0, 0.0]), [1.0, 3.0, 3.0]);
        assert!(is_rigid(&f, 1e-12));
        let inv = inverse(&f).expect("invertible");
        assert!(close(&mul(&f, &inv), &IDENTITY));
        // An x direction along z falls back to the model x axis.
        let g = frame([0.0; 3], Some([1.0, 0.0, 0.0]), Some([1.0, 0.0, 0.0]));
        assert!(is_rigid(&g, 1e-12));
        assert_eq!(column(&g, 2), [1.0, 0.0, 0.0]);
        let s = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, -2.0, 0.0],
            [0.0, 2.0, 0.0, 5.0],
        ];
        assert_eq!(uniform_scale(&s, 1e-9), Some(2.0));
        assert!(!is_rigid(&s, 1e-9));
        let n = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        assert_eq!(uniform_scale(&n, 1e-9), None);
    }
}
