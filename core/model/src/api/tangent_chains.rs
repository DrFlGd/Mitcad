// SPDX-License-Identifier: MIT
//! The `tangent_chains` query (mitcad#290, see `commands.md`, "Tangent
//! chains"): the edges a fillet or chamfer with a tangent chain rounds
//! together, and the faces a face tool with one takes together, for the
//! application's hover.

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use crate::document::Document;
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::topo::{EdgeName, FaceName};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TangentChainsQuery {
    body: BodyUid,
    /// The chains of faces instead of edges.
    #[serde(default)]
    faces: bool,
    /// Only the chain of this edge (or face, with `faces`).
    #[serde(default)]
    edge: Option<EdgeName>,
    #[serde(default)]
    face: Option<FaceName>,
}

impl<K: Kernel> Document<K> {
    pub(super) fn tangent_chains_json(
        &self,
        query: &TangentChainsQuery,
    ) -> Result<Value, ApiError> {
        let shape = self.require_body(query.body)?;
        let faces = query.faces || query.face.is_some();
        if query.edge.is_some() && faces {
            return Err(ApiError(
                "give an edge or a face (with \"faces\"), not both".to_owned(),
            ));
        }
        let kernel = self.kernel();
        // The one item asked for, which must be in the body.
        let item = if let Some(edge) = &query.edge {
            if kernel.count_edges(shape, edge) == 0 {
                return Err(ApiError(format!("the body has no edge {edge}")));
            }
            Some(edge.to_string())
        } else if let Some(face) = &query.face {
            let count = kernel
                .count_faces(shape, face)
                .map_err(|e| ApiError(e.to_string()))?;
            if count == 0 {
                return Err(ApiError(format!("the body has no face {face}")));
            }
            Some(face.to_string())
        } else {
            None
        };
        let chains = kernel
            .tangent_chains(shape, faces)
            .map_err(|e| ApiError(e.to_string()))?;
        Ok(match item {
            Some(item) => {
                let chain = chains
                    .into_iter()
                    .find(|chain| chain.contains(&item))
                    .unwrap_or_else(|| vec![item]);
                json!({"count": chain.len(), "chain": chain})
            }
            None => json!({"kind": if faces { "faces" } else { "edges" }, "chains": chains}),
        })
    }
}
