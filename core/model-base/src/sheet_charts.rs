// SPDX-License-Identifier: MIT
//! Named material charts and the two geometry operations that consume them.

use crate::ids::FeatureUid;
use crate::profile::{SegmentGeometry, SketchFrame};
use crate::topo::SegmentKey;
use crate::transform::Transform;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SheetDomainId {
    pub feature: FeatureUid,
    pub slot: u32,
}

impl fmt::Display for SheetDomainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.p{}", self.feature, self.slot)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SheetCurveId {
    Sketch {
        sketch: FeatureUid,
        segment: SegmentKey,
    },
    Generated {
        feature: FeatureUid,
        slot: u32,
    },
}

impl fmt::Display for SheetCurveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sketch { sketch, segment } => write!(f, "{sketch}/{segment}"),
            Self::Generated { feature, slot } => write!(f, "{feature}/g{slot}"),
        }
    }
}

pub type PanelId = SheetDomainId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BendId {
    pub feature: FeatureUid,
    pub slot: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum SheetDomainRef {
    Panel(PanelId),
    Bend(BendId),
}

impl SheetDomainRef {
    pub fn feature(self) -> FeatureUid {
        match self {
            Self::Panel(id) => id.feature,
            Self::Bend(id) => id.feature,
        }
    }
    pub fn slot(self) -> u32 {
        match self {
            Self::Panel(id) => id.slot,
            Self::Bend(id) => id.slot,
        }
    }
}

/// Directed native support parameters, not a normalized ancestry interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParamSpan {
    pub first: f64,
    pub last: f64,
}

impl ParamSpan {
    pub fn reversed(self) -> Self {
        Self {
            first: self.last,
            last: self.first,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurvePieceRef {
    pub curve: SheetCurveId,
    pub piece: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryPieceRef {
    pub domain: SheetDomainRef,
    pub piece: CurvePieceRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundedCurve2 {
    pub geometry: SegmentGeometry,
    pub range: ParamSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialEdge {
    pub piece: CurvePieceRef,
    pub curve: BoundedCurve2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialLoop {
    pub edges: Vec<MaterialEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialRegion {
    pub loops: Vec<MaterialLoop>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Isometry2 {
    pub origin: [f64; 2],
    pub x_axis: [f64; 2],
    pub y_axis: [f64; 2],
}

impl Isometry2 {
    pub const IDENTITY: Self = Self {
        origin: [0.0, 0.0],
        x_axis: [1.0, 0.0],
        y_axis: [0.0, 1.0],
    };
    pub fn point(self, p: [f64; 2]) -> [f64; 2] {
        std::array::from_fn(|i| self.origin[i] + p[0] * self.x_axis[i] + p[1] * self.y_axis[i])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AxialMap {
    pub offset: f64,
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrimDomain {
    pub domain: SheetDomainRef,
    pub regions: Vec<MaterialRegion>,
    pub to_operation: Isometry2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrimSeamSide {
    pub boundary: BoundaryPieceRef,
    pub span: ParamSpan,
    pub axial: AxialMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrimSeam {
    pub token: u32,
    pub first: TrimSeamSide,
    pub second: TrimSeamSide,
    pub axial_interval: ParamSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetTrimOperation {
    Union,
    Difference,
    Intersection,
}

pub struct SheetTrimSpec<'a> {
    pub operation: SheetTrimOperation,
    pub domains: &'a [TrimDomain],
    pub tools: &'a [MaterialRegion],
    pub seams: &'a [TrimSeam],
    pub tolerance: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrimSource {
    Domain { boundary: BoundaryPieceRef },
    Tool { piece: CurvePieceRef },
}

/// Coverage on the input curve. This is not an affine output-parameter map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub source: TrimSource,
    pub span: ParamSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrimEdge {
    pub token: u32,
    pub curve: BoundedCurve2,
    pub lineage: Vec<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrimLoop {
    pub edges: Vec<TrimEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrimRegion {
    pub token: u32,
    pub loops: Vec<TrimLoop>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrimDomainResult {
    pub domain: SheetDomainRef,
    pub regions: Vec<TrimRegion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrimEndpoint {
    pub domain: SheetDomainRef,
    pub edge_token: u32,
    pub parameter: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeamEventKind {
    Crossing,
    Tangency,
    Endpoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeamSplit {
    pub seam_token: u32,
    pub split_token: u32,
    pub axial: f64,
    pub kind: SeamEventKind,
    pub first: Vec<TrimEndpoint>,
    pub second: Vec<TrimEndpoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrimEndpointSpan {
    pub domain: SheetDomainRef,
    pub edge_token: u32,
    pub range: ParamSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrimPortSpan {
    pub seam_token: u32,
    pub axial_interval: ParamSpan,
    pub first: TrimEndpointSpan,
    pub second: TrimEndpointSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetTrimResult {
    pub domains: Vec<TrimDomainResult>,
    pub splits: Vec<SeamSplit>,
    pub ports: Vec<TrimPortSpan>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CylinderEmbedding {
    pub centre: [f64; 3],
    pub axis: [f64; 3],
    pub radial_start: [f64; 3],
    pub angular_direction: [f64; 3],
    pub inside_radius: f64,
    pub neutral_radius: f64,
    pub angle_extent: f64,
    pub inner_is_front: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialParity {
    Positive,
    Negative,
}

impl MaterialParity {
    pub fn sign(self) -> f64 {
        match self {
            Self::Positive => 1.0,
            Self::Negative => -1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SheetEmbedding {
    Plane {
        front_frame: SketchFrame,
        front_to_back_sign: MaterialParity,
    },
    Cylinder(CylinderEmbedding),
}

impl SheetEmbedding {
    pub fn transformed(&self, t: &Transform) -> Self {
        match self {
            Self::Plane {
                front_frame,
                front_to_back_sign,
            } => Self::Plane {
                front_frame: t.apply_frame(front_frame),
                front_to_back_sign: *front_to_back_sign,
            },
            Self::Cylinder(c) => Self::Cylinder(CylinderEmbedding {
                centre: t.apply_point(c.centre),
                axis: t.apply_vector(c.axis),
                radial_start: t.apply_vector(c.radial_start),
                angular_direction: t.apply_vector(c.angular_direction),
                ..c.clone()
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetPatch {
    pub domain: SheetDomainRef,
    pub regions: Vec<MaterialRegion>,
    pub embedding: SheetEmbedding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapePortSide {
    pub boundary: BoundaryPieceRef,
    pub span: ParamSpan,
    pub axial: AxialMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapePort {
    pub token: u32,
    pub axial_interval: ParamSpan,
    pub first: ShapePortSide,
    pub second: ShapePortSide,
}

pub struct SheetShapeSpec<'a> {
    pub thickness: f64,
    pub patches: &'a [SheetPatch],
    pub ports: &'a [ShapePort],
    pub tolerance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RegionMembership {
    pub domain: SheetDomainRef,
    pub region: usize,
}

pub struct SheetSolid<S> {
    pub shape: S,
    pub membership: Vec<RegionMembership>,
}
pub struct SheetGeometry<S> {
    pub solids: Vec<SheetSolid<S>>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BendDomain {
    pub id: BendId,
    pub stationary: BoundaryPieceRef,
    pub moving: BoundaryPieceRef,
    pub folded_embedding: CylinderEmbedding,
    pub signed_angle: f64,
    pub k_factor: f64,
    pub radius: f64,
    pub allowance: f64,
    pub axial_interval: ParamSpan,
    pub rules: FeatureUid,
    pub rules_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenedBend {
    pub bend: BendId,
    pub by: FeatureUid,
    /// The panel held fixed when `by` opened the bend; its Refold holds the
    /// same panel so that refolding restores the folded placement exactly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stationary: Option<PanelId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveLineage {
    pub boundary: BoundaryPieceRef,
    /// Content generation, not persistent identity: fragment aliases apply
    /// only to the exact bounded curve produced by this trim result.
    pub curve_generation: String,
    /// Transitive semantic identities. Source spans below describe only the
    /// immediate trim input; they never imply an ancestral parameter map.
    pub aliases: Vec<CurvePieceRef>,
    pub sources: Vec<SourceSpan>,
}

impl CurveLineage {
    pub fn generation(curve: &BoundedCurve2) -> String {
        let mut fp = crate::fingerprint::Fingerprint::new("sheet-curve-v1");
        serde_json::to_writer(&mut fp, curve).expect("bounded curve serializes");
        format!("{:032x}", fp.finish())
    }
    pub fn applies_to(&self, domain: SheetDomainRef, edge: &MaterialEdge) -> bool {
        self.boundary.domain == domain
            && self.boundary.piece == edge.piece
            && self.curve_generation == Self::generation(&edge.curve)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryRef {
    pub panel: PanelId,
    pub curve: SheetCurveId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<ParamSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BendPortMode {
    VirtualSharp,
    TangentPorts,
    /// Two opposite bends and a straight between them (a jog) joining two
    /// parallel panels of separate bodies (mitcad#280).
    Double,
}

/// Which boundary of a double bend keeps its panel whole (the first bend
/// starts on it); the other panel is cut back to where the second bend
/// ends (mitcad#280).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoubleBendFixed {
    #[default]
    Stationary,
    Moving,
}
