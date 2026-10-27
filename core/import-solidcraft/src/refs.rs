// SPDX-License-Identifier: MIT
//! Faces, edges and vertices by the points the file picks them at.
//!
//! SolidCraft keeps a pick as a point on the face or edge (and a name of
//! its own that it looks up first). The import finds the face or edge of
//! the rebuilt bodies nearest to the point, within [`REACH`] (picks on
//! curved faces lie on the faces' triangles, a little inside the
//! surface), and refers to it by Mitcad's topological name, which follows
//! the face through later edits as SolidCraft's name does.

use mitcad_model::datum::PathParameter;
use mitcad_model::{BodyUid, ComponentUid, EdgeName, FaceName, Kernel, VertexName};

use super::Importer;
use mitcad_import_base::geom::distance;

/// How far a pick may lie from the face or edge it picks (mm).
pub(crate) const REACH: f64 = 0.05;

impl<K: Kernel> Importer<'_, '_, K> {
    /// The bodies a pick is looked for on: one by the file's name, else
    /// the component's.
    fn pick_bodies(
        &self,
        component: ComponentUid,
        body: Option<&str>,
    ) -> Result<Vec<BodyUid>, String> {
        match body.filter(|b| !b.is_empty()) {
            Some(name) => Ok(vec![self.body(name)?]),
            None => Ok(self.component_bodies(component)),
        }
    }

    /// The face nearest to `p`.
    pub(crate) fn face_at(
        &self,
        component: ComponentUid,
        body: Option<&str>,
        p: [f64; 3],
    ) -> Result<(BodyUid, String), String> {
        let kernel = self.doc.kernel();
        let mut best: Option<(f64, BodyUid, String)> = None;
        for uid in self.pick_bodies(component, body)? {
            let Some(shape) = self.doc.body_shape(uid) else {
                continue;
            };
            let Ok(faces) = kernel.faces(shape) else {
                continue;
            };
            for face in faces {
                let Some(name) = face.names.first() else {
                    continue;
                };
                let Ok(parsed) = name.parse::<FaceName>() else {
                    continue;
                };
                let Ok(near) = kernel.face_point_normal(shape, &parsed, p) else {
                    continue;
                };
                let d = distance(near.point, p);
                if best.as_ref().is_none_or(|(b, ..)| d < *b) {
                    best = Some((d, uid, name.clone()));
                }
            }
        }
        match best {
            Some((d, uid, name)) if d <= REACH => Ok((uid, name)),
            Some((d, ..)) => Err(format!("no face at {p:?} (the nearest is {d:.3} mm away)")),
            None => Err(format!("no face at {p:?}: no body")),
        }
    }

    /// The edge nearest to `p`.
    pub(crate) fn edge_at(
        &self,
        component: ComponentUid,
        body: Option<&str>,
        p: [f64; 3],
    ) -> Result<(BodyUid, String), String> {
        let kernel = self.doc.kernel();
        let mut best: Option<(f64, BodyUid, String)> = None;
        for uid in self.pick_bodies(component, body)? {
            let Some(shape) = self.doc.body_shape(uid) else {
                continue;
            };
            let Ok(edges) = kernel.edges(shape) else {
                continue;
            };
            for edge in edges {
                let Some(name) = edge.name else { continue };
                let Ok(parsed) = name.parse::<EdgeName>() else {
                    continue;
                };
                let Ok(near) =
                    kernel.path_point(shape, std::slice::from_ref(&parsed), PathParameter::Near(p))
                else {
                    continue;
                };
                let d = distance(near.point, p);
                if best.as_ref().is_none_or(|(b, ..)| d < *b) {
                    best = Some((d, uid, name));
                }
            }
        }
        match best {
            Some((d, uid, name)) if d <= REACH => Ok((uid, name)),
            Some((d, ..)) => Err(format!("no edge at {p:?} (the nearest is {d:.3} mm away)")),
            None => Err(format!("no edge at {p:?}: no body")),
        }
    }

    /// The vertex at `p`: where the edges that end there meet.
    pub(crate) fn vertex_at(
        &self,
        component: ComponentUid,
        p: [f64; 3],
    ) -> Result<(BodyUid, String), String> {
        for uid in self.component_bodies(component) {
            if let Some(v) = self.vertex_of(uid, p) {
                return Ok((uid, v));
            }
        }
        Err(format!("no vertex at {p:?}"))
    }

    /// The vertex of a body at `p`: where the edges that end there meet.
    fn vertex_of(&self, uid: BodyUid, p: [f64; 3]) -> Option<String> {
        let kernel = self.doc.kernel();
        let shape = self.doc.body_shape(uid)?;
        let edges = kernel.edges(shape).ok()?;
        let mut faces = Vec::new();
        for edge in edges {
            let Some(name) = edge.name else { continue };
            let Ok(parsed) = name.parse::<EdgeName>() else {
                continue;
            };
            let Ok(g) = kernel.edge_geometry(shape, &parsed) else {
                continue;
            };
            let (start, end) = curve_ends(g);
            if distance(start, p) <= REACH || distance(end, p) <= REACH {
                faces.extend(parsed.faces().iter().cloned());
            }
        }
        let v = VertexName::new(faces)?;
        kernel
            .vertex_point(shape, &v)
            .is_ok_and(|q| distance(q, p) <= REACH)
            .then(|| v.to_string())
    }

    /// The vertex at the end of an edge of a body nearest `p` (where a
    /// spline fillet radius starts, mitcad#464).
    pub(crate) fn edge_end_vertex(
        &self,
        body: BodyUid,
        edge: &str,
        p: [f64; 3],
    ) -> Result<String, String> {
        let shape = self
            .doc
            .body_shape(body)
            .ok_or_else(|| format!("body {body} has no shape"))?;
        let parsed: EdgeName = edge.parse().map_err(|e| format!("edge {edge}: {e}"))?;
        let g = self
            .doc
            .kernel()
            .edge_geometry(shape, &parsed)
            .map_err(|e| format!("edge {edge}: {e}"))?;
        let (start, end) = curve_ends(g);
        let near = if distance(start, p) <= distance(end, p) {
            start
        } else {
            end
        };
        self.vertex_of(body, near)
            .ok_or_else(|| format!("no vertex at the end of edge {edge} at {near:?}"))
    }
}

/// The ends of an edge's curve.
fn curve_ends(g: mitcad_model::datum::CurveGeometry) -> ([f64; 3], [f64; 3]) {
    match g {
        mitcad_model::datum::CurveGeometry::Line { start, end }
        | mitcad_model::datum::CurveGeometry::Circle { start, end, .. }
        | mitcad_model::datum::CurveGeometry::Other { start, end, .. } => (start, end),
    }
}
