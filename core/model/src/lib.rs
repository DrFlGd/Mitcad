// SPDX-License-Identifier: MIT
//! Mitcad parametric model: parameters, features in a timeline, the body
//! state they build, topological names, recompute with a cache, undo and
//! redo, the design file and the JSON command interface.
//!
//! The model does not depend on any geometry kernel. Solid operations go
//! through the [`Kernel`] trait, so the timeline and recompute logic is
//! tested without OCCT (see the mock kernel in `testing.rs`).

pub mod analysis;
pub mod api;
// Appearances: physically based looks of bodies (mitcad#46).
pub mod appearance;
pub mod assembly;
mod base64;
pub mod datum;
mod document;
pub mod exchange;
pub mod expr;
pub mod features;
pub mod file;
mod ids;
mod kernel;
mod parameters;
mod profile;
mod recompute;
pub mod sheet;
#[cfg(test)]
mod sheet_tests;
// Sheet-metal Unfold, Refold and Flat Patterns (mitcad#208).
#[cfg(test)]
mod sheet_flat_tests;
// What the sheet-metal command UI asks of the model (mitcad#210).
#[cfg(test)]
mod sheet_ui_tests;
pub mod sketch;
pub mod topo;
mod transform;

// Components and occurrences (F6).
#[cfg(test)]
mod component_tests;
#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod point_pattern_tests;
/// The mock kernel of the model tests; other crates' tests use it through
/// the `testing` feature (the .f3d importer, T1).
#[cfg(any(test, feature = "testing"))]
pub mod testing;
// Profile features (F1).
#[cfg(test)]
mod extrude_scope_tests;
#[cfg(test)]
mod profile_feature_tests;

pub use document::{
    Added, BodyView, ComponentCopy, DocState, Document, FeatureError, ModelError, PreviewReport,
    ProfileView, RecomputeStats, ShapeAdded, TimelineItem,
};
pub use exchange::{BaseInput, BodyKind, ImportBody, Imported};
pub use features::{FeatureDef, FeatureEntry, ValueInput};
pub use file::FileError;
pub use ids::{BodyUid, EntityUid, FeatureUid, IdError};
pub use kernel::{
    BooleanOp, BooleanOutput, BooleanPiece, BoundingBox, Chamfer, ChamferSize, Curve3, EdgeInfo,
    ExtrudeSpec, FaceInfo, Kernel, KernelError, MassProperties,
};
// .f3d import (T1d).
pub use kernel::EdgeMiddle;
// The faces a run of features made, copied by a pattern.
pub use kernel::{AddedMaterial, FeatureChanges};
// Replace face of curved faces (P5).
pub use kernel::FacePoints;
// FreeCAD import (.FCStd).
pub use kernel::{ElementKind, IndexedElement};
// Profile features (F1).
pub use kernel::{
    Axis, Bound, Cylinder, ExtrudeFeatureSpec, ExtrudeSide, ExtrudeStart, HoleEnd, HoleShape,
    HoleSpec, Plane, RevolveSpec, SideEnd, Target, ThinWall, ThreadSpec, WallLocation,
};
pub use parameters::{ChangeSet, ParamId, Parameter, ParameterError, Parameters};
pub use profile::{ProfileLoop, ProfileRegion, ProfileSegment, SegmentGeometry, SketchFrame};
pub use recompute::FeatureStatus;
pub use topo::{EdgeName, FaceName, RegionKey, SegmentEnd, SegmentKey, TopoName, VertexName};
pub use transform::{Instance, PrimitiveShape, PrimitiveSpec, Transform};

// Face operations (F2).
pub use kernel::{DraftSpec, FilletSet, FilletSize, ShellSpec, ToolInput};
// Construction geometry, analysis and body attributes (F5).
pub use datum::{Datum, DatumAxis, DatumKind, DatumPlane, DatumPoint, OriginDatum};
pub use document::{BodyAttributes, DatumView, DeletedAppearance, FaceAppearance};
pub use features::geom_ref::{GeomRef, PathRef};
// Sweeps, lofts, pipes, coils, ribs and webs (F3).
pub mod sweeps;
pub use sweeps::{
    CoilPosition, CoilSection, CoilSpec, LoftEnd, LoftEndKind, LoftSection, LoftSpec, PathCurve,
    PipeSection, PipeSpec, ProfileScaling, RibSpec, SweepGuide, SweepOrientation, SweepSpec,
    ThicknessLocation,
};
// FreeCAD helices (mitcad#4).
pub use sweeps::HelixSpec;
// Sweeps' rail points (mitcad#469).
pub use sweeps::RailPoint;
// Components and occurrences (F6).
pub use document::{InsertOptions, InstanceView};
pub use ids::{ComponentUid, OccurrenceUid};
// Named views (U5).
pub use document::NamedView;
// Analyses kept in the document (mitcad#41).
pub use document::{Analysis, AnalysisDef, SectionAnalysis, SectionPlane};
// Sketch text (P3).
pub use kernel::{ConeFace, FontGlyphs, FontRequest, FontSource, Glyph};
pub use topo::CurveId;
// Sweeps along guide surfaces (mitcad#157).
pub use sweeps::GuideSurface;
// Progress and cancellation of recomputes (P7).
pub mod monitor;
pub use monitor::{Cancelled, Progress, RecomputeMonitor};
#[cfg(test)]
mod monitor_tests;
// Deterministic result versions and the result store (P7d).
mod fingerprint;
pub mod store;
pub use store::{GcReport, PersistReport, ResultStore, StoreStats};
#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod version_tests;
// Design files of version 3 and their projects' B-rep store (P12a).
mod sha256;
pub use file::{BlobStore, DesignError, FileFormat, FsStore, MemoryStore, Project, Sha256};
// Comparison of two designs or versions (P12c).
pub mod diff;
pub use diff::DesignDiff;
// Render settings of the document (mitcad#47).
pub mod render_settings;
pub use render_settings::RenderSettings;
// Joints between occurrences (mitcad#55).
#[cfg(test)]
mod joint_solver_tests;
#[cfg(test)]
mod joint_tests;
pub mod joints;

// Configuration tables and library components (mitcad#64, mitcad#63).
pub mod configurations;
pub mod library;
pub use configurations::{ConfigurationRow, Configurations};
pub use document::{LibraryChange, LibraryPart, PartsListRow};
pub use library::{LibraryRef, LibraryRequest, LibrarySource, LinkResolver, set_link_resolver};
#[cfg(test)]
mod library_tests;

// Geometry of other components in sketches (mitcad#100).
pub mod links;
pub use links::OccurrenceLink;
#[cfg(test)]
mod link_tests;

// An operation's new faces (the .f3d import's geometric check, mitcad#138).
pub use kernel::NewFace;

// Assembly constraints (mitcad#148).
#[cfg(test)]
mod constraint_tests;
pub mod constraints;

// Move Features into a Component (mitcad#171).
pub use document::AutoselectProfiles;
pub use document::{MoveConflict, MoveGroup, MovePlan, MovedGroup, PlannedGroup};
#[cfg(test)]
mod move_features_tests;

// Selected profile areas: toggled regions, sets of references (mitcad#199).
#[cfg(test)]
mod profile_selection_tests;

// Design policy: per-component creation and link permissions (mitcad#220).
pub mod policy;
pub use document::{PolicyItem, PolicyReport};
pub use policy::{Policy, Setting};
#[cfg(test)]
mod policy_tests;
// Moving an occurrence to another parent (mitcad#219).
#[cfg(test)]
mod reparent_tests;
// A preview's bodies in placed, hidden and linked components (mitcad#266).
#[cfg(test)]
mod preview_placement_tests;
// Bodies from their boundaries and from triangles (the IFC import,
// mitcad#273).
pub mod boundary;
pub use document::FeatureBatch;
// Extrude's automatic operation: the contacts of its tool (mitcad#289).
pub use document::AutoOperation;
pub use kernel::ToolContact;
#[cfg(test)]
mod auto_operation_tests;
// The BUILD tab: library sources, derived parts, bound lengths and the
// placement rules of slot-profile assemblies (mitcad#375).
pub mod build;
// Section curves of sketches (mitcad#389).
pub use kernel::SectionCurve;
#[cfg(test)]
mod section_curve_tests;
// Body ids of a split's pieces after upstream edits (mitcad#408).
#[cfg(test)]
mod piece_ids_tests;
// Motion studies and contact sets of assemblies (mitcad#454).
pub mod contact;
pub mod motion;
pub use contact::{ContactBody, ContactHit};
pub use document::{Driven, JointValues, MotionOutcome};
pub use kernel::PlacedPair;
pub use motion::{ContactSet, MotionStudy, StudyKey};
#[cfg(test)]
mod motion_tests;
// Contour Flange, Convert to Sheet Metal and Perforate (mitcad#461).
#[cfg(test)]
mod sheet_more_tests;
// Motion links between joints: gears, racks, screws and cams (mitcad#466).
pub mod motion_links;
pub use motion_links::LinkResult;
#[cfg(test)]
mod motion_link_tests;
// Emboss: profiles raised or sunk on faces, wrapped on cylinders (mitcad#458).
#[cfg(test)]
mod emboss_tests;
pub use kernel::EmbossWrapSpec;
// Boundary Fill: solids of the cells tool bodies enclose (mitcad#457).
pub use kernel::BoundaryCell;
#[cfg(test)]
mod boundary_fill_tests;
// Patch, Stitch and Thicken: surface bodies filled, sewn and made solid
// (mitcad#456).
pub mod surfaces;
pub use surfaces::{OpenBoundary, PatchBoundary, PatchContinuity, PatchLoopSpec};
#[cfg(test)]
mod surface_tests;
// Surface modelling: surface bodies and their features (mitcad#455).
pub mod surface_modelling;
#[cfg(test)]
mod surface_modelling_tests;
pub use kernel::Unstitched;
pub use surface_modelling::{
    BlendEnd, BlendSpec, BodyEdges, BoundaryCurve, BoundaryCurves, BoundarySpec, ExtendMode,
    Fairing, OpeningEdges, RuledCorners, RuledMode, RuledSpec, TrimSpec, TrimTool,
};
// Plastic part features and plastic rules (mitcad#459).
pub use features::plastic::PlasticValues;
pub use kernel::RimBandSpec;
// Bodies moved into another component on the timeline (mitcad#465).
pub use document::TransferDestination;
#[cfg(test)]
mod body_transfer_tests;
// Drawings: views with dimensions and a hole table as PDF, SVG or DXF
// (mitcad#471).
pub mod drawing;
// Gears: spur, helical, herringbone, ring, rack and bevel (mitcad#472).
pub use kernel::{GearConeSpec, GearStep, GearSweepSpec};
// Typed property sets on components, occurrences and bodies (mitcad#496).
pub mod properties;
pub use document::PropertyTypeChange;
#[cfg(test)]
mod properties_tests;
// Reorient Design: the whole design turned by quarter turns (mitcad#502).
pub mod reorient;
pub use document::ReorientReport;
#[cfg(test)]
mod reorient_tests;
// Mount frames of any part and placing a part by one (mitcad#525).
pub use document::{
    GridPlace, Inserted, MountAt, MountFrame, MountInput, MountPlan, MountPreview, MountSource,
};
pub use features::joint::{MountKind, MountSpec};
#[cfg(test)]
mod mount_tests;
// Extrude: a planar body face as the profile (mitcad#535).
pub use kernel::FaceExtrudeSpec;
#[cfg(test)]
mod face_extrude_tests;
