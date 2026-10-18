// SPDX-License-Identifier: MIT
//! Importer tests on synthetic dumps with the model's mock kernel.

use mitcad_f3d::design::ir::Dump;
use mitcad_model::testing::MockKernel;
use mitcad_model::{Document, FeatureDef};
use serde_json::{Value, json};

use super::*;

fn dump(value: Value) -> Dump {
    Dump::from_json(&value.to_string()).expect("a dump")
}

fn param(name: &str, expression: &str, value: f64, unit: &str) -> Value {
    json!({"kind": "parameter", "name": name, "expression": expression, "value": value, "unit": unit})
}

/// A 40 x 20 mm rectangle on XY from (0, 0) (cm in the IR), its width
/// dimensioned with d1.
fn rectangle_sketch(index: i64) -> Value {
    let p = |id: &str, x: f64, y: f64| json!({"id": id, "xyz": [x, y, 0.0]});
    let line = |id: &str, a: &str, b: &str| json!({"id": id, "type": "SketchLine", "startSketchPoint": a, "endSketchPoint": b});
    json!({
        "index": index, "name": format!("Sketch{}", index + 1), "objectType": "Sketch",
        "detail": {
            "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
            "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
            "points": [p("p0", 0.0, 0.0), p("p1", 4.0, 0.0), p("p2", 4.0, 2.0), p("p3", 0.0, 2.0)],
            "curves": [line("c0", "p0", "p1"), line("c1", "p1", "p2"),
                       line("c2", "p2", "p3"), line("c3", "p3", "p0")],
            "constraints": [
                {"id": "k0", "type": "HorizontalConstraint", "refs": {"entities": ["c0"]}},
                {"id": "k1", "type": "VerticalConstraint", "refs": {"entities": ["c1"]}},
                {"id": "k2", "type": "PolygonConstraint", "refs": {"entities": ["c0", "c1"]}}
            ],
            "dimensions": [
                {"id": "d0", "type": "SketchLinearDimension",
                 "parameter": param("d1", "40 mm", 4.0, "mm"),
                 "props": {"orientation": "HorizontalDimensionOrientation"},
                 "refs": {"entities": ["p0", "p1"]}, "textPosition": [2.0, -0.5, 0.0]}
            ]
        }
    })
}

fn extrude(index: i64, sketch: i64, operation: &str, distance: &str) -> Value {
    json!({
        "index": index, "name": format!("Extrude{index}"), "objectType": "ExtrudeFeature",
        "detail": {
            "operation": operation,
            "profile": [{"kind": "profile", "sketch": "Sketch1", "sketch_timeline_index": sketch}],
            "extentType": "OneSideFeatureExtentType",
            "extentOne": {"_type": "DistanceExtentDefinition",
                          "distance": param(distance, "10 mm", 1.0, "mm")},
            "taperAngleOne": param("d3", "0.0 deg", 0.0, "deg")
        },
        "_f3d": {"extrude": {"operation_code": 4, "direction": 1.0}}
    })
}

fn block_dump() -> Dump {
    dump(json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "block.f3d"},
        "parameters": {
            "user": [{"name": "width", "expression": "height * 2", "value": 4.0, "unit": "mm"},
                     {"name": "height", "expression": "20 mm", "value": 2.0, "unit": "mm",
                      "isFavorite": true}],
            "model": [
                {"name": "d1", "expression": "width", "value": 4.0, "unit": "mm",
                 "createdBy": {"kind": "sketch_dimension", "sketch": "Sketch1",
                               "sketch_timeline_index": 0, "id": "d0"}},
                {"name": "d2", "expression": "10 mm", "value": 1.0, "unit": "mm",
                 "createdBy": {"kind": "feature", "objectType": "ExtrudeFeature",
                               "name": "Extrude1", "timeline_index": 1}},
                {"name": "d3", "expression": "0.0 deg", "value": 0.0, "unit": "deg",
                 "createdBy": {"kind": "feature", "objectType": "ExtrudeFeature",
                               "name": "Extrude1", "timeline_index": 1}}
            ]
        },
        "timeline": {"items": [rectangle_sketch(0), extrude(1, 0, "NewBodyFeatureOperation", "d2")]}
    }))
}

fn import(doc: &mut Document<MockKernel>, dump: &Dump) -> DesignReport {
    import_design(doc, dump, &mut NoGeometry, &Options::default())
}

#[test]
fn the_imports_joins_fuse_within_its_geometric_tolerance() {
    // A join on the block: the document keeps the import's geometric
    // tolerance as its join tolerance (mitcad#298), and the join is made
    // within it.
    let mut source = serde_json::to_value(block_dump()).unwrap();
    source["timeline"]["items"]
        .as_array_mut()
        .unwrap()
        .push(extrude(2, 0, "JoinFeatureOperation", "d2"));
    let mut doc = Document::new(MockKernel::default());
    let options = Options {
        geometric_tolerance: 0.02,
        undo_label: Some("Import".to_owned()),
        ..Options::default()
    };
    let report = import_design(&mut doc, &dump(source), &mut NoGeometry, &options);
    assert_eq!(report.items[2].outcome, Outcome::Parametric);
    assert_eq!(doc.join_tolerance(), Some(0.02));
    assert!(doc.kernel().count("join_within") > 0);
    assert!(
        doc.kernel()
            .specs
            .borrow()
            .iter()
            .all(|s| !s.starts_with("join within") || s == "join within 0.02")
    );
    // Part of the import's one undo step.
    doc.undo();
    assert_eq!(doc.join_tolerance(), None);
    assert!(doc.features().next().is_none());
}

#[test]
fn the_imports_cuts_are_retried_within_its_geometric_tolerance() {
    // A cut into the block goes through the kernel's cut within the
    // document's join tolerance (mitcad#235), which retries a cut that
    // fails exactly with a fuzzy value up to it.
    let mut source = serde_json::to_value(block_dump()).unwrap();
    source["timeline"]["items"]
        .as_array_mut()
        .unwrap()
        .push(extrude(2, 0, "CutFeatureOperation", "d2"));
    let mut doc = Document::new(MockKernel::default());
    let options = Options {
        geometric_tolerance: 0.02,
        ..Options::default()
    };
    import_design(&mut doc, &dump(source), &mut NoGeometry, &options);
    assert!(doc.kernel().count("cut_within") > 0);
    assert!(
        doc.kernel()
            .specs
            .borrow()
            .iter()
            .any(|s| s == "cut within 0.02")
    );
}

#[test]
fn an_extrusion_of_texts_the_file_no_longer_has_says_so() {
    // The text profiles name texts the design no longer has (mitcad#332):
    // no region of the sketch stands in for them.
    let mut source = serde_json::to_value(block_dump()).unwrap();
    source["timeline"]["items"][1]["detail"]["_f3d_missing_texts"] = json!([2696, 2741]);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump(source));
    assert_ne!(report.items[1].outcome, Outcome::Parametric);
    let note = report.items[1].note.clone().unwrap_or_default();
    assert!(
        note.contains(
            "its profile is 2 sketch texts the file no longer has (deleted from Sketch1)"
        ),
        "{note}"
    );
}

#[test]
fn input_provenance_keeps_unknowns_and_reports_the_last_sketch_default() {
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &block_dump());
    // A successful first candidate alone is no proof that every input
    // was read. The sketch is not a modelling candidate and is not counted.
    assert_eq!(report.items[0].input_provenance, None);
    assert_eq!(
        report.items[1].input_provenance,
        Some(InputProvenance::Unknown)
    );
    assert_eq!(
        report.input_counts(),
        BTreeMap::from([(InputProvenance::Unknown, 1)])
    );

    let mut source = serde_json::to_value(block_dump()).unwrap();
    source["timeline"]["items"][1]["detail"]
        .as_object_mut()
        .unwrap()
        .remove("profile");
    let mut doc = Document::new(MockKernel::default());
    let guessed = import(&mut doc, &dump(source));
    // Provenance does not change the established no-history replay rule.
    assert_eq!(guessed.items[1].outcome, Outcome::Parametric);
    assert_eq!(guessed.items[1].features, report.items[1].features);
    assert_eq!(guessed.items[1].verified, None);
    assert_eq!(
        guessed.items[1].input_provenance,
        Some(InputProvenance::Inferred)
    );
    assert_eq!(
        guessed.input_counts(),
        BTreeMap::from([(InputProvenance::Inferred, 1)])
    );
    let json = serde_json::to_value(&guessed).unwrap();
    assert_eq!(json["items"][1]["input_provenance"], "inferred");
    assert!(
        guessed
            .text()
            .contains("modelling inputs: 0 read, 1 inferred, 0 unknown")
    );
}

#[test]
fn parameters_keep_names_and_expressions_in_dependency_order() {
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &block_dump());
    assert_eq!(report.parameters.imported, 5, "{:?}", report.parameters);
    assert!(report.parameters.literal.is_empty());
    assert!(
        report.parameters.mismatched.is_empty(),
        "{:?}",
        report.parameters
    );
    let p = |name: &str| {
        let params = doc.parameters();
        let id = params.find(name).expect(name);
        let p = params.get(id).unwrap();
        (p.expression().to_owned(), p.value())
    };
    assert_eq!(p("width"), ("height * 2".to_owned(), 40.0));
    assert_eq!(p("d1"), ("width".to_owned(), 40.0));
    // Model parameters belong to the items that made them.
    let params = doc.parameters();
    let owner = |name: &str| params.owner(params.find(name).unwrap());
    assert_eq!(owner("d1"), Some(FeatureUid(1)));
    assert_eq!(owner("d2"), Some(FeatureUid(2)));
    assert_eq!(owner("width"), None);
    // The file's favorites keep their star.
    assert!(doc.is_favorite_parameter("height"));
    assert!(!doc.is_favorite_parameter("width"));
}

#[test]
fn sketches_and_extrudes_replay_with_parameters() {
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &block_dump());
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    // The polygon constraint (of two lines) is left out, so the sketch is
    // partial.
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Parametric],
        "{}",
        report.text()
    );
    assert!(
        report.items[0]
            .note
            .as_deref()
            .unwrap()
            .contains("PolygonConstraint: a polygon needs its corners and centre")
    );
    let sketch = doc.feature(FeatureUid(1)).unwrap();
    assert_eq!(sketch.name, "Sketch1");
    let FeatureDef::Sketch(s) = &sketch.def else {
        panic!("a sketch");
    };
    assert_eq!(s.entities.len(), 8);
    assert_eq!(s.constraints.len(), 2);
    assert_eq!(s.dimensions.len(), 1);
    assert!(s.frame.is_none());
    let extrude = doc.feature(FeatureUid(2)).unwrap();
    assert_eq!(extrude.name, "Extrude1");
    let def = doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap();
    let def: Value = serde_json::from_str(&def).unwrap();
    assert_eq!(def["def"]["extent"]["distance"], "d2");
    assert_eq!(def["def"]["operation"], "new_body");
    assert_eq!(doc.bodies().len(), 1);
    // A parameter edit flows through the imported expressions.
    doc.set_parameter_expression("height", "30 mm", None)
        .unwrap();
    assert_eq!(doc.status(FeatureUid(1)).map(|s| s.is_ok()), Some(true));
}

/// The mock kernel with mass properties: a solid's "volume" and "area"
/// come from its history and face names with feature numbers left out,
/// so the same operations give the same values in another document, and
/// a body read back from B-rep data keeps the values of the one written.
#[derive(Default)]
struct TestKernel {
    mock: MockKernel,
    /// Mock B-rep history → the written shape's fingerprint.
    written: std::cell::RefCell<std::collections::HashMap<String, String>>,
    /// Whether edges have middles (see edge_point), so that the edge
    /// fingerprints of external dumps resolve.
    edge_points: bool,
    /// Whether faces are listed with a point each (see face_point), so
    /// that face fingerprints resolve.
    face_listing: bool,
    /// Whether the solids this kernel's extrusions make come out 3e-4
    /// larger than the same extrusion of another kernel (a marker in their
    /// history), as an extrusion of regions next to the file's would.
    skew: bool,
    /// Whether solids without copies of a pattern all measure the same, so
    /// that only patterns tell extrusions of different regions apart.
    alike: bool,
    /// Whether the kernel lists the faces a body has and others lack
    /// (`Kernel::new_faces`): by their names without feature numbers, a
    /// point each (see face_point), so that a skewed body lies on the
    /// stored one (mitcad#283).
    lists_new_faces: bool,
    /// Whether the solids this kernel's fillets make come out 3e-4 larger
    /// (as `skew` for extrusions): an approximation of the file's own kind
    /// that the bodies after them carry (mitcad#283).
    skew_fillets: bool,
    /// Whether this kernel's fillets carry a marker (`≈` in their history)
    /// that makes them measure `fillet_noise` mm³ more than they bound, as
    /// a stored rounding whose edges lie off its faces measures
    /// (mitcad#235).
    noisy_fillets: bool,
    /// mm³ this kernel's fillets take from their body, where given (else
    /// their volume comes from their history as other solids').
    fillet_takes: Option<f64>,
    fillet_noise: f64,
    /// mm³ this kernel's fillets (with `fillet_takes`) leave on their body
    /// beyond what they should take, marked `∆` in their history: a result
    /// that deviates from the file's in its volume only (mitcad#379).
    fillet_off: f64,
    /// The slack `Kernel::changed_volume` gives, where it measures at all:
    /// the change as the bodies' volumes, noise left out.
    change_slack: Option<f64>,
    /// Runs in each kernel call of this name ("extrude_feature",
    /// "offset_faces", "mass_properties") before it is made: a call that
    /// blocks or takes long (mitcad#68, mitcad#69, mitcad#71, mitcad#82).
    #[allow(clippy::type_complexity)]
    on_call: std::cell::RefCell<Option<(&'static str, Box<dyn FnMut(&MockKernel) + Send>)>>,
    /// How many of the next extrusions fail as an operation whose
    /// allocation failed does (mitcad#80).
    out_of_memory: std::cell::Cell<usize>,
    /// Extrusions that take long, every time (mitcad#78).
    slow: Option<SlowExtrusions>,
    /// How many of them were asked for.
    slow_calls: std::cell::Cell<usize>,
    /// Whether extrusions against the sketch's normal differ from those
    /// along it (a marker in their history; the mock kernel's are alike).
    directed: bool,
    /// Shapes whose history contains this are invalid for the kernel's
    /// checker (`Kernel::is_valid`).
    invalid: Option<&'static str>,
    /// How many slow extrusions began, and how many were cut short by a
    /// request to stop, in this kernel and its forks (mitcad#95).
    began: Arc<std::sync::atomic::AtomicUsize>,
    stopped: Arc<std::sync::atomic::AtomicUsize>,
    /// How many times points were classified (probing regions against the
    /// history, mitcad#181); the kernel does not classify them.
    probes: Arc<std::sync::atomic::AtomicUsize>,
}

/// Extrusions of `regions` (along the sketch's normal only, with `along`)
/// take `seconds`, checking for a request to stop as `slow_call`.
#[derive(Clone, Copy)]
struct SlowExtrusions {
    regions: &'static [&'static str],
    along: bool,
    /// Against the sketch's normal only (mitcad#95).
    against: bool,
    seconds: f64,
}

impl TestKernel {
    fn hook(&self, call: &str) {
        if let Some((name, hook)) = self.on_call.borrow_mut().as_mut()
            && *name == call
        {
            hook(&self.mock);
        }
    }

    /// The volume of a solid of the fingerprint `f` (see `mass_properties`):
    /// the last `[=v]` in it (`fillet_takes`), else its hash; `~` scales it
    /// by 1.0003, `≈` adds `fillet_noise`.
    fn volume_of(&self, f: &str) -> f64 {
        let scale = if f.contains('~') { 1.0003 } else { 1.0 };
        let noise = if f.contains('≈') {
            self.fillet_noise
        } else {
            0.0
        };
        let given = f.rfind("[=").and_then(|at| {
            let rest = &f[at + 2..];
            rest[..rest.find(']')?].parse::<f64>().ok()
        });
        let off = if f.contains('∆') {
            self.fillet_off
        } else {
            0.0
        };
        given.unwrap_or_else(|| hash(&f.replace(['~', '≈', '∆'], ""), 1)) * scale + noise + off
    }
}

/// A kernel call that takes up to 30 s, checking for a request to stop as
/// the geometry kernel's long operations do (`interruptible`).
fn slow_call(mock: &MockKernel) {
    slow_call_for(mock, 30.0);
}

/// A kernel call that takes up to `seconds`, as [`slow_call`].
fn slow_call_for(mock: &MockKernel, seconds: f64) -> bool {
    mock.stop_on_cancel.set(true);
    let begun = std::time::Instant::now();
    while begun.elapsed().as_secs_f64() < seconds {
        if mock
            .interrupt
            .borrow()
            .as_ref()
            .is_some_and(|m| m.is_cancelled())
        {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    false
}

use mitcad_model::testing::MockShape;
use mitcad_model::{
    BodyKind, BooleanOp, BooleanOutput, BoundingBox, Chamfer, EdgeName, ExtrudeFeatureSpec,
    ExtrudeSpec, FilletSet, KernelError, MassProperties, Plane,
};

/// Feature numbers left out: `F12` becomes `F`.
fn without_uids(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == 'F' {
            while chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
            }
        }
    }
    out
}

impl TestKernel {
    fn fingerprint(&self, shape: &MockShape) -> String {
        if let Some(inner) = shape.history.strip_prefix("import(")
            && let Some((_, rest)) = inner.split_once(',')
        {
            let original = &rest[..rest.len() - 1];
            if let Some(f) = self.written.borrow().get(original) {
                return f.clone();
            }
        }
        let mut faces: Vec<String> = shape
            .faces
            .iter()
            .map(|f| without_uids(&f.to_string()))
            .collect();
        faces.sort();
        format!("{}|{}", without_uids(&shape.history), faces.join(","))
    }
}

fn hash(text: &str, salt: u64) -> f64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (text, salt).hash(&mut h);
    100.0 + (h.finish() % 100_000) as f64 / 10.0
}

impl Kernel for TestKernel {
    type Shape = MockShape;

    fn is_valid(&self, shape: &MockShape) -> Result<bool, KernelError> {
        match self.invalid {
            Some(marker) => Ok(!shape.history.contains(marker)),
            None => Err(KernelError::Unsupported("validity checks")),
        }
    }

    fn extrude(&self, spec: &ExtrudeSpec<'_>) -> Result<MockShape, KernelError> {
        self.mock.extrude(spec)
    }

    fn extrude_feature(
        &self,
        spec: &ExtrudeFeatureSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.hook("extrude_feature");
        if self.out_of_memory.get() > 0 {
            self.out_of_memory.set(self.out_of_memory.get() - 1);
            return Err(KernelError::failed(format!(
                "extrude: {}",
                KernelError::OUT_OF_MEMORY
            )));
        }
        let keys: BTreeSet<String> = spec.regions.iter().map(|r| r.key.to_string()).collect();
        if let Some(slow) = self.slow
            && keys == slow.regions.iter().map(|r| r.to_string()).collect()
            && (!slow.along || spec.direction[2] > 0.0)
            && (!slow.against || spec.direction[2] < 0.0)
        {
            self.slow_calls.set(self.slow_calls.get() + 1);
            self.began.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if slow_call_for(&self.mock, slow.seconds) {
                self.stopped
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let mut shape = self.mock.extrude_feature(spec)?;
        if self.skew {
            shape.history.push('~');
        }
        if self.directed && spec.direction[2] < 0.0 {
            shape.history.push_str("(flipped)");
        }
        Ok(shape)
    }

    fn solids(&self, shape: &MockShape) -> Result<Vec<MockShape>, KernelError> {
        self.mock.solids(shape)
    }

    fn interruptible<R>(&self, monitor: &Arc<RecomputeMonitor>, f: impl FnOnce() -> R) -> R {
        self.mock.interruptible(monitor, f)
    }

    fn boolean(
        &self,
        op: BooleanOp,
        targets: &[&MockShape],
        tool: &MockShape,
    ) -> Result<BooleanOutput<MockShape>, KernelError> {
        self.mock.boolean(op, targets, tool)
    }

    fn fillet(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        sets: &[FilletSet<'_>],
        rolling: bool,
    ) -> Result<MockShape, KernelError> {
        let mut shape = self.mock.fillet(feature, body, sets, rolling)?;
        if self.skew_fillets {
            shape.history.push('~');
        }
        if let Some(takes) = self.fillet_takes {
            // (A body that deviates already: its `∆` counts once, below.)
            let body_print = self.fingerprint(body);
            let off = if body_print.contains('∆') {
                self.fillet_off
            } else {
                0.0
            };
            let base = self.volume_of(&body_print) - off;
            shape.history.push_str(&format!("[={}]", base - takes));
            if self.fillet_off != 0.0 && !shape.history.contains('∆') {
                shape.history.push('∆');
            }
        }
        if self.noisy_fillets {
            shape.history.push('≈');
        }
        Ok(shape)
    }

    fn chamfer(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        sets: &[Chamfer<'_>],
        corner: mitcad_model::features::ChamferCorner,
    ) -> Result<MockShape, KernelError> {
        self.mock.chamfer(feature, body, sets, corner)
    }

    fn count_edges(&self, shape: &MockShape, edge: &EdgeName) -> usize {
        self.mock.count_edges(shape, edge)
    }

    fn count_faces(
        &self,
        shape: &MockShape,
        face: &mitcad_model::FaceName,
    ) -> Result<usize, KernelError> {
        self.mock.count_faces(shape, face)
    }

    fn bounding_box(&self, shape: &MockShape) -> Result<Option<BoundingBox>, KernelError> {
        self.mock.bounding_box(shape)
    }

    fn face_plane(
        &self,
        shape: &MockShape,
        face: &mitcad_model::FaceName,
    ) -> Result<Plane, KernelError> {
        self.mock.face_plane(shape, face)
    }

    fn face_geometry(
        &self,
        shape: &MockShape,
        face: &mitcad_model::FaceName,
    ) -> Result<mitcad_model::datum::SurfaceGeometry, KernelError> {
        self.mock.face_geometry(shape, face)
    }

    fn import_brep(
        &self,
        feature: FeatureUid,
        data: &[u8],
        first: u32,
    ) -> Result<(MockShape, u32), KernelError> {
        self.mock.import_brep(feature, data, first)
    }

    fn brep_data(&self, shape: &MockShape) -> Result<Vec<u8>, KernelError> {
        self.written
            .borrow_mut()
            .insert(shape.history.clone(), self.fingerprint(shape));
        self.mock.brep_data(shape)
    }

    fn compound(&self, shapes: &[MockShape]) -> Result<MockShape, KernelError> {
        self.mock.compound(shapes)
    }

    fn transform_shape(
        &self,
        shape: &MockShape,
        transform: &mitcad_model::Transform,
        instance: Option<mitcad_model::Instance>,
    ) -> Result<MockShape, KernelError> {
        self.mock.transform_shape(shape, transform, instance)
    }

    fn unite(&self, shapes: &[&MockShape]) -> Result<MockShape, KernelError> {
        self.mock.unite(shapes)
    }

    fn body_kind(&self, shape: &MockShape) -> Result<BodyKind, KernelError> {
        self.mock.body_kind(shape)
    }

    fn mass_properties(&self, shape: &MockShape) -> Result<MassProperties, KernelError> {
        self.hook("mass_properties");
        let f = self.fingerprint(shape);
        if self.alike && !f.contains("moved(") {
            // (Less than a prism of either rectangle adds.)
            return Ok(MassProperties {
                volume: 1000.0,
                area: 1000.0,
                center: [0.0; 3],
            });
        }
        Ok(MassProperties {
            volume: self.volume_of(&f),
            area: hash(&f.replace(['~', '≈', '∆'], ""), 2),
            center: [0.0; 3],
        })
    }

    // The bodies' volumes, where `change_slack` is set.
    fn changed_volume(
        &self,
        before: &[&MockShape],
        after: &[&MockShape],
    ) -> Result<(f64, f64), KernelError> {
        let slack = self
            .change_slack
            .ok_or(KernelError::Unsupported("changed volume"))?;
        let total = |shapes: &[&MockShape]| -> f64 {
            shapes
                .iter()
                .map(|s| self.volume_of(&self.fingerprint(s)))
                .sum()
        };
        Ok((total(after) - total(before), slack))
    }

    // Replace face (P5): the faces go, a replace(<first>) face comes.
    fn replace_faces(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        faces: &[mitcad_model::FaceName],
        _target: &mitcad_model::ToolInput<'_, MockShape>,
        tangent_chain: bool,
    ) -> Result<MockShape, KernelError> {
        let names: Vec<String> = faces.iter().map(|f| without_uids(&f.to_string())).collect();
        let mut shape = body.clone();
        shape.history = format!(
            "replace({},{}{})",
            body.history,
            names.join(";"),
            if tangent_chain { ",chain" } else { "" }
        );
        shape.faces.retain(|f| !faces.contains(f));
        shape
            .edges
            .retain(|(a, b)| !faces.contains(a) && !faces.contains(b));
        shape
            .faces
            .push(format!("{feature}:replace({})", faces[0]).parse().unwrap());
        Ok(shape)
    }

    fn shell(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        spec: &mitcad_model::ShellSpec<'_>,
    ) -> Result<MockShape, KernelError> {
        self.mock.shell(feature, body, spec)
    }

    fn loft(&self, spec: &mitcad_model::LoftSpec<'_, MockShape>) -> Result<MockShape, KernelError> {
        self.mock.loft(spec)
    }

    // Sweeps, plain and along guide surfaces (mitcad#157).
    fn sweep(&self, spec: &mitcad_model::SweepSpec<'_>) -> Result<MockShape, KernelError> {
        self.mock.sweep(spec)
    }

    // Primitives and coils (mitcad#25).
    fn primitive(&self, spec: &mitcad_model::PrimitiveSpec) -> Result<MockShape, KernelError> {
        self.mock.primitive(spec)
    }

    fn coil(&self, spec: &mitcad_model::CoilSpec) -> Result<MockShape, KernelError> {
        self.mock.coil(spec)
    }

    fn surface_sweep(
        &self,
        spec: &mitcad_model::SweepSpec<'_>,
        surfaces: &[mitcad_model::GuideSurface<'_, MockShape>],
    ) -> Result<MockShape, KernelError> {
        self.mock.surface_sweep(spec, surfaces)
    }

    // With edge_points, a point per edge between its faces' points (see
    // edge_point), 1 mm long, so that fingerprints of edges resolve.
    fn edge_middles(
        &self,
        shape: &MockShape,
    ) -> Result<Vec<mitcad_model::EdgeMiddle>, KernelError> {
        if !self.edge_points {
            return Err(KernelError::Unsupported("edge middles"));
        }
        Ok(shape
            .edges
            .iter()
            .map(|(a, b)| mitcad_model::EdgeMiddle {
                name: EdgeName::new(a.clone(), b.clone()).to_string(),
                point: edge_point(a, b),
                tangent: [1.0, 0.0, 0.0],
                length: 1.0,
            })
            .collect())
    }

    // With face_listing, the faces (the mock's cylinders as such) and
    // each face's point (see face_point) as the nearest to any.
    fn faces(&self, shape: &MockShape) -> Result<Vec<mitcad_model::FaceInfo>, KernelError> {
        if !self.face_listing {
            return Err(KernelError::Unsupported("face listing"));
        }
        Ok(shape
            .faces
            .iter()
            .map(|f| mitcad_model::FaceInfo {
                names: vec![f.to_string()],
                surface: if self.mock.face_cylinder(shape, f).is_ok() {
                    "cylinder"
                } else {
                    "plane"
                }
                .to_owned(),
                area: 1.0,
            })
            .collect())
    }

    fn hole_tool(
        &self,
        spec: &mitcad_model::HoleSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.mock.hole_tool(spec)
    }

    fn modeled_thread(
        &self,
        body: &MockShape,
        spec: &mitcad_model::ThreadSpec<'_>,
    ) -> Result<MockShape, KernelError> {
        self.mock.modeled_thread(body, spec)
    }

    fn offset_faces(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        faces: &[mitcad_model::FaceName],
        distance: f64,
    ) -> Result<MockShape, KernelError> {
        self.hook("offset_faces");
        if !self.face_listing {
            return Err(KernelError::Unsupported("offset faces"));
        }
        self.mock.offset_faces(feature, body, faces, distance)
    }

    fn face_cylinder(
        &self,
        shape: &MockShape,
        face: &mitcad_model::FaceName,
    ) -> Result<mitcad_model::Cylinder, KernelError> {
        if !self.face_listing {
            return Err(KernelError::Unsupported("face cylinders"));
        }
        self.mock.face_cylinder(shape, face)
    }

    fn face_point_normal(
        &self,
        shape: &MockShape,
        face: &mitcad_model::FaceName,
        _near: [f64; 3],
    ) -> Result<mitcad_model::datum::SurfacePoint, KernelError> {
        if !self.face_listing || !shape.faces.contains(face) {
            return Err(KernelError::Unsupported("face normals"));
        }
        Ok(mitcad_model::datum::SurfacePoint {
            point: face_point(face),
            normal: [1.0, 0.0, 0.0],
        })
    }

    // A point per face from its name, on the faces of the shapes that have it.
    fn face_points(&self, shape: &MockShape) -> Result<Vec<mitcad_model::FacePoints>, KernelError> {
        Ok(shape
            .faces
            .iter()
            .map(|f| mitcad_model::FacePoints {
                name: f.to_string(),
                points: vec![face_point(f)],
            })
            .collect())
    }

    fn boundary_distances(
        &self,
        shape: &MockShape,
        points: &[[f64; 3]],
    ) -> Result<Vec<f64>, KernelError> {
        Ok(points
            .iter()
            .map(|p| {
                if shape.faces.iter().any(|f| face_point(f) == *p) {
                    0.0
                } else {
                    1.0
                }
            })
            .collect())
    }

    fn new_faces(
        &self,
        before: &[&MockShape],
        after: &MockShape,
        _count: usize,
    ) -> Result<Vec<mitcad_model::NewFace>, KernelError> {
        if !self.lists_new_faces {
            return Err(KernelError::Unsupported("new faces"));
        }
        let had: BTreeSet<String> = before
            .iter()
            .flat_map(|s| s.faces.iter().map(|f| without_uids(&f.to_string())))
            .collect();
        Ok(after
            .faces
            .iter()
            .filter(|f| !had.contains(&without_uids(&f.to_string())))
            .map(|f| mitcad_model::NewFace {
                area: 1.0,
                points: vec![face_point(f)],
            })
            .collect())
    }

    // Definitions evaluated ahead (mitcad#95): the same settings; not with
    // the hooks of a test that counts or blocks calls.
    fn fork(&self) -> Option<Self> {
        if self.on_call.borrow().is_some() || self.out_of_memory.get() > 0 {
            return None;
        }
        Some(TestKernel {
            mock: self.mock.fork()?,
            written: self.written.clone(),
            edge_points: self.edge_points,
            face_listing: self.face_listing,
            skew: self.skew,
            alike: self.alike,
            lists_new_faces: self.lists_new_faces,
            skew_fillets: self.skew_fillets,
            noisy_fillets: self.noisy_fillets,
            fillet_takes: self.fillet_takes,
            fillet_noise: self.fillet_noise,
            fillet_off: self.fillet_off,
            change_slack: self.change_slack,
            on_call: Default::default(),
            out_of_memory: Default::default(),
            slow: self.slow,
            slow_calls: Default::default(),
            directed: self.directed,
            invalid: self.invalid,
            began: Arc::clone(&self.began),
            stopped: Arc::clone(&self.stopped),
            probes: Arc::clone(&self.probes),
        })
    }

    fn points_inside(
        &self,
        _shape: &MockShape,
        _points: &[[f64; 3]],
    ) -> Result<Vec<bool>, KernelError> {
        self.probes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(KernelError::Unsupported("point classification"))
    }
}

/// The test kernel's point inside a face (the same in every document).
fn face_point(face: &mitcad_model::FaceName) -> [f64; 3] {
    let name = without_uids(&face.to_string());
    [hash(&name, 3), hash(&name, 4), hash(&name, 5)]
}

/// The TestKernel's middle of the edge between two faces (mm).
fn edge_point(a: &mitcad_model::FaceName, b: &mitcad_model::FaceName) -> [f64; 3] {
    let (p, q) = (face_point(a), face_point(b));
    [
        0.5 * (p[0] + q[0]),
        0.5 * (p[1] + q[1]),
        0.5 * (p[2] + q[2]),
    ]
}

/// History states and stored bodies given by the test.
#[derive(Default)]
struct TestGeometry {
    states: Vec<Vec<MockShape>>,
    /// The state each item made, by item index.
    items: HashMap<i64, usize>,
    /// States that cannot be built.
    broken: Vec<usize>,
    /// The component (object id) of every body and of the items' states.
    component: Option<u64>,
    /// Bodies have ids: the same shape in different states the same.
    ids: bool,
    /// Items whose state the history does not hold (suppressed).
    without_result: Vec<i64>,
    /// Items whose operation changed no body (failed in the file).
    unchanged: Vec<i64>,
    /// The state before the operations no item names right before an
    /// item's own, by item index.
    starts: HashMap<i64, usize>,
    /// Per state, the lineage (`StoredBody::lineage`) of each body.
    lineages: Vec<Vec<u64>>,
}

impl StoredGeometry<MockShape> for TestGeometry {
    fn state_count(&mut self) -> usize {
        self.states.len()
    }

    fn item_state(&mut self, index: i64) -> Option<usize> {
        self.items.get(&index).copied()
    }

    fn item_without_result(&mut self, index: i64) -> bool {
        self.without_result.contains(&index)
    }

    fn item_unchanged(&mut self, index: i64) -> bool {
        self.unchanged.contains(&index)
    }

    fn item_start(&mut self, index: i64) -> Option<usize> {
        self.starts.get(&index).copied()
    }

    fn state(&mut self, index: usize) -> Result<Vec<StoredBody<MockShape>>, String> {
        if self.broken.contains(&index) {
            return Err(format!("state {index} cannot be built"));
        }
        Ok(self.states[index]
            .iter()
            .enumerate()
            .map(|(i, shape)| StoredBody {
                shape: shape.clone(),
                name: None,
                source: format!("state {index} body {i}"),
                component: self.component,
                id: self
                    .ids
                    .then(|| self.states.iter().flatten().position(|s| s == shape))
                    .flatten()
                    .map(|p| p as u64),
                lineage: self.lineages.get(index).and_then(|l| l.get(i)).copied(),
            })
            .collect())
    }

    fn final_bodies(&mut self) -> Result<Vec<StoredBody<MockShape>>, String> {
        let last = self.states.len() - 1;
        self.state(last)
    }

    fn item_components(&mut self, index: i64) -> Option<Vec<u64>> {
        self.items
            .contains_key(&index)
            .then(|| self.component.into_iter().collect())
    }
}

fn shapes(doc: &Document<TestKernel>) -> Vec<MockShape> {
    doc.bodies().iter().map(|b| b.shape.clone()).collect()
}

/// Two rectangles side by side: 40 x 20 and 10 x 20 mm.
fn two_rectangles(index: i64) -> Value {
    let p = |id: &str, x: f64, y: f64| json!({"id": id, "xyz": [x, y, 0.0]});
    let line = |id: &str, a: &str, b: &str| json!({"id": id, "type": "SketchLine", "startSketchPoint": a, "endSketchPoint": b});
    json!({
        "index": index, "name": format!("Sketch{}", index + 1), "objectType": "Sketch",
        "detail": {
            "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
            "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
            "points": [p("p0", 0.0, 0.0), p("p1", 4.0, 0.0), p("p2", 4.0, 2.0), p("p3", 0.0, 2.0),
                       p("p4", 5.0, 0.0), p("p5", 6.0, 0.0), p("p6", 6.0, 2.0), p("p7", 5.0, 2.0)],
            "curves": [line("c0", "p0", "p1"), line("c1", "p1", "p2"),
                       line("c2", "p2", "p3"), line("c3", "p3", "p0"),
                       line("c4", "p4", "p5"), line("c5", "p5", "p6"),
                       line("c6", "p6", "p7"), line("c7", "p7", "p4")]
        }
    })
}

fn timeline_dump(items: Vec<Value>) -> Dump {
    dump(json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "test.f3d"},
        "parameters": {"model": [
            {"name": "d2", "expression": "10 mm", "value": 1.0, "unit": "mm"},
            {"name": "d3", "expression": "0.0 deg", "value": 0.0, "unit": "deg"},
            {"name": "d5", "expression": "2 mm", "value": 0.2, "unit": "mm"}
        ]},
        "timeline": {"items": items}
    }))
}

fn add(doc: &mut Document<TestKernel>, def: Value) {
    let def: FeatureDef<mitcad_model::ValueInput> = serde_json::from_value(def).unwrap();
    let added = doc.add_feature(&def, None).unwrap();
    assert_eq!(doc.status(added.uid).and_then(|s| s.error()), None);
}

#[test]
fn the_history_chooses_the_profile() {
    // The file's design extruded the small rectangle; the decoder only
    // says "one profile of Sketch1", and the larger one is the first guess.
    let items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let full = timeline_dump(items.clone());
    // What the file has: the same sketch with the small rectangle extruded.
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items[..1].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": small}],
                               "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}),
    );
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), shapes(&reference)],
        ..TestGeometry::default()
    };

    // Without the history the first guess is used, the larger rectangle.
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(&mut doc, &full, &mut NoGeometry, &Options::default());
    assert_eq!(report.items[1].outcome, Outcome::Parametric);
    assert_eq!(report.items[1].verified, None);
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    assert_ne!(def["def"]["profiles"][0]["region"], small);

    let mut doc = Document::new(TestKernel::default());
    let report = import_design(&mut doc, &full, &mut geometry, &Options::default());
    assert_eq!(
        report.items[1].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert_eq!(report.items[1].verified, Some(true));
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    assert_eq!(def["def"]["profiles"][0]["region"], small);
    assert_eq!(def["def"]["extent"]["distance"], "d2");
    assert_eq!(report.history.matched, 1);
    assert!(report.history.reached_end);
    assert_eq!(report.bodies.len(), 1);
    assert_eq!(report.bodies[0].volume_difference, Some(0.0));
}

/// The two rectangles with their curves' geometry and an extrusion whose
/// decoded profile loops select the small one (mitcad#96).
fn decoded_small_rectangle() -> Vec<Value> {
    let mut sketch = two_rectangles(0);
    let corners = [
        ([0.0, 0.0], [4.0, 0.0]),
        ([4.0, 0.0], [4.0, 2.0]),
        ([4.0, 2.0], [0.0, 2.0]),
        ([0.0, 2.0], [0.0, 0.0]),
        ([5.0, 0.0], [6.0, 0.0]),
        ([6.0, 0.0], [6.0, 2.0]),
        ([6.0, 2.0], [5.0, 2.0]),
        ([5.0, 2.0], [5.0, 0.0]),
    ];
    for (c, (a, b)) in sketch["detail"]["curves"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(corners)
    {
        c["geometry"] = json!({"type": "Line3D", "startPoint": [a[0], a[1], 0.0],
                               "endPoint": [b[0], b[1], 0.0]});
    }
    let record = |id: &str, k: u64| {
        json!({"id": id, "primary": k, "secondary": 0, "tag": 3, "reversed": false,
               "piece": 1, "pieces": 1})
    };
    let mut item = extrude(1, 0, "NewBodyFeatureOperation", "d2");
    item["detail"]["_f3d_profile_loops"] = json!([{"loops": [{"outer": true, "curves": [
        record("c4", 4), record("c5", 5), record("c6", 6), record("c7", 7)]}], "children": []}]);
    vec![sketch, item]
}

#[test]
fn the_decoded_profile_loops_choose_the_profile() {
    // mitcad#96: the decoder lists the small rectangle's loop (the file's
    // curve ids with their geometry), so that even without a history the
    // import extrudes it, not the first guess (the larger one).
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(decoded_small_rectangle()),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(report.items[1].outcome, Outcome::Parametric);
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    assert_eq!(def["def"]["profiles"][0]["region"], small);
    assert_eq!(def["def"]["profiles"].as_array().unwrap().len(), 1);
}

/// Imports the decoded small rectangle's extrusion with the given stream
/// fields (and thin wall `(location, thickness)` of the detail): the
/// extrusion's definition and the provenance of its inputs (mitcad#186).
fn decoded_extrusion(raw: Value, thin: Option<(&str, &str)>) -> (Value, Option<InputProvenance>) {
    let mut items = decoded_small_rectangle();
    items[1]["_f3d"]["extrude"] = raw;
    if let Some((location, thickness)) = thin {
        let detail = &mut items[1]["detail"];
        detail["isThinExtrude"] = json!(true);
        detail["thinExtrudeWallLocationOne"] = json!(location);
        detail["thinExtrudeWallThicknessOne"] = param(thickness, "2 mm", 0.2, "mm");
    }
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[1].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    (def["def"].clone(), report.items[1].input_provenance)
}

#[test]
fn extrusions_with_every_input_in_the_stream_are_read() {
    // A one-sided distance whose stream carries every field: operation,
    // codes, start, a solid's thin-wall block and an exact profile
    // selection. The stored vector against the sketch's normal flips
    // nothing: the distance's sign gives the direction.
    let raw = |extra: Value| {
        let mut raw = json!({"operation_code": 4, "extent_a": 1, "extent_b": 2,
                             "direction": 1.0, "direction_vector": [0.0, 0.0, -1.0],
                             "flag": 0, "solid": true, "start": 0, "sides": [1, 0],
                             "thin": false, "thin_location": 0});
        for (key, value) in extra.as_object().unwrap() {
            raw[key] = value.clone();
        }
        raw
    };
    let (def, provenance) = decoded_extrusion(raw(json!({})), None);
    assert_ne!(def["flip"], true);
    assert_eq!(def["extent"]["type"], "distance");
    assert!(def.get("thin").is_none());
    assert_eq!(provenance, Some(InputProvenance::Read));
    // The same stream without the side kinds, the start or the thin-wall
    // block leaves an input open: not read.
    for missing in ["sides", "start", "thin"] {
        let mut value = raw(json!({}));
        value.as_object_mut().unwrap().remove(missing);
        let (_, provenance) = decoded_extrusion(value, None);
        assert_ne!(provenance, Some(InputProvenance::Read), "{missing}");
    }
    // A thin wall: the file's side one lies inside the profile, the
    // model's side one outside, so the sides swap; the centre stays.
    for (file, model) in [("Side1", "side2"), ("Center", "center"), ("Side2", "side1")] {
        let location = match file {
            "Side1" => 0,
            "Center" => 1,
            _ => 2,
        };
        let (def, provenance) = decoded_extrusion(
            raw(json!({"thin": true, "thin_location": location})),
            Some((file, "d4")),
        );
        assert_eq!(def["thin"]["location"], model, "{file}");
        assert_eq!(provenance, Some(InputProvenance::Read), "{file}");
    }
    // A thin extrusion the stream does not mark as one is no read input.
    let (_, provenance) = decoded_extrusion(raw(json!({"thin": null})), Some(("Side1", "d4")));
    assert_ne!(provenance, Some(InputProvenance::Read));
}

/// A one-sided extrusion through all of the decoded small rectangle, after a
/// first one that makes a body: its direction follows the flag byte of the
/// stream (the API reference dumps' `isPositiveDirection`: 0 along the
/// sketch's normal, 1 against it), also where the stored vector says the
/// other way, and the extent is read.
#[test]
fn through_all_direction_follows_the_flag_byte() {
    let import_through_all = |flag: Option<u32>, positive: Option<bool>, two_sided: bool| {
        let mut items = decoded_small_rectangle();
        // The body lies on the side the through-all extrusion goes to.
        let down = if flag == Some(1) { -1.0 } else { 1.0 };
        items[1]["_f3d"]["extrude"] = json!({"operation_code": 4, "direction": down,
                                             "direction_vector": [0.0, 0.0, down]});
        let mut second = items[1].clone();
        second["index"] = json!(2);
        second["name"] = json!("Extrude2");
        // A cut of the first body (the stored vector says the other way).
        let mut raw = json!({"operation_code": 2, "extent_a": 1, "extent_b": 0,
                             "direction": 1.0, "direction_vector": [0.0, 0.0, -down],
                             "solid": true, "start": 0, "sides": [4, 0],
                             "thin": false, "thin_location": 0});
        if let Some(flag) = flag {
            raw["flag"] = json!(flag);
        }
        second["_f3d"]["extrude"] = raw;
        second["detail"]["operation"] = json!("CutFeatureOperation");
        second["detail"]["participantBodies"] = json!([{"kind": "body", "objectType": "BRepBody",
                    "_f3d": {"producer": 1, "body_index": 0}}]);
        let mut extent = json!({"_type": "ThroughAllExtentDefinition"});
        if let Some(positive) = positive {
            extent["isPositiveDirection"] = json!(positive);
        }
        second["detail"]["extentOne"] = extent;
        if two_sided {
            // Side one through all, side two a distance: the codes 4 and 1.
            second["_f3d"]["extrude"]["sides"] = json!([4, 1]);
            second["detail"]["extentType"] = json!("TwoSidesFeatureExtentType");
            second["detail"]["hasTwoExtents"] = json!(true);
            second["detail"]["extentTwo"] = json!({"_type": "DistanceExtentDefinition",
                "distance": param("d9", "1 mm", 0.1, "mm"), "isSymmetric": false});
        }
        items.push(second);
        let mut doc = Document::new(MockKernel::default());
        let report = import(&mut doc, &timeline_dump(items));
        (
            report.items[2].outcome,
            doc,
            report.items[2].input_provenance,
        )
    };
    let def_of = |doc: &Document<MockKernel>| -> Value {
        let def: Value =
            serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F3"}"#).unwrap())
                .unwrap();
        def["def"].clone()
    };
    // Flag 0: along the normal; flag 1: against it.
    let (outcome, doc, provenance) = import_through_all(Some(0), Some(true), false);
    assert_eq!(outcome, Outcome::Parametric);
    let def = def_of(&doc);
    assert_eq!(def["extent"]["type"], "through_all");
    assert_ne!(def["extent"]["both_sides"], true);
    assert_ne!(def["flip"], true);
    assert_eq!(provenance, Some(InputProvenance::Read));
    let (outcome, doc, provenance) = import_through_all(Some(1), Some(false), false);
    assert_eq!(outcome, Outcome::Parametric);
    assert_eq!(def_of(&doc)["flip"], true);
    assert_eq!(provenance, Some(InputProvenance::Read));
    // Without the flag byte the side is a guess: with no history to check
    // it the item is left out.
    let (outcome, _, _) = import_through_all(None, None, false);
    assert_eq!(outcome, Outcome::Skipped);
    // Side one through all and side two a distance: side one goes the
    // way the flag says, and the extent is read.
    for (flag, positive, against) in [(0, true, false), (1, false, true)] {
        let (outcome, doc, provenance) = import_through_all(Some(flag), Some(positive), true);
        assert_eq!(outcome, Outcome::Parametric, "{flag}");
        let def = def_of(&doc);
        assert_eq!(def["extent"]["type"], "two_sides", "{def}");
        assert_eq!(def["flip"] == json!(true), against, "{def}");
        assert_eq!(provenance, Some(InputProvenance::Read), "{flag}");
    }
}
/// Imports the extrusion of the decoded small rectangle against a history
/// whose state is the extrusion of `region` (mitcad#181): its outcome, the
/// region it took and how many times regions were probed.
fn decoded_against_history(region: &str) -> (Outcome, Value, usize) {
    let items = decoded_small_rectangle();
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items[..1].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": region}],
               "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}),
    );
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), shapes(&reference)],
        items: HashMap::from([(1, 1)]),
        ..TestGeometry::default()
    };
    let kernel = TestKernel::default();
    let probes = Arc::clone(&kernel.probes);
    let mut doc = Document::new(kernel);
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    (
        report.items[1].outcome,
        def["def"]["profiles"].clone(),
        probes.load(std::sync::atomic::Ordering::SeqCst),
    )
}

#[test]
fn decoded_profiles_that_give_the_state_are_taken_without_probing_the_others() {
    // mitcad#181: the decoded profiles' definitions are tried on their own
    // first; the regions the history suggests (found by probing the next
    // states, seconds on large bodies) only when those give no state.
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    let (outcome, profiles, probes) = decoded_against_history(small);
    assert_eq!(outcome, Outcome::Parametric);
    assert_eq!(profiles, json!([{"sketch": "F1", "region": small}]));
    assert_eq!(probes, 0);
}

#[test]
fn decoded_profiles_that_give_no_state_bring_in_the_other_regions() {
    // The file extruded the large rectangle where the decoder read the
    // small one: the other sets of regions come in and give the state.
    let large = "r{c9[c12,c10],c10[c9,c11],c11[c10,c12],c12[c11,c9]}";
    let (outcome, profiles, probes) = decoded_against_history(large);
    assert_eq!(outcome, Outcome::Parametric);
    assert_eq!(profiles, json!([{"sketch": "F1", "region": large}]));
    assert!(probes > 0);
}

/// A fillet whose edges the decoder does not give, and an extrusion after
/// it.
fn fillet_item(index: i64) -> Value {
    json!({"index": index, "name": "Fillet1", "objectType": "FilletFeature",
           "detail": {"edgeSets": [{"radius": param("d5", "2 mm", 0.2, "mm")}]}})
}

#[test]
fn a_feature_that_cannot_be_replayed_takes_the_files_bodies_and_the_timeline_goes_on() {
    fallback_and_continue(false, false);
}

#[test]
fn with_the_items_states_known_a_failed_item_takes_its_own_state() {
    fallback_and_continue(true, false);
}

#[test]
fn in_a_component_the_fallback_goes_into_the_component() {
    fallback_and_continue(true, true);
}

/// A fillet the decoder gives no edges of between an extrusion and a join;
/// `known`: the history names each item's state (`result_no`);
/// `component`: the bodies are those of component Plate (object 10), placed
/// 5 cm along X.
fn fallback_and_continue(known: bool, component: bool) {
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        fillet_item(2),
        extrude(3, 0, "JoinFeatureOperation", "d2"),
    ];
    // The file's states: the block, the block rounded (some fillet), then the
    // second extrusion joined to it.
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items[..2].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let block = shapes(&reference);
    let edge = "E{F2:side(c5[c8,c6])|F2:side(c6[c5,c7])}";
    add(
        &mut reference,
        json!({"type": "fillet", "body": "F2.b0", "edges": [edge], "radius": 2}),
    );
    let rounded = shapes(&reference);
    // The join as Mitcad makes it on the stored rounded body.
    reference.undo();
    reference
        .add_base_feature(BaseInput {
            replaces: vec![mitcad_model::BodyUid::new(FeatureUid(2), 0)],
            ..BaseInput::new(vec![ImportBody {
                name: None,
                color: None,
                shape: rounded[0].clone(),
            }])
        })
        .unwrap();
    let region = "r{c5[c8,c6],c6[c5,c7],c7[c6,c8],c8[c7,c5]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": region}],
                               "extent": {"type": "distance", "distance": "d2"}, "flip": false,
                               "operation": "join"}),
    );
    let joined = shapes(&reference);
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), block, rounded, joined],
        items: if known {
            HashMap::from([(1, 1), (2, 2), (3, 3)])
        } else {
            HashMap::new()
        },
        component: component.then_some(10),
        ..TestGeometry::default()
    };
    let mut dump = timeline_dump(items);
    if component {
        let mut value = serde_json::to_value(&dump).unwrap();
        value["components"] = json!([{"name": "Plate", "_f3d": {"object_id": 10}}]);
        value["occurrences"] = json!([{"component": "Plate", "_f3d": {"component_object": 10},
            "transform": [[1.0, 0.0, 0.0, 5.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0],
                          [0.0, 0.0, 0.0, 1.0]]}]);
        dump = Dump::from_json(&value.to_string()).unwrap();
    }

    let mut doc = Document::new(TestKernel::default());
    let report = import_design(&mut doc, &dump, &mut geometry, &Options::default());
    if component {
        // Every item and the fallback in Plate; its body placed with it.
        let plate = doc.assembly().components[0].uid;
        assert!(
            doc.features().all(|f| f.component == plate),
            "{}",
            report.text()
        );
        assert_eq!(doc.body_component(doc.bodies()[0].uid), Some(plate));
        let instances = doc.instances();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].transform.translation, [50.0, 0.0, 0.0]);
    }
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [
            Outcome::Partial,
            Outcome::Parametric,
            Outcome::Fallback,
            Outcome::Parametric
        ],
        "{}",
        report.text()
    );
    // The fallback replaced the block, keeping its body id.
    let fallback = doc.feature(FeatureUid(3)).unwrap();
    assert_eq!(fallback.name, "Fillet1");
    let FeatureDef::Base(base) = &fallback.def else {
        panic!("a base feature");
    };
    assert_eq!(
        base.replaces,
        [mitcad_model::BodyUid::new(FeatureUid(2), 0)]
    );
    assert_eq!(report.items[3].verified, Some(true));
    assert!(report.history.reached_end, "{}", report.text());
    assert_eq!(doc.bodies().len(), 1);
}

/// The block's history: nothing, the block, the block joined again.
fn block_history(broken: Vec<usize>) -> (Vec<Value>, TestGeometry) {
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        extrude(2, 0, "JoinFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items[..2].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let block = shapes(&reference);
    let region = "r{c5[c8,c6],c6[c5,c7],c7[c6,c8],c8[c7,c5]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": region}],
               "extent": {"type": "distance", "distance": "d2"}, "flip": false,
               "operation": "join"}),
    );
    let geometry = TestGeometry {
        states: vec![Vec::new(), block, shapes(&reference)],
        items: HashMap::from([(1, 1), (2, 2)]),
        broken,
        component: None,
        ids: false,
        ..TestGeometry::default()
    };
    (items, geometry)
}

/// A circle of 10 mm diameter on XY (cm in the IR).
fn circle_sketch(index: i64) -> Value {
    json!({
        "index": index, "name": format!("Sketch{}", index + 1), "objectType": "Sketch",
        "detail": {
            "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
            "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
            "points": [{"id": "p0", "xyz": [2.0, 1.0, 0.0]}],
            "curves": [{"id": "c0", "type": "SketchCircle", "centerSketchPoint": "p0", "radius": 0.5}]
        }
    })
}

/// A thread item as the decoder gives it: its size, flags and the face
/// found in the history (its cylinder's axis as the file has it, `axis_z`
/// along z; a point on it, cm).
fn thread_item(index: i64, info: Value, full: bool, axis_z: f64, point: [f64; 3]) -> Value {
    json!({
        "index": index, "name": format!("Thread{index}"), "objectType": "ThreadFeature",
        "detail": {
            "threadInfo": info, "isModeled": false, "isFullLength": full,
            "threadLength": param("d4", "12 mm", 1.2, "mm"),
            "threadOffset": param("d5", "1 mm", 0.1, "mm"),
            "threadLocation": "HighEndThreadLocation",
            "inputCylindricalFaces": [{
                "kind": "face", "objectType": "BRepFace",
                "geometry": {"type": "Cylinder", "origin": [2.0, 1.0, 0.0],
                             "axis": [0.0, 0.0, axis_z], "radius": 0.5},
                "point_on_face": point,
                "_f3d": {"recipe": "bounded_face", "found": "before state 2"}
            }]
        }
    })
}

/// Threads (mitcad#35) on the faces the decoder found in the history: the
/// size as Mitcad lists it, a partial thread from the end the file's axis
/// points to (here against the mock cylinder's), a class the face's side
/// does not take left out, pipe threads by Mitcad's names.
#[test]
fn threads_come_in_on_the_faces_the_decoder_found() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut base = vec![
        circle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(base.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let side = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "side")
        .cloned()
        .expect("a side face");
    let point = face_point(&side).map(|x| x / 10.0);
    let iso = |class: &str| {
        json!({"_type": "ThreadInfo", "threadType": "ISO Metric profile",
               "threadDesignation": "M10x1.5", "threadClass": class, "isInternal": false})
    };
    let pipe = json!({"_type": "ThreadInfo", "threadType": "BSP Pipe Threads",
                      "threadDesignation": "G 1/4-19", "threadClass": "A"});
    base.push(thread_item(2, iso("6g"), false, -1.0, point));
    base.push(thread_item(3, iso("6H"), true, 1.0, point));
    base.push(thread_item(4, pipe, true, 1.0, point));
    // The forming screw threads, with and without the `ST` of the size.
    let forming = |designation: &str| {
        json!({"_type": "ThreadInfo", "threadType": "Metric Forming Screw Threads",
               "threadDesignation": designation, "threadClass": "6g", "isInternal": false})
    };
    base.push(thread_item(5, forming("ST3.5x1.3"), true, 1.0, point));
    base.push(thread_item(6, forming("2.9x1.4"), true, 1.0, point));
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(base),
        &mut NoGeometry,
        &Options::default(),
    );
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(outcomes[2..], [Outcome::Parametric; 5], "{}", report.text());
    let def = |uid: u64| -> Value {
        serde_json::from_str::<Value>(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "F{uid}"}}"#))
                .unwrap(),
        )
        .unwrap()["def"]
            .clone()
    };
    let partial = def(3);
    assert_eq!(partial["faces"][0]["face"], side.to_string());
    assert_eq!(
        partial["thread"],
        json!({"designation": "M10x1.5", "class": "6g"})
    );
    // The length a parameter of the thread's own, the offset the file's d5.
    assert!(partial["length"].is_string(), "{partial}");
    assert_eq!(partial["offset"], "d5");
    // The file's axis runs down, the mock cylinder's up: the high end in
    // the file is Mitcad's low end.
    assert_eq!(partial["location"], "low_end");
    let full = def(4);
    assert_eq!(full["thread"], json!({"designation": "M10x1.5"}));
    assert!(full.get("length").is_none());
    assert!(
        report.items[3]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("class")),
        "{}",
        report.text()
    );
    assert_eq!(
        def(5)["thread"],
        json!({"standard": "whitworth", "designation": "G 1/4", "class": "A"})
    );
    assert_eq!(
        def(6)["thread"],
        json!({"standard": "forming_screw", "designation": "ST3.5x1.3", "class": "6g"})
    );
    assert_eq!(
        def(7)["thread"],
        json!({"standard": "forming_screw", "designation": "2.9x1.4", "class": "6g"})
    );
}

/// A thread's handedness read from the file comes in as the
/// thread's, and a thread of which every input is read says so: here a
/// left-handed one; one whose handedness the file does not give, or whose
/// class Mitcad does not list for the face's side, does not.
#[test]
fn a_threads_handedness_comes_in_and_its_inputs_are_read() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut base = vec![
        circle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(base.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let side = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "side")
        .cloned()
        .expect("a side face");
    let point = face_point(&side).map(|x| x / 10.0);
    let iso = |class: &str, right: Option<bool>| {
        let mut info = json!({"_type": "ThreadInfo", "threadType": "ISO Metric profile",
                              "threadDesignation": "M10x1.5", "threadClass": class,
                              "isInternal": false});
        if let Some(right) = right {
            info["isRightHanded"] = json!(right);
        }
        info
    };
    base.push(thread_item(2, iso("6g", Some(false)), true, 1.0, point));
    base.push(thread_item(3, iso("6g", None), true, 1.0, point));
    base.push(thread_item(4, iso("6H", Some(true)), true, 1.0, point));
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(base),
        &mut NoGeometry,
        &Options::default(),
    );
    let text = report.text();
    assert_eq!(
        report.items[2..]
            .iter()
            .map(|i| i.outcome)
            .collect::<Vec<_>>(),
        [Outcome::Parametric; 3],
        "{text}"
    );
    let def = |uid: u64| -> Value {
        serde_json::from_str::<Value>(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "F{uid}"}}"#))
                .unwrap(),
        )
        .unwrap()["def"]
            .clone()
    };
    assert_eq!(def(3)["thread"]["right_handed"], json!(false), "{}", def(3));
    assert_eq!(def(4)["thread"].get("right_handed"), None, "{}", def(4));
    let provenance: Vec<_> = report.items[2..]
        .iter()
        .map(|i| i.input_provenance)
        .collect();
    assert_eq!(
        provenance,
        [
            Some(InputProvenance::Read),
            Some(InputProvenance::Unknown),
            // (6H is no class of an external thread: left out.)
            Some(InputProvenance::Unknown)
        ],
        "{text}"
    );
}

/// A modelled thread (mitcad#57) comes in with the file's diameters: the
/// thread alone first (it sizes its part of the face itself). Mitcad
/// models only the 60-degree threads: a modelled pipe thread falls back.
#[test]
fn modelled_threads_take_the_files_diameters() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut items = vec![
        circle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let side = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "side")
        .cloned()
        .expect("a side face");
    let point = face_point(&side).map(|x| x / 10.0);
    let info = json!({"_type": "ThreadInfo", "threadType": "ISO Metric profile",
        "threadDesignation": "M10x1.5", "threadClass": "6g", "isInternal": false,
        "majorDiameter": 0.985, "minorDiameter": 0.8141, "pitchDiameter": 0.8928});
    let mut modelled = thread_item(2, info, true, 1.0, point);
    modelled["detail"]["isModeled"] = json!(true);
    items.push(modelled);
    let pipe = json!({"_type": "ThreadInfo", "threadType": "BSP Pipe Threads",
                      "threadDesignation": "G 1/4-19", "threadClass": "A"});
    let mut modelled_pipe = thread_item(3, pipe, true, 1.0, point);
    modelled_pipe["detail"]["isModeled"] = json!(true);
    items.push(modelled_pipe);
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def: Value =
        serde_json::from_str::<Value>(&doc.query(r#"{"query": "feature", "uid": "F3"}"#).unwrap())
            .unwrap()["def"]
            .clone();
    assert_eq!(def["type"], "thread", "{def}");
    assert_eq!(def["modeled"], true, "{def}");
    let spec = doc.kernel().mock.specs.borrow().join("\n");
    assert!(
        spec.contains(&format!(
            "thread {side} pitch 1.5 diameters 9.8500/8.1410/8.9280"
        )),
        "{spec}"
    );
    // Without a history the angle is not measured.
    assert!(def.get("angle").is_none(), "{def}");
    assert_ne!(report.items[3].outcome, Outcome::Parametric);
    assert!(
        report.items[3]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("Whitworth")),
        "{}",
        report.text()
    );
}

/// A thread sizes its face to the thread's major diameter first, as the
/// file does (mitcad#35); the thread keeps the item's name.
#[test]
fn a_thread_sizes_its_face_first() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut items = vec![
        circle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let side = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "side")
        .cloned()
        .expect("a side face");
    let info = json!({"_type": "ThreadInfo", "threadType": "ISO Metric profile",
        "threadDesignation": "M10x1.5", "threadClass": "6g", "isInternal": false,
        "majorDiameter": 0.985, "minorDiameter": 0.8141});
    items.push(thread_item(
        2,
        info,
        true,
        1.0,
        face_point(&side).map(|x| x / 10.0),
    ));
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert!(
        report.items[2]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("sized")),
        "{}",
        report.text()
    );
    let offset = doc.feature(FeatureUid(3)).unwrap();
    assert!(
        matches!(offset.def, FeatureDef::OffsetFace(_)),
        "{offset:?}"
    );
    let thread = doc.feature(FeatureUid(4)).unwrap();
    assert!(matches!(thread.def, FeatureDef::Thread(_)));
    assert_eq!(thread.name, "Thread2");
}

/// A tapped hole (mitcad#35): the hole, and a thread on its wall from the
/// hole's start.
#[test]
fn a_tapped_hole_comes_in_with_its_thread_on_the_wall() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let top = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "end")
        .cloned()
        .expect("a top face");
    items.push(json!({
        "index": 2, "name": "Hole1", "objectType": "HoleFeature",
        "detail": {
            "holeType": "SimpleHoleType", "holeTapType": "TappedHoleTapType",
            "holeDiameter": param("d6", "5 mm", 0.5, "mm"),
            "tipAngle": param("d7", "118 deg", 2.0594885173533086, "deg"),
            "extentDefinition": {"_type": "DistanceExtentDefinition",
                                 "distance": param("d8", "15 mm", 1.5, "mm")},
            "position": [2.0, 1.0, 1.0],
            "holePositionDefinition": {"planarEntity": {
                "kind": "face", "geometry": {"type": "Plane"},
                "point_on_face": face_point(&top).map(|x| x / 10.0)}},
            "tappedHoleInfo": {"_type": "ThreadInfo", "threadType": "ISO Metric profile",
                               "threadDesignation": "M6x1", "threadClass": "6H",
                               "isInternal": true, "minorDiameter": 0.5035},
            "thread": {"_type": "ThreadFeature", "isModeled": false, "isFullLength": false,
                       "threadLength": param("d9", "10 mm", 1.0, "mm"),
                       "threadOffset": param("d10", "0 mm", 0.0, "mm")}
        }
    }));
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def = |uid: &str| -> Value {
        serde_json::from_str::<Value>(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                .unwrap(),
        )
        .unwrap()["def"]
            .clone()
    };
    let hole = def("F3");
    assert_eq!(hole["type"], "hole");
    assert!(hole.get("thread").is_none(), "{hole}");
    let thread = def("F4");
    assert_eq!(thread["faces"][0]["face"], "F3:hole0.wall", "{thread}");
    assert_eq!(
        thread["thread"],
        json!({"designation": "M6x1", "class": "6H"})
    );
    assert_eq!(thread["location"], "low_end");
    // The bore is the hole's diameter (its own state's); the minor
    // diameter is the next guess.
    let params = doc.parameters();
    let bore = hole["diameter"].as_str().and_then(|n| params.find(n));
    assert_eq!(
        bore.and_then(|id| params.get(id)).map(|p| p.value()),
        Some(5.0),
        "{hole}"
    );
}

/// Sheets (mitcad#35): an item the import does not translate made a surface
/// body; it comes in with the item's fallback, and a later join leaves it
/// alone (its participants are the solids).
#[test]
fn sheets_of_the_history_come_in_as_stored_bodies() {
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 2, "name": "Patch1", "objectType": "PatchFeature", "detail": {}}),
        extrude(3, 0, "JoinFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items[..2].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let block = shapes(&reference);
    let region = "r{c5[c8,c6],c6[c5,c7],c7[c6,c8],c8[c7,c5]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": region}],
               "extent": {"type": "distance", "distance": "d2"}, "flip": false,
               "operation": "join"}),
    );
    let joined = shapes(&reference);
    let sheet = MockShape {
        // The mock kernel's surface body.
        history: "patch,sheet".to_owned(),
        faces: block[0].faces[..1].to_vec(),
        edges: Vec::new(),
        bounds: None,
        parts: Vec::new(),
    };
    let mut geometry = TestGeometry {
        states: vec![
            Vec::new(),
            block.clone(),
            vec![block[0].clone(), sheet.clone()],
            vec![joined[0].clone(), sheet],
        ],
        items: HashMap::from([(1, 1), (2, 2), (3, 3)]),
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [
            Outcome::Partial,
            Outcome::Parametric,
            Outcome::Fallback,
            Outcome::Parametric
        ],
        "{}",
        report.text()
    );
    // The patch's fallback holds the sheet; the join works on the block.
    let FeatureDef::Base(base) = &doc.feature(FeatureUid(3)).unwrap().def else {
        panic!("a base feature");
    };
    assert_eq!(base.bodies.len(), 1);
    let FeatureDef::Extrude(join) = &doc.feature(FeatureUid(4)).unwrap().def else {
        panic!("an extrusion");
    };
    assert_eq!(
        join.participants,
        [mitcad_model::BodyUid::new(FeatureUid(2), 0)]
    );
    assert_eq!(report.items[3].verified, Some(true));
    let kinds: Vec<BodyKind> = doc
        .bodies()
        .iter()
        .map(|b| doc.kernel().body_kind(b.shape).unwrap())
        .collect();
    assert_eq!(kinds, [BodyKind::Solid, BodyKind::Sheet]);
    // The sheet is the file's: not an extra body, and no last fallback.
    assert!(report.extra_bodies.is_empty(), "{}", report.text());
    assert_eq!(doc.features().count(), 4, "{}", report.text());
}

#[test]
fn an_item_whose_state_cannot_be_rebuilt_is_taken_unchecked() {
    let (items, mut geometry) = block_history(vec![1]);
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Parametric, Outcome::Parametric],
        "{}",
        report.text()
    );
    assert_eq!(report.items[1].verified, None);
    assert!(
        report.items[1]
            .note
            .as_deref()
            .unwrap()
            .contains("not checked"),
        "{}",
        report.text()
    );
    assert_eq!(report.items[2].verified, Some(true));
    assert_eq!(report.history.unbuilt, 1);
}

#[test]
fn a_definition_that_makes_an_invalid_body_is_not_taken() {
    for validate in [false, true] {
        let (items, mut geometry) = block_history(Vec::new());
        let mut doc = Document::new(TestKernel {
            invalid: Some("join("),
            ..TestKernel::default()
        });
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut geometry,
            &Options {
                validate,
                ..Options::default()
            },
        );
        let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
        let join = if validate {
            Outcome::Fallback
        } else {
            Outcome::Parametric
        };
        assert_eq!(
            outcomes,
            [Outcome::Partial, Outcome::Parametric, join],
            "{}",
            report.text()
        );
        if validate {
            assert!(report.text().contains("checker finds"), "{}", report.text());
        }
    }
}

#[test]
fn an_unchecked_item_does_not_take_every_body_away() {
    // A cut whose state cannot be rebuilt and that would remove the block.
    let (mut items, mut geometry) = block_history(vec![2]);
    items[2] = extrude(2, 0, "CutFeatureOperation", "d2");
    let mut doc = Document::new(TestKernel::default());
    doc.kernel().mock.split.set(Some(0));
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    assert_ne!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert!(
        report.items[2]
            .note
            .as_deref()
            .unwrap_or_default()
            .contains("it would leave no bodies"),
        "{}",
        report.text()
    );
    assert_eq!(doc.bodies().len(), 1, "{}", report.text());
}

#[test]
fn after_the_time_limit_items_take_their_states() {
    let (items, mut geometry) = block_history(Vec::new());
    let mut doc = Document::new(TestKernel::default());
    let options = Options {
        time_limit: Some(0.0),
        ..Options::default()
    };
    let report = import_design(&mut doc, &timeline_dump(items), &mut geometry, &options);
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Fallback, Outcome::Fallback],
        "{}",
        report.text()
    );
    assert!(
        report.items[1]
            .note
            .as_deref()
            .unwrap()
            .contains("time limit")
    );
    assert_eq!(doc.bodies().len(), 1);
}

#[test]
fn items_the_kernel_hung_on_take_their_states_and_progress_is_shared() {
    let (items, mut geometry) = block_history(Vec::new());
    let mut doc = Document::new(TestKernel::default());
    let progress = std::sync::Arc::new(crate::Progress::new());
    let options = Options {
        hung_items: vec![1],
        progress: Some(progress.clone()),
        ..Options::default()
    };
    let report = import_design(&mut doc, &timeline_dump(items), &mut geometry, &options);
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    // (The mock kernel's join on the base feature's body is not the state.)
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Fallback, Outcome::Fallback],
        "{}",
        report.text()
    );
    let note = |i: usize| report.items[i].note.clone().unwrap_or_default();
    assert!(note(1).contains("did not finish"), "{}", report.text());
    assert!(!note(2).contains("did not finish"), "{}", report.text());
    use std::sync::atomic::Ordering;
    assert_eq!(
        progress.item.load(Ordering::Relaxed),
        crate::Progress::FINISHING
    );
    assert!(progress.step.load(Ordering::Relaxed) >= 4);
}

#[test]
fn items_tried_in_vain_take_their_states_at_once_in_a_try_run_again() {
    // A first try (the kernel hung on item 1 before): the join's
    // definitions give no state (the mock kernel's join on the base
    // feature's body), which the progress keeps with why.
    use std::sync::atomic::Ordering;
    let run = |failed_items: Vec<(i64, String)>| {
        let (items, mut geometry) = block_history(Vec::new());
        let mut doc = Document::new(TestKernel::default());
        let progress = Arc::new(crate::Progress::new());
        let options = Options {
            hung_items: vec![1],
            failed_items,
            progress: Some(progress.clone()),
            ..Options::default()
        };
        let report = import_design(&mut doc, &timeline_dump(items), &mut geometry, &options);
        (report, progress)
    };
    let (first, progress) = run(Vec::new());
    let failed = progress.failed_items();
    assert_eq!(
        failed.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        [2],
        "{}",
        first.text()
    );
    let steps = progress.step.load(Ordering::Relaxed);
    // Run again with them: the join takes its state with the same note,
    // without its definitions being tried (fewer steps), and is not kept
    // again.
    let (second, progress) = run(failed);
    let outcomes = |r: &DesignReport| r.items.iter().map(|i| i.outcome).collect::<Vec<_>>();
    assert_eq!(outcomes(&second), outcomes(&first), "{}", second.text());
    assert_eq!(
        second.items[2].note,
        first.items[2].note,
        "{}",
        second.text()
    );
    assert!(progress.step.load(Ordering::Relaxed) < steps);
    assert!(progress.failed_items().is_empty());
}

/// The block's import (sketch, new body, join) with these options: the
/// report, the document and the outcomes.
fn import_block(options: &Options) -> (DesignReport, Document<TestKernel>, Vec<Outcome>) {
    let (items, mut geometry) = block_history(Vec::new());
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(&mut doc, &timeline_dump(items), &mut geometry, options);
    let outcomes = report.items.iter().map(|i| i.outcome).collect();
    (report, doc, outcomes)
}

#[test]
fn after_a_stop_the_features_take_their_states_and_the_report_says_where() {
    let (report, _, outcomes) = import_block(&Options::default());
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Parametric, Outcome::Parametric],
        "{}",
        report.text()
    );
    assert_eq!(report.stopped, None);

    // Stopped before it started: the sketch still comes in, the features
    // take their history states.
    let stop = Arc::new(RecomputeMonitor::new());
    stop.cancel();
    let progress = Arc::new(crate::Progress::new());
    let (report, doc, outcomes) = import_block(&Options {
        stop: Some(stop),
        progress: Some(progress.clone()),
        ..Options::default()
    });
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Fallback, Outcome::Fallback],
        "{}",
        report.text()
    );
    let note = |i: usize| report.items[i].note.clone().unwrap_or_default();
    assert!(note(1).contains(STOPPED), "{}", report.text());
    assert!(note(2).contains(STOPPED), "{}", report.text());
    let name = report.items[1].name.clone();
    assert_eq!(
        report.stopped,
        Some(StopReport {
            item: Some(1),
            name: Some(name.clone())
        })
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains(&format!("stopped at {name}"))),
        "{}",
        report.text()
    );
    assert_eq!(progress.stopped_at(), Some(1));
    assert_eq!(doc.bodies().len(), 1);
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["stopped"]["item"], 1, "{json}");
}

#[test]
fn a_stop_while_a_feature_is_tried_keeps_the_items_before_it() {
    // Cancelled while the first definition is evaluated (made to take 20 s
    // longer): the stop's monitor is the document's while definitions are
    // tried, and cuts the evaluation short.
    let stop = Arc::new(RecomputeMonitor::new());
    stop.set_test_delay(20_000);
    let canceller = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            stop.cancel();
        })
    };
    let started = std::time::Instant::now();
    let (report, doc, outcomes) = import_block(&Options {
        stop: Some(stop),
        ..Options::default()
    });
    canceller.join().unwrap();
    assert!(
        started.elapsed().as_secs_f64() < 10.0,
        "the stop did not cut the evaluation short"
    );
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Fallback, Outcome::Fallback],
        "{}",
        report.text()
    );
    assert_eq!(report.stopped.as_ref().and_then(|s| s.item), Some(1));
    assert!(
        doc.monitor().is_none(),
        "the document's monitor is left as it was"
    );
    assert_eq!(doc.bodies().len(), 1);

    // A try run again after a hang stops where the stopped one did: the
    // items before it are replayed.
    let (report, _, outcomes) = import_block(&Options {
        stop_at: Some(2),
        ..Options::default()
    });
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Parametric, Outcome::Fallback],
        "{}",
        report.text()
    );
    assert_eq!(report.stopped.as_ref().and_then(|s| s.item), Some(2));
}

#[test]
fn bodies_an_unreplayed_item_added_in_the_same_step_come_in_as_its_fallback() {
    // The file's history has the sweep's body and the extrusion in one state.
    let items = vec![
        two_rectangles(0),
        json!({"index": 1, "name": "Sweep1", "objectType": "SweepFeature"}),
        extrude(2, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items[..1].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let large = "r{c9[c12,c10],c10[c9,c11],c11[c10,c12],c12[c11,c9]}";
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    for (region, distance) in [(large, 5), (small, 10)] {
        add(
            &mut reference,
            json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": region}],
                   "extent": {"type": "distance", "distance": distance}, "operation": "new_body"}),
        );
    }
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), shapes(&reference)],
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes[1..],
        [Outcome::Fallback, Outcome::Parametric],
        "{}",
        report.text()
    );
    assert_eq!(report.items[2].verified, Some(true));
    assert!(report.history.reached_end, "{}", report.text());
    // The extrusion, then the sweep's body as a base feature named after it.
    assert_eq!(doc.bodies().len(), 2);
    let base = doc.feature(FeatureUid(3)).unwrap();
    assert_eq!(base.name, "Sweep1");
    assert!(matches!(base.def, FeatureDef::Base(_)));
}

#[test]
fn without_a_history_a_failure_takes_the_stored_bodies() {
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        fillet_item(2),
        extrude(3, 0, "JoinFeatureOperation", "d2"),
    ];
    struct Stored(Vec<MockShape>);
    impl StoredGeometry<MockShape> for Stored {
        fn state_count(&mut self) -> usize {
            0
        }
        fn state(&mut self, _: usize) -> Result<Vec<StoredBody<MockShape>>, String> {
            Err("no history".to_owned())
        }
        fn final_bodies(&mut self) -> Result<Vec<StoredBody<MockShape>>, String> {
            Ok(self
                .0
                .iter()
                .map(|s| StoredBody {
                    shape: s.clone(),
                    name: Some("Block".to_owned()),
                    source: "stored".to_owned(),
                    component: None,
                    id: None,
                    lineage: None,
                })
                .collect())
        }
    }
    let stored = MockShape::imported("stored block", 9);
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut Stored(vec![stored]),
        &Options::default(),
    );
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [
            Outcome::Partial,
            Outcome::Parametric,
            Outcome::Fallback,
            Outcome::Skipped
        ],
        "{}",
        report.text()
    );
    assert_eq!(report.bodies.len(), 1);
    assert_eq!(report.bodies[0].volume_difference, Some(0.0));
}

#[test]
fn an_external_dump_replays_with_profiles_matched_by_area_and_centroid() {
    // An external dump (SCHEMA.md, made by a test producer):
    // a 20 x 10 mm rectangle with a construction circle, extruded
    // 10 mm, and a construction plane.
    let text = include_str!("../../f3d/tests/data/external_dump_box.json");
    let dump = Dump::from_json(text).unwrap();
    let mut doc = Document::new(MockKernel::default());
    let report = import_design(&mut doc, &dump, &mut NoGeometry, &Options::default());
    let sketch = &report.items[0];
    assert_eq!(sketch.outcome, Outcome::Parametric, "{}", report.text());
    let extrude = &report.items[1];
    assert_eq!(extrude.object_type, "ExtrudeFeature");
    assert_eq!(extrude.outcome, Outcome::Parametric, "{}", report.text());
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    // The rectangle's region, not the circle's (construction).
    assert_eq!(def["def"]["profiles"].as_array().unwrap().len(), 1);
    assert_eq!(def["def"]["extent"]["distance"], "d2");
    let FeatureDef::Sketch(s) = &doc.feature(FeatureUid(1)).unwrap().def else {
        panic!("a sketch");
    };
    assert!(s.entities.iter().any(|e| e.construction));
    assert_eq!(doc.bodies().len(), 1);
}

#[test]
fn a_measured_profile_may_be_a_union_of_regions() {
    // One profile of both rectangles' area (10 cm²) and centroid: no
    // region of the sketch has them, the two together do.
    let mut feature = extrude(1, 0, "NewBodyFeatureOperation", "d2");
    feature["detail"]["profile"][0]["area"] = json!(10.0);
    feature["detail"]["profile"][0]["centroid"] = json!([2.7, 1.0, 0.0]);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &timeline_dump(vec![two_rectangles(0), feature]));
    assert_eq!(
        report.items[1].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    assert_eq!(
        def["def"]["profiles"].as_array().map(Vec::len),
        Some(2),
        "{def}"
    );
}

/// The body fingerprint of an external dump for a replayed body (its volume in
/// cm³, as the test kernel measures it).
fn body_fingerprint(doc: &Document<TestKernel>, uid: &str) -> Value {
    let shape = doc.body_shape(uid.parse().unwrap()).unwrap();
    let volume = doc.kernel().mass_properties(shape).unwrap().volume / 1000.0;
    json!({"kind": "body", "objectType": "BRepBody", "name": doc.body_name(uid.parse().unwrap()),
           "volume": volume})
}

#[test]
fn add_in_body_operations_map_to_mitcad_features() {
    // Two blocks, then a combine, a circular pattern of the first extrude
    // and a mirror of the remaining body, as external dumps give them.
    let first = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        extrude(2, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(first.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let bodies: Vec<String> = reference
        .bodies()
        .iter()
        .map(|b| b.uid.to_string())
        .collect();
    assert_eq!(bodies.len(), 2, "two extrudes of the same region set");
    let target = body_fingerprint(&reference, &bodies[0]);
    let tool = body_fingerprint(&reference, &bodies[1]);
    let mut items = first;
    items.push(
        json!({"index": 3, "name": "Combine1", "objectType": "CombineFeature",
        "detail": {"targetBody": target, "toolBodies": [tool], "operation": "CutFeatureOperation",
                   "isKeepToolBodies": true, "isNewComponent": false}}),
    );
    items.push(json!({"index": 4, "name": "C-Pattern1", "objectType": "CircularPatternFeature",
        "detail": {"inputEntities": [{"kind": "feature", "objectType": "ExtrudeFeature",
                                      "name": "Extrude1", "timeline_index": 1}],
                   "patternEntityType": "FeaturesPatternType",
                   "axis": {"kind": "construction_axis", "name": "Z", "origin": "Z"},
                   "quantity": param("d6", "4", 4.0, ""), "totalAngle": param("d7", "360 deg", std::f64::consts::TAU, "deg"),
                   "isSymmetric": false, "patternComputeOption": "IdenticalPatternCompute",
                   "suppressedElementsIds": [2]}}));
    let mut dump = timeline_dump(items);
    for (name, expression, unit) in [("d6", "4", ""), ("d7", "360 deg", "deg")] {
        dump.parameters
            .as_mut()
            .unwrap()
            .model
            .as_mut()
            .unwrap()
            .push(
                serde_json::from_value(
                    json!({"name": name, "expression": expression, "unit": unit}),
                )
                .unwrap(),
            );
    }
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(&mut doc, &dump, &mut NoGeometry, &Options::default());
    let def = |uid: &str| -> Value {
        serde_json::from_str::<Value>(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                .unwrap(),
        )
        .unwrap()["def"]
            .clone()
    };
    assert_eq!(
        report.items[3].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let combine = def(&report.items[3].features[0]);
    assert_eq!(combine["target"], bodies[0].as_str());
    assert_eq!(combine["tools"], json!([bodies[1]]));
    assert_eq!(combine["operation"], "cut");
    assert_eq!(combine["keep_tools"], true);
    let pattern = &report.items[4];
    assert_eq!(pattern.outcome, Outcome::Parametric, "{}", report.text());
    let pattern = def(&pattern.features[0]);
    assert_eq!(
        pattern["objects"],
        json!({"type": "features", "features": ["F2"]})
    );
    assert_eq!(pattern["axis"], json!("z"));
    assert_eq!(pattern["quantity"], "d6");
    assert_eq!(pattern["angle"], "d7");
    assert_eq!(pattern["compute"], "identical");
    assert_eq!(pattern["suppressed_elements"], json!([2]));
}

#[test]
fn pattern_provenance_marks_only_omitted_dressup_inputs() {
    let mut doc = Document::new(MockKernel::default());
    let dump = timeline_dump(Vec::new());
    let options = Options::default();
    let mut geometry = NoGeometry;
    let mut oracle = Oracle::new(&mut geometry);
    oracle.enabled = false;
    // Candidate translation needs only the imported feature identities;
    // no geometry is evaluated or selected in this test.
    let mut importer = Importer {
        oracle,
        doc: &mut doc,
        dump: &dump,
        options: &options,
        params: ParamMap::default(),
        sketches: HashMap::new(),
        features: HashMap::from([
            (1, "F2".parse().unwrap()),
            (2, "F3".parse().unwrap()),
            (3, "F4".parse().unwrap()),
        ]),
        report: DesignReport::default(),
        pending: Vec::new(),
        defined: Default::default(),
        at_final: false,
        started: std::time::Instant::now(),
        components: components::Components::default(),
        component: ComponentUid::ROOT,
        sketch_copies: HashMap::new(),
        stopped_at: Cell::new(None),
        trying: false,
        alternatives: HashMap::new(),
        guesses: None,
        defer_guesses: false,
        kernel_failed: false,
        start_bases: HashMap::new(),
        deadline: None,
        captures: captures::Captures::default(),
        memory_released: 0,
        frame_entities: Default::default(),
        workers: Default::default(),
        learn: None,
        dressups: HashMap::from([
            (2, vec!["F3".parse().unwrap()]),
            (3, vec!["F4".parse().unwrap()]),
        ]),
        notes: Default::default(),
        suppressed: Default::default(),
        made_ahead: HashMap::new(),
        exact_pairs: None,
        waited: Default::default(),
        deviations: Default::default(),
        deviated: Default::default(),
        deviated_log: Default::default(),
        deviation_points: Default::default(),
        sources: None,
        linker: None,
        focus_paths: HashMap::new(),
        link_paths: Default::default(),
    };
    let feature = |index: i64, kind: &str| json!({"kind": "feature", "objectType": kind, "timeline_index": index});
    let plain = feature(1, "ExtrudeFeature");
    let fillet = feature(2, "FilletFeature");
    let chamfer = feature(3, "ChamferFeature");
    for kind in [
        "CircularPatternFeature",
        "RectangularPatternFeature",
        "MirrorFeature",
    ] {
        for (inputs, expected) in [
            (
                vec![plain.clone()],
                vec![(vec!["F2"], InputProvenance::Unknown)],
            ),
            (
                vec![plain.clone(), fillet.clone(), chamfer.clone()],
                vec![
                    (vec!["F2", "F3", "F4"], InputProvenance::Unknown),
                    (vec!["F2"], InputProvenance::Inferred),
                ],
            ),
            (
                vec![fillet.clone(), chamfer.clone()],
                vec![(vec!["F3", "F4"], InputProvenance::Unknown)],
            ),
        ] {
            let item = serde_json::from_value(json!({"index": 4, "objectType": kind,
            "detail": {
                "inputEntities": inputs,
                "axis": {"kind": "construction_axis", "origin": "Z"},
                "quantity": {"kind": "parameter", "value": 2.0, "unit": ""},
                "totalAngle": {"kind": "parameter", "value": 180.0, "unit": "deg"},
                "directionOne": [1.0, 0.0, 0.0],
                "quantityOne": {"kind": "parameter", "value": 2.0, "unit": ""},
                "distanceOne": {"kind": "parameter", "value": 1.0, "unit": "cm"},
                "mirrorPlane": {"kind": "construction_plane", "origin": "YZ"},
                "isCombine": false
            }}))
            .unwrap();
            let candidates = importer.translate_op(kind, &item).unwrap();
            let multiplier = if kind == "MirrorFeature" { 2 } else { 1 };
            assert_eq!(candidates.len(), expected.len() * multiplier, "{kind}");
            // Without history, the complete selection remains first. The
            // mirror's opposite-combine candidates keep their prior flags.
            for (candidate, (features, provenance)) in candidates.iter().zip(&expected) {
                assert_eq!(candidate.input_provenance, *provenance, "{kind}");
                assert_eq!(
                    candidate.defs[0]["objects"],
                    json!({"type": "features", "features": features}),
                    "{kind}"
                );
                assert!(!candidate.guess, "{kind}");
            }
            for candidate in &candidates[expected.len()..] {
                assert!(candidate.guess);
                assert_eq!(candidate.input_provenance, InputProvenance::Inferred);
            }
        }
    }
    // The settings the file stores (mitcad#186) make the complete selection
    // read, and tried before the history's suggestions; without them (or
    // with an omitted dressup) it stays unknown.
    for (kind, settings, key, expected) in [
        (
            "CircularPatternFeature",
            json!({"isSymmetric": true, "patternComputeOption": "OptimizedPatternCompute",
                   "suppressedElementsIds": [2]}),
            "compute",
            "optimized",
        ),
        (
            "RectangularPatternFeature",
            json!({"isSymmetricInDirectionOne": true,
                   "patternDistanceType": "ExtentPatternDistanceType",
                   "patternComputeOption": "IdenticalPatternCompute",
                   "suppressedElementsIds": []}),
            "distance_type",
            "extent",
        ),
        (
            "MirrorFeature",
            json!({"patternComputeOption": "IdenticalPatternCompute"}),
            "compute",
            "identical",
        ),
    ] {
        for with in [true, false] {
            let mut detail = json!({
                "inputEntities": [plain.clone()],
                "axis": {"kind": "construction_axis", "origin": "Z"},
                "quantity": {"kind": "parameter", "value": 2.0, "unit": ""},
                "totalAngle": {"kind": "parameter", "value": 180.0, "unit": "deg"},
                "directionOne": [1.0, 0.0, 0.0],
                "quantityOne": {"kind": "parameter", "value": 2.0, "unit": ""},
                "distanceOne": {"kind": "parameter", "value": 1.0, "unit": "cm"},
                "mirrorPlane": {"kind": "construction_plane", "origin": "YZ"},
                "isCombine": false
            });
            if with {
                for (k, v) in settings.as_object().unwrap() {
                    detail[k] = v.clone();
                }
            }
            let item =
                serde_json::from_value(json!({"index": 4, "objectType": kind, "detail": detail}))
                    .unwrap();
            let candidates = importer.translate_op(kind, &item).unwrap();
            let c = &candidates[0];
            assert_eq!(c.first, with, "{kind} {with}");
            assert_eq!(
                c.input_provenance,
                if with {
                    InputProvenance::Read
                } else {
                    InputProvenance::Unknown
                },
                "{kind} {with}"
            );
            if with {
                assert_eq!(c.defs[0][key], expected, "{kind}");
            }
        }
    }
    // A direction named by an axis that is not an origin axis is tried by
    // its stored vector as well, after the axis (the replay's axis need not
    // point where the file's does); an origin axis points where its vector
    // does, so only one definition is made.
    for (axis, count) in [
        (json!({"kind": "construction_axis", "timeline_index": 2}), 2),
        (json!({"kind": "construction_axis", "origin": "X"}), 1),
    ] {
        let item = serde_json::from_value(
            json!({"index": 4, "objectType": "RectangularPatternFeature",
            "detail": {"inputEntities": [plain.clone()],
                       "directionOneEntity": axis, "directionOne": [0.0, -1.0, 0.0],
                       "quantityOne": {"kind": "parameter", "value": 2.0, "unit": ""},
                       "distanceOne": {"kind": "parameter", "value": -1.0, "unit": "cm"}}}),
        )
        .unwrap();
        let candidates = importer
            .translate_op("RectangularPatternFeature", &item)
            .unwrap();
        assert_eq!(candidates.len(), count, "{axis}");
        if count == 2 {
            assert_eq!(candidates[0].defs[0]["direction1"]["axis"], "F3");
            assert_eq!(
                candidates[1].defs[0]["direction1"]["axis"],
                json!({"origin": [0.0, 0.0, 0.0], "direction": [0.0, -1.0, 0.0]})
            );
            assert_eq!(
                candidates[1].defs[0]["direction1"]["distance"],
                candidates[0].defs[0]["direction1"]["distance"]
            );
        }
    }
}

#[test]
fn a_rectangular_pattern_uses_named_bodies_and_decoded_directions_without_history() {
    for sketch_axes in [false, true] {
        let mut detail = json!({
            "inputEntities": [{"kind": "body", "objectType": "BRepBody",
                               "_f3d": {"producer": 1, "body_index": 0}}],
            "quantityOne": {"kind": "parameter", "value": 2.0, "unit": ""},
            "quantityTwo": {"kind": "parameter", "value": 3.0, "unit": ""},
            "distanceOne": {"kind": "parameter", "value": -5.0},
            "distanceTwo": {"kind": "parameter", "value": 4.0},
            "directionOne": [2.0, 0.0, 0.0],
            "directionTwo": [0.0, 7.0, 0.0]
        });
        if sketch_axes {
            detail.as_object_mut().unwrap().remove("directionOne");
            detail.as_object_mut().unwrap().remove("directionTwo");
            for (key, curve) in [("directionOneEntity", "c0"), ("directionTwoEntity", "c1")] {
                detail[key] = json!({"kind": "sketch_entity", "objectType": "SketchLine",
                    "sketch_timeline_index": 0, "id": curve});
            }
        }
        let dump = timeline_dump(vec![
            rectangle_sketch(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
            json!({"index": 2, "name": "Pattern", "objectType": "RectangularPatternFeature",
                   "detail": detail}),
        ]);
        let mut doc = Document::new(MockKernel::default());
        let report = import(&mut doc, &dump);
        let item = &report.items[2];
        assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
        assert_eq!(doc.bodies().len(), 6);
        let value: Value = serde_json::from_str(
            &doc.query(&format!(
                r#"{{"query": "feature", "uid": "{}"}}"#,
                item.features[0]
            ))
            .unwrap(),
        )
        .unwrap();
        let def = &value["def"];
        assert_eq!(
            def["objects"],
            json!({"type": "bodies", "bodies": ["F2.b0"]})
        );
        let parameters = doc.parameters();
        let distance = |direction: &str| {
            parameters.value(
                parameters
                    .find(def[direction]["distance"].as_str().unwrap())
                    .unwrap(),
            )
        };
        assert_eq!(distance("direction1"), Some(-50.0));
        assert_eq!(distance("direction2"), Some(40.0));
        if sketch_axes {
            assert_eq!(def["direction1"]["axis"]["sketch"], "F1");
            assert_eq!(def["direction2"]["axis"]["sketch"], "F1");
            assert_ne!(
                def["direction1"]["axis"]["curve"],
                def["direction2"]["axis"]["curve"]
            );
        } else {
            assert_eq!(
                def["direction1"]["axis"]["direction"],
                json!([1.0, 0.0, 0.0])
            );
            assert_eq!(
                def["direction2"]["axis"]["direction"],
                json!([0.0, 1.0, 0.0])
            );
        }
    }
}

#[test]
fn a_circular_pattern_uses_its_decoded_sketch_axis_without_history() {
    let dump = timeline_dump(vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 2, "name": "Pattern", "objectType": "CircularPatternFeature",
        "detail": {
            "inputEntities": [{"kind": "body", "objectType": "BRepBody",
                               "_f3d": {"producer": 1, "body_index": 0}}],
            "axis": {"kind": "sketch_entity", "objectType": "SketchLine",
                     "sketch_timeline_index": 0, "id": "c0"},
            "quantity": {"kind": "parameter", "value": 3.0, "unit": ""},
            "totalAngle": {"kind": "parameter", "value": std::f64::consts::PI, "unit": "rad"}
        }}),
    ]);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    let item = &report.items[2];
    assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(doc.bodies().len(), 3);
    let value: Value = serde_json::from_str(
        &doc.query(&format!(
            r#"{{"query": "feature", "uid": "{}"}}"#,
            item.features[0]
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(value["def"]["axis"]["sketch"], "F1");
    assert_eq!(
        value["def"]["objects"],
        json!({"type": "bodies", "bodies": ["F2.b0"]})
    );
}

#[test]
fn bodies_the_decoder_named_are_found_by_points_on_their_edges() {
    // A block, then a move of it, the body given as the decoder gives it:
    // names and middle points of some of its edges (cm).
    let first = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel {
        edge_points: true,
        ..TestKernel::default()
    });
    import_design(
        &mut reference,
        &timeline_dump(first.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (uid, shape) = (reference.bodies()[0].uid, shapes(&reference)[0].clone());
    let points: Vec<[f64; 3]> = shape
        .edges
        .iter()
        .take(3)
        .map(|(a, b)| edge_point(a, b).map(|x| x / 10.0))
        .collect();
    let body = |points: Value| {
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"recipe": "body", "entities": [[{"tag": "304", "kind": 7, "ops": []}]],
                        "edge_points": points}})
    };
    let replay_with = |body: Value, definition: Value| {
        let mut items = first.clone();
        let transform = json!([
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 0.5],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0]
        ]);
        let mut detail = json!({"inputEntities": [body], "transform": transform});
        if !definition.is_null() {
            detail["definition"] = definition;
        }
        let item = json!({"index": 2, "name": "Move1", "objectType": "MoveFeature",
                          "detail": detail});
        items.push(item);
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        );
        let def = report.items[2].features.first().map(|uid| {
            serde_json::from_str::<Value>(
                &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                    .unwrap(),
            )
            .unwrap()["def"]
                .clone()
        });
        (report, def)
    };
    let replay = |body: Value| replay_with(body, Value::Null);
    let (report, def) = replay(body(json!(points)));
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def = def.unwrap();
    assert_eq!(def["bodies"], json!([uid.to_string()]));
    assert_eq!(def["transform"]["matrix"][0][3], json!(10.0));
    // Without its definition decoded: the matrix stands in.
    assert_eq!(
        report.items[2].input_provenance,
        Some(InputProvenance::Unknown)
    );
    // A free move as decoded: the matrix is its input.
    let (report, def) = replay_with(
        body(json!(points)),
        json!({"_type": "MoveFeatureFreeMoveDefinition"}),
    );
    assert_eq!(def.unwrap()["transform"]["type"], "free");
    assert_eq!(
        report.items[2].input_provenance,
        Some(InputProvenance::Read)
    );
    // A translation along the axes by its distances (cm).
    let (report, def) = replay_with(
        body(json!(points)),
        json!({"_type": "MoveFeatureTranslateXYZDefinition",
               "xDistance": param("d5", "10 mm", 1.0, "mm"),
               "yDistance": param("d6", "5 mm", 0.5, "mm"),
               "zDistance": param("d7", "0 mm", 0.0, "mm")}),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def = def.unwrap();
    assert_eq!(def["transform"]["type"], "translate_xyz", "{def}");
    assert_eq!(
        report.items[2].input_provenance,
        Some(InputProvenance::Read)
    );
    // A point on no edge of the replay: not found.
    let (report, _) = replay(body(json!([[123.0, 0.0, 0.0]])));
    assert_ne!(report.items[2].outcome, Outcome::Parametric);
}

/// A removal and a copy of bodies, found by the points of the body's edges.
#[test]
fn a_removal_and_a_copy_of_bodies_are_features() {
    let first = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel {
        edge_points: true,
        ..TestKernel::default()
    });
    import_design(
        &mut reference,
        &timeline_dump(first.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (uid, shape) = (reference.bodies()[0].uid, shapes(&reference)[0].clone());
    let points: Vec<[f64; 3]> = shape
        .edges
        .iter()
        .take(3)
        .map(|(a, b)| edge_point(a, b).map(|x| x / 10.0))
        .collect();
    let body = json!({"kind": "body", "objectType": "BRepBody",
                      "_f3d": {"recipe": "body", "entities": [[{"tag": "304", "kind": 7, "ops": []}]],
                               "edge_points": points}});
    let replay = |object_type: &str, name: &str, detail: Value| {
        let mut items = first.clone();
        items.push(json!({"index": 2, "name": name, "objectType": object_type,
                          "detail": detail,
                          "_f3d": {"base_name": name.trim_end_matches(char::is_numeric)}}));
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        );
        let def = report.items[2].features.first().map(|uid| {
            serde_json::from_str::<Value>(
                &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                    .unwrap(),
            )
            .unwrap()["def"]
                .clone()
        });
        (report, doc, def)
    };
    let (report, doc, def) = replay(
        "RemoveFeature",
        "RemoveBody-Body1",
        json!({"itemToRemove": body}),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert_eq!(
        def.unwrap(),
        json!({"type": "remove_body", "bodies": [uid.to_string()]})
    );
    assert!(doc.bodies().is_empty());
    let (report, doc, def) = replay(
        "CopyPasteBodies",
        "CopyPasteBodies1",
        json!({"sourceBody": [body]}),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def = def.unwrap();
    assert_eq!(def["type"], json!("move"));
    assert_eq!(def["copy"], json!(true));
    assert_eq!(doc.bodies().len(), 2);
    // A cut whose body the file lost keeps the cached result: not a feature.
    let (report, _, _) = replay("CopyPasteBodies", "CutPasteBodies1", json!({}));
    assert_ne!(report.items[2].outcome, Outcome::Parametric);
    assert!(
        report.items[2]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("missing in the file")),
        "{}",
        report.text()
    );
}

#[test]
fn route_b_patterns_and_holes_fall_back() {
    // The stream decoder gives a pattern's angle only: no inputs, no axis.
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 2, "name": "C-Pattern1", "objectType": "CircularPatternFeature",
               "detail": {"totalAngle": param("d7", "360 deg", std::f64::consts::TAU, "deg")}}),
    ];
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Skipped,
        "{}",
        report.text()
    );
    assert!(
        report.items[2]
            .note
            .as_deref()
            .unwrap()
            .contains("no inputs"),
        "{}",
        report.text()
    );
}

/// The block dump with components: the root (object 3), Plate (10) placed
/// twice 10 cm apart, each with Pin (20) 1 cm along X in it, and an
/// occurrence of a component of another document.
fn assembly_dump(named: bool) -> Dump {
    let mut value: Value = serde_json::to_value(block_dump()).unwrap();
    let matrix = |x: f64| {
        json!([
            [1.0, 0.0, 0.0, x],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0]
        ])
    };
    let pin = json!({"component": "Pin", "isReferencedComponent": false,
                     "_f3d": {"component_object": 20, "local_transform": matrix(1.0)}, "children": []});
    value["document"] = json!({"root_component": "Assembly"});
    value["components"] = json!([
        {"name": "Assembly", "is_root": true, "_f3d": {"object_id": 3}},
        {"name": "Plate", "_f3d": {"object_id": 10}},
        {"name": "Pin", "_f3d": {"object_id": 20}}]);
    value["occurrences"] = json!([
        {"component": "Plate", "transform": matrix(0.0), "isGrounded": true,
         "_f3d": {"component_object": 10}, "children": [pin]},
        {"component": "Plate", "transform": matrix(10.0), "isLightBulbOn": false,
         "_f3d": {"component_object": 10}, "children": [pin]},
        {"component": null, "isReferencedComponent": true, "transform": matrix(0.0),
         "_f3d": {"component_object": 3, "external_key": "00ff"}}]);
    if named {
        value["timeline"]["items"][1]["component"] = json!("Plate");
    }
    Dump::from_json(&value.to_string()).unwrap()
}

/// History data without states: which components items changed.
#[derive(Default)]
struct ComponentHistory {
    items: HashMap<i64, Vec<u64>>,
}

impl StoredGeometry<MockShape> for ComponentHistory {
    fn state_count(&mut self) -> usize {
        0
    }

    fn state(&mut self, index: usize) -> Result<Vec<StoredBody<MockShape>>, String> {
        Err(format!("there is no history state {index}"))
    }

    fn final_bodies(&mut self) -> Result<Vec<StoredBody<MockShape>>, String> {
        Ok(Vec::new())
    }

    fn item_components(&mut self, index: i64) -> Option<Vec<u64>> {
        self.items.get(&index).cloned()
    }
}

#[test]
fn components_and_occurrences_come_in_with_their_items() {
    for named in [true, false] {
        let mut doc = Document::new(MockKernel::default());
        // An external dump names the extrude's component; the decoder leaves it
        // to the history, which says the extrude changed Plate's bodies.
        let mut geometry = ComponentHistory::default();
        if !named {
            geometry.items.insert(1, vec![10]);
        }
        let report = import_design(
            &mut doc,
            &assembly_dump(named),
            &mut geometry,
            &Options::default(),
        );
        let c = &report.components;
        assert_eq!(
            (c.components, c.occurrences, c.external),
            (2, 3, 1),
            "{}",
            report.text()
        );
        let a = doc.assembly();
        assert_eq!(a.root_name, "Assembly");
        let names: Vec<String> = a
            .occurrences
            .iter()
            .map(|o| a.occurrence_name(o.uid))
            .collect();
        // The occurrence of a component of another document: an empty
        // component stands for it (mitcad#75).
        assert_eq!(
            names,
            ["Plate:1", "Pin:1", "Plate:2", "Inserted component:1"]
        );
        let inserted = a.occurrences[3].component;
        assert!(doc.component_bodies(inserted).is_empty());
        assert!(a.occurrences[0].grounded && !a.occurrences[2].visible);
        assert_eq!(a.occurrences[1].transform.translation, [10.0, 0.0, 0.0]);
        assert_eq!(a.occurrences[2].transform.translation, [100.0, 0.0, 0.0]);
        // The sketch follows the extrude that uses it into Plate.
        let plate = a.components[0].uid;
        assert!(doc.features().all(|f| f.component == plate));
        assert_eq!(report.items[0].component.as_deref(), Some("Plate"));
        assert_eq!(c.items.get("Plate"), Some(&2));
        // The plate's body in each occurrence.
        doc.set_occurrence_visible(a.occurrences[2].uid, true)
            .unwrap();
        let placed: Vec<f64> = doc
            .instances()
            .iter()
            .map(|i| i.transform.translation[0])
            .collect();
        assert_eq!(placed, [0.0, 100.0]);
        assert!(
            report
                .text()
                .contains("components: 2 made, 3 occurrences placed, 1 of other")
        );
    }
}

/// The block's sketch and extrusion owned by the given components
/// (`_f3d.component`, the decoder's), in a design with one occurrence of
/// Plate (object 10) placed 5 cm along X.
fn owned_dump(sketch_owner: u64, extrude_owner: u64) -> Dump {
    let mut value: Value = serde_json::to_value(block_dump()).unwrap();
    value["document"] = json!({"root_component": "Assembly"});
    value["components"] = json!([
        {"name": "Assembly", "is_root": true, "_f3d": {"object_id": 3}},
        {"name": "Plate", "_f3d": {"object_id": 10}}]);
    value["occurrences"] = json!([
        {"component": "Plate", "_f3d": {"component_object": 10},
         "transform": [[1.0, 0.0, 0.0, 5.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0],
                       [0.0, 0.0, 0.0, 1.0]]}]);
    value["timeline"]["items"][0]["_f3d"] = json!({"component": sketch_owner});
    value["timeline"]["items"][1]["_f3d"]["component"] = json!(extrude_owner);
    Dump::from_json(&value.to_string()).unwrap()
}

#[test]
fn items_go_into_the_components_that_own_them() {
    // Without a history the decoder's owners place the items: both in
    // Plate (before mitcad#37 they went into the root).
    let mut doc = Document::new(MockKernel::default());
    let report = import_design(
        &mut doc,
        &owned_dump(10, 10),
        &mut NoGeometry,
        &Options::default(),
    );
    let plate = doc.assembly().components[0].uid;
    assert!(
        doc.features().all(|f| f.component == plate),
        "{}",
        report.text()
    );
    assert_eq!(report.items[1].outcome, Outcome::Parametric);
    assert_eq!(report.components.items.get("Plate"), Some(&2));

    // A sketch of the root used by Plate's extrusion: its geometry is in
    // the root's coordinates, so it stays there, and the extrusion uses a
    // copy moved into Plate's (5 cm back along X).
    let mut doc = Document::new(MockKernel::default());
    let report = import_design(
        &mut doc,
        &owned_dump(3, 10),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        doc.feature(FeatureUid(1)).unwrap().component,
        ComponentUid::ROOT
    );
    assert_eq!(
        report.items[1].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let extrude: FeatureUid = report.items[1]
        .features
        .iter()
        .filter_map(|f| f.parse().ok())
        .find(|f| {
            matches!(
                doc.feature(*f).map(|e| &e.def),
                Some(FeatureDef::Extrude(_))
            )
        })
        .expect("an extrusion");
    assert_eq!(doc.feature(extrude).unwrap().component, plate);
    let def: Value = serde_json::from_str(
        &doc.query(&format!(r#"{{"query": "feature", "uid": "{extrude}"}}"#))
            .unwrap(),
    )
    .unwrap();
    let copy: FeatureUid = def["def"]["profiles"][0]["sketch"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(copy, FeatureUid(1));
    assert_eq!(doc.feature(copy).unwrap().component, plate);
    let origin = doc.sketch_output(copy).unwrap().frame.origin;
    assert!((origin[0] + 50.0).abs() < 1e-9, "{origin:?}");
}

/// [`owned_dump`] with Plate's occurrence stored `stored` cm along X and
/// a captured position at timeline index `captured` putting it 20 cm
/// along X; the extrusion is at index 1, or 2 after a captured position
/// at 1.
fn captured_dump(sketch_owner: u64, stored: f64, captured: i64) -> Dump {
    let mut value: Value = serde_json::to_value(owned_dump(sketch_owner, 10)).unwrap();
    value["occurrences"][0]["_f3d"]["object_id"] = json!(100);
    value["occurrences"][0]["transform"][0][3] = json!(stored);
    let position = json!({"index": captured, "name": "Position1", "objectType": "Snapshot",
        "_f3d": {"component": 3},
        "detail": {"positions": [{
            "occurrence": {"kind": "occurrence", "_f3d": {"path": [100], "context_component": 3}},
            "transform": [[1.0, 0.0, 0.0, 20.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0],
                          [0.0, 0.0, 0.0, 1.0]]}]}});
    let items = value["timeline"]["items"].as_array_mut().unwrap();
    if captured == 1 {
        items[1]["index"] = json!(2);
        items.insert(1, position);
        for p in value["parameters"]["model"].as_array_mut().unwrap() {
            if p["createdBy"]["timeline_index"] == json!(1) {
                p["createdBy"]["timeline_index"] = json!(2);
            }
        }
    } else {
        items.push(position);
    }
    Dump::from_json(&value.to_string()).unwrap()
}

/// The X of the origin of the sketch Plate's extrusion uses, and the
/// component it is in.
fn extruded_sketch(doc: &Document<MockKernel>, report: &DesignReport) -> (f64, ComponentUid) {
    let extrude = report
        .items
        .iter()
        .flat_map(|i| &i.features)
        .filter_map(|f| f.parse::<FeatureUid>().ok())
        .find(|f| {
            matches!(
                doc.feature(*f).map(|e| &e.def),
                Some(FeatureDef::Extrude(_))
            )
        })
        .expect("an extrusion");
    let def: Value = serde_json::from_str(
        &doc.query(&format!(r#"{{"query": "feature", "uid": "{extrude}"}}"#))
            .unwrap(),
    )
    .unwrap();
    let sketch: FeatureUid = def["def"]["profiles"][0]["sketch"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let origin = doc.sketch_output(sketch).unwrap().frame.origin;
    (origin[0], doc.feature(sketch).unwrap().component)
}

/// Items before a captured position were made with the occurrences where
/// they were before it (mitcad#86): a sketch of the root used by Plate's
/// extrusion is moved into Plate by the placements at the extrusion's
/// point of the timeline, although Plate starts at its last captured
/// position (mitcad#75).
#[test]
fn sketches_move_by_the_placements_at_their_users() {
    let run = |dump: &Dump| {
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, dump, &mut NoGeometry, &Options::default());
        assert_eq!(report.joints.positions, 1, "{}", report.text());
        let extrusion = report
            .items
            .iter()
            .find(|i| i.object_type == "ExtrudeFeature")
            .unwrap();
        assert_eq!(extrusion.outcome, Outcome::Parametric, "{}", report.text());
        // Plate starts at its captured position in every case.
        let plate = &doc.assembly().occurrences[0];
        assert_eq!(plate.transform.translation[0], 200.0);
        let (x, component) = extruded_sketch(&doc, &report);
        (x, component == plate.component)
    };
    // Captured after the extrusion: Plate was at its stored 5 cm.
    assert_eq!(run(&captured_dump(3, 5.0, 2)), (-50.0, true));
    // Captured before it: at the captured 20 cm.
    assert_eq!(run(&captured_dump(3, 5.0, 1)), (-200.0, true));
    // Plate stored where the root is: the sketch follows the extrusion into
    // Plate as it is, unless a captured position before the extrusion had
    // moved Plate away (a copy moved by 20 cm then).
    assert_eq!(run(&captured_dump(3, 0.0, 2)), (0.0, true));
    assert_eq!(run(&captured_dump(3, 0.0, 1)), (-200.0, true));
}

#[test]
fn an_item_without_a_state_leaves_the_next_items_state_to_it() {
    // Two extrusions of the same block; the history names the second one's
    // state, so the first one (whose operation the file does not have:
    // rolled back, say) does not take it.
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        extrude(2, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items[..2].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), shapes(&reference)],
        items: HashMap::from([(2, 1)]),
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Skipped, Outcome::Parametric],
        "{}",
        report.text()
    );
    assert_eq!(report.items[2].verified, Some(true));
}

#[test]
fn undo_takes_back_the_whole_import() {
    let mut doc = Document::new(MockKernel::default());
    let options = Options {
        undo_label: Some("Import block.f3d".to_owned()),
        ..Options::default()
    };
    import_design(&mut doc, &block_dump(), &mut NoGeometry, &options);
    assert_eq!(doc.undo_label(), Some("Import block.f3d"));
    doc.undo();
    assert_eq!(doc.features().count(), 0);
    assert_eq!(doc.parameters().len(), 0);
}

// Sweeps, pipes and lofts (F3).

#[test]
fn sweeps_pipes_and_lofts_of_the_add_in_replay() {
    // Sketch1: the 40 x 20 rectangle; Sketch2: a line, the path.
    let path_sketch = json!({
        "index": 1, "name": "Sketch2", "objectType": "Sketch",
        "detail": {
            "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
            "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
            "points": [{"id": "p0", "xyz": [0.0, 0.0, 0.0]}, {"id": "p1", "xyz": [0.0, 5.0, 0.0]}],
            "curves": [{"id": "c0", "type": "SketchLine", "startSketchPoint": "p0", "endSketchPoint": "p1"}]
        }
    });
    let line = json!({"kind": "sketch_entity", "objectType": "SketchLine", "sketch": "Sketch2",
                      "sketch_timeline_index": 1, "id": "c0"});
    let profile = json!({"kind": "profile", "sketch": "Sketch1", "sketch_timeline_index": 0,
                         "area": 8.0, "centroid": [2.0, 1.0, 0.0]});
    let sweep = json!({"index": 2, "name": "Sweep1", "objectType": "SweepFeature", "detail": {
        "profile": [profile], "operation": "NewBodyFeatureOperation",
        "path": [{"_type": "PathEntity", "entity": line, "isOpposedToEntity": false}],
        "orientation": "ParallelOrientationType", "isSolid": true,
        "distanceOne": param("d7", "0.5", 0.5, ""), "distanceTwo": param("d8", "1", 1.0, ""),
        "twistAngle": param("d9", "30 deg", 0.5235987755982988, "deg"),
        "taperAngle": param("d10", "0 deg", 0.0, "deg"), "isDirectionFlipped": true}});
    let pipe = json!({"index": 3, "name": "Pipe1", "objectType": "PipeFeature", "detail": {
        "path": [{"_type": "PathEntity", "entity": line}], "sectionType": "SquarePipeSectionType",
        "sectionSize": param("d11", "8 mm", 0.8, "mm"), "isHollow": true,
        "sectionThickness": param("d12", "1 mm", 0.1, "mm"),
        "distanceOne": param("d13", "1", 1.0, ""), "distanceTwo": param("d14", "0", 0.0, ""),
        "operation": "NewBodyFeatureOperation"}});
    let loft = json!({"index": 4, "name": "Loft1", "objectType": "LoftFeature", "detail": {
        "loftSections": [
            {"_type": "LoftSection", "index": 1, "entity": {"kind": "construction_point", "name": "Origin",
             "origin": "Origin"}, "endCondition": {"_type": "PointSharpEndCondition"}},
            {"_type": "LoftSection", "index": 0, "entity": profile,
             "endCondition": {"_type": "FreeEndCondition"}}],
        "isClosed": false, "isSolid": true, "operation": "NewBodyFeatureOperation"}});
    let dump = dump(json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "sweeps.f3d"},
        "timeline": {"items": [rectangle_sketch(0), path_sketch, sweep, pipe, loft]}
    }));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(outcomes[1..], [Outcome::Parametric; 4], "{}", report.text());
    let def = |uid: u64| {
        let query = json!({"query": "feature", "uid": format!("F{uid}")}).to_string();
        let answer: Value = serde_json::from_str(&doc.query(&query).unwrap()).unwrap();
        answer["def"].clone()
    };
    let sweep = def(3);
    assert_eq!(sweep["type"], "sweep");
    assert_eq!(sweep["path"]["sketch"], "F2");
    assert_eq!(sweep["orientation"], "parallel");
    assert_eq!(sweep["flip"], true);
    assert_eq!(sweep["extent"]["type"], "partial");
    assert!(sweep.get("twist_angle").is_some() && sweep.get("taper_angle").is_none());
    let fraction = doc.parameters().get(
        doc.parameters()
            .find(sweep["extent"]["fraction"].as_str().unwrap())
            .unwrap(),
    );
    assert_eq!(fraction.map(|p| p.value()), Some(0.5));
    let pipe = def(4);
    assert_eq!(pipe["section"], "square");
    assert!(pipe.get("thickness").is_some() && pipe.get("extent").is_none());
    let loft = def(5);
    assert_eq!(loft["sections"][0]["type"], "profile");
    assert_eq!(
        loft["sections"][1],
        json!({"type": "point", "point": "origin"})
    );
    assert_eq!(loft["end_condition"], json!({"type": "point_sharp"}));
    assert!(loft.get("start_condition").is_none());
}

/// The stream decoder's sweeps, pipes and lofts (mitcad#34): profiles by
/// their sketch only, a second fraction 0, no direction, orientation,
/// section type or wall. Without a history the first reading is taken: a
/// region of the sketch, the whole path, a solid circular pipe.
#[test]
fn sweeps_pipes_and_lofts_of_the_stream_decoder() {
    let path_sketch = json!({
        "index": 1, "name": "Sketch2", "objectType": "Sketch",
        "detail": {
            "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
            "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
            "points": [{"id": "p0", "xyz": [0.0, 0.0, 0.0]}, {"id": "p1", "xyz": [0.0, 5.0, 0.0]}],
            "curves": [{"id": "c0", "type": "SketchLine", "startSketchPoint": "p0", "endSketchPoint": "p1"}]
        }
    });
    let line = json!([{"_type": "PathEntity", "entity": {"kind": "sketch_entity",
        "objectType": "SketchLine", "sketch": "Sketch2", "sketch_timeline_index": 1, "id": "c0"}}]);
    let profile = json!({"kind": "profile", "sketch": "Sketch1", "sketch_timeline_index": 0});
    let sweep = json!({"index": 2, "name": "Sweep1", "objectType": "SweepFeature", "detail": {
        "profile": [profile], "operation": "NewBodyFeatureOperation", "path": line,
        "distanceOne": param("d7", "1", 1.0, ""), "distanceTwo": param("d8", "0", 0.0, ""),
        "twistAngle": param("d9", "0 deg", 0.0, "deg"),
        "taperAngle": param("d10", "0 deg", 0.0, "deg")}});
    let pipe = json!({"index": 3, "name": "Pipe1", "objectType": "PipeFeature", "detail": {
        "path": line, "sectionSize": param("d11", "8 mm", 0.8, "mm"),
        "sectionThickness": param("d12", "1 mm", 0.1, "mm"),
        "distanceOne": param("d13", "1", 1.0, ""), "distanceTwo": param("d14", "0", 0.0, ""),
        "operation": "NewBodyFeatureOperation"}});
    let loft = json!({"index": 4, "name": "Loft1", "objectType": "LoftFeature", "detail": {
        "operation": "NewBodyFeatureOperation",
        "loftSections": [
            {"_type": "LoftSection", "index": 0, "entity": profile,
             "endCondition": {"_type": "LoftFreeEndCondition"}},
            {"_type": "LoftSection", "index": 1, "entity": {"kind": "sketch_entity",
             "objectType": "SketchPoint", "sketch": "Sketch2", "sketch_timeline_index": 1, "id": "p1"},
             "endCondition": {"_type": "LoftPointTangentEndCondition",
                              "weight": param("d15", "1.5", 1.5, "")}}],
        "centerLineOrRails": [], "centerLineOrRails.isCenterLine": false}});
    let dump = dump(json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "sweeps.f3d"},
        "timeline": {"items": [rectangle_sketch(0), path_sketch, sweep, pipe, loft]}
    }));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(outcomes[1..], [Outcome::Parametric; 4], "{}", report.text());
    let def = |uid: u64| {
        let query = json!({"query": "feature", "uid": format!("F{uid}")}).to_string();
        let answer: Value = serde_json::from_str(&doc.query(&query).unwrap()).unwrap();
        answer["def"].clone()
    };
    let sweep = def(3);
    assert_eq!(sweep["profiles"][0]["sketch"], "F1");
    assert_eq!(sweep["path"]["sketch"], "F2");
    for key in ["extent", "flip", "orientation"] {
        assert!(sweep.get(key).is_none(), "{key}: {sweep}");
    }
    let pipe = def(4);
    for key in ["section", "thickness", "extent"] {
        assert!(pipe.get(key).is_none(), "{key}: {pipe}");
    }
    let loft = def(5);
    assert_eq!(loft["sections"][0]["type"], "profile");
    assert_eq!(loft["sections"][1]["type"], "point");
    assert_eq!(loft["sections"][1]["point"]["sketch"], "F2");
    assert_eq!(loft["end_condition"]["type"], "point_tangent");
}

/// Sweeps along guide surfaces (mitcad#157): faces found by their
/// fingerprints become `guide_surfaces` (a guide rail the stream decoder
/// also gives is not one); without the faces' geometry the item is not
/// replayed.
#[test]
fn sweeps_along_guide_surfaces_take_the_faces_found() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let base = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(base.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let top = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "end")
        .cloned()
        .expect("a top face");
    let point = face_point(&top).map(|x| x / 10.0);
    let path_sketch = json!({
        "index": 2, "name": "Sketch2", "objectType": "Sketch",
        "detail": {
            "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
            "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
            "points": [{"id": "p0", "xyz": [0.0, 0.0, 0.0]}, {"id": "p1", "xyz": [0.0, 5.0, 0.0]}],
            "curves": [{"id": "c0", "type": "SketchLine", "startSketchPoint": "p0", "endSketchPoint": "p1"}]
        }
    });
    let line = json!([{"_type": "PathEntity", "entity": {"kind": "sketch_entity",
        "objectType": "SketchLine", "sketch": "Sketch2", "sketch_timeline_index": 2, "id": "c0"}}]);
    let sweep = |face: Value| {
        json!({"index": 3, "name": "Sweep1", "objectType": "SweepFeature", "detail": {
            "profile": [{"kind": "profile", "sketch": "Sketch1", "sketch_timeline_index": 0}],
            "operation": "NewBodyFeatureOperation", "path": line, "guideRail": line,
            "guideSurfaces": [face],
            "distanceOne": param("d7", "1", 1.0, ""), "distanceTwo": param("d8", "1", 1.0, ""),
            "twistAngle": param("d9", "30 deg", 0.5235987755982988, "deg")}})
    };
    let run = |face: Value| {
        let mut items = base.clone();
        items.extend([path_sketch.clone(), sweep(face)]);
        let mut doc = Document::new(kernel());
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        );
        let query = json!({"query": "feature", "uid": "F4"}).to_string();
        let def = doc
            .query(&query)
            .ok()
            .and_then(|a| serde_json::from_str::<Value>(&a).ok())
            .map(|a| a["def"].clone())
            .filter(|d| d["type"] == "sweep");
        (report, def)
    };

    let (report, def) = run(json!({"kind": "face", "objectType": "BRepFace",
        "geometry": {"type": "Plane"}, "point_on_face": point,
        "_f3d": {"recipe": "bounded_face"}}));
    let item = report.items.iter().find(|i| i.name == "Sweep1").unwrap();
    assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
    let def = def.expect("a sweep");
    assert_eq!(
        def["guide_surfaces"],
        json!([{"body": "F2.b0", "face": top.to_string()}])
    );
    for key in ["guide_rail", "twist_angle", "orientation"] {
        assert!(def.get(key).is_none(), "{key}: {def}");
    }

    // The stream decoder's faces by their names only: not replayed (no
    // stored bodies here to fall back on).
    let (report, def) = run(json!({"kind": "face", "objectType": "BRepFace",
        "_f3d": {"recipe": "bounded_face"}}));
    let item = report.items.iter().find(|i| i.name == "Sweep1").unwrap();
    assert_ne!(item.outcome, Outcome::Parametric, "{}", report.text());
    assert!(
        item.note
            .as_deref()
            .is_some_and(|n| n.contains("guide surfaces' geometry was not decoded")),
        "{}",
        report.text()
    );
    assert!(def.is_none());
}

/// A face the decoder found by the file's own names is resolved uniquely
/// when it is the only face of the replay through its point that faces
/// the way the file's does; a face without the file's names, or one that
/// faces the other way, is not.
#[test]
fn a_decoded_face_is_resolved_uniquely_only_with_its_names_and_side() {
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut doc = Document::new(TestKernel {
        face_listing: true,
        ..TestKernel::default()
    });
    import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    let top = doc.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "end")
        .cloned()
        .expect("a top face");
    let body = doc.bodies()[0].uid;
    let point = face_point(&top).map(|x| x / 10.0);
    let face = |extra: Value| {
        let mut map = json!({"kind": "face", "objectType": "BRepFace",
            "geometry": {"type": "Plane"}, "point_on_face": point,
            "normal_at_point": [1.0, 0.0, 0.0]});
        for (key, value) in extra.as_object().unwrap() {
            map[key] = value.clone();
        }
        match mitcad_f3d::design::ir::Reference::from_map(map.as_object().unwrap().clone()) {
            mitcad_f3d::design::ir::Reference::Face(fp) => *fp,
            other => panic!("{other:?}"),
        }
    };
    let named = json!({"_f3d": {"recipe": "bounded_face", "entities": [[{"tag": "9", "kind": 0, "ops": [5]}]]}});
    let fp = face(named.clone());
    let (found_body, found) = crate::refs::resolve_face(&doc, &fp).expect("the face");
    assert_eq!((found_body, &found), (body, &top));
    assert!(crate::refs::face_resolved_uniquely(&doc, &fp, (body, &top)));
    // Without the file's names nothing says the face is the file's.
    let unnamed = face(json!({}));
    assert!(!crate::refs::face_resolved_uniquely(
        &doc,
        &unnamed,
        (body, &top)
    ));
    // Facing the other way than the replay's face.
    let opposed = face(json!({"normal_at_point": [-1.0, 0.0, 0.0],
                              "_f3d": named["_f3d"].clone()}));
    assert!(!crate::refs::face_resolved_uniquely(
        &doc,
        &opposed,
        (body, &top)
    ));
}

#[test]
fn prisms_bound_the_change_of_extrusions() {
    let extrude = |extent: Value| Candidate {
        defs: vec![json!({"type": "extrude", "extent": extent, "operation": "cut"})],
        note: None,
        guess: true,
        predicted: Some(-100.0),
        input_provenance: crate::InputProvenance::Unknown,
        first: false,
    };
    assert!(bounded(&extrude(
        json!({"type": "distance", "distance": "d1"})
    )));
    let tapered = json!({"type": "two_sides", "side1": {"type": "distance", "taper": "d2"},
                         "side2": {"type": "distance"}});
    assert!(!bounded(&extrude(tapered)));
    assert!(!bounded(&extrude(
        json!({"type": "to_object", "object": {}})
    )));
    // A cut of at most 100 mm³ can remove 90 but not 120.
    assert!(can_change(-100.0, -90.0) && !can_change(-100.0, -120.0));
    assert!(can_change(-100.0, 50.0) && can_change(-100.0, 0.0));
}

#[test]
fn joins_that_remove_material_are_not_taken() {
    let sig = |volume: f64| Sig {
        volume,
        area: 10.0,
        center: [0.0; 3],
        faces: 6,
    };
    let feature = |operation: &str| {
        Candidate::new(json!({"type": "extrude", "operation": operation, "extent": {}}))
    };
    let (join, cut) = (feature("join"), feature("cut"));
    assert!(implausible(&join, &[sig(100.0)], &[sig(99.0)]));
    assert!(!implausible(&join, &[sig(100.0)], &[sig(101.0)]));
    assert!(implausible(&cut, &[sig(100.0)], &[sig(101.0)]));
    assert!(!implausible(&cut, &[sig(100.0)], &[sig(99.0)]));
    // Bodies joined into one hold their overlap once.
    assert!(!implausible(&join, &[sig(100.0), sig(50.0)], &[sig(140.0)]));
}

/// The smaller of the two rectangles' regions in Sketch1 (F1), 10 x 20 mm.
const SMALL: &str = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";

/// A document with Sketch1 of the two rectangles (F1) and the features
/// `defs` after it, made by the plain test kernel: the file's bodies.
fn reference(defs: Vec<Value>) -> Document<TestKernel> {
    let mut doc = Document::new(TestKernel::default());
    import_design(
        &mut doc,
        &timeline_dump(vec![two_rectangles(0)]),
        &mut NoGeometry,
        &Options::default(),
    );
    for def in defs {
        add(&mut doc, def);
    }
    doc
}

fn extrusion_of(region: &str) -> Value {
    json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": region}],
           "extent": {"type": "distance", "distance": 10}, "operation": "new_body"})
}

/// Extrude1 of [`SMALL`] imported on a kernel whose extrusions come out
/// 3e-4 larger than the file's (as regions next to the file's would), which
/// lists new faces with `lists_new_faces` (so it can tell that the larger
/// body's faces lie on the file's within the geometric tolerance).
fn approximate_extrusion(lists_new_faces: bool) -> (DesignReport, Document<TestKernel>) {
    let items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let file = reference(vec![extrusion_of(SMALL)]);
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), shapes(&file)],
        items: HashMap::from([(1, 1)]),
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel {
        skew: true,
        lists_new_faces,
        ..TestKernel::default()
    });
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    (report, doc)
}

#[test]
fn an_approximate_extrusion_keeps_its_body_within_the_tolerance() {
    // The closest extrusion is kept. Its body's faces lie on the file's
    // within the geometric tolerance: it was imported correctly and stays
    // (mitcad#285), no base feature replaces it.
    let (report, doc) = approximate_extrusion(true);
    let text = report.text();
    assert_eq!(report.items[1].outcome, Outcome::Parametric, "{text}");
    assert_eq!(report.items[1].verified, Some(true));
    assert!(
        !doc.features().any(|f| matches!(f.def, FeatureDef::Base(_))),
        "{text}"
    );
    assert!(
        !report
            .warnings
            .iter()
            .any(|w| w.contains("brings the bodies")),
        "{text}"
    );
    assert_eq!(report.bodies.len(), 1);
    assert!(
        report.bodies[0]
            .mitcad
            .as_deref()
            .is_some_and(|m| m.starts_with("F2.b0")),
        "{text}"
    );
}

#[test]
fn an_approximate_extrusion_does_not_carry_its_difference() {
    // Where the kernel cannot tell the extrusion's faces to lie on the
    // file's, its body differs: a base feature brings that body to the
    // file's state after it, so that the final bodies are the stored ones
    // exactly. (Before mitcad#285 this happened to a body within the
    // tolerance too, as above.)
    let (report, doc) = approximate_extrusion(false);
    assert_eq!(
        report.items[1].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert_eq!(report.items[1].verified, Some(true));
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    assert_eq!(def["def"]["profiles"][0]["region"], SMALL);
    // The base feature after it, and the stored body exactly.
    assert!(
        matches!(
            doc.feature(FeatureUid(3)).map(|f| &f.def),
            Some(FeatureDef::Base(_))
        ),
        "{}",
        report.text()
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("brings the bodies that differ to the file's state after Extrude1")),
        "{}",
        report.text()
    );
    assert_eq!(report.bodies.len(), 1);
    assert_eq!(report.bodies[0].volume_difference, Some(0.0));
    // A difference the bodies carry from an approximate item before is not
    // the item's own.
    assert!(introduced(3e-4, 0.0) && !introduced(3e-4, 3e-4) && introduced(6e-4, 3e-4));
    assert!(!introduced(5e-6, 0.0));
    // A closer extrusion takes an approximate one's place; a fillet only
    // when it comes closer by a quarter.
    let fillet = Candidate::new(json!({"type": "fillet", "body": "F2.b0", "sets": []}));
    let extrude = Candidate::new(json!({"type": "extrude"}));
    assert!(closer(&extrude, 4.0e-4, 4.4e-4) && !closer(&fillet, 4.0e-4, 4.4e-4));
    assert!(closer(&fillet, 5.2e-6, 1.0e-5) && !closer(&extrude, 5e-4, 4.4e-4));
}

#[test]
fn a_pattern_gets_the_extrusion_profiles_it_copies() {
    for learning in [false, true] {
        check_pattern_reselects_decoded_profiles(learning);
    }
}

fn check_pattern_reselects_decoded_profiles(learning: bool) {
    // Both rectangles extruded give the same body as far as the history
    // tells (the test kernel measures every body alike), but only the
    // file's region gives the pattern's copies: the extrusion takes the
    // first, and the pattern after it changes it to the file's.
    let mut items = decoded_small_rectangle();
    // The decoded loops select the large rectangle first. Without a
    // learning dump, deferring every other set used to discard the small
    // rectangle needed by the later pattern (mitcad#181).
    for (k, curve) in items[1]["detail"]["_f3d_profile_loops"][0]["loops"][0]["curves"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        curve["id"] = json!(format!("c{k}"));
        curve["primary"] = json!(k);
    }
    items.push(
        json!({"index": 2, "name": "C-Pattern1", "objectType": "CircularPatternFeature",
                      "detail": {"quantity": param("d6", "2", 2.0, "")}}),
    );
    let file = reference(vec![
        extrusion_of(SMALL),
        json!({"type": "circular_pattern", "objects": {"type": "features", "features": ["F2"]},
               "axis": "z", "quantity": 2, "angle": std::f64::consts::TAU, "symmetric": false,
               "compute": "adjust"}),
    ]);
    let extruded = file.bodies()[..1].iter().map(|b| b.shape.clone()).collect();
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), extruded, shapes(&file)],
        items: HashMap::from([(1, 1), (2, 2)]),
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel {
        alike: true,
        ..TestKernel::default()
    });
    let dir = std::env::temp_dir().join(format!("mitcad-learn-revised-{}", std::process::id()));
    let options = Options {
        learn: learning.then(|| crate::learn::LearnOptions {
            dir: dir.clone(),
            path: None,
        }),
        ..Options::default()
    };
    let report = import_design(&mut doc, &timeline_dump(items), &mut geometry, &options);
    let settled = learning.then(|| {
        let settled = std::fs::read_dir(dir.join("settled"))
            .unwrap()
            .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
            .collect::<String>();
        let _ = std::fs::remove_dir_all(&dir);
        settled
    });
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [
            Outcome::Parametric,
            Outcome::Parametric,
            Outcome::Parametric
        ],
        "{}",
        report.text()
    );
    assert_eq!(report.items[2].verified, Some(true));
    assert_eq!(
        report.items[1].input_provenance,
        Some(InputProvenance::Inferred)
    );
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    assert_eq!(def["def"]["profiles"][0]["region"], SMALL);
    // (The large rectangle was the first guess.)
    assert!(
        report.items[1]
            .note
            .as_deref()
            .unwrap_or_default()
            .contains("chosen for C-Pattern1"),
        "{}",
        report.text()
    );
    // The learning answer follows the final extrusion, not the initially
    // accepted region that the pattern made it replace.
    let Some(settled) = settled else { return };
    let lines: Vec<Value> = settled
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let line = lines.iter().find(|line| line["item"] == 1).unwrap();
    assert_eq!(line["answer"]["input_provenance"], "inferred");
    assert_eq!(line["status"], "settled");
    let profile = &line["answer"]["defs"][0]["profiles"][0];
    assert_eq!(profile["sketch"], "T0");
    let region = profile["region"].as_u64().unwrap() as usize;
    let mut outer: Vec<&str> = line["context"]["sketches"]["T0"]["regions"][region]["outer"]
        .as_array()
        .unwrap()
        .iter()
        .map(|curve| curve.as_str().unwrap())
        .collect();
    outer.sort_unstable();
    assert_eq!(outer, ["c4", "c5", "c6", "c7"]);
}

#[test]
fn a_rectangular_pattern_without_directions_finds_a_hexagonal_lattice() {
    // The oldest item version stores no directions (mitcad#74): the file's
    // pattern repeats the extrusion along x and at 60° to it, on both sides.
    let lattice = json!({"type": "rectangular_pattern",
        "objects": {"type": "features", "features": ["F2"]},
        "direction1": {"axis": "x", "quantity": 2, "distance": 30, "symmetric": true},
        "direction2": {"axis": {"origin": [0.0, 0.0, 0.0],
                                "direction": [0.5, 3f64.sqrt() / 2.0, 0.0]},
                       "quantity": 2, "distance": 30, "symmetric": true},
        "distance_type": "spacing", "original_bodies": true});
    let file = reference(vec![extrusion_of(SMALL), lattice]);
    let extruded = file.bodies()[..1].iter().map(|b| b.shape.clone()).collect();
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), extruded, shapes(&file)],
        items: HashMap::from([(1, 1), (2, 2)]),
        ..TestGeometry::default()
    };
    let distance = param("d9", "30 mm", 3.0, "mm");
    let items = vec![
        two_rectangles(0),
        json!({"index": 1, "name": "Extrude1", "objectType": "ExtrudeFeature",
               "detail": {"operation": "NewBodyFeatureOperation",
                          "profile": [{"kind": "profile", "sketch": "Sketch1",
                                       "sketch_timeline_index": 0}],
                          "extentType": "OneSideFeatureExtentType",
                          "extentOne": {"_type": "DistanceExtentDefinition",
                                        "distance": param("d2", "10 mm", 1.0, "mm")}}}),
        json!({"index": 2, "name": "R-Pattern1", "objectType": "RectangularPatternFeature",
               "detail": {"inputEntities": [{"kind": "feature", "objectType": "ExtrudeFeature",
                                             "name": "Extrude1", "timeline_index": 1}],
                          "patternEntityType": "FeaturesPatternType",
                          "quantityOne": param("d7", "2", 2.0, ""),
                          "quantityTwo": param("d8", "2", 2.0, ""),
                          "distanceOne": distance, "distanceTwo": distance}}),
    ];
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F3"}"#).unwrap()).unwrap();
    let def = &def["def"];
    assert_eq!(def["direction1"]["symmetric"], json!(true), "{def}");
    let across = def["direction2"]["axis"]["direction"][1]
        .as_f64()
        .unwrap_or_default();
    assert!((across - 3f64.sqrt() / 2.0).abs() < 1e-9, "{def}");
    assert_eq!(def["original_bodies"], json!(true), "{def}");
}

#[test]
fn later_definitions_refer_to_earlier_features() {
    let uids: Vec<FeatureUid> = vec!["F7".parse().unwrap()];
    let mut def = json!({"type": "extrude", "profiles": [{"sketch": "$0", "region": "r{c1}"}],
                         "note": "$1", "other": "$x"});
    substitute(&mut def, &uids);
    assert_eq!(def["profiles"][0]["sketch"], "F7");
    // Unknown indices and other strings stay.
    assert_eq!(def["note"], "$1");
    assert_eq!(def["other"], "$x");
}

#[test]
fn sketch_texts_keep_style_frame_and_path() {
    // A text in a frame of four construction lines turned 90 degrees, and a
    // text fitted on an arc below it (cm in the IR).
    let p = |id: &str, x: f64, y: f64| json!({"id": id, "xyz": [x, y, 0.0]});
    let line = |id: &str, a: &str, b: &str| {
        json!({"id": id, "type": "SketchLine", "startSketchPoint": a, "endSketchPoint": b,
               "isConstruction": true})
    };
    let sketch = json!({
        "index": 0, "name": "Sketch1", "objectType": "Sketch",
        "detail": {
            "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
            "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
            "points": [p("p0", 0.0, 0.0), p("p1", 0.0, 4.0), p("p2", -2.0, 4.0), p("p3", -2.0, 0.0),
                       p("p4", 5.0, 0.0), p("p5", 7.0, 0.0), p("p6", 3.0, 0.0)],
            "curves": [line("c0", "p0", "p1"), line("c1", "p1", "p2"),
                       line("c2", "p2", "p3"), line("c3", "p3", "p0"),
                       {"id": "c4", "type": "SketchArc", "centerSketchPoint": "p4",
                        "startSketchPoint": "p5", "endSketchPoint": "p6"}],
            "texts": [
                {"id": "t0", "text": "AB", "height": 0.5, "position": [0.0, 0.0, 0.0],
                 "angle": std::f64::consts::FRAC_PI_2, "fontName": "Arial", "textStyle": 3,
                 "isHorizontalFlip": true,
                 "definition": {"_type": "MultiLineTextDefinition",
                                "rectangleLines": [{"kind": "sketch_entity", "id": "c0"},
                                                   {"kind": "sketch_entity", "id": "c1"},
                                                   {"kind": "sketch_entity", "id": "c2"},
                                                   {"kind": "sketch_entity", "id": "c3"}],
                                "horizontalAlignment": 1, "verticalAlignment": "MiddleVerticalAlignment",
                                "characterSpacing": 10.0}},
                {"id": "t1", "text": "C", "height": 0.4, "position": [5.0, 2.0, 0.0],
                 "definition": {"_type": "FitOnPathTextDefinition",
                                "path": {"kind": "sketch_entity", "id": "c4"}, "isAbovePath": false}}
            ]
        }
    });
    let mut doc = Document::new(MockKernel::default());
    import(&mut doc, &timeline_dump(vec![sketch]));
    let s: Value =
        serde_json::from_str(&doc.query(r#"{"query": "sketch", "uid": "F1"}"#).unwrap()).unwrap();
    let t = &s["texts"][0];
    assert_eq!(
        (t["bold"].clone(), t["italic"].clone()),
        (json!(true), json!(true))
    );
    assert_eq!(t["flip_x"], true);
    assert_eq!(t["height"], 5.0);
    assert_eq!(
        (t["align"].clone(), t["valign"].clone()),
        (json!("center"), json!("middle"))
    );
    assert_eq!(t["spacing"], 10.0);
    // Turned a quarter: the baseline runs up from (0, 0) along x = 0, and
    // the text's up is -x.
    let id = |stored: &str| {
        s["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| {
                let at = &e["at"];
                e["type"] == "point"
                    && match stored {
                        "p0" => at == &json!([0.0, 0.0]),
                        "p1" => at == &json!([0.0, 40.0]),
                        _ => at == &json!([-20.0, 0.0]),
                    }
            })
            .unwrap()["id"]
            .clone()
    };
    assert_eq!(t["frame"], json!([id("p0"), id("p1"), id("p3")]));
    let path = &s["texts"][1]["path"];
    assert_eq!(
        (path["above"].clone(), path["fit"].clone()),
        (json!(false), json!(true))
    );
    assert!(path["curve"].as_str().unwrap().starts_with('c'));
    assert!(
        s["regions"].as_array().unwrap().len() > 2,
        "the letters bound regions"
    );
}

#[test]
fn a_text_in_a_box_needs_no_position_of_its_own() {
    // The file's box: its first line runs along the text's baseline (here
    // towards -x, a half turn) from the text's corner; the API gives the
    // text no position and angle. A text without a box has no place.
    let p = |id: &str, x: f64, y: f64| json!({"id": id, "xyz": [x, y, 0.0]});
    let line = |id: &str, a: &str, b: &str| {
        json!({"id": id, "type": "SketchLine", "startSketchPoint": a, "endSketchPoint": b,
               "isConstruction": true})
    };
    let sketch = json!({
        "index": 0, "name": "Sketch1", "objectType": "Sketch",
        "detail": {
            "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
            "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
            "points": [p("p0", 4.0, 2.0), p("p1", 3.0, 2.0), p("p2", 3.0, 1.0), p("p3", 4.0, 1.0)],
            "curves": [line("c0", "p0", "p1"), line("c1", "p1", "p2"),
                       line("c2", "p2", "p3"), line("c3", "p3", "p0")],
            "texts": [
                {"id": "t0", "text": "A B", "height": 0.3, "fontName": "Arial",
                 "lineSpacing": 0.25, "penAligned": true, "inkTop": true,
                 "glyphs": [
                     {"advance": 0.2, "contours": [[[[0.0, 0.0], [0.15, 0.0]],
                                                    [[0.15, 0.0], [0.075, 0.21]],
                                                    [[0.075, 0.21], [0.0, 0.0]]]]},
                     {"advance": 0.0},
                     {"advance": 0.2, "contours": [[[[0.0, 0.0], [0.15, 0.0]],
                                                    [[0.15, 0.0], [0.15, 0.21]],
                                                    [[0.15, 0.21], [0.0, 0.0]]]]}
                 ],
                 "definition": {"_type": "MultiLineTextDefinition",
                                "rectangleLines": [{"kind": "sketch_entity", "id": "c0"},
                                                   {"kind": "sketch_entity", "id": "c1"},
                                                   {"kind": "sketch_entity", "id": "c2"},
                                                   {"kind": "sketch_entity", "id": "c3"}],
                                "horizontalAlignment": "LeftHorizontalAlignment",
                                "verticalAlignment": "TopVerticalAlignment"}},
                {"id": "t1", "text": "X", "height": 0.3,
                 "definition": {"_type": "MultiLineTextDefinition"}}
            ]
        }
    });
    let mut doc = Document::new(MockKernel::default());
    import(&mut doc, &timeline_dump(vec![sketch]));
    let s: Value =
        serde_json::from_str(&doc.query(r#"{"query": "sketch", "uid": "F1"}"#).unwrap()).unwrap();
    let texts = s["texts"].as_array().unwrap();
    assert_eq!(texts.len(), 1, "{s}");
    let t = &texts[0];
    assert_eq!(
        (t["cap_height"].clone(), t["wrap"].clone()),
        (json!(true), json!(true))
    );
    // The `.ipt` decoder's measures (mitcad#414).
    assert_eq!(
        (
            t["line_spacing"].clone(),
            t["pen_align"].clone(),
            t["ink_top"].clone()
        ),
        (json!(2.5), json!(true), json!(true))
    );
    // The glyphs the file stores (mitcad#416) are the shapes while the font
    // is missing (the mock kernel has no Arial): the A a triangle 1.5 mm
    // wide and 2.1 mm high.
    assert_eq!(
        (
            t["has_glyphs"].clone(),
            t["stored_outlines"].clone(),
            t["family"].clone()
        ),
        (json!(true), json!(true), json!("Arial"))
    );
    let id = t["id"].as_str().unwrap();
    let area = s["regions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["key"] == format!("r{{{id}.g0.c0}}"))
        .unwrap_or_else(|| panic!("{}", s["regions"]))["area"]
        .as_f64()
        .unwrap();
    assert!((area - 1.575).abs() < 1e-9, "{area}");
    assert!((t["angle"].as_f64().unwrap() - std::f64::consts::PI).abs() < 1e-9);
    let at = |x: f64, y: f64| {
        s["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["type"] == "point" && e["at"] == json!([x, y]))
            .unwrap()["id"]
            .clone()
    };
    // The corner, the corner along the baseline and the one up (-y).
    assert_eq!(
        t["frame"],
        json!([at(40.0, 20.0), at(30.0, 20.0), at(40.0, 10.0)])
    );
}

// Lofts with end conditions and rails (P2).

#[test]
fn lofts_with_end_conditions_and_rails_come_in_as_definitions() {
    // Sketch1: the rectangle; Sketch2: a line, the rail.
    let path_sketch = json!({
        "index": 1, "name": "Sketch2", "objectType": "Sketch",
        "detail": {
            "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
            "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
            "points": [{"id": "p0", "xyz": [0.0, 0.0, 0.0]}, {"id": "p1", "xyz": [0.0, 5.0, 0.0]}],
            "curves": [{"id": "c0", "type": "SketchLine", "startSketchPoint": "p0", "endSketchPoint": "p1"}]
        }
    });
    let line = json!({"kind": "sketch_entity", "objectType": "SketchLine", "sketch": "Sketch2",
                      "sketch_timeline_index": 1, "id": "c0"});
    let profile = json!({"kind": "profile", "sketch": "Sketch1", "sketch_timeline_index": 0,
                         "area": 8.0, "centroid": [2.0, 1.0, 0.0]});
    let loft = json!({"index": 2, "name": "Loft1", "objectType": "LoftFeature", "detail": {
        "loftSections": [
            {"_type": "LoftSection", "index": 0, "entity": profile,
             "endCondition": {"_type": "DirectionEndCondition",
                              "angle": param("d20", "10 deg", 0.17453292519943295, "deg"),
                              "weight": param("d21", "2", 2.0, "")}},
            {"_type": "LoftSection", "index": 1, "entity": {"kind": "construction_point", "name": "Origin",
             "origin": "Origin"},
             "endCondition": {"_type": "PointTangentEndCondition", "weight": param("d22", "0.5", 0.5, "")}}],
        "centerLineOrRails": [[{"_type": "PathEntity", "entity": line}]],
        "isClosed": false, "isSolid": true, "operation": "NewBodyFeatureOperation"}});
    let dump = dump(json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "lofts.f3d"},
        "timeline": {"items": [rectangle_sketch(0), path_sketch, loft]}
    }));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let query = json!({"query": "feature", "uid": "F3"}).to_string();
    let answer: Value = serde_json::from_str(&doc.query(&query).unwrap()).unwrap();
    let def = &answer["def"];
    assert_eq!(def["start_condition"]["type"], "direction");
    assert_eq!(def["end_condition"]["type"], "point_tangent");
    assert_eq!(def["rails"][0]["sketch"], "F2");
    let value = |slot: &Value| {
        let parameters = doc.parameters();
        parameters
            .get(parameters.find(slot.as_str().unwrap()).unwrap())
            .map(|p| p.value())
    };
    assert_eq!(value(&def["start_condition"]["weight"]), Some(2.0));
    assert_eq!(value(&def["end_condition"]["weight"]), Some(0.5));
    // Mitcad's own rules for end conditions: matched against
    // the history more loosely.
    assert_eq!(
        Candidate::new(def.clone()).tolerance(),
        history::CONVENTIONS
    );
    let mut free = def.clone();
    for key in ["start_condition", "end_condition", "rails"] {
        free.as_object_mut().unwrap().remove(key);
    }
    assert_eq!(Candidate::new(free).tolerance(), history::RELATIVE);
}

#[test]
fn closed_and_surface_lofts_come_in_as_definitions() {
    let section = |i: i64| {
        json!({"_type": "LoftSection", "index": i,
               "entity": {"kind": "profile", "sketch": format!("Sketch{}", i + 1),
                          "sketch_timeline_index": i, "area": 8.0, "centroid": [2.0, 1.0, 0.0]}})
    };
    for (closed, solid) in [(true, true), (false, false)] {
        let loft = json!({"index": 3, "name": "Loft1", "objectType": "LoftFeature", "detail": {
            "loftSections": [section(0), section(1), section(2)],
            "isClosed": closed, "isSolid": solid, "operation": "NewBodyFeatureOperation"}});
        let dump = dump(json!({
            "schema": "mitcad-f3d-dump", "schema_version": 2,
            "source": {"mode": "f3d_stream", "file": "lofts.f3d"},
            "timeline": {"items": [rectangle_sketch(0), rectangle_sketch(1), rectangle_sketch(2), loft]}
        }));
        let mut doc = Document::new(MockKernel::default());
        let report = import(&mut doc, &dump);
        assert_eq!(
            report.items[3].outcome,
            Outcome::Parametric,
            "{}",
            report.text()
        );
        let query = json!({"query": "feature", "uid": "F4"}).to_string();
        let answer: Value = serde_json::from_str(&doc.query(&query).unwrap()).unwrap();
        let def = &answer["def"];
        assert_eq!(def["closed"] == json!(true), closed, "{def}");
        assert_eq!(def["surface"] == json!(true), !solid, "{def}");
    }
}

#[test]
fn a_loft_section_of_edges_is_the_face_they_go_round() {
    // A tangent condition is taken only at a section of edges: here the
    // loop round the block's top, as a Path of its BRepEdges (cm).
    let block_items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(block_items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let block = shapes(&reference)[0].clone();
    let top = block
        .faces
        .iter()
        .find(|f| f.to_string().starts_with("F2:end("))
        .unwrap()
        .clone();
    let edge = |a: &mitcad_model::FaceName, b: &mitcad_model::FaceName| {
        let p = edge_point(a, b);
        json!({"_type": "PathEntity", "entity": {"kind": "edge", "objectType": "BRepEdge",
               "mid_point": [p[0] / 10.0, p[1] / 10.0, p[2] / 10.0], "length": 0.1}})
    };
    let round_top: Vec<Value> = block
        .edges
        .iter()
        .filter(|(a, b)| *a == top || *b == top)
        .map(|(a, b)| edge(a, b))
        .collect();
    assert_eq!(round_top.len(), 4);
    let loft = |entity: Value| {
        json!({"index": 2, "name": "Loft1", "objectType": "LoftFeature", "detail": {
            "loftSections": [
                {"_type": "LoftSection", "index": 0, "entity": entity,
                 "endCondition": {"_type": "LoftTangentEndCondition", "weight": param("d9", "1", 1.0, "")}},
                {"_type": "LoftSection", "index": 1, "entity": {"kind": "construction_point",
                 "name": "Origin", "origin": "Origin"},
                 "endCondition": {"_type": "LoftPointSharpEndCondition"}}],
            "isClosed": false, "isSolid": true, "operation": "NewBodyFeatureOperation"}})
    };
    let replay = |entity: Value| {
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        let mut items = block_items.clone();
        items.push(loft(entity));
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        );
        let def = report.items[2].features.first().map(|uid| {
            serde_json::from_str::<Value>(
                &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                    .unwrap(),
            )
            .unwrap()["def"]
                .clone()
        });
        (report, def)
    };
    let (report, def) = replay(json!(round_top));
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def = def.unwrap();
    assert_eq!(
        def["sections"][0],
        json!({"type": "face", "body": "F2.b0", "face": top.to_string()})
    );
    // The dump's type names (`LoftTangentEndCondition`).
    assert_eq!(def["start_condition"]["type"], "tangent");
    assert_eq!(def["end_condition"], json!({"type": "point_sharp"}));
    // One edge of the loop (a BRepEdge as the section) goes round no face.
    let (report, _) = replay(round_top[0]["entity"].clone());
    assert_ne!(report.items[2].outcome, Outcome::Parametric);
    assert!(
        report.text().contains("its edges do not go round one face"),
        "{}",
        report.text()
    );
}

#[test]
fn edges_the_decoder_found_by_their_names_come_first() {
    let block_items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(block_items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (a, b) = shapes(&reference)[0].edges[0].clone();
    let p = edge_point(&a, &b);
    let names = json!({"recipe": "edge", "entities": [[{"tag": "3", "kind": 0, "ops": [301]}],
                       [{"tag": "4", "kind": 0, "ops": [301]}]]});
    let replay = |edge: Value| {
        let mut items = block_items.clone();
        let set = json!({"radius": param("d5", "2 mm", 0.2, "mm"), "edges": [edge]});
        let fillet = json!({"index": 2, "name": "Fillet1", "objectType": "FilletFeature",
                            "detail": {"edgeSets": [set]}});
        items.push(fillet);
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        )
    };
    // Found in the history: the decoder wrote its geometry (cm).
    let report = replay(json!({"kind": "edge", "length": 0.1,
        "mid_point": [p[0] / 10.0, p[1] / 10.0, p[2] / 10.0],
        "_f3d": {"found": "BREP.x.smbh before state 4", "recipe": "edge", "entities": names["entities"]}}));
    let item = &report.items[2];
    assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(
        item.note.as_deref(),
        Some("edges found by their names in the file"),
        "{}",
        report.text()
    );
    // Not found: the edges are left to the history (here there is none).
    let report = replay(json!({"kind": "edge",
        "_f3d": {"found": "a face of the edge is not in the history state", "recipe": "edge",
                 "entities": names["entities"]}}));
    let item = &report.items[2];
    assert_ne!(item.outcome, Outcome::Parametric);
    assert!(
        report.text().contains("no history to find them"),
        "{}",
        report.text()
    );
    assert!(
        report
            .text()
            .contains("edge set 0, input 0: decoder_unresolved")
    );
    assert!(
        report
            .text()
            .contains("a face of the edge is not in the history state")
    );
    // The decoder succeeded, but the replay does not have that edge:
    // keep this separate from a recipe the decoder could not resolve.
    let report = replay(json!({"kind": "edge", "length": 0.1,
        "mid_point": [1000.0, 1000.0, 1000.0],
        "_f3d": {"found": "BREP.x.smbh before state 4", "recipe": "edge", "entities": names["entities"]}}));
    assert!(
        report
            .text()
            .contains("edge set 0, input 0: missing_in_replay")
    );
    assert!(report.text().contains("BREP.x.smbh before state 4"));
}

/// An edge the file names whose faces its history no longer joins (the
/// decoder: "no edge between the faces named") is no input of the file's
/// operation: the others come first, noted as left out and not read; an
/// edge the decoder did not resolve for another reason is not left out.
#[test]
fn edges_the_file_no_longer_has_are_left_out() {
    let block_items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(block_items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (a, b) = shapes(&reference)[0].edges[0].clone();
    let p = edge_point(&a, &b);
    let names = json!([[{"tag": "3", "kind": 0, "ops": [301]}],
                       [{"tag": "4", "kind": 0, "ops": [301]}]]);
    let found = json!({"kind": "edge", "length": 0.1,
        "mid_point": [p[0] / 10.0, p[1] / 10.0, p[2] / 10.0],
        "_f3d": {"object_id": 7, "found": "BREP.x.smbh before state 4", "recipe": "edge",
                 "entities": names}});
    let replay = |why: &str| {
        let lost = json!({"kind": "edge",
            "_f3d": {"object_id": 8, "found": why, "recipe": "edge", "entities": names}});
        let mut items = block_items.clone();
        let set = json!({"radius": param("d5", "2 mm", 0.2, "mm"), "edges": [found, lost]});
        items.push(
            json!({"index": 2, "name": "Fillet1", "objectType": "FilletFeature",
                          "detail": {"edgeSets": [set]}}),
        );
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        )
    };
    for why in [
        "no edge between the faces named",
        "a face of the edge is not in the history state",
    ] {
        let report = replay(why);
        let item = &report.items[2];
        assert_eq!(
            item.outcome,
            Outcome::Parametric,
            "{why}: {}",
            report.text()
        );
        assert_eq!(
            item.note.as_deref(),
            Some("edges found by their names in the file; 1 edge the file no longer has left out"),
            "{why}"
        );
        assert_eq!(item.input_provenance, Some(InputProvenance::Inferred));
    }
    // Another reason (the names fit several edges): the set is not
    // reduced.
    let report = replay("2 edges fit the names");
    assert_ne!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
}

/// The edges of a face a chamfer's set selects (the decoder turned the face
/// into its edges: a `bounded_face` recipe found "an edge of a face") are
/// named by the file as the edges of an edge recipe are: the chamfer's
/// inputs are read; an edge of that recipe the decoder did not take from a
/// face is not.
#[test]
fn a_chamfer_of_a_faces_edges_is_read() {
    let block_items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(block_items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (a, b) = shapes(&reference)[0].edges[0].clone();
    let p = edge_point(&a, &b);
    let entities = json!([[{"tag": "3", "kind": 0, "ops": [301]}],
                          [{"tag": "4", "kind": 0, "ops": [301]}]]);
    let provenance = |recipe: &str, found: &str| {
        let edge = json!({"kind": "edge", "length": 0.1,
            "mid_point": [p[0] / 10.0, p[1] / 10.0, p[2] / 10.0],
            "_f3d": {"object_id": 7, "found": found, "recipe": recipe,
                     "entities": entities}});
        let mut items = block_items.clone();
        items.push(
            json!({"index": 2, "name": "Chamfer1", "objectType": "ChamferFeature",
            "detail": {"chamferType": "EqualDistanceChamferType",
                       "cornerType": "ChamferCornerType",
                       "edgeSets": [{"edges": [edge], "isTangentChain": true,
                                     "distance": param("d5", "2 mm", 0.2, "mm")}]}}),
        );
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        );
        assert_eq!(
            report.items[2].outcome,
            Outcome::Parametric,
            "{}",
            report.text()
        );
        report.items[2].input_provenance
    };
    let state = "BREP.x.smbh before state 4";
    assert_eq!(provenance("edge", state), Some(InputProvenance::Read));
    assert_eq!(
        provenance("bounded_face", &format!("{state} (an edge of a face)")),
        Some(InputProvenance::Read)
    );
    assert_ne!(
        provenance("bounded_face", state),
        Some(InputProvenance::Read)
    );
}

#[test]
fn fillet_and_chamfer_options_come_in_as_their_definitions() {
    // The block's first edge, fingerprinted as external dumps do it (cm).
    let block_items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(block_items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (a, b) = shapes(&reference)[0].edges[0].clone();
    let p = edge_point(&a, &b);
    let edge = json!({"kind": "edge", "mid_point": [p[0] / 10.0, p[1] / 10.0, p[2] / 10.0],
                      "length": 0.1});
    let fillet = |set: Value| {
        let mut set = set;
        set["edges"] = json!([edge]);
        json!({"index": 2, "name": "Fillet1", "objectType": "FilletFeature",
               "detail": {"edgeSets": [set]}})
    };
    let radius = param("d5", "2 mm", 0.2, "mm");
    let three = param("d6", "3 mm", 0.3, "mm");
    for item in [
        fillet(
            json!({"radius": radius, "continuity": "CurvatureSurfaceContinuityType",
                      "tangencyWeight": param("d7", "1.5", 1.5, "")}),
        ),
        fillet(
            json!({"_type": "VariableRadiusFilletEdgeSet", "startRadius": radius,
                      "endRadius": radius, "midRadii": [three], "midPositions": [0.25]}),
        ),
        fillet(
            json!({"_type": "AsymmetricFilletEdgeSet", "offsetOne": radius,
                      "offsetTwo": three, "isFlipped": true}),
        ),
        json!({"index": 2, "name": "Chamfer1", "objectType": "ChamferFeature",
               "detail": {"edgeSets": [{"edges": [edge], "distance": radius}],
                          "cornerType": "MiterCornerType"}}),
    ] {
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        let mut items = block_items.clone();
        items.push(item);
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        );
        let outcome = &report.items[2];
        assert_eq!(outcome.outcome, Outcome::Parametric, "{}", report.text());
        let def: Value = serde_json::from_str::<Value>(
            &doc.query(&format!(
                r#"{{"query": "feature", "uid": "{}"}}"#,
                outcome.features[0]
            ))
            .unwrap(),
        )
        .unwrap()["def"]
            .clone();
        let value = |slot: &Value| {
            let parameters = doc.parameters();
            match slot.as_str() {
                Some(name) => parameters
                    .get(parameters.find(name).unwrap())
                    .map(|p| p.value()),
                None => slot.as_f64(),
            }
        };
        let set = &def["sets"][0];
        match def["type"].as_str().unwrap() {
            "chamfer" => assert_eq!(def["corner"], "miter"),
            _ if set["continuity"] == "curvature" => {
                assert_eq!(value(&set["tangency_weight"]), Some(1.5));
            }
            _ if set["size"]["type"] == "variable" => {
                assert_eq!(set["size"]["mid"][0]["position"], 0.25);
                assert_eq!(value(&set["size"]["mid"][0]["radius"]), Some(3.0));
            }
            _ => {
                assert_eq!(set["size"]["type"], "asymmetric");
                assert_eq!(value(&set["size"]["distance1"]), Some(2.0));
                assert_eq!(value(&set["size"]["distance2"]), Some(3.0));
                assert_eq!(set["size"]["flip"], true);
            }
        }
        // Mitcad's own shapes: matched against the history more loosely.
        assert_eq!(
            Candidate::new(def.clone()).tolerance(),
            history::CONVENTIONS
        );
    }
    let plain = json!({"type": "fillet", "sets": [{"size": {"type": "constant", "radius": 2}}]});
    assert_eq!(Candidate::new(plain).tolerance(), history::RELATIVE);
}

/// A distance and angle chamfer whose face the file does not say (no
/// `isFlipped`, as the `.ipt` import reads it): the distance may lie on
/// either face of the edge, and the history state chooses (mitcad#235).
#[test]
fn a_distance_and_angle_chamfers_face_comes_from_the_history() {
    let block_items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut block = Document::new(TestKernel::default());
    import_design(
        &mut block,
        &timeline_dump(block_items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (a, b) = shapes(&block)[0].edges[0].clone();
    let p = edge_point(&a, &b);
    let edge = json!({"kind": "edge", "mid_point": [p[0] / 10.0, p[1] / 10.0, p[2] / 10.0],
                      "length": 0.1});
    let mut items = block_items.clone();
    items.push(
        json!({"index": 2, "name": "Chamfer1", "objectType": "ChamferFeature",
        "detail": {"edgeSets": [{"_type": "DistanceAndAngleChamferEdgeSet", "edges": [edge],
                                 "distance": param("d5", "2 mm", 0.2, "mm"),
                                 "angle": param("d6", "15 deg", 0.2617993877991494, "rad")}]}}),
    );
    let full = timeline_dump(items);
    let import = |geometry: &mut dyn StoredGeometry<MockShape>| {
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        let report = import_design(&mut doc, &full, geometry, &Options::default());
        let uid = report.items[2].features[0].clone();
        let def: Value = serde_json::from_str::<Value>(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                .unwrap(),
        )
        .unwrap()["def"]
            .clone();
        (doc, report, def)
    };
    // Without the history: the distance on the edge's first face.
    let (mut reference, report, def) = import(&mut NoGeometry);
    assert_eq!(report.items[2].outcome, Outcome::Parametric);
    assert_ne!(def["flip"], true, "{def}");
    let before = shapes(&reference);
    // The file's state: the distance on the other face.
    let mut flipped = def.clone();
    flipped["flip"] = json!(true);
    let uid: mitcad_model::FeatureUid = report.items[2].features[0].parse().unwrap();
    reference
        .edit_feature(uid, &serde_json::from_value(flipped).unwrap())
        .unwrap();
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), shapes(&block), shapes(&reference)],
        items: HashMap::from([(1, 1), (2, 2)]),
        ..TestGeometry::default()
    };
    assert_ne!(before, shapes(&reference));
    let (_, report, def) = import(&mut geometry);
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert_eq!(report.items[2].verified, Some(true), "{}", report.text());
    assert_eq!(def["flip"], true, "{def}");
}

#[test]
fn patterns_of_features_with_a_chamfer_repeat_it_on_the_copies() {
    // The block with a chamfer on its first edge, and a circular pattern
    // of the extrusion and the chamfer (mitcad#105).
    let block_items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(block_items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (a, b) = shapes(&reference)[0].edges[0].clone();
    let p = edge_point(&a, &b);
    let edge = json!({"kind": "edge", "mid_point": [p[0] / 10.0, p[1] / 10.0, p[2] / 10.0],
                      "length": 0.1});
    let mut items = block_items;
    items.push(
        json!({"index": 2, "name": "Chamfer1", "objectType": "ChamferFeature",
        "detail": {"edgeSets": [{"edges": [edge], "distance": param("d5", "2 mm", 0.2, "mm")}]}}),
    );
    items.push(
        json!({"index": 3, "name": "C-Pattern1", "objectType": "CircularPatternFeature",
        "detail": {"inputEntities": [
                       {"kind": "feature", "objectType": "ExtrudeFeature", "name": "Extrude1",
                        "timeline_index": 1},
                       {"kind": "feature", "objectType": "ChamferFeature", "name": "Chamfer1",
                        "timeline_index": 2}],
                   "patternEntityType": "FeaturesPatternType",
                   "axis": {"kind": "construction_axis", "name": "Z", "origin": "Z"},
                   "quantity": param("d6", "2", 2.0, ""),
                   "totalAngle": param("d7", "360 deg", std::f64::consts::TAU, "deg"),
                   "isSymmetric": false}}),
    );
    let mut dump = timeline_dump(items);
    for (name, expression, unit) in [("d6", "2", ""), ("d7", "360 deg", "deg")] {
        dump.parameters
            .as_mut()
            .unwrap()
            .model
            .as_mut()
            .unwrap()
            .push(
                serde_json::from_value(
                    json!({"name": name, "expression": expression, "unit": unit}),
                )
                .unwrap(),
            );
    }
    let mut doc = Document::new(TestKernel {
        edge_points: true,
        ..TestKernel::default()
    });
    let report = import_design(&mut doc, &dump, &mut NoGeometry, &Options::default());
    let pattern = &report.items[3];
    assert_eq!(pattern.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(
        pattern.note.as_deref(),
        Some("its fillets and chamfers repeated on the copies")
    );
    let chamfer = &report.items[2].features[0];
    let def: Value = serde_json::from_str::<Value>(
        &doc.query(&format!(
            r#"{{"query": "feature", "uid": "{}"}}"#,
            pattern.features[0]
        ))
        .unwrap(),
    )
    .unwrap()["def"]
        .clone();
    // The chamfer among the objects (the first definition tried).
    assert_eq!(
        def["objects"],
        json!({"type": "features", "features": ["F2", chamfer]})
    );
    // The copy of the block got the chamfer's copy, named after the pattern.
    let prefix = format!("{}:chamfer(", pattern.features[0]);
    assert!(
        shapes(&doc)
            .iter()
            .any(|s| s.faces.iter().any(|f| f.to_string().starts_with(&prefix))),
        "{:?}",
        shapes(&doc)
    );
}

/// A replace face of the block's top onto the XY plane, as external dumps give
/// it (no source faces), and the history with the file's result.
fn replace_face_history() -> (Vec<Value>, TestGeometry, String) {
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 2, "name": "ReplaceFace1", "objectType": "ReplaceFaceFeature",
               "detail": {"targetFaces": [{"kind": "construction_plane", "name": "XY", "origin": "XY"}],
                          "isTangentChain": true}}),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items[..2].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let block = shapes(&reference);
    let top = block[0]
        .faces
        .iter()
        .find(|f| f.to_string().starts_with("F2:end("))
        .expect("the block's top")
        .to_string();
    add(
        &mut reference,
        json!({"type": "replace_face", "body": "F2.b0", "faces": [top], "target": "xy",
               "tangent_chain": false}),
    );
    let geometry = TestGeometry {
        states: vec![Vec::new(), block, shapes(&reference)],
        items: HashMap::from([(1, 1), (2, 2)]),
        ..TestGeometry::default()
    };
    (items, geometry, top)
}

#[test]
fn replace_faces_come_in_with_the_faces_the_history_lost() {
    let (items, mut geometry, top) = replace_face_history();
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items.clone()),
        &mut geometry,
        &Options::default(),
    );
    let item = &report.items[2];
    assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(item.verified, Some(true));
    let def: Value = serde_json::from_str::<Value>(
        &doc.query(&format!(
            r#"{{"query": "feature", "uid": "{}"}}"#,
            item.features[0]
        ))
        .unwrap(),
    )
    .unwrap()["def"]
        .clone();
    assert_eq!(def["type"], "replace_face");
    assert_eq!(def["faces"], json!([top]));
    assert_eq!(def["target"], "xy");
    assert_eq!(def["tangent_chain"], false);

    // Without a history the faces are not known: the file's bodies instead.
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_ne!(report.items[2].outcome, Outcome::Parametric);
    let note = report.items[2].note.clone().unwrap_or_default();
    assert!(
        note.contains("source faces were not decoded"),
        "{}",
        report.text()
    );
    // Replace faces follow Mitcad's conventions: matched more loosely.
    assert_eq!(
        Candidate::new(json!({"type": "replace_face"})).tolerance(),
        history::CONVENTIONS
    );
}

/// mitcad#6: the file's light bulbs are kept where they differ from Mitcad's
/// default (a sketch hidden while a feature uses it, construction geometry
/// shown), and only there; unknown ones follow the default.
#[test]
fn light_bulbs_are_kept_where_they_differ_from_the_default() {
    let sketch = |index: i64, bulb: Option<bool>| {
        let mut s = rectangle_sketch(index);
        s["detail"]["dimensions"] = json!([]);
        if let Some(on) = bulb {
            s["props"] = json!({"isLightBulbOn": on});
        }
        s
    };
    let plane = |index: i64, on: bool| {
        json!({"index": index, "name": format!("Plane{index}"), "objectType": "ConstructionPlane",
               "detail": {"geometry": {"type": "Plane", "origin": [0.0, 0.0, 1.0],
                                       "normal": [0.0, 0.0, 1.0]}},
               "props": {"isLightBulbOn": on}})
    };
    let items = vec![
        sketch(0, Some(true)), // Sketch1: used and shown: kept
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        sketch(2, Some(false)), // Sketch3: unused and hidden: kept
        sketch(3, Some(false)), // Sketch4: used and hidden: the default
        extrude(4, 3, "NewBodyFeatureOperation", "d2"),
        sketch(5, Some(true)), // Sketch6: unused and shown: the default
        sketch(6, None),       // Sketch7: not known: the default
        plane(7, false),       // hidden: kept
        plane(8, true),        // shown: the default
    ];
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &timeline_dump(items));
    let kept: Vec<(String, bool)> = doc
        .features()
        .filter_map(|f| Some((f.name.clone(), doc.feature_visible(f.uid)?)))
        .collect();
    assert_eq!(
        kept,
        [
            ("Sketch1".to_owned(), true),
            ("Sketch3".to_owned(), false),
            ("Plane7".to_owned(), false)
        ],
        "{}",
        report.text()
    );
    assert_eq!(
        report.light_bulbs,
        LightBulbReport {
            sketch_bulbs: 5,
            sketch_bulbs_unknown: 1,
            construction_bulbs: 2,
            construction_bulbs_unknown: 0,
            bulbs_set: 3,
        }
    );
    assert!(
        report
            .text()
            .contains("light bulbs: 5 sketches (1 unknown)")
    );
    let uid = |name: &str| doc.features().find(|f| f.name == name).unwrap().uid;
    let shown = |doc: &Document<MockKernel>, name: &str| {
        doc.feature_shown(doc.features().find(|f| f.name == name).unwrap().uid)
    };
    assert_eq!(shown(&doc, "Sketch4"), Some(false));
    assert_eq!(shown(&doc, "Sketch6"), Some(true));
    assert_eq!(shown(&doc, "Sketch7"), Some(true));
    assert_eq!(shown(&doc, "Plane8"), Some(true));
    // A sketch that follows the default shows when its consumer goes.
    let extrude4 = uid("Extrude4");
    doc.delete_feature(extrude4, false).unwrap();
    assert_eq!(shown(&doc, "Sketch4"), Some(true));
}

/// An extrusion of the stream decoder with its raw extent codes and
/// direction vector, without an extent of its own unless `distance`.
fn stream_extrude(index: i64, operation: &str, raw: Value, distance: Option<&str>) -> Value {
    let mut item = extrude(index, 0, operation, "d2");
    item["_f3d"]["extrude"] = raw;
    let detail = item["detail"].as_object_mut().unwrap();
    detail.remove("extentType");
    match distance {
        Some(d) => detail["extentOne"]["distance"] = param(d, "10 mm", 1.0, "mm"),
        None => {
            detail.remove("extentOne");
        }
    }
    item
}

#[test]
fn stream_extent_codes_and_direction_vector_choose_the_extent() {
    // Without a history only the first definition that is not a guess (of
    // the last item).
    let last = |items: Vec<Value>| {
        let n = items.len();
        let dump = timeline_dump(items);
        let mut doc = Document::new(MockKernel::default());
        let report = import(&mut doc, &dump);
        // (The sketch is partial: its polygon constraint is left out.)
        assert!(
            report.items[1..]
                .iter()
                .all(|i| i.outcome == Outcome::Parametric),
            "{}",
            report.text()
        );
        let def: Value = serde_json::from_str(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "F{n}"}}"#))
                .unwrap(),
        )
        .unwrap();
        def["def"].clone()
    };
    let new_body = "NewBodyFeatureOperation";
    let first = |raw: Value, distance: Option<&str>| {
        last(vec![
            rectangle_sketch(0),
            stream_extrude(1, new_body, raw, distance),
        ])
    };
    // The vector against the sketch's normal flips it, whatever the first
    // ±1 of the stream says.
    let def = first(
        json!({"operation_code": 4, "extent_a": 1, "extent_b": 2, "direction": 1.0,
               "direction_vector": [0.0, 0.0, -1.0]}),
        Some("d2"),
    );
    assert_eq!(def["extent"]["type"], "distance");
    assert_eq!(def["flip"], true);
    let def = first(
        json!({"operation_code": 4, "extent_a": 1, "extent_b": 2, "direction": -1.0,
               "direction_vector": [0.0, 0.0, 1.0]}),
        Some("d2"),
    );
    assert_ne!(def["flip"], true);
    // A vector oblique to the sketch's normal tells no side: along the
    // normal (mitcad#104).
    let def = first(
        json!({"operation_code": 4, "extent_a": 1, "extent_b": 2,
               "direction_vector": [0.0, 0.6, -0.8]}),
        Some("d2"),
    );
    assert_ne!(def["flip"], true);
    // Code 3: symmetric, a half each way.
    let def = first(
        json!({"operation_code": 4, "extent_a": 3, "extent_b": 2,
               "direction_vector": [0.0, 0.0, 1.0]}),
        Some("d2"),
    );
    assert_eq!(def["extent"]["type"], "symmetric");
    assert_ne!(def["extent"]["full_length"], true);
    // One side through all (code 0) of a block below the sketch: the
    // stored vector points away from the extrusion.
    let def = last(vec![
        rectangle_sketch(0),
        stream_extrude(
            1,
            new_body,
            json!({"operation_code": 4, "extent_a": 1, "extent_b": 2,
                   "direction_vector": [0.0, 0.0, -1.0]}),
            Some("d2"),
        ),
        stream_extrude(
            2,
            "CutFeatureOperation",
            json!({"operation_code": 2, "extent_a": 1, "extent_b": 0,
                   "direction_vector": [0.0, 0.0, 1.0]}),
            None,
        ),
    ]);
    assert_eq!(def["extent"]["type"], "through_all");
    assert_eq!(def["flip"], true);
}

// Polygons, sketch patterns, offsets and concentric circle dimensions
// (mitcad#36).

/// A dump of one sketch on XY with the given model parameters.
fn group_sketch_dump(
    params: Vec<Value>,
    points: Vec<Value>,
    curves: Vec<Value>,
    constraints: Value,
    dimensions: Value,
) -> Dump {
    dump(json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "groups.f3d"},
        "parameters": {"model": params},
        "timeline": {"items": [{
            "index": 0, "name": "Sketch1", "objectType": "Sketch",
            "detail": {
                "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
                "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                    [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
                "points": points, "curves": curves,
                "constraints": constraints, "dimensions": dimensions
            }
        }]}
    }))
}

fn gpoint(id: &str, x: f64, y: f64) -> Value {
    json!({"id": id, "xyz": [x, y, 0.0]})
}

fn gfixed(id: &str, x: f64, y: f64) -> Value {
    json!({"id": id, "xyz": [x, y, 0.0], "isFixed": true})
}

fn gline(id: &str, a: &str, b: &str) -> Value {
    json!({"id": id, "type": "SketchLine", "startSketchPoint": a, "endSketchPoint": b})
}

fn gcircle(id: &str, center: &str, radius: f64) -> Value {
    json!({"id": id, "type": "SketchCircle", "centerSketchPoint": center, "radius": radius})
}

fn sketch_query(doc: &Document<MockKernel>) -> Value {
    serde_json::from_str(&doc.query(r#"{"query": "sketch", "uid": "F1"}"#).unwrap()).unwrap()
}

/// The id of the point at `at` (sketch mm).
fn point_at(s: &Value, at: [f64; 2]) -> String {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e["type"] == "point"
                && (e["at"][0].as_f64().unwrap() - at[0]).abs() < 1e-6
                && (e["at"][1].as_f64().unwrap() - at[1]).abs() < 1e-6
        })
        .unwrap_or_else(|| panic!("a point at {at:?}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn entity<'a>(s: &'a Value, id: &str) -> &'a Value {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("{id}"))
}

fn at(s: &Value, id: &str) -> [f64; 2] {
    let e = entity(s, id);
    [e["at"][0].as_f64().unwrap(), e["at"][1].as_f64().unwrap()]
}

/// The circle whose centre is the point at `center` and whose radius is
/// `radius` (sketch mm).
fn circle_id(s: &Value, center: [f64; 2], radius: f64) -> String {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e["type"] == "circle"
                && e["center"]
                    .as_str()
                    .is_some_and(|c| (at(s, c)[0] - center[0]).abs() < 1e-6)
                && (e["geometry"]["radius"].as_f64().unwrap() - radius).abs() < 1e-6
        })
        .unwrap_or_else(|| panic!("a circle of {radius} at {center:?}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn radius(s: &Value, id: &str) -> f64 {
    entity(s, id)["geometry"]["radius"].as_f64().unwrap()
}

fn assert_parametric(report: &DesignReport) {
    assert_eq!(
        report.items[0].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
}

#[test]
fn a_polygon_stays_regular() {
    // A hexagon of radius 10 mm about the fixed origin, dimensioned centre
    // to corner.
    let corner = |i: usize| {
        let a = std::f64::consts::FRAC_PI_3 * i as f64;
        gpoint(&format!("p{i}"), a.cos(), a.sin())
    };
    let mut points: Vec<Value> = (0..6).map(corner).collect();
    points.push(gfixed("p6", 0.0, 0.0));
    let curves = (0..6)
        .map(|i| {
            gline(
                &format!("c{i}"),
                &format!("p{i}"),
                &format!("p{}", (i + 1) % 6),
            )
        })
        .collect();
    let dump = group_sketch_dump(
        vec![json!({"name": "d1", "expression": "10 mm", "value": 1.0, "unit": "mm"})],
        points,
        curves,
        json!([{"id": "k0", "type": "PolygonConstraint",
                "refs": {"entities": ["p0", "p1", "p2", "p3", "p4", "p5", "p6"]}}]),
        json!([{"id": "d0", "type": "SketchLinearDimension",
                "parameter": param("d1", "10 mm", 1.0, "mm"),
                "refs": {"entities": ["p6", "p0"]}}]),
    );
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let s = sketch_query(&doc);
    let corners: Vec<String> = (0..6)
        .map(|i| {
            let a = std::f64::consts::FRAC_PI_3 * i as f64;
            point_at(&s, [10.0 * a.cos(), 10.0 * a.sin()])
        })
        .collect();
    let center = point_at(&s, [0.0, 0.0]);
    // A construction circle through the corners and equal sides.
    let circles = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "circle" && e["construction"] == true)
        .count();
    assert_eq!(circles, 1);
    let kinds = |t: &str| {
        s["constraints"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["type"] == t)
            .count()
    };
    assert_eq!((kinds("coincident"), kinds("equal")), (6, 5));
    doc.set_parameter_expression("d1", "15 mm", None).unwrap();
    let s = sketch_query(&doc);
    let c = at(&s, &center);
    for i in 0..6 {
        let (a, b) = (at(&s, &corners[i]), at(&s, &corners[(i + 1) % 6]));
        assert!(((a[0] - c[0]).hypot(a[1] - c[1]) - 15.0).abs() < 1e-6);
        assert!(((a[0] - b[0]).hypot(a[1] - b[1]) - 15.0).abs() < 1e-6);
    }
}

#[test]
fn circular_pattern_copies_follow_their_original() {
    // A circle 20 mm from the fixed centre, and its copies at 120 and 240
    // degrees.
    let s3 = 3f64.sqrt();
    let points = vec![
        gfixed("p0", 0.0, 0.0),
        gpoint("p1", 2.0, 0.0),
        gpoint("p2", -1.0, s3),
        gpoint("p3", -1.0, -s3),
    ];
    let curves = vec![
        gcircle("c0", "p1", 0.3),
        gcircle("c1", "p2", 0.3),
        gcircle("c2", "p3", 0.3),
    ];
    let tau = std::f64::consts::TAU;
    let dump = group_sketch_dump(
        vec![
            json!({"name": "d1", "expression": "20 mm", "value": 2.0, "unit": "mm"}),
            json!({"name": "d6", "expression": "3", "value": 3.0, "unit": ""}),
            json!({"name": "d7", "expression": "360 deg", "value": tau, "unit": "deg"}),
        ],
        points,
        curves,
        json!([{"id": "k0", "type": "CircularPatternConstraint",
                "refs": {"entities": ["c0", "c1", "c2", "p0"]},
                "props": {"centerPoint": "p0", "createdEntities": ["c1", "c2"],
                          "quantity": param("d6", "3", 3.0, ""),
                          "totalAngle": param("d7", "360 deg", tau, "deg")}}]),
        json!([{"id": "d0", "type": "SketchLinearDimension",
                "parameter": param("d1", "20 mm", 2.0, "mm"),
                "refs": {"entities": ["p0", "p1"]}}]),
    );
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let s = sketch_query(&doc);
    let pattern = &s["patterns"][0];
    assert_eq!(
        (pattern["type"].clone(), pattern["count"].clone()),
        (json!("circular"), json!(3))
    );
    assert_eq!(pattern["copies"].as_array().unwrap().len(), 2);
    let copy = point_at(&s, [-10.0, 10.0 * s3]);
    doc.set_parameter_expression("d1", "30 mm", None).unwrap();
    let s = sketch_query(&doc);
    let p = at(&s, &copy);
    assert!(
        (p[0] + 15.0).abs() < 1e-6 && (p[1] - 15.0 * s3).abs() < 1e-6,
        "{p:?}"
    );
}

#[test]
fn rectangular_pattern_with_a_negative_distance() {
    // A circle at the fixed origin and its copies 20 and 40 mm to the left
    // (direction +x, distance -20 mm).
    let curves = vec![
        gcircle("c0", "p0", 0.5),
        gcircle("c1", "p1", 0.5),
        gcircle("c2", "p2", 0.5),
    ];
    let params = vec![
        json!({"name": "d6", "expression": "3", "value": 3.0, "unit": ""}),
        json!({"name": "d7", "expression": "1", "value": 1.0, "unit": ""}),
        json!({"name": "d8", "expression": "-20 mm", "value": -2.0, "unit": "mm"}),
        json!({"name": "d9", "expression": "0 mm", "value": 0.0, "unit": "mm"}),
    ];
    let constraints = json!([{"id": "k0", "type": "RectangularPatternConstraint",
        "refs": {"entities": ["c0", "c1", "c2"]},
        "props": {"createdEntities": ["c1", "c2"],
                  "quantityOne": param("d6", "3", 3.0, ""),
                  "quantityTwo": param("d7", "1", 1.0, ""),
                  "distanceOne": param("d8", "-20 mm", -2.0, "mm"),
                  "distanceTwo": param("d9", "0 mm", 0.0, "mm"),
                  "directionOne": [1.0, 0.0, 0.0], "directionTwo": [0.0, 1.0, 0.0],
                  "flags": [0, 0, 0]}}]);
    let points = vec![
        gfixed("p0", 0.0, 0.0),
        gpoint("p1", -2.0, 0.0),
        gpoint("p2", -4.0, 0.0),
    ];
    let dump = group_sketch_dump(
        params.clone(),
        points,
        curves.clone(),
        constraints.clone(),
        json!([]),
    );
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let s = sketch_query(&doc);
    let pattern = &s["patterns"][0];
    assert_eq!(pattern["direction"], json!([-1.0, 0.0]));
    assert_eq!(pattern["count"], json!([3, 1]));
    let far = point_at(&s, [-40.0, 0.0]);
    doc.set_parameter_expression("d8", "-30 mm", None).unwrap();
    let s = sketch_query(&doc);
    let p = at(&s, &far);
    assert!((p[0] + 60.0).abs() < 1e-6 && p[1].abs() < 1e-6, "{p:?}");

    // Copies on both sides of the original: not what Mitcad's pattern
    // makes, so it is left out.
    let points = vec![
        gfixed("p0", 0.0, 0.0),
        gpoint("p1", -2.0, 0.0),
        gpoint("p2", 2.0, 0.0),
    ];
    let dump = group_sketch_dump(params, points, curves, constraints, json!([]));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_eq!(report.items[0].outcome, Outcome::Partial);
    assert!(
        report.items[0]
            .note
            .as_deref()
            .unwrap()
            .contains("not where the pattern puts them")
    );
}

#[test]
fn a_symmetric_rectangular_pattern_keeps_the_originals_in_the_middle() {
    // A circle at the fixed origin and its copies 20 mm on both sides
    // (the stream's flags say symmetric in direction one).
    let curves = vec![
        gcircle("c0", "p0", 0.5),
        gcircle("c1", "p1", 0.5),
        gcircle("c2", "p2", 0.5),
    ];
    let params = vec![
        json!({"name": "d6", "expression": "3", "value": 3.0, "unit": ""}),
        json!({"name": "d7", "expression": "1", "value": 1.0, "unit": ""}),
        json!({"name": "d8", "expression": "20 mm", "value": 2.0, "unit": "mm"}),
        json!({"name": "d9", "expression": "0 mm", "value": 0.0, "unit": "mm"}),
    ];
    let constraints = json!([{"id": "k0", "type": "RectangularPatternConstraint",
        "refs": {"entities": ["c0", "c1", "c2"]},
        "props": {"createdEntities": ["c1", "c2"],
                  "quantityOne": param("d6", "3", 3.0, ""),
                  "quantityTwo": param("d7", "1", 1.0, ""),
                  "distanceOne": param("d8", "20 mm", 2.0, "mm"),
                  "distanceTwo": param("d9", "0 mm", 0.0, "mm"),
                  "directionOne": [1.0, 0.0, 0.0], "directionTwo": [0.0, 1.0, 0.0],
                  "flags": [0, 1, 0]}}]);
    let points = vec![
        gfixed("p0", 0.0, 0.0),
        gpoint("p1", -2.0, 0.0),
        gpoint("p2", 2.0, 0.0),
    ];
    let dump = group_sketch_dump(params, points, curves, constraints, json!([]));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let s = sketch_query(&doc);
    let pattern = &s["patterns"][0];
    assert_eq!(pattern["count"], json!([3, 1]), "{s}");
    assert_eq!(pattern["symmetric"], json!([true, false]), "{s}");
    let right = point_at(&s, [20.0, 0.0]);
    let left = point_at(&s, [-20.0, 0.0]);
    // A new spacing moves both copies, the originals stay.
    doc.set_parameter_expression("d8", "30 mm", None).unwrap();
    let s = sketch_query(&doc);
    let (r, l) = (at(&s, &right), at(&s, &left));
    assert!((r[0] - 30.0).abs() < 1e-6 && r[1].abs() < 1e-6, "{r:?}");
    assert!((l[0] + 30.0).abs() < 1e-6 && l[1].abs() < 1e-6, "{l:?}");
    assert_eq!(at(&s, "p1"), [0.0, 0.0]);
}

#[test]
fn offsets_keep_their_distance() {
    // A fixed 40 x 20 mm rectangle and its offset 5 mm inside (distance
    // parameter -5 mm); a circle of radius 10 mm and its offset 5 mm out,
    // and a third circle 5 mm outside that (a concentric circle dimension).
    let points = vec![
        gfixed("p0", 0.0, 0.0),
        gfixed("p1", 4.0, 0.0),
        gfixed("p2", 4.0, 2.0),
        gfixed("p3", 0.0, 2.0),
        gpoint("p4", 0.5, 0.5),
        gpoint("p5", 3.5, 0.5),
        gpoint("p6", 3.5, 1.5),
        gpoint("p7", 0.5, 1.5),
        gfixed("p8", 10.0, 0.0),
        gpoint("p9", 10.0, 0.0),
    ];
    let curves = vec![
        gline("c0", "p0", "p1"),
        gline("c1", "p1", "p2"),
        gline("c2", "p2", "p3"),
        gline("c3", "p3", "p0"),
        gline("c4", "p4", "p5"),
        gline("c5", "p5", "p6"),
        gline("c6", "p6", "p7"),
        gline("c7", "p7", "p4"),
        gcircle("c8", "p8", 1.0),
        gcircle("c9", "p9", 1.5),
        gcircle("c10", "p8", 2.0),
    ];
    let dump = group_sketch_dump(
        vec![
            json!({"name": "d1", "expression": "-5 mm", "value": -0.5, "unit": "mm"}),
            json!({"name": "d2", "expression": "5 mm", "value": 0.5, "unit": "mm"}),
            json!({"name": "d3", "expression": "5 mm", "value": 0.5, "unit": "mm"}),
            json!({"name": "d4", "expression": "20 mm", "value": 2.0, "unit": "mm"}),
        ],
        points,
        curves,
        json!([
            {"id": "k0", "type": "OffsetConstraint",
             "refs": {"entities": ["c0", "c4", "c1", "c5", "c2", "c6", "c3", "c7"]},
             "props": {"distance": -0.5, "dimension": "d0",
                       "parentCurves": ["c0", "c1", "c2", "c3"],
                       "childCurves": ["c4", "c5", "c6", "c7"]}},
            {"id": "k1", "type": "OffsetConstraint", "refs": {"entities": ["c8", "c9"]},
             "props": {"distance": 0.5, "dimension": "d1",
                       "parentCurves": ["c8"], "childCurves": ["c9"]}}
        ]),
        json!([
            {"id": "d0", "type": "SketchOffsetCurvesDimension",
             "parameter": param("d1", "-5 mm", -0.5, "mm"), "refs": {"entities": ["c0"]}},
            {"id": "d1", "type": "SketchOffsetCurvesDimension",
             "parameter": param("d2", "5 mm", 0.5, "mm"), "refs": {"entities": ["c8"]}},
            {"id": "d2", "type": "SketchConcentricCircleDimension",
             "parameter": param("d3", "5 mm", 0.5, "mm"), "refs": {"entities": ["c9", "c10"]},
             "textPosition": [10.0, 2.2, 0.0]},
            {"id": "d3", "type": "SketchDiameterDimension",
             "parameter": param("d4", "20 mm", 2.0, "mm"), "refs": {"entities": ["c8"]}}
        ]),
    );
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let s = sketch_query(&doc);
    let offsets = s["offsets"].as_array().unwrap();
    assert_eq!(offsets.len(), 2);
    // Inside the rectangle, which runs counter-clockwise: its left; the
    // distance is the magnitude of d1.
    assert_eq!(offsets[0]["left"], true);
    assert_eq!(offsets[0]["distance"], "d1_offset");
    assert_eq!(offsets[1].get("left"), None);
    assert_eq!(offsets[1]["distance"], "d2");
    let p = doc.parameters();
    let magnitude = p.get(p.find("d1_offset").unwrap()).unwrap();
    assert_eq!((magnitude.expression(), magnitude.value()), ("-(d1)", 5.0));
    let corner = point_at(&s, [5.0, 5.0]);
    let (c9, c10) = (
        circle_id(&s, [100.0, 0.0], 15.0),
        circle_id(&s, [100.0, 0.0], 20.0),
    );
    doc.set_parameter_expression("d1", "-8 mm", None).unwrap();
    doc.set_parameter_expression("d2", "7 mm", None).unwrap();
    let s = sketch_query(&doc);
    let p = at(&s, &corner);
    assert!(
        (p[0] - 8.0).abs() < 1e-6 && (p[1] - 8.0).abs() < 1e-6,
        "{p:?}"
    );
    assert!((radius(&s, &c9) - 17.0).abs() < 1e-6);
    assert!((radius(&s, &c10) - 22.0).abs() < 1e-6);
}

/// The flags the decoder reads from the sketch curves (mitcad#186): a
/// linked curve is a fixed reference entity, a fixed one is fixed, and
/// the three spline kinds come in with their stored NURBS data.
#[test]
fn curve_flags_and_spline_kinds_come_into_the_sketch() {
    let points = vec![
        gpoint("p0", 0.0, 0.0),
        gpoint("p1", 4.0, 0.0),
        gpoint("p2", 4.0, 2.0),
        gpoint("p3", 0.0, 5.0),
        gpoint("p4", 1.0, 5.0),
        gpoint("p5", 2.0, 5.0),
    ];
    let spline = |id: &str, kind: &str, y: f64, flags: Value| {
        let mut c = json!({
            "id": id, "type": kind,
            "geometry": {"type": "NurbsCurve3D", "nurbs": {
                "degree": 2, "isRational": false, "isPeriodic": false, "controlPointCount": 3,
                "knots": [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                "controlPoints": [[0.0, y, 0.0], [1.0, y + 1.0, 0.0], [2.0, y, 0.0]],
                "weights": []}}});
        for (k, v) in flags.as_object().unwrap() {
            c[k] = v.clone();
        }
        c
    };
    let mut linked = gline("c0", "p0", "p1");
    linked["isLinked"] = json!(true);
    linked["isReference"] = json!(true);
    let mut fixed = gline("c1", "p1", "p2");
    fixed["isFixed"] = json!(true);
    let curves = vec![
        linked,
        fixed,
        spline("c2", "SketchFixedSpline", 10.0, json!({})),
        spline(
            "c3",
            "SketchFittedSpline",
            20.0,
            json!({"fitPoints": ["p3", "p4", "p5"]}),
        ),
        spline(
            "c4",
            "SketchControlPointSpline",
            30.0,
            json!({"isFixed": true}),
        ),
    ];
    let dump = group_sketch_dump(Vec::new(), points, curves, json!([]), json!([]));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let s = sketch_query(&doc);
    let of = |kind: &str| -> Vec<&Value> {
        s["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["type"] == kind)
            .collect()
    };
    let lines = of("line");
    assert_eq!(lines.len(), 2);
    let flag = |e: &Value, k: &str| e.get(k) == Some(&json!(true));
    // The line from (0, 0) is the linked one.
    let linked = lines
        .iter()
        .find(|l| flag(l, "reference"))
        .expect("a reference line");
    assert!(flag(linked, "fixed"));
    assert_eq!(lines.iter().filter(|l| flag(l, "fixed")).count(), 2);
    assert_eq!(lines.iter().filter(|l| flag(l, "reference")).count(), 1);
    // Three splines, the control-point one fixed as its flag says.
    let splines = of("spline");
    assert_eq!(splines.len(), 3);
    assert_eq!(splines.iter().filter(|e| flag(e, "fixed")).count(), 3);
}

/// Items the decoder names without translating them (mitcad#43): those
/// that change no body are skipped at once with what they are; one that
/// changes bodies takes its history state with the reason.
#[test]
fn named_items_without_geometry_are_skipped_and_others_say_why_they_fall_back() {
    let (mut items, mut geometry) = block_history(Vec::new());
    let named = |index: i64, name: &str, t: &str| json!({"index": index, "name": name, "objectType": t, "detail": {}});
    items[2] = named(6, "EdgeFlange1", "FlangeFeature");
    items.splice(
        2..2,
        [
            named(2, "Pin", "Occurrence"),
            named(3, "GeometricRelationship1", "GeometricRelationship"),
            named(4, "Fixings", "Group"),
            named(5, "R-Pattern1", "RectangularOccurrencePattern"),
        ],
    );
    geometry.items = HashMap::from([(1, 1), (6, 2)]);
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let got: Vec<(&str, Outcome)> = report.items[2..]
        .iter()
        .map(|i| (i.object_type.as_str(), i.outcome))
        .collect();
    assert_eq!(
        got,
        [
            ("Occurrence", Outcome::Skipped),
            ("GeometricRelationship", Outcome::Skipped),
            ("Group", Outcome::Skipped),
            ("RectangularOccurrencePattern", Outcome::Skipped),
            ("FlangeFeature", Outcome::Fallback),
        ],
        "{}",
        report.text()
    );
    let note = |k: usize| report.items[k].note.clone().unwrap_or_default();
    assert!(
        note(2).starts_with("no geometry: places occurrences"),
        "{}",
        note(2)
    );
    assert!(
        note(3).starts_with("its constraints were not decoded"),
        "{}",
        note(3)
    );
    assert!(
        note(4).starts_with("no geometry: a timeline group"),
        "{}",
        note(4)
    );
    assert!(note(6).starts_with("a sheet metal flange"), "{}", note(6));
    assert!(report.history.reached_end, "{}", report.text());
}

/// The block's import (sketch, new body, join) on a thread of its own, its
/// first extrusion's kernel call running `hook`: the report and how many
/// extrusions the kernel was asked for.
fn import_block_on_thread(
    options: Options,
    hook: Box<dyn FnMut(&MockKernel) + Send>,
) -> std::thread::JoinHandle<(DesignReport, usize)> {
    std::thread::spawn(move || {
        let (items, mut geometry) = block_history(Vec::new());
        let kernel = TestKernel::default();
        let mut hook = Some(hook);
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        *kernel.on_call.borrow_mut() = Some((
            "extrude_feature",
            Box::new(move |mock: &MockKernel| {
                counted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if let Some(mut h) = hook.take() {
                    h(mock);
                }
            }),
        ));
        let mut doc = Document::new(kernel);
        let report = import_design(&mut doc, &timeline_dump(items), &mut geometry, &options);
        (report, calls.load(std::sync::atomic::Ordering::Relaxed))
    })
}

#[test]
fn a_try_the_watchdog_gave_up_does_no_more_items() {
    // The first extrusion's kernel call blocks past the hang limit (until
    // released); the watchdog gives the try up meanwhile (mitcad#71).
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    let progress = Arc::new(crate::Progress::new());
    let (entered, blocked) = mpsc::channel::<()>();
    let (release, released) = mpsc::channel::<()>();
    let worker = import_block_on_thread(
        Options {
            progress: Some(progress.clone()),
            ..Options::default()
        },
        Box::new(move |_| {
            entered.send(()).unwrap();
            released.recv().unwrap();
        }),
    );
    blocked
        .recv_timeout(std::time::Duration::from_secs(30))
        .unwrap();
    // No step while the call blocks: the watchdog's hang.
    let step = progress.step.load(Ordering::Relaxed);
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(progress.step.load(Ordering::Relaxed), step);
    assert_eq!(progress.item.load(Ordering::Relaxed), 1);
    progress.abandon();
    assert!(progress.abandoned());
    release.send(()).unwrap();
    let (report, extrusions) = worker.join().unwrap();
    // Once released it did no more items nor definitions: the join after
    // it was never started, nor the final comparison, and no definition
    // was tried after the blocked one.
    assert_eq!(report.items.len(), 2, "{}", report.text());
    assert_eq!(extrusions, 1, "{}", report.text());
    assert_eq!(progress.item.load(Ordering::Relaxed), 1);
    assert!(report.bodies.is_empty(), "{}", report.text());
    assert!(progress.failed_items().is_empty());
}

#[test]
fn an_extrusion_out_of_memory_takes_its_state_as_do_the_items_while_memory_is_low() {
    // The kernel's first extrusion fails for lack of memory (a C++
    // allocation that failed, mitcad#80): the process is low on memory, and
    // with no memory guard to say it recovered, it stays so.
    let (items, mut geometry) = block_history(Vec::new());
    let kernel = TestKernel::default();
    kernel.out_of_memory.set(1);
    let mut doc = Document::new(kernel);
    let progress = Arc::new(crate::Progress::new());
    let options = Options {
        progress: Some(progress.clone()),
        ..Options::default()
    };
    let report = import_design(&mut doc, &timeline_dump(items), &mut geometry, &options);
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Fallback, Outcome::Fallback],
        "{}",
        report.text()
    );
    let note = |i: usize| report.items[i].note.clone().unwrap_or_default();
    assert!(note(1).contains(LOW_MEMORY), "{}", report.text());
    assert!(note(2).contains(LOW_MEMORY), "{}", report.text());
    // No extrusion was evaluated after the one that ran out of memory.
    assert_eq!(doc.kernel().mock.count("extrude_feature"), 0);
    assert_eq!(doc.bodies().len(), 1);
    let low = report.low_memory.clone().expect("the report says where");
    assert_eq!(low.item, Some(1));
    assert_eq!(low.name.as_deref(), Some("Extrude1"));
    assert_eq!(low.items, 2);
    assert!(
        low.memory.contains(KernelError::OUT_OF_MEMORY),
        "{}",
        low.memory
    );
    assert!(
        report.warnings.iter().any(|w| w.contains(
            "ran low on memory at Extrude1 (a geometry kernel operation ran out of memory"
        ) && w.ends_with("compared by volume only")),
        "{}",
        report.text()
    );
    // Not the items' own failures: a try run again tries them.
    assert!(progress.failed_items().is_empty());
}

#[test]
fn a_definition_cut_short_for_lack_of_memory_falls_back_and_the_import_goes_on() {
    // While the first extrusion is evaluated, the memory guard finds the
    // process low on memory (mitcad#80): the evaluation stops, as the
    // kernel's long operations do on request, and the item takes its
    // state. The memory has recovered for the join after it.
    let progress = Arc::new(crate::Progress::new());
    let guard = progress.clone();
    let worker = import_block_on_thread(
        Options {
            progress: Some(progress.clone()),
            ..Options::default()
        },
        Box::new(move |mock| {
            guard.low_memory("9.9 GiB of 10.0 GiB (test)");
            slow_call(mock);
            guard.memory_recovered();
        }),
    );
    let (report, extrusions) = worker.join().unwrap();
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Fallback, Outcome::Fallback],
        "{}",
        report.text()
    );
    let note = |i: usize| report.items[i].note.clone().unwrap_or_default();
    assert!(note(1).contains(LOW_MEMORY), "{}", report.text());
    // The join was tried (the mock kernel's join on the fallback's body is
    // not its state).
    assert!(!note(2).contains(LOW_MEMORY), "{}", report.text());
    assert!(extrusions > 1, "{}", report.text());
    let low = report.low_memory.clone().expect("the report says where");
    assert_eq!((low.item, low.items), (Some(1), 1));
    assert_eq!(low.memory, "9.9 GiB of 10.0 GiB (test)");
    let warning = report
        .warnings
        .iter()
        .find(|w| w.contains("ran low on memory"))
        .expect("a warning");
    assert!(
        warning.ends_with("cut short or not tried and 1 modelling items took the file's bodies"),
        "{warning}"
    );
}

#[test]
fn the_failed_operation_names_a_failed_allocation_the_guard_saw_first() {
    // The memory guard polls on its own thread: it can count a failed
    // allocation before the operation that failed reports it. The report
    // tells the operation's words, once, and not those of a later low;
    // also when the guard saw the memory recover meanwhile (a failed
    // allocation leaves the memory as it was).
    let progress = crate::Progress::new();
    progress.low_memory_unattributed("an allocation in the geometry kernel failed (test)");
    progress.low_memory("building a body of the file: out of memory (test)");
    progress.low_memory("extrude: out of memory (test)");
    assert_eq!(progress.memory_lows(), 1);
    let (memory, _) = progress.first_low_memory().expect("low");
    assert_eq!(memory, "building a body of the file: out of memory (test)");
    let progress = crate::Progress::new();
    progress.low_memory_unattributed("an allocation in the geometry kernel failed (test)");
    progress.memory_recovered();
    progress.low_memory("extrude: out of memory (test)");
    assert!(progress.is_memory_low());
    let (memory, _) = progress.first_low_memory().expect("low");
    assert_eq!(memory, "extrude: out of memory (test)");
    // A first low the memory guard measured is not renamed.
    let progress = crate::Progress::new();
    progress.low_memory("9.9 GiB of 10.0 GiB (test)");
    progress.low_memory("extrude: out of memory (test)");
    progress.name_failed_allocation("fillet: out of memory (test)");
    let (memory, _) = progress.first_low_memory().expect("low");
    assert_eq!(memory, "9.9 GiB of 10.0 GiB (test)");
    // A worker's operation names the failure, before or after the guard
    // counted it, without making the import low on memory itself.
    let progress = crate::Progress::new();
    progress.name_failed_allocation("extrude: out of memory (test)");
    assert!(!progress.is_memory_low());
    progress.low_memory_unattributed("an allocation in the geometry kernel failed (test)");
    let (memory, _) = progress.first_low_memory().expect("low");
    assert_eq!(memory, "extrude: out of memory (test)");
    let progress = crate::Progress::new();
    progress.low_memory_unattributed("an allocation in the geometry kernel failed (test)");
    progress.name_failed_allocation("extrude: out of memory (test)");
    let (memory, _) = progress.first_low_memory().expect("low");
    assert_eq!(memory, "extrude: out of memory (test)");
}

#[test]
fn the_next_item_waits_for_the_memory_guard_to_see_the_memory_recover() {
    // The memory guard measures the process every so often: it sees the
    // memory recover after the definition cut short only a little later,
    // and the join after it is tried all the same.
    let progress = Arc::new(crate::Progress::new());
    let guard = progress.clone();
    let worker = import_block_on_thread(
        Options {
            progress: Some(progress.clone()),
            ..Options::default()
        },
        Box::new(move |mock| {
            guard.low_memory("9.9 GiB of 10.0 GiB (test)");
            slow_call(mock);
            let later = guard.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                later.memory_recovered();
            });
        }),
    );
    let (report, _) = worker.join().unwrap();
    let note = |i: usize| report.items[i].note.clone().unwrap_or_default();
    assert!(note(1).contains(LOW_MEMORY), "{}", report.text());
    assert!(!note(2).contains(LOW_MEMORY), "{}", report.text());
}

#[test]
fn memory_tight_drops_what_the_import_can_build_again() {
    // Asked to while the first extrusion is evaluated (mitcad#80), the
    // import drops before its next item the cached results of the
    // definitions it tried and the bodies of the history states behind it;
    // what it imports stays the same.
    let run = |tight: bool| {
        let progress = Arc::new(crate::Progress::new());
        let guard = progress.clone();
        let worker = import_block_on_thread(
            Options {
                progress: Some(progress.clone()),
                ..Options::default()
            },
            Box::new(move |_| {
                if tight {
                    guard.tight_on_memory();
                }
            }),
        );
        let (report, _) = worker.join().unwrap();
        (report, progress)
    };
    let (plain, _) = run(false);
    let (report, progress) = run(true);
    let outcomes = |r: &DesignReport| r.items.iter().map(|i| i.outcome).collect::<Vec<_>>();
    assert_eq!(outcomes(&report), outcomes(&plain), "{}", report.text());
    let json = |r: &DesignReport| serde_json::to_value(r).unwrap();
    assert_eq!(json(&report), json(&plain));
    assert!(report.low_memory.is_none());
    assert!(!progress.take_tight());
}

#[test]
fn the_oracle_builds_released_states_again() {
    let (_, mut geometry) = block_history(Vec::new());
    let kernel = TestKernel::default();
    let mut oracle = Oracle::new(&mut geometry);
    let before: Vec<Sig> = oracle.sigs(&kernel, 1);
    oracle.sigs(&kernel, 2);
    oracle.cursor = Some(2);
    assert_eq!(oracle.kept(), 2);
    assert_eq!(oracle.release_behind(), 1);
    assert_eq!(oracle.kept(), 1);
    assert_eq!(oracle.sigs(&kernel, 1), before);
    // (Built again, not counted again.)
    assert_eq!(oracle.built, 2);
}

#[test]
fn a_definition_that_takes_longer_than_its_item_is_cut_short() {
    // The first extrusion's kernel call would take 30 s, checking for a
    // request to stop as the geometry kernel's long operations do; the item
    // has a quarter of a second (mitcad#69). Its only definition has the
    // whole time (of several, the first would give way at its share).
    let started = std::time::Instant::now();
    let worker = import_block_on_thread(
        Options {
            item_seconds: 0.25,
            max_candidates: 1,
            ..Options::default()
        },
        Box::new(slow_call),
    );
    let (report, _) = worker.join().unwrap();
    let elapsed = started.elapsed().as_secs_f64();
    assert!(
        elapsed < 10.0,
        "the item's time did not cut it short: {elapsed} s"
    );
    // The item gave up like one out of time and took its state; the join
    // after it was tried in its own time (the mock kernel's join on the
    // fallback's body is not the state).
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [Outcome::Partial, Outcome::Fallback, Outcome::Fallback],
        "{}",
        report.text()
    );
    let note = |i: usize| report.items[i].note.clone().unwrap_or_default();
    assert!(note(1).contains(OUT_OF_TIME), "{}", report.text());
    // It says which definition the time went to (mitcad#78).
    assert!(
        note(1).contains("; cut short after 0.")
            && note(1).ends_with(" s: definition 1 of 1 (extrude, new_body, 1 profile)"),
        "{}",
        report.text()
    );
    assert!(!note(2).contains(OUT_OF_TIME), "{}", report.text());
}

/// The hang watchdog of `mitcad-ffi` in short: watches the progress of the
/// import `worker` runs and gives the try up ([`Progress::abandon`]) once it
/// made no step for longer than `limit`. The worker's result, and the
/// longest time the import made no step.
fn watched<T>(
    progress: &Progress,
    limit: std::time::Duration,
    worker: std::thread::JoinHandle<T>,
) -> (T, std::time::Duration) {
    use std::sync::atomic::Ordering;
    let mut step = progress.step.load(Ordering::Relaxed);
    let mut moved = std::time::Instant::now();
    let mut longest = std::time::Duration::ZERO;
    while !worker.is_finished() {
        std::thread::sleep(std::time::Duration::from_millis(5));
        let now = progress.step.load(Ordering::Relaxed);
        if now != step {
            step = now;
            moved = std::time::Instant::now();
        }
        longest = longest.max(moved.elapsed());
        if moved.elapsed() > limit {
            progress.abandon();
        }
    }
    (worker.join().unwrap(), longest)
}

/// A design whose first item is a base feature of `bodies` stored bodies
/// (the file's first history state), imported on a thread of its own by a
/// test kernel whose `mass_properties` calls run `hook`.
fn import_base_feature_on_thread(
    bodies: usize,
    progress: Arc<Progress>,
    hook: Box<dyn FnMut(&MockKernel) + Send>,
) -> std::thread::JoinHandle<DesignReport> {
    std::thread::spawn(move || {
        let stored: Vec<MockShape> = (0..bodies)
            .map(|i| MockShape::imported(&format!("stored body {i}"), 6))
            .collect();
        let mut geometry = TestGeometry {
            states: vec![Vec::new(), stored],
            items: HashMap::from([(0, 1)]),
            ..TestGeometry::default()
        };
        let items = vec![json!({"index": 0, "name": "Base Feature1", "objectType": "BaseFeature"})];
        let kernel = TestKernel::default();
        *kernel.on_call.borrow_mut() = Some(("mass_properties", hook));
        let mut doc = Document::new(kernel);
        let options = Options {
            progress: Some(progress),
            ..Options::default()
        };
        import_design(&mut doc, &timeline_dump(items), &mut geometry, &options)
    })
}

#[test]
fn long_loops_of_the_import_tick_the_watchdog() {
    // Measuring the bodies of a large history state takes long, a kernel
    // call each (mitcad#82: 110 s on the first item of a large design,
    // whose hang limit was 60 s): here 30 bodies of 20 ms each, measured
    // as the state, as the replay's and as the stored design's, under a
    // hang limit of 0.3 s. The import ticks between the calls, so the
    // watchdog does not take it for a hang.
    let limit = std::time::Duration::from_millis(300);
    let progress = Arc::new(Progress::new());
    let started = std::time::Instant::now();
    let worker = import_base_feature_on_thread(
        30,
        progress.clone(),
        Box::new(|_| std::thread::sleep(std::time::Duration::from_millis(20))),
    );
    let (report, longest) = watched(&progress, limit, worker);
    let took = started.elapsed();
    assert!(took > 2 * limit, "the import took only {took:?}");
    assert!(
        !progress.abandoned(),
        "no step for {longest:?}: {}",
        report.text()
    );
    // (The file's own base feature: its stored bodies are its translation.)
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(outcomes, [Outcome::Parametric], "{}", report.text());
    assert_eq!(report.bodies.len(), 30, "{}", report.text());
}

#[test]
fn a_kernel_call_that_does_not_return_still_looks_hung() {
    // The same import, one measurement blocking for longer than the hang
    // limit (a kernel call that does not return): the ticks around it do
    // not hide it, and the watchdog gives the try up.
    let limit = std::time::Duration::from_millis(300);
    let progress = Arc::new(Progress::new());
    let mut calls = 0;
    let worker = import_base_feature_on_thread(
        30,
        progress.clone(),
        Box::new(move |_| {
            calls += 1;
            if calls == 5 {
                std::thread::sleep(std::time::Duration::from_millis(1500));
            }
        }),
    );
    let (report, longest) = watched(&progress, limit, worker);
    assert!(progress.abandoned(), "{}", report.text());
    assert!(longest > limit, "{longest:?}");
    // Given up: the stored design was not compared.
    assert!(report.bodies.is_empty(), "{}", report.text());
}

#[test]
fn a_join_of_the_same_bodies_waits_behind_one_that_gave_way() {
    let mut turns = Turns::default();
    let combine = |target: &str, tools: &[&str], operation: &str, keep: bool| {
        Candidate::new(json!({"type": "combine", "target": target, "tools": tools,
                              "operation": operation, "keep_tools": keep}))
    };
    let candidates = vec![
        combine("F2.b0", &["F12.b0"], "join", false),
        combine("F12.b0", &["F2.b0"], "join", false),
        combine("F12.b0", &["F2.b0"], "join", true),
        combine("F12.b0", &["F2.b0"], "cut", false),
        combine("F2.b0", &["F12.b0", "F3.b0"], "join", false),
        combine("F12.b0", &["F2.b0"], "intersect", false),
    ];
    assert!(!turns.waits_behind(&candidates, 1));
    turns.cut_short(0, 20.0, true);
    // The same union with the other body as the target, keeping its tools
    // or not, waits behind the first; a cut, a union of more bodies and an
    // intersection are other booleans.
    assert!(turns.waits_behind(&candidates, 1));
    assert!(turns.waits_behind(&candidates, 2));
    assert!(!turns.waits_behind(&candidates, 3));
    assert!(!turns.waits_behind(&candidates, 4));
    assert!(!turns.waits_behind(&candidates, 5));
    // With the time they ran: tried again only when more is left.
    assert_eq!(turns.again(25.0), Some(0));
    assert_eq!(turns.again(25.0), Some(1));
    assert_eq!(turns.again(15.0), None);
}

#[test]
fn definitions_that_gave_way_wait_for_the_rest_of_the_time() {
    let mut turns = Turns::default();
    // A share of the item's time each, the last one the rest.
    assert_eq!(turns.share(60.0, false, false), Some(SHARE * 60.0));
    assert_eq!(turns.share(60.0, true, false), None);
    turns.cut_short(1, 15.0, true);
    turns.cut_short(0, 16.0, true);
    // While some wait, the last one has a share too; they have the rest.
    assert_eq!(turns.share(60.0, true, false), Some(SHARE * 60.0));
    assert_eq!(turns.share(60.0, false, true), None);
    let join = json!({"type": "extrude", "operation": "join",
                      "profiles": [{"sketch": "F1", "region": "r{c1}"}]});
    let mut limited = join.clone();
    limited["participants"] = json!(["F2.b0"]);
    let candidates = vec![
        Candidate::new(json!({"type": "fillet"})),
        Candidate::new(join),
        Candidate::new(limited),
    ];
    // The join limited to participants waits behind the join untried.
    assert!(turns.waits_behind(&candidates, 2));
    assert!(!turns.waits_behind(&candidates, 0));
    // By rank, while more time is left than they ran.
    assert_eq!(turns.again(20.0), Some(1));
    assert_eq!(turns.again(16.0), Some(2));
    assert_eq!(turns.again(14.0), None);
    turns.cut_short(1, 19.0, false);
    let (seconds, note) = turns.note(&candidates).unwrap();
    assert_eq!(seconds, 19.0);
    assert_eq!(
        note,
        "cut short after 19.0 s: definition 2 of 3 (extrude, join, 1 profile) (and 1 other)"
    );
}

/// The large rectangle of `two_rectangles` (Mitcad's key; [`SMALL`] the
/// small one).
const LARGE: &str = "r{c9[c12,c10],c10[c9,c11],c11[c10,c12],c12[c11,c9]}";

/// One profile of the two rectangles extruded as a new body by 10 mm (the
/// decoded direction along the sketch's normal first), against the file's
/// state `right`: an extrusion of a region by a distance (mm), flipped or
/// not, which the history names when `known`; `slow` extrusions take
/// long (mitcad#78). The report, the definition of the item's feature
/// (when it has one), how many slow extrusions were asked for, and the
/// seconds the import took.
fn slow_profile_import(
    known: bool,
    right: (&str, f64, bool),
    slow: SlowExtrusions,
    item_seconds: f64,
) -> (DesignReport, Value, usize, f64) {
    let items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let kernel = || TestKernel {
        directed: true,
        ..TestKernel::default()
    };
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(items[..1].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (region, distance, flip) = right;
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": region}],
               "extent": {"type": "distance", "distance": distance}, "flip": flip,
               "operation": "new_body"}),
    );
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), shapes(&reference)],
        items: if known {
            HashMap::from([(1, 1)])
        } else {
            HashMap::new()
        },
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel {
        slow: Some(slow),
        ..kernel()
    });
    let started = std::time::Instant::now();
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options {
            item_seconds,
            ..Options::default()
        },
    );
    let elapsed = started.elapsed().as_secs_f64();
    let def = report.items[1]
        .features
        .first()
        .and_then(|uid| {
            doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                .ok()
        })
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .map(|def| def["def"].clone())
        .unwrap_or_default();
    (report, def, doc.kernel().slow_calls.get(), elapsed)
}

#[test]
fn a_slow_definition_gives_way_to_the_next_ones() {
    // The file's extrusion goes against the sketch's normal; the one
    // along it is tried first and would take 30 s: at its share of the
    // item's 2 s it gives way, and the next one gives the state (mitcad#78).
    // Before, it used up the item's time.
    let slow = SlowExtrusions {
        regions: &[LARGE],
        along: true,
        against: false,
        seconds: 30.0,
    };
    for known in [true, false] {
        let (report, def, slow, elapsed) =
            slow_profile_import(known, (LARGE, 10.0, true), slow, 2.0);
        assert_eq!(
            report.items[1].outcome,
            Outcome::Parametric,
            "{}",
            report.text()
        );
        assert_eq!(report.items[1].verified, Some(true));
        assert_eq!(def["profiles"][0]["region"], LARGE, "{}", report.text());
        assert_eq!(def["flip"], true, "{}", report.text());
        assert_eq!(slow, 1, "{}", report.text());
        assert!(elapsed < 10.0, "{elapsed} s");
    }
}

#[test]
fn a_definition_that_gave_way_gets_the_rest_of_the_time() {
    // The file's extrusion, tried first, takes 1 s, more than its share (a
    // third of 2 s): it gives way, the others give no state, and it is
    // tried again with the rest of the item's time and gives the state.
    let slow = SlowExtrusions {
        regions: &[LARGE],
        along: true,
        against: false,
        seconds: 1.0,
    };
    for known in [true, false] {
        let (report, def, slow, _) = slow_profile_import(known, (LARGE, 10.0, false), slow, 2.0);
        assert_eq!(
            report.items[1].outcome,
            Outcome::Parametric,
            "{}",
            report.text()
        );
        assert_eq!(def["profiles"][0]["region"], LARGE, "{}", report.text());
        assert_ne!(def["flip"], true, "{}", report.text());
        assert_eq!(slow, 2, "{}", report.text());
    }
}

#[test]
fn an_item_out_of_time_names_the_definition_its_time_went_to() {
    // No definition gives the file's state (an extrusion by 20 mm), and
    // the one of both rectangles along the sketch's normal takes longer
    // than the item's second: it gives way, is tried again with the rest
    // of the time and cut short there; the item falls back and says which
    // definition its time went to.
    let slow = SlowExtrusions {
        regions: &[LARGE, SMALL],
        along: true,
        against: false,
        seconds: 30.0,
    };
    for known in [true, false] {
        let (report, _, slow, elapsed) =
            slow_profile_import(known, (SMALL, 20.0, false), slow, 1.0);
        assert_eq!(
            report.items[1].outcome,
            Outcome::Fallback,
            "{}",
            report.text()
        );
        let note = report.items[1].note.clone().unwrap_or_default();
        assert!(
            note.contains(&format!("{OUT_OF_TIME}; cut short after 0."))
                && note.contains(" (extrude, new_body, 2 profiles)"),
            "{}",
            report.text()
        );
        assert!(slow >= 1, "{}", report.text());
        assert!(elapsed < 10.0, "{elapsed} s");
    }
}

#[test]
fn points_along_a_segment_are_inside_between_its_crossings() {
    use crate::ops::inside_at;
    // In at a quarter, out at a half, in again at three quarters.
    let crossings = [(0.25, true), (0.5, false), (0.75, true)];
    let inside: Vec<bool> = [0.1, 0.3, 0.6, 0.9]
        .iter()
        .map(|t| inside_at(&crossings, *t))
        .collect();
    assert_eq!(inside, [false, true, false, true]);
    // Out at a half only: inside before it.
    assert!(inside_at(&[(0.5, false)], 0.2));
    assert!(!inside_at(&[(0.5, false)], 0.7));
    // No crossing: outside.
    assert!(!inside_at(&[], 0.5));
}

#[test]
fn a_sizing_offset_that_takes_long_gives_way_to_the_next_definition() {
    // The offset of the threaded face would take 30 s: it is cut short at
    // its few seconds and the thread comes in without it (mitcad#68).
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut items = vec![
        circle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let side = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "side")
        .cloned()
        .expect("a side face");
    let info = json!({"_type": "ThreadInfo", "threadType": "ISO Metric profile",
        "threadDesignation": "M10x1.5", "threadClass": "6g", "isInternal": false,
        "majorDiameter": 0.985, "minorDiameter": 0.8141});
    items.push(thread_item(
        2,
        info,
        true,
        1.0,
        face_point(&side).map(|x| x / 10.0),
    ));
    let k = kernel();
    *k.on_call.borrow_mut() = Some(("offset_faces", Box::new(slow_call)));
    let mut doc = Document::new(k);
    let started = std::time::Instant::now();
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    let elapsed = started.elapsed().as_secs_f64();
    assert!(
        (SIZING_OFFSET_SECONDS..10.0).contains(&elapsed),
        "{elapsed} s"
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert!(
        !doc.features()
            .any(|f| matches!(f.def, FeatureDef::OffsetFace(_))),
        "{}",
        report.text()
    );
}

#[test]
fn patterns_with_read_inputs_or_a_missing_feature_are_not_searched() {
    let read = |provenance, guess| {
        let mut c = Candidate::new(json!({"type": "rectangular_pattern"}));
        c.input_provenance = provenance;
        c.guess = guess;
        c
    };
    let not_searched = ops::pattern_not_searched;
    // The decoded definitions have read inputs: no history suggestions.
    for kind in ["CircularPatternFeature", "RectangularPatternFeature"] {
        let decoded = Ok(vec![
            read(InputProvenance::Read, false),
            read(InputProvenance::Unknown, false),
        ]);
        assert!(not_searched(kind, &decoded), "{kind}");
        // Inputs not read, or only guessed ones: the suggestions come too.
        let unknown = Ok(vec![read(InputProvenance::Unknown, false)]);
        assert!(!not_searched(kind, &unknown), "{kind}");
        let guessed = Ok(vec![read(InputProvenance::Read, true)]);
        assert!(!not_searched(kind, &guessed), "{kind}");
        // The copied feature did not come in: the copies of another one
        // are not the file's.
        let missing = Err(ops::PATTERNED_NOT_IMPORTED.to_owned());
        assert!(not_searched(kind, &missing), "{kind}");
        let other = Err("direction one was not decoded".to_owned());
        assert!(!not_searched(kind, &other), "{kind}");
    }
    // Mirrors and other items keep their suggestions.
    let decoded = Ok(vec![read(InputProvenance::Read, false)]);
    assert!(!not_searched("MirrorFeature", &decoded));
}

// Joints, as-built joints, joint origins and ground items (mitcad#55).
mod joint_tests;

// Assembly relationships as constraint sets (mitcad#155).
mod relationship_tests;

// The import's outcomes on the timeline (mitcad#170).
mod timeline_note_tests;

// Items suppressed in the file as suppressed features (mitcad#180).
mod suppressed_tests;

// Components of other documents filled from their documents (mitcad#196).
mod reference_tests;

// Retries on the file's exact state keep the matching bodies (mitcad#283).
mod retry_tests;

// Results that deviate from the file's, kept at the user's choice
// (mitcad#379).
mod deviation_tests;

/// The learning dump (mitcad#96): the item the history settled is written
/// with its candidates and answer in the file's terms.
#[test]
fn the_learning_dump_keeps_what_the_history_chose() {
    let items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items[..1].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": small}],
               "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}),
    );
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), shapes(&reference)],
        ..TestGeometry::default()
    };
    let dir = std::env::temp_dir().join(format!("mitcad-learn-{}", std::process::id()));
    let options = Options {
        learn: Some(crate::learn::LearnOptions {
            dir: dir.clone(),
            path: None,
        }),
        ..Options::default()
    };
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(&mut doc, &timeline_dump(items), &mut geometry, &options);
    assert_eq!(report.items[1].verified, Some(true), "{}", report.text());
    let settled = std::fs::read_dir(dir.join("settled"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect::<String>();
    let _ = std::fs::remove_dir_all(&dir);
    let lines: Vec<Value> = settled
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert_eq!(line["item"], 1);
    assert_eq!(line["type"], "ExtrudeFeature");
    assert_eq!(line["status"], "settled");
    // The sketch's regions by the file's curve ids; the answer names the
    // small rectangle by its index among them.
    let regions = line["context"]["sketches"]["T0"]["regions"]
        .as_array()
        .unwrap();
    assert_eq!(regions.len(), 2);
    let answer = &line["answer"]["defs"][0]["profiles"][0];
    assert_eq!(answer["sketch"], "T0");
    let chosen = &regions[answer["region"].as_u64().unwrap() as usize];
    let mut outer: Vec<&str> = chosen["outer"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    outer.sort_unstable();
    assert_eq!(outer, ["c4", "c5", "c6", "c7"]);
    assert!(chosen["inside"].is_array());
    // The candidates in their order, the answer among them.
    let rank = line["accepted"].as_u64().unwrap() as usize;
    assert_eq!(line["candidates"][rank], line["answer"]);
    assert!(line["candidates"].as_array().unwrap().len() >= 2);
    // No file: no raw record.
    assert!(line["record"].is_null());
}

#[test]
fn the_learning_dump_captures_the_dressup_matcher_result() {
    let mut items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (a, b) = shapes(&reference)[0].edges[0].clone();
    let p = edge_point(&a, &b);
    items.push(
        json!({"index": 2, "name": "Fillet1", "objectType": "FilletFeature",
        "detail": {"edgeSets": [{"radius": param("d5", "2 mm", 0.2, "mm"), "edges": [{
            "kind": "edge", "length": 0.1,
            "mid_point": p.map(|x| x / 10.0),
            "_f3d": {"recipe": "edge", "found": "BREP.x.smbh before state 4"}
        }]}]}}),
    );
    let dir = std::env::temp_dir().join(format!("mitcad-learn-edges-{}", std::process::id()));
    let mut doc = Document::new(TestKernel {
        edge_points: true,
        ..TestKernel::default()
    });
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options {
            learn: Some(crate::learn::LearnOptions {
                dir: dir.clone(),
                path: None,
            }),
            ..Options::default()
        },
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let text = std::fs::read_dir(dir.join("unsettled"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect::<String>();
    let _ = std::fs::remove_dir_all(&dir);
    let line = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|line| line["item"] == 2)
        .unwrap();
    let diagnostic = &line["context"]["edge_matches"]["/detail/edgeSets/0/edges/0"];
    assert_eq!(diagnostic["status"], "matched");
    assert_eq!(diagnostic["count"], 1);
    assert_eq!(diagnostic["matches"][0]["body"], "T1.b0");
    assert_eq!(diagnostic["decoder_found"], "BREP.x.smbh before state 4");
    assert!(line.get("translation_error").is_none());
    // The additive field leaves existing readers' resolved input intact.
    assert_eq!(
        line["context"]["resolved"]["/detail/edgeSets/0/edges/0"]["kind"],
        "edge"
    );
}

#[test]
fn the_learning_dump_keeps_a_failed_dressup_without_history_or_candidates() {
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 2, "name": "Fillet1", "objectType": "FilletFeature",
        "detail": {"edgeSets": [{"radius": param("d5", "2 mm", 0.2, "mm"), "edges": [{
            "kind": "edge", "_f3d": {"recipe": "edge",
                "found": "a face of the edge is not in the history state"}
        }]}]}}),
    ];
    let dir =
        std::env::temp_dir().join(format!("mitcad-learn-failed-edges-{}", std::process::id()));
    let mut doc = Document::new(TestKernel {
        edge_points: true,
        ..TestKernel::default()
    });
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options {
            learn: Some(crate::learn::LearnOptions {
                dir: dir.clone(),
                path: None,
            }),
            ..Options::default()
        },
    );
    assert_ne!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert!(report.text().contains("decoder_unresolved"));
    let text = std::fs::read_dir(dir.join("unsettled"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect::<String>();
    let _ = std::fs::remove_dir_all(&dir);
    let line = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|line| line["item"] == 2)
        .expect("candidate-free dressup must have an unsettled learning record");
    assert_eq!(line["status"], "unsettled");
    assert_eq!(line["candidates"], json!([]));
    assert_eq!(line["cost"]["candidates"], 0);
    assert!(line["accepted"].is_null());
    assert!(line["answer"].is_null());
    let error = line["translation_error"].as_str().unwrap();
    assert!(error.contains("edge set 0, input 0: decoder_unresolved"));
    assert!(error.contains("no history to find them"));
    let pointer = "/detail/edgeSets/0/edges/0";
    let diagnostic = &line["context"]["edge_matches"][pointer];
    assert_eq!(diagnostic["status"], "decoder_unresolved");
    assert_eq!(
        diagnostic["decoder_found"],
        "a face of the edge is not in the history state"
    );
    assert_eq!(diagnostic["count"], 0);
    assert_eq!(diagnostic["candidates"], json!([]));
    assert!(line["context"]["resolved"][pointer].is_null());
    assert!(!line["context"]["bodies"].as_array().unwrap().is_empty());
}

// An item's definitions evaluated ahead, in parallel (mitcad#95).

/// One profile of the two rectangles extruded as a new body (as
/// [`slow_profile_import`]) against the file's state `right` on `kernel`,
/// with `threads`: the report, each feature's name and definition, the
/// seconds the import took, and the kernel (whose forks count the slow
/// extrusions stopped in its `stopped`).
fn ahead_import(
    threads: usize,
    known: bool,
    right: (&str, f64, bool),
    kernel: impl Fn() -> TestKernel,
    item_seconds: f64,
) -> (
    DesignReport,
    Vec<Value>,
    f64,
    Arc<std::sync::atomic::AtomicUsize>,
) {
    let items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel {
        slow: None,
        ..kernel()
    });
    import_design(
        &mut reference,
        &timeline_dump(items[..1].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (region, distance, flip) = right;
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": region}],
               "extent": {"type": "distance", "distance": distance}, "flip": flip,
               "operation": "new_body"}),
    );
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), shapes(&reference)],
        items: if known {
            HashMap::from([(1, 1)])
        } else {
            HashMap::new()
        },
        ..TestGeometry::default()
    };
    let mut doc = Document::new(kernel());
    let started = std::time::Instant::now();
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options {
            item_seconds,
            threads,
            ..Options::default()
        },
    );
    let elapsed = started.elapsed().as_secs_f64();
    let features = doc
        .features()
        .map(|f| f.uid)
        .collect::<Vec<_>>()
        .into_iter()
        .map(|uid| {
            let text = doc
                .query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                .unwrap();
            serde_json::from_str::<Value>(&text).unwrap()
        })
        .collect();
    (report, features, elapsed, Arc::clone(&doc.kernel().stopped))
}

#[test]
fn definitions_evaluated_ahead_give_the_same_import() {
    // The history picks the profile and direction; with workers the
    // definitions after the one evaluated are evaluated ahead, and the one
    // taken (here a worker's, as it is not ranked first) comes in with the
    // same features, names and report as without them.
    let kernel = || TestKernel {
        directed: true,
        ..TestKernel::default()
    };
    for known in [true, false] {
        for right in [(LARGE, 10.0, false), (LARGE, 10.0, true)] {
            let (one, one_features, _, _) = ahead_import(1, known, right, kernel, 60.0);
            let (eight, eight_features, _, _) = ahead_import(8, known, right, kernel, 60.0);
            assert_eq!(
                one.items[1].outcome,
                Outcome::Parametric,
                "{right:?} {known}: {}",
                one.text()
            );
            assert_eq!(one.text(), eight.text());
            assert_eq!(
                serde_json::to_string(&one.items).unwrap(),
                serde_json::to_string(&eight.items).unwrap()
            );
            assert_eq!(one_features, eight_features);
        }
    }
}

#[test]
fn the_first_definition_that_gives_the_state_is_taken() {
    // Every extrusion measures the same here, so each definition gives the
    // state. The first one ranked takes a while; the workers have the
    // others done before it, but it is the one taken, as without workers.
    let kernel = || TestKernel {
        alike: true,
        directed: true,
        slow: Some(SlowExtrusions {
            regions: &[SMALL],
            along: true,
            against: false,
            seconds: 0.5,
        }),
        ..TestKernel::default()
    };
    for known in [true, false] {
        let (one, one_features, _, _) = ahead_import(1, known, (SMALL, 10.0, false), kernel, 60.0);
        let (eight, eight_features, _, _) =
            ahead_import(8, known, (SMALL, 10.0, false), kernel, 60.0);
        assert_eq!(one.items[1].verified, Some(true), "{}", one.text());
        assert_eq!(one.text(), eight.text());
        assert_eq!(one_features, eight_features);
        let def = &eight_features.last().unwrap()["def"];
        assert_eq!(def["profiles"][0]["region"], SMALL, "{}", eight.text());
        assert_ne!(def["flip"], true, "{}", eight.text());
    }
}

#[test]
fn definitions_after_the_one_taken_are_cut_short() {
    // The definitions of the small rectangle give no state, the third (the
    // large one along the sketch's normal) gives it, and the fourth
    // (against the normal) would take 30 s. Its worker is stopped as soon
    // as the third is taken, and the import does not wait for it.
    let began = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let stopped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let kernel = || TestKernel {
        directed: true,
        slow: Some(SlowExtrusions {
            regions: &[LARGE],
            along: false,
            against: true,
            seconds: 30.0,
        }),
        began: Arc::clone(&began),
        stopped: Arc::clone(&stopped),
        ..TestKernel::default()
    };
    let (report, features, elapsed, _) = ahead_import(8, false, (LARGE, 10.0, false), kernel, 60.0);
    assert_eq!(report.items[1].verified, Some(true), "{}", report.text());
    let def = &features.last().unwrap()["def"];
    assert_eq!(def["profiles"][0]["region"], LARGE, "{}", report.text());
    assert_ne!(def["flip"], true, "{}", report.text());
    assert!(elapsed < 10.0, "{elapsed} s");
    // Its worker stops within a few milliseconds of the stop (unless the
    // stop came before its extrusion began).
    let load =
        |n: &Arc<std::sync::atomic::AtomicUsize>| n.load(std::sync::atomic::Ordering::SeqCst);
    let waited = std::time::Instant::now();
    while load(&stopped) < load(&began) && waited.elapsed().as_secs_f64() < 10.0 {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(load(&stopped), load(&began));
}

#[test]
fn a_document_fork_adds_features_as_the_document_would() {
    // A fork's feature gets the uid the document's would, and its result,
    // adopted, makes adding the same definition to the document reuse it.
    let mut doc = Document::new(MockKernel::default());
    import(&mut doc, &block_dump());
    let def: FeatureDef<mitcad_model::ValueInput> = serde_json::from_value(json!({
        "type": "extrude", "profiles": [{"sketch": "F1", "region": "r{c5[c8,c6],c6[c5,c7],c7[c6,c8],c8[c7,c5]}"}],
        "extent": {"type": "distance", "distance": 5}, "operation": "new_body"}))
    .unwrap();
    let mut fork = doc.fork().expect("the mock kernel forks");
    let forked = fork.add_feature(&def, None).unwrap().uid;
    let extrudes = |d: &Document<MockKernel>| d.kernel().calls.borrow().get("extrude").copied();
    assert_eq!(extrudes(&fork), Some(1));
    assert_eq!(doc.adopt_results(&fork, &[forked]), 1);
    let before = extrudes(&doc);
    let added = doc.add_feature(&def, None).unwrap().uid;
    assert_eq!(added, forked);
    assert_eq!(extrudes(&doc), before, "evaluated again");
    assert_eq!(doc.status(added).and_then(|s| s.error()), None);
}

/// A combine named only by the items that made its bodies (the decoder's
/// `_f3d.producer`, mitcad#96), whose tool the file consumed: the history
/// keeps the tool as it was, so it is left out from the combine's state
/// on, and the combine comes in with its tool consumed.
#[test]
fn a_combine_finds_its_bodies_by_their_producers_and_consumes_its_tool() {
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(vec![
            two_rectangles(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        ]),
        &mut NoGeometry,
        &Options::default(),
    );
    let s0 = shapes(&reference);
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": small}],
               "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}),
    );
    let s1 = shapes(&reference);
    let tool = reference
        .body_shape("F3.b0".parse().unwrap())
        .unwrap()
        .clone();
    add(
        &mut reference,
        json!({"type": "combine", "target": "F2.b0", "tools": ["F3.b0"], "operation": "join"}),
    );
    let mut s2 = shapes(&reference);
    s2.push(tool);
    let produced = |producer: i64| {
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"producer": producer, "body_index": 0}})
    };
    let items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        extrude(2, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 3, "name": "Combine1", "objectType": "CombineFeature",
               "detail": {"targetBody": produced(1), "toolBodies": [produced(2)],
                          "operation": "JoinFeatureOperation", "isKeepToolBodies": false}}),
    ];
    let mut geometry = TestGeometry {
        states: vec![s0, s1, s2],
        items: HashMap::from([(1, 0), (2, 1), (3, 2)]),
        ids: true,
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let combine = &report.items[3];
    assert_eq!(combine.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(combine.verified, Some(true));
    let def: Value = serde_json::from_str(
        &doc.query(&format!(
            r#"{{"query": "feature", "uid": "{}"}}"#,
            combine.features[0]
        ))
        .unwrap(),
    )
    .unwrap();
    let tool = &report.items[2].features[0];
    assert_eq!(def["def"]["tools"], json!([format!("{tool}.b0")]));
    assert_ne!(def["def"]["keep_tools"], true);
    // The stored design without the consumed tool: nothing to replace.
    assert_eq!(doc.bodies().len(), 1);
    assert_eq!(report.bodies.len(), 1, "{}", report.text());
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

/// A fallback that takes a state where a body was split (mitcad#181): the
/// piece the file keeps as the body (`StoredBody::lineage`) keeps the body's
/// id, so that the split's other pieces are the bodies the file counts as
/// its new ones (a later cut names them by their index among those).
/// Without lineages the first piece of the state replaces the body.
#[test]
fn a_fallback_keeps_the_body_the_file_continues() {
    let whole = MockShape::imported("whole", 12);
    let kept = MockShape::imported("kept piece", 6);
    let first = MockShape::imported("first new piece", 7);
    let second = MockShape::imported("second new piece", 8);
    let items = vec![
        json!({"index": 0, "name": "Base Feature1", "objectType": "BaseFeature"}),
        json!({"index": 1, "name": "Split1", "objectType": "SplitBodyFeature", "detail": {}}),
    ];
    for lineages in [true, false] {
        let mut geometry = TestGeometry {
            states: vec![
                vec![whole.clone()],
                vec![first.clone(), kept.clone(), second.clone()],
            ],
            items: HashMap::from([(0, 0), (1, 1)]),
            ids: true,
            lineages: if lineages {
                vec![vec![1], vec![2, 1, 3]]
            } else {
                Vec::new()
            },
            ..TestGeometry::default()
        };
        let mut doc = Document::new(TestKernel::default());
        let report = import_design(
            &mut doc,
            &timeline_dump(items.clone()),
            &mut geometry,
            &Options::default(),
        );
        assert_eq!(
            report.items[1].outcome,
            Outcome::Fallback,
            "{}",
            report.text()
        );
        let base = &report.items[0].features[0];
        let body = doc
            .body_shape(format!("{base}.b0").parse().unwrap())
            .unwrap();
        let expected = if lineages {
            "kept piece"
        } else {
            "first new piece"
        };
        assert!(body.history.contains(expected), "{}", report.text());
    }
}

/// A split whose history holds two operations (the first piece cut off,
/// then the rest split), only the second named by the item, comes in as two
/// base features (mitcad#181): the bodies the item made are the new bodies
/// of both, in the file's order, the bodies the file keeps keeping their
/// ids (`StoredBody::lineage`). A later input naming the split's first new
/// body (`body_index` 0) is the piece the first operation cut off, as it is
/// after the second.
#[test]
fn a_split_in_two_operations_made_the_new_bodies_of_both() {
    let whole = MockShape::imported("whole", 12);
    let kept = MockShape::imported("kept piece", 6);
    let rest = MockShape::imported("rest", 9);
    let first = MockShape::imported("first new piece", 7);
    let second = MockShape::imported("second new piece", 8);
    let items = vec![
        json!({"index": 0, "name": "Base Feature1", "objectType": "BaseFeature"}),
        json!({"index": 1, "name": "Split1", "objectType": "SplitBodyFeature", "detail": {}}),
    ];
    let mut geometry = TestGeometry {
        states: vec![
            vec![whole.clone()],
            vec![rest.clone(), kept.clone()],
            vec![first.clone(), kept.clone(), second.clone()],
        ],
        items: HashMap::from([(0, 0), (1, 2)]),
        starts: HashMap::from([(1, 0)]),
        ids: true,
        lineages: vec![vec![1], vec![2, 1], vec![2, 1, 3]],
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel::default());
    let dump = timeline_dump(items);
    let options = Options::default();
    let mut importer = Importer {
        oracle: Oracle::new(&mut geometry),
        doc: &mut doc,
        dump: &dump,
        options: &options,
        params: ParamMap::default(),
        sketches: HashMap::new(),
        features: HashMap::new(),
        report: DesignReport::default(),
        pending: Vec::new(),
        defined: Default::default(),
        at_final: false,
        started: std::time::Instant::now(),
        components: components::Components::default(),
        component: ComponentUid::ROOT,
        sketch_copies: HashMap::new(),
        stopped_at: Cell::new(None),
        trying: false,
        alternatives: HashMap::new(),
        guesses: None,
        defer_guesses: false,
        kernel_failed: false,
        start_bases: HashMap::new(),
        deadline: None,
        captures: captures::Captures::default(),
        memory_released: 0,
        frame_entities: Default::default(),
        workers: Default::default(),
        learn: None,
        dressups: HashMap::new(),
        notes: Default::default(),
        suppressed: Default::default(),
        made_ahead: HashMap::new(),
        exact_pairs: None,
        waited: Default::default(),
        deviations: Default::default(),
        deviated: Default::default(),
        deviated_log: Default::default(),
        deviation_points: Default::default(),
        sources: None,
        linker: None,
        focus_paths: HashMap::new(),
        link_paths: Default::default(),
    };
    importer.run();
    assert_eq!(
        importer.report.items[1].outcome,
        Outcome::Fallback,
        "{}",
        importer.report.text()
    );
    let (made, own) = importer.made_by(1);
    assert!(!own);
    let histories: Vec<String> = made
        .iter()
        .map(|b| importer.doc.body_shape(*b).unwrap().history.clone())
        .collect();
    assert_eq!(histories.len(), 2, "{histories:?}");
    assert!(histories[0].contains("first new piece"), "{histories:?}");
    assert!(histories[1].contains("second new piece"), "{histories:?}");
}

/// The bodies a cut with decoded participants is tried on (mitcad#181):
/// where the history's state changed only the bodies the file names, not
/// all bodies as well (on all bodies the cut gives those the same, and the
/// others must stay); where it changed others too, all bodies are tried.
#[test]
fn named_participants_alone_when_the_history_changed_only_them() {
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(vec![
            two_rectangles(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        ]),
        &mut NoGeometry,
        &Options::default(),
    );
    let s0 = shapes(&reference);
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": small}],
               "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}),
    );
    let s1 = shapes(&reference);
    let items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        extrude(2, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let named = mitcad_f3d::design::ir::Reference::from_map(
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"producer": 1, "body_index": 0}})
        .as_object()
        .unwrap()
        .clone(),
    );
    let cut_first = MockShape::imported("the first block cut", 9);
    let cut_second = MockShape::imported("the second block cut", 9);
    for (after, alone) in [
        (vec![cut_first.clone(), s1[1].clone()], true),
        (vec![cut_first, cut_second], false),
    ] {
        let mut geometry = TestGeometry {
            // (The stored design is the bodies before the cut, so that the
            // end of the replay leaves them.)
            states: vec![s0.clone(), s1.clone(), after, s1.clone()],
            items: HashMap::from([(1, 0), (2, 1), (3, 2)]),
            ids: true,
            ..TestGeometry::default()
        };
        let mut doc = Document::new(TestKernel::default());
        let dump = timeline_dump(items.clone());
        let options = Options::default();
        let mut importer = Importer {
            oracle: Oracle::new(&mut geometry),
            doc: &mut doc,
            dump: &dump,
            options: &options,
            params: ParamMap::default(),
            sketches: HashMap::new(),
            features: HashMap::new(),
            report: DesignReport::default(),
            pending: Vec::new(),
            defined: Default::default(),
            at_final: false,
            started: std::time::Instant::now(),
            components: components::Components::default(),
            component: ComponentUid::ROOT,
            sketch_copies: HashMap::new(),
            stopped_at: Cell::new(None),
            trying: false,
            alternatives: HashMap::new(),
            guesses: None,
            defer_guesses: false,
            kernel_failed: false,
            start_bases: HashMap::new(),
            deadline: None,
            captures: captures::Captures::default(),
            memory_released: 0,
            frame_entities: Default::default(),
            workers: Default::default(),
            learn: None,
            dressups: HashMap::new(),
            notes: Default::default(),
            suppressed: Default::default(),
            made_ahead: HashMap::new(),
            exact_pairs: None,
            waited: Default::default(),
            deviations: Default::default(),
            deviated: Default::default(),
            deviated_log: Default::default(),
            deviation_points: Default::default(),
            sources: None,
            linker: None,
            focus_paths: HashMap::new(),
            link_paths: Default::default(),
        };
        importer.run();
        let first = importer.report.items[1].features[0].clone();
        let sketch = importer.sketches[&0].uid;
        let options = importer.participant_options(3, &Some(vec![named.clone()]), "cut", sketch);
        let decoded = vec![format!("{first}.b0")];
        if alone {
            assert_eq!(options, vec![decoded], "{}", importer.report.text());
        } else {
            assert_eq!(
                options,
                vec![decoded, Vec::new()],
                "{}",
                importer.report.text()
            );
        }
    }
}

/// A symmetric extent's length from the stream decoder (mitcad#96): tried
/// first, without a history the only one.
#[test]
fn decoded_symmetric_length_comes_first() {
    for full in [true, false] {
        let dump = timeline_dump(vec![
            rectangle_sketch(0),
            stream_extrude(
                1,
                "NewBodyFeatureOperation",
                json!({"operation_code": 4, "extent_a": 3, "extent_b": 2,
                       "direction_vector": [0.0, 0.0, 1.0], "full_length": full}),
                Some("d2"),
            ),
        ]);
        let mut doc = Document::new(MockKernel::default());
        import(&mut doc, &dump);
        let def: Value =
            serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap())
                .unwrap();
        assert_eq!(def["def"]["extent"]["type"], "symmetric");
        let length = def["def"]["extent"]["full_length"].as_bool();
        assert_eq!(length.unwrap_or(false), full);
    }
}

#[test]
fn an_item_the_file_keeps_no_result_for_is_skipped() {
    // mitcad#96: result number -1 with the third state byte set (its
    // change is in parts in other components, mitcad#180; the suppressed
    // ones are in `suppressed_tests`); one with none set is tried as
    // before.
    let mut fillet = fillet_item(2);
    fillet["_f3d"] = json!({"result_no": -1, "flags": "000001"});
    let mut failed = extrude(3, 0, "JoinFeatureOperation", "d2");
    failed["_f3d"] = json!({"result_no": -1, "flags": "000001"});
    let mut plain = extrude(4, 0, "NewBodyFeatureOperation", "d2");
    plain["_f3d"] = json!({"result_no": -1, "flags": "000000"});
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        fillet,
        failed,
        plain,
    ];
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    for i in [2, 3] {
        assert_eq!(
            report.items[i].outcome,
            Outcome::Skipped,
            "{}",
            report.text()
        );
        assert!(
            report.items[i]
                .note
                .as_deref()
                .is_some_and(|n| n.contains("keeps no result")),
            "{}",
            report.text()
        );
    }
    assert_eq!(
        report.items[4].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
}

#[test]
fn an_item_whose_state_the_history_does_not_hold_is_skipped() {
    // A feature suppressed in an `.ipt` part: its state is gone from the
    // history. It is not tried; the join after it gives its own state.
    let (mut items, mut geometry) = block_history(Vec::new());
    let mut joined = extrude(3, 0, "JoinFeatureOperation", "d2");
    joined["name"] = json!("Extrude3");
    items.push(joined);
    geometry.items = HashMap::from([(1, 1), (3, 2)]);
    geometry.without_result = vec![2];
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [
            Outcome::Partial,
            Outcome::Parametric,
            Outcome::Skipped,
            Outcome::Parametric
        ],
        "{}",
        report.text()
    );
    assert!(
        report.items[2]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("keeps no result")),
        "{}",
        report.text()
    );
    assert_eq!(report.items[3].verified, Some(true), "{}", report.text());
}

#[test]
fn an_item_whose_operation_changed_no_body_is_skipped_as_failed() {
    // mitcad#156: the history holds the item's operation, but it left the
    // bodies as they were (an extrusion that found no body to cut in the
    // file); no definition does that, so it is skipped, not a fallback.
    let (mut items, mut geometry) = block_history(Vec::new());
    let mut failed = extrude(3, 0, "JoinFeatureOperation", "d2");
    failed["name"] = json!("Extrude3");
    items.push(failed);
    geometry.items.insert(3, 2);
    geometry.unchanged = vec![3];
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let failed = &report.items[3];
    assert_eq!(failed.outcome, Outcome::Skipped, "{}", report.text());
    assert!(
        failed
            .note
            .as_deref()
            .is_some_and(|n| n.contains("failed in the file")),
        "{}",
        report.text()
    );
    assert!(failed.features.is_empty(), "{}", report.text());
    assert_eq!(report.items[2].outcome, Outcome::Parametric);
    // Without the history's word it is not called failed.
    let (mut items, mut geometry) = block_history(Vec::new());
    items.push(extrude(3, 0, "JoinFeatureOperation", "d2"));
    geometry.items.insert(3, 2);
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let note = report.items[3].note.as_deref().unwrap_or_default();
    assert!(!note.contains("failed in the file"), "{}", report.text());
}

#[test]
fn operations_before_an_item_s_own_may_be_its_own() {
    // mitcad#156: a pattern makes an operation per copy, and the item names
    // the last; the states of the others come between the item before and
    // its own. From the state the replay is at, the item gives its own
    // state, with no base feature for the states between.
    let (items, mut geometry) = block_history(Vec::new());
    let joined = geometry.states.pop().unwrap();
    let mut other = Document::new(TestKernel::default());
    import_design(
        &mut other,
        &timeline_dump(vec![
            circle_sketch(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        ]),
        &mut NoGeometry,
        &Options::default(),
    );
    geometry.states.push(shapes(&other));
    geometry.states.push(joined);
    geometry.items = HashMap::from([(1, 1), (2, 3)]);
    let run = |geometry: &mut TestGeometry| {
        let mut doc = Document::new(TestKernel::default());
        import_design(
            &mut doc,
            &timeline_dump(items.clone()),
            geometry,
            &Options::default(),
        )
    };
    // Without the history's word the state between comes in first, and the
    // join on it gives no state.
    let report = run(&mut geometry);
    assert_eq!(
        report.items[2].outcome,
        Outcome::Fallback,
        "{}",
        report.text()
    );
    geometry.starts = HashMap::from([(2, 1)]);
    let report = run(&mut geometry);
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert_eq!(report.items[2].verified, Some(true));
    assert!(report.warnings.is_empty(), "{}", report.text());
}

/// History states whose bodies belong to components: (shape, component
/// object, body id) per body.
#[derive(Default)]
struct OwnedStates {
    states: Vec<Vec<(MockShape, u64, u64)>>,
    items: HashMap<i64, usize>,
    components: HashMap<i64, Vec<u64>>,
}

impl StoredGeometry<MockShape> for OwnedStates {
    fn state_count(&mut self) -> usize {
        self.states.len()
    }

    fn item_state(&mut self, index: i64) -> Option<usize> {
        self.items.get(&index).copied()
    }

    fn state(&mut self, index: usize) -> Result<Vec<StoredBody<MockShape>>, String> {
        Ok(self.states[index]
            .iter()
            .map(|(shape, component, id)| StoredBody {
                shape: shape.clone(),
                name: None,
                source: format!("state {index} body {id}"),
                component: Some(*component),
                id: Some(*id),
                lineage: None,
            })
            .collect())
    }

    fn final_bodies(&mut self) -> Result<Vec<StoredBody<MockShape>>, String> {
        let last = self.states.len() - 1;
        self.state(last)
    }

    fn item_components(&mut self, index: i64) -> Option<Vec<u64>> {
        self.components.get(&index).cloned()
    }

    fn history_components(&mut self) -> Option<std::collections::HashSet<u64>> {
        Some(self.components.values().flatten().copied().collect())
    }
}

#[test]
fn a_component_made_from_bodies_takes_them_into_the_placed_component() {
    // mitcad#156: the root's block becomes the body of the component Base
    // (placed by the occurrence tree); Base's history starts with it, while
    // the root's keeps it as it was. The item moves the replay's block into
    // Base, and the root's copy leaves the history from its state on.
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(vec![
            rectangle_sketch(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        ]),
        &mut NoGeometry,
        &Options::default(),
    );
    let block = shapes(&reference).remove(0);
    let mut sketch = rectangle_sketch(0);
    sketch["_f3d"] = json!({"component": 3});
    let mut made = extrude(1, 0, "NewBodyFeatureOperation", "d2");
    made["_f3d"]["component"] = json!(3);
    let moved = json!({"index": 2, "name": "Body->Comp1", "objectType": "ComponentFromBodies",
                       "_f3d": {"component": 3, "result_no": -1, "made_result_no": 7}});
    let mut value: Value = serde_json::to_value(timeline_dump(vec![sketch, made, moved])).unwrap();
    value["document"] = json!({"root_component": "Assembly"});
    value["components"] = json!([
        {"name": "Assembly", "is_root": true, "_f3d": {"object_id": 3}},
        {"name": "Base", "_f3d": {"object_id": 10}}]);
    value["occurrences"] = json!([{"component": "Base", "_f3d": {"component_object": 10},
                                   "isReferencedComponent": false, "children": []}]);
    let dump = Dump::from_json(&value.to_string()).unwrap();
    let mut geometry = OwnedStates {
        states: vec![
            Vec::new(),
            vec![(block.clone(), 3, 1)],
            vec![(block.clone(), 3, 1), (block.clone(), 10, 2)],
        ],
        items: HashMap::from([(1, 1), (2, 2)]),
        components: HashMap::from([(1, vec![3]), (2, vec![10])]),
    };
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(&mut doc, &dump, &mut geometry, &Options::default());
    let item = &report.items[2];
    assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(item.note.as_deref(), Some("moves its body into Base"));
    let base = doc
        .assembly()
        .components
        .iter()
        .find(|c| c.name == "Base")
        .map(|c| c.uid)
        .unwrap();
    // One body, Base's, made by the root's extrusion; no other component.
    assert_eq!(doc.assembly().components.len(), 1, "{}", report.text());
    let bodies = doc.bodies();
    assert_eq!(bodies.len(), 1);
    assert_eq!(doc.body_component(bodies[0].uid), Some(base));
    assert_eq!(report.history.matched, 2, "{}", report.text());
    assert!(
        report.warnings.iter().all(|w| !w.contains("replaces")),
        "{}",
        report.text()
    );
}

#[test]
fn items_that_make_components_together_each_take_one() {
    // Three items make two components in one history state: the first item
    // moves a body into each of them (one feature per component), the
    // second item takes the second component's feature, and the state is
    // matched once.
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(vec![
            rectangle_sketch(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        ]),
        &mut NoGeometry,
        &Options::default(),
    );
    let block = shapes(&reference).remove(0);
    let mut sketch = rectangle_sketch(0);
    sketch["_f3d"] = json!({"component": 3});
    let mut first = extrude(1, 0, "NewBodyFeatureOperation", "d2");
    first["_f3d"]["component"] = json!(3);
    let mut second = extrude(2, 0, "NewBodyFeatureOperation", "d4");
    second["_f3d"]["component"] = json!(3);
    let moved = |index: i64, name: &str| {
        json!({"index": index, "name": name, "objectType": "ComponentFromBodies",
               "_f3d": {"component": 3, "result_no": -1, "made_result_no": 7}})
    };
    let mut value: Value = serde_json::to_value(timeline_dump(vec![
        sketch,
        first,
        second,
        moved(3, "Body->Comp1"),
        moved(4, "Body->Comp2"),
    ]))
    .unwrap();
    value["document"] = json!({"root_component": "Assembly"});
    value["components"] = json!([
        {"name": "Assembly", "is_root": true, "_f3d": {"object_id": 3}},
        {"name": "One", "_f3d": {"object_id": 10}},
        {"name": "Two", "_f3d": {"object_id": 11}}]);
    value["occurrences"] = json!([
        {"component": "One", "_f3d": {"component_object": 10},
         "isReferencedComponent": false, "children": []},
        {"component": "Two", "_f3d": {"component_object": 11},
         "isReferencedComponent": false, "children": []}]);
    let dump = Dump::from_json(&value.to_string()).unwrap();
    let mut geometry = OwnedStates {
        states: vec![
            Vec::new(),
            vec![(block.clone(), 3, 1)],
            vec![(block.clone(), 3, 1), (block.clone(), 3, 2)],
            vec![
                (block.clone(), 3, 1),
                (block.clone(), 3, 2),
                (block.clone(), 10, 3),
                (block.clone(), 11, 4),
            ],
        ],
        items: HashMap::from([(1, 1), (2, 2), (3, 3), (4, 3)]),
        components: HashMap::from([
            (1, vec![3]),
            (2, vec![3]),
            (3, vec![10, 11]),
            (4, vec![10, 11]),
        ]),
    };
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(&mut doc, &dump, &mut geometry, &Options::default());
    let one = &report.items[3];
    let two = &report.items[4];
    assert_eq!(one.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(two.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(one.note.as_deref(), Some("moves its body into One"));
    assert_eq!(two.note.as_deref(), Some("moves its body into Two"));
    assert_eq!(one.features.len(), 1);
    assert_eq!(two.features.len(), 1);
    assert_ne!(one.features, two.features);
    let bodies = doc.bodies();
    assert_eq!(bodies.len(), 2, "{}", report.text());
    let mut in_components: Vec<String> = bodies
        .iter()
        .filter_map(|b| doc.body_component(b.uid))
        .map(|c| doc.assembly().name(c))
        .collect();
    in_components.sort();
    assert_eq!(in_components, ["One", "Two"], "{}", report.text());
}

#[test]
fn a_fillet_with_edge_references_the_file_lost_says_so() {
    // The file names an edge that is not in the model where the fillet was
    // made (no edge lies between the faces it names): the application that
    // wrote the file keeps the geometry the item had before. The history's
    // state is not what any definition of the item's inputs makes (the
    // state's radius is another), so it falls back, and its note says why.
    use mitcad_model::FilletSize;
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let kernel = TestKernel {
        edge_points: true,
        ..TestKernel::default()
    };
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let block = shapes(&reference);
    let data = kernel.brep_data(&block[0]).unwrap();
    let (stored, _) = kernel.import_brep(FeatureUid(9), &data, 0).unwrap();
    let edge = EdgeName::new(stored.faces[0].clone(), stored.faces[1].clone());
    let set = FilletSet {
        edges: std::slice::from_ref(&edge),
        faces: &[],
        size: FilletSize::Constant { radius: 3.0 },
        tangent_chain: true,
        curvature: false,
        weight: 0.0,
    };
    let filleted = kernel
        .fillet(FeatureUid(10), &stored, &[set], true)
        .unwrap();
    let lost = |found: &str| {
        json!({"index": 2, "name": "Fillet1", "objectType": "FilletFeature",
        "detail": {"edgeSets": [{"radius": param("d5", "2 mm", 0.2, "mm"), "edges": [{
            "kind": "edge",
            "_f3d": {"found": found, "recipe": "edge",
                     "entities": [[{"tag": "3", "kind": 0, "ops": [301]}],
                                  [{"tag": "4", "kind": 0, "ops": [301]}]]}
        }]}]}})
    };
    let import = |found: &str| {
        let mut with = items.clone();
        with.push(lost(found));
        let mut geometry = TestGeometry {
            states: vec![Vec::new(), block.clone(), vec![filleted.clone()]],
            items: HashMap::from([(1, 1), (2, 2)]),
            ..TestGeometry::default()
        };
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        import_design(
            &mut doc,
            &timeline_dump(with),
            &mut geometry,
            &Options::default(),
        )
    };
    for found in [
        "no edge between the faces named",
        "a face of the edge is not in the history state",
    ] {
        let report = import(found);
        let item = &report.items[2];
        assert_eq!(item.outcome, Outcome::Fallback, "{}", report.text());
        let note = item.note.clone().unwrap_or_default();
        assert!(
            note.starts_with("1 of its 1 edge references are lost in the file"),
            "{note}"
        );
    }
    // Other reasons the decoder found no edge are not the file's.
    let report = import("2 edges fit the names");
    let note = report.items[2].note.clone().unwrap_or_default();
    assert!(!note.contains("are lost in the file"), "{note}");
}

// Work beside the replay in parallel (mitcad#103).

#[test]
fn in_order_hands_the_results_over_in_order() {
    for threads in [1, 2, 8] {
        for count in [0, 1, 5, 40] {
            let mut seen = Vec::new();
            crate::in_order(
                count,
                threads,
                |k| {
                    // Later ones finish first.
                    std::thread::sleep(std::time::Duration::from_micros(((count - k) * 50) as u64));
                    k * k
                },
                |k, square| seen.push((k, square)),
            );
            let expected: Vec<(usize, usize)> = (0..count).map(|k| (k, k * k)).collect();
            assert_eq!(seen, expected, "{threads} threads, {count} items");
        }
    }
}

#[test]
fn in_order_passes_a_panic_on() {
    let caught = std::panic::catch_unwind(|| {
        crate::in_order(
            20,
            4,
            |k| {
                assert!(k != 7, "item 7");
                k
            },
            |_, _| {},
        );
    });
    assert!(caught.is_err());
}

#[test]
fn in_order_shares_its_threads_with_work_beside_it() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let share = crate::Share::new(4);
    let a = share.take(1).expect("four left");
    let b = share.take(1).expect("three left");
    let c = share.take(1).expect("two left");
    assert!(share.take(1).is_none(), "the workers keep one");
    drop(c);
    assert_eq!(share.left(), 2);
    // Two threads taken: at most two workers at once, until one is given
    // back half way.
    let (running, most, done) = (
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    );
    let given = std::sync::Mutex::new(Some(a));
    let mut seen = Vec::new();
    crate::in_order_sharing(
        40,
        4,
        Some(&share),
        |k| {
            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
            if done.load(Ordering::SeqCst) < 20 {
                most.fetch_max(now, Ordering::SeqCst);
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
            running.fetch_sub(1, Ordering::SeqCst);
            if done.fetch_add(1, Ordering::SeqCst) + 1 == 20 {
                given.lock().unwrap().take();
            }
            k * 2
        },
        |k, twice| seen.push((k, twice)),
    );
    assert!(most.load(Ordering::SeqCst) <= 2, "{most:?}");
    assert_eq!(seen, (0..40).map(|k| (k, k * 2)).collect::<Vec<_>>());
    drop(b);
    assert_eq!(share.left(), 4);
}

#[test]
fn threads_beside_the_import_share_one_budget() {
    let progress = Progress::new();
    progress.set_threads(3);
    let a = progress.helper().expect("one of three");
    let b = progress.helper().expect("two of three");
    assert!(progress.helper().is_none(), "the import's own is the third");
    drop(a);
    let c = progress.helper().expect("given back");
    drop((b, c));
    // Not while memory is short.
    progress.set_memory_share(0.55);
    assert!(progress.helper().is_none());
    progress.set_memory_share(0.1);
    assert!(progress.helper().is_some());
    progress.set_threads(1);
    assert!(progress.helper().is_none(), "one thread: the import's own");
}

#[test]
fn planar_faces_are_found_once_per_body_shape() {
    let mut doc = Document::new(MockKernel::default());
    import(&mut doc, &block_dump());
    let found = || crate::refs::planar_faces(&doc);
    let plain = found();
    assert!(!plain.is_empty());
    let kept = crate::refs::KeptPlanes::new();
    assert_eq!(found(), plain);
    assert_eq!(found(), plain);
    // Kept by version: the same version is not looked at again.
    let finds = std::cell::Cell::new(0);
    let find = || {
        finds.set(finds.get() + 1);
        Vec::new()
    };
    crate::refs::planes_of(1, find);
    crate::refs::planes_of(1, find);
    crate::refs::planes_of(2, find);
    assert_eq!(finds.get(), 2);
    drop(kept);
    // Not kept outside an import.
    crate::refs::planes_of(1, find);
    assert_eq!(finds.get(), 3);
}

/// A combine whose target is in one component and its tool in another
/// (the file's assembly context, mitcad#104): the tool comes in by its
/// occurrence link, and the combine goes where its target is.
#[test]
fn a_combine_takes_a_tool_of_another_component_by_its_link() {
    let produced = |producer: i64| {
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"producer": producer, "body_index": 0}})
    };
    let owned = |mut item: Value, owner: u64| {
        item["_f3d"]["component"] = json!(owner);
        item
    };
    let mut value: Value = serde_json::to_value(timeline_dump(vec![
        owned(rectangle_sketch(0), 3),
        owned(extrude(1, 0, "NewBodyFeatureOperation", "d2"), 3),
        owned(rectangle_sketch(2), 10),
        owned(extrude(3, 2, "NewBodyFeatureOperation", "d2"), 10),
        owned(
            json!({"index": 4, "name": "Combine1", "objectType": "CombineFeature",
                   "detail": {"targetBody": produced(3), "toolBodies": [produced(1)],
                              "operation": "CutFeatureOperation", "isKeepToolBodies": true}}),
            10,
        ),
    ]))
    .unwrap();
    value["document"] = json!({"root_component": "Assembly"});
    value["components"] = json!([
        {"name": "Assembly", "is_root": true, "_f3d": {"object_id": 3}},
        {"name": "Plate", "_f3d": {"object_id": 10}}]);
    value["occurrences"] = json!([
        {"component": "Plate", "_f3d": {"component_object": 10},
         "transform": [[1.0, 0.0, 0.0, 5.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0],
                       [0.0, 0.0, 0.0, 1.0]]}]);
    let dump = Dump::from_json(&value.to_string()).unwrap();
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    let combine = &report.items[4];
    assert_eq!(combine.outcome, Outcome::Parametric, "{}", report.text());
    let uid: FeatureUid = combine.features[0].parse().unwrap();
    let plate = doc.assembly().components[0].uid;
    assert_eq!(doc.feature(uid).unwrap().component, plate);
    let def: Value = serde_json::from_str(
        &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
            .unwrap(),
    )
    .unwrap();
    let tool = format!("{}.b0", report.items[1].features[0]);
    assert_eq!(def["def"]["tools"], json!([tool]));
    assert_eq!(
        def["def"]["tool_links"],
        json!({tool.clone(): {"target": "O1"}})
    );
    // The root's block, moved into Plate (5 cm back along X), cut it.
    let target = format!("{}.b0", report.items[3].features[0]);
    let cut = &doc.body_shape(target.parse().unwrap()).unwrap().history;
    assert!(
        cut.starts_with("cut(") && cut.contains(",-50,0,0)"),
        "{cut}"
    );
}

/// A two-sided extrusion with both sides up to faces the replay does not
/// have (mitcad#104): each side goes up to its face's plane, side one
/// towards the side of the sketch its face lies on.
#[test]
fn two_sided_extrusion_goes_up_to_both_decoded_planes() {
    let face = |z: f64| {
        json!({"kind": "face", "objectType": "BRepFace",
               "point_on_face": [1.0, 1.0, z],
               "geometry": {"type": "Plane", "origin": [0.0, 0.0, z], "normal": [0.0, 0.0, 1.0]},
               "_f3d": {"recipe": "bounded_face", "entities": [[{"tag": "9", "ops": [5]}]]}})
    };
    let mut item = stream_extrude(
        1,
        "NewBodyFeatureOperation",
        json!({"operation_code": 4, "extent_a": 2, "extent_b": 1, "flag": 0,
               "direction_vector": [0.0, 0.0, 1.0], "slot_roles": [17, 17, 65]}),
        None,
    );
    item["detail"]["extentType"] = json!("TwoSidesFeatureExtentType");
    item["detail"]["extentOne"] =
        json!({"_type": "ToEntityExtentDefinition", "entity": face(-0.3)});
    item["detail"]["extentTwo"] = json!({"_type": "ToEntityExtentDefinition", "entity": face(0.5)});
    let dump = timeline_dump(vec![rectangle_sketch(0), item]);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_eq!(
        report.items[1].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    let extent = &def["def"]["extent"];
    assert_eq!(extent["type"], "two_sides", "{def}");
    let origin = |side: &str| extent[side]["object"]["plane"]["origin"][2].as_f64();
    assert_eq!((origin("side1"), origin("side2")), (Some(-3.0), Some(5.0)));
    assert_eq!(def["def"]["flip"], true);
}

/// A two-sided extrusion up to two objects is read when the stream gives
/// its side codes and start, side one's code and side two's block of the
/// object (its nearest-solution byte), and the direction follows the side
/// of the sketch side one's object lies on.
#[test]
fn two_sides_up_to_objects_are_read_with_the_second_objects_block() {
    let face = |z: f64| {
        json!({"kind": "face", "objectType": "BRepFace",
               "point_on_face": [1.0, 1.0, z],
               "geometry": {"type": "Plane", "origin": [0.0, 0.0, z], "normal": [0.0, 0.0, 1.0]},
               "_f3d": {"recipe": "bounded_face", "entities": [[{"tag": "9", "kind": 0, "ops": [5]}]]}})
    };
    let provenance = |second_minimum: Option<bool>| {
        let mut items = decoded_small_rectangle();
        let mut item = stream_extrude(
            1,
            "NewBodyFeatureOperation",
            json!({"operation_code": 4, "extent_a": 2, "extent_b": 1, "flag": 0,
                   "direction_vector": [0.0, 0.0, 1.0], "slot_roles": [17, 17, 65],
                   "solid": true, "start": 0, "sides": [2, 0],
                   "thin": false, "thin_location": 0}),
            None,
        );
        item["detail"]["extentType"] = json!("TwoSidesFeatureExtentType");
        let one = json!({"_type": "ToEntityExtentDefinition", "entity": face(-0.3),
                             "isMinimumSolution": false});
        let mut two = json!({"_type": "ToEntityExtentDefinition", "entity": face(0.5)});
        if let Some(m) = second_minimum {
            two["isMinimumSolution"] = json!(m);
        }
        item["detail"]["extentOne"] = one;
        item["detail"]["extentTwo"] = two;
        let mut doc = Document::new(TestKernel::default());
        item["detail"]["_f3d_profile_loops"] = items[1]["detail"]["_f3d_profile_loops"].clone();
        items[1] = item;
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        );
        assert_eq!(
            report.items[1].outcome,
            Outcome::Parametric,
            "{}",
            report.text()
        );
        report.items[1].input_provenance
    };
    assert_eq!(provenance(Some(false)), Some(InputProvenance::Read));
    assert_ne!(provenance(None), Some(InputProvenance::Read));
}

#[test]
fn a_shell_reports_when_its_body_is_inferred_from_the_only_solid() {
    let flags = json!({"isTangentChain": true, "shellType": "RoundedOffsetShellType"});
    for (body, with_flags, provenance) in [
        (
            json!({"kind": "body", "objectType": "BRepBody",
                   "_f3d": {"producer": 1, "body_index": 0}}),
            false,
            InputProvenance::Unknown,
        ),
        // The body, thickness, type and tangent-chain flag all decoded.
        (
            json!({"kind": "body", "objectType": "BRepBody",
                   "_f3d": {"producer": 1, "body_index": 0}}),
            true,
            InputProvenance::Read,
        ),
        (
            json!({"kind": "body", "objectType": "BRepBody"}),
            false,
            InputProvenance::Inferred,
        ),
    ] {
        let mut detail = json!({
            "inputEntities": [body],
            "insideThickness": {"kind": "parameter", "value": 0.1}
        });
        if with_flags {
            for (key, value) in flags.as_object().unwrap() {
                detail[key] = value.clone();
            }
        }
        let items = vec![
            rectangle_sketch(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
            json!({"index": 2, "objectType": "ShellFeature", "detail": detail}),
        ];
        let mut doc = Document::new(MockKernel::default());
        let report = import(&mut doc, &timeline_dump(items));
        let shell = &report.items[2];
        assert_eq!(shell.outcome, Outcome::Parametric, "{}", report.text());
        assert_eq!(shell.input_provenance, Some(provenance));
        let feature: Value = serde_json::from_str(
            &doc.query(&json!({"query": "feature", "uid": shell.features[0]}).to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(feature["def"]["body"], "F2.b0");
    }
}

/// A shell of the block whose removed face the dump does not name, only
/// how many it removes (the `.ipt` import's): the face the next history
/// state lost.
#[test]
fn a_shell_removes_the_faces_the_history_lost() {
    let shell = json!({"index": 2, "name": "Shell1", "objectType": "ShellFeature",
        "detail": {"inputEntities": [{"kind": "body", "objectType": "BRepBody",
                                      "_f3d": {"producer": 1, "body_index": 0}}],
                   "insideThickness": {"kind": "parameter", "value": 0.1},
                   "isTangentChain": false, "shellType": "SharpOffsetShellType",
                   "_ipt_removed_faces": 1}});
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        shell,
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(items[..2].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let block = shapes(&reference);
    let top = block[0]
        .faces
        .iter()
        .find(|f| f.to_string().starts_with("F2:end("))
        .expect("the block's top")
        .to_string();
    add(
        &mut reference,
        json!({"type": "shell", "body": "F2.b0", "faces": [top], "inside": 1,
               "tangent_chain": false}),
    );
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), block, shapes(&reference)],
        items: HashMap::from([(1, 1), (2, 2)]),
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items.clone()),
        &mut geometry,
        &Options::default(),
    );
    let item = &report.items[2];
    assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
    let def: Value = serde_json::from_str::<Value>(
        &doc.query(&format!(
            r#"{{"query": "feature", "uid": "{}"}}"#,
            item.features[0]
        ))
        .unwrap(),
    )
    .unwrap()["def"]
        .clone();
    assert_eq!(def["faces"], json!([top]));

    // Without a history the faces are not known.
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_ne!(report.items[2].outcome, Outcome::Parametric);
}

/// A combine whose history state is a cut while its detail says join: the
/// history's guesses find the cut, unless the `.ipt` decoder read the
/// operation from the record (`_ipt_inputs`), whose operation is not
/// guessed (mitcad#60: a guessed cut of a coil's body took gigabytes).
#[test]
fn a_combine_operation_read_from_its_record_is_not_guessed() {
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(vec![
            two_rectangles(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        ]),
        &mut NoGeometry,
        &Options::default(),
    );
    let s0 = shapes(&reference);
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": small}],
               "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}),
    );
    let s1 = shapes(&reference);
    add(
        &mut reference,
        json!({"type": "combine", "target": "F2.b0", "tools": ["F3.b0"], "operation": "cut",
               "keep_tools": true}),
    );
    let s2 = shapes(&reference);
    let produced = |producer: i64| {
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"producer": producer, "body_index": 0}})
    };
    let outcome = |laid_out: bool| {
        let mut detail = json!({"targetBody": produced(1), "toolBodies": [produced(2)],
                                "operation": "JoinFeatureOperation", "isKeepToolBodies": true});
        if laid_out {
            detail["_ipt_inputs"] = json!(true);
        }
        let items = vec![
            two_rectangles(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
            extrude(2, 0, "NewBodyFeatureOperation", "d2"),
            json!({"index": 3, "name": "Combine1", "objectType": "CombineFeature",
                   "detail": detail}),
        ];
        let mut geometry = TestGeometry {
            states: vec![s0.clone(), s1.clone(), s2.clone()],
            items: HashMap::from([(1, 0), (2, 1), (3, 2)]),
            ids: true,
            ..TestGeometry::default()
        };
        let mut doc = Document::new(TestKernel::default());
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut geometry,
            &Options::default(),
        );
        (report.items[3].outcome, report.text())
    };
    let (guessed, text) = outcome(false);
    assert_eq!(guessed, Outcome::Parametric, "{text}");
    let (read, text) = outcome(true);
    assert_ne!(read, Outcome::Parametric, "{text}");
}

#[test]
fn a_part_of_an_item_in_another_component_is_replayed_after_it() {
    // mitcad#169: the item changed no body of its own component (no result
    // of its own); its part in the other component is an item of its own
    // after the timeline's items, at the item's point of the timeline.
    let mut item = extrude(1, 0, "NewBodyFeatureOperation", "d2");
    item["_f3d"]["result_no"] = json!(-1);
    item["_f3d"]["flags"] = json!("000001");
    let mut part = extrude(2, 0, "NewBodyFeatureOperation", "d2");
    part["name"] = json!("Extrude1");
    part["_f3d"]["part_of"] = json!(1);
    let mut doc = Document::new(MockKernel::default());
    let report = import(
        &mut doc,
        &timeline_dump(vec![rectangle_sketch(0), item, part]),
    );
    assert_eq!(
        report.items[1].outcome,
        Outcome::Skipped,
        "{}",
        report.text()
    );
    assert_eq!(
        report.items[1].note.as_deref(),
        Some("it changed no body of its own component; its change in another component is item 2")
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert!(
        report.items[2]
            .note
            .as_deref()
            .is_some_and(|n| n.starts_with("part of item 1: its change in this component")),
        "{}",
        report.text()
    );
    assert_eq!(doc.bodies().len(), 1);
}

#[test]
fn a_sketch_whose_points_lie_off_its_frame_is_placed_where_they_are() {
    // mitcad#169: a sketch on a face keeps its points 1.6 mm along its
    // normal from its stored frame; the sketch goes where they are.
    let mut sketch = rectangle_sketch(0);
    sketch["detail"]["referencePlane"] = json!({"kind": "face"});
    for p in sketch["detail"]["points"].as_array_mut().unwrap() {
        p["xyz"][2] = json!(0.16);
    }
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &timeline_dump(vec![sketch]));
    assert_ne!(
        report.items[0].outcome,
        Outcome::Skipped,
        "{}",
        report.text()
    );
    let uid = report.items[0]
        .features
        .iter()
        .filter_map(|f| f.parse::<mitcad_model::FeatureUid>().ok())
        .find(|uid| doc.sketch_output(*uid).is_some())
        .expect("the sketch");
    let origin = doc.sketch_output(uid).unwrap().frame.origin;
    assert!((origin[2] - 1.6).abs() < 1e-9, "{origin:?}");
}

/// A modelled thread's groove found where the faces cross a segment one
/// pitch long at the pitch radius (mitcad#161): its middle, also where the
/// groove runs across the segment's ends; none for two grooves.
#[test]
fn a_thread_groove_middle_comes_from_the_crossings() {
    use crate::ops::groove_middle;
    // Out at 0.2, in at 0.7: the groove's middle is at 0.45.
    let middle = groove_middle(&[(0.2, false), (0.7, true)]).unwrap();
    assert!((middle - 0.45).abs() < 1e-12, "{middle}");
    // In at 0.3, out at 0.8: the groove runs from 0.8 round to 0.3, its
    // middle at 0.05.
    let middle = groove_middle(&[(0.3, true), (0.8, false)]).unwrap();
    assert!((middle - 0.05).abs() < 1e-12, "{middle}");
    assert_eq!(
        groove_middle(&[(0.1, false), (0.2, true), (0.5, false), (0.6, true)]),
        None
    );
}

/// A join or intersection that consumes its tools gives the same solid
/// whichever of its bodies is the target (mitcad#181): such combines share
/// a key, those keeping their tools or cutting have none.
#[test]
fn combines_of_the_same_bodies_give_the_same_result() {
    let combine = |target: &str, tools: &[&str], operation: &str, keep: bool| {
        Candidate::new(json!({"type": "combine", "target": target, "tools": tools,
                              "operation": operation, "keep_tools": keep}))
    };
    let a = same_result_combine(&combine("F1.b0", &["F2.b0", "F3.b0"], "join", false));
    let b = same_result_combine(&combine("F2.b0", &["F1.b0", "F3.b0"], "join", false));
    assert!(a.is_some());
    assert_eq!(a, b);
    assert_ne!(
        a,
        same_result_combine(&combine("F1.b0", &["F2.b0"], "join", false))
    );
    assert!(same_result_combine(&combine("F1.b0", &["F2.b0"], "intersect", false)).is_some());
    assert_eq!(
        same_result_combine(&combine("F1.b0", &["F2.b0"], "join", true)),
        None
    );
    assert_eq!(
        same_result_combine(&combine("F1.b0", &["F2.b0"], "cut", false)),
        None
    );
}

// Construction points, planes and axes from what they are made from
// (mitcad#186).

#[test]
fn construction_datums_follow_their_definitions() {
    let origin_plane = |name: &str| json!({"kind": "construction_plane", "name": name, "origin": name, "timeline_index": null});
    let point_ref = |index: i64| {
        json!({"kind": "construction_point", "name": format!("WorkPoint{}", index + 1),
               "origin": null, "timeline_index": index})
    };
    let fixed_point = |index: i64, at: [f64; 3]| {
        json!({"index": index, "name": format!("WorkPoint{}", index + 1),
               "objectType": "ConstructionPoint", "detail": {"geometry": at}})
    };
    let items = vec![
        // Where the three origin planes meet.
        json!({"index": 0, "name": "Center", "objectType": "ConstructionPoint",
               "detail": {"geometry": [0.0, 0.0, 0.0], "definition": {
                   "_type": "ConstructionPointThreePlanesDefinition",
                   "planeOne": origin_plane("XY"), "planeTwo": origin_plane("YZ"),
                   "planeThree": origin_plane("XZ")}}}),
        // Points the file stores only by their positions (cm).
        fixed_point(1, [0.3, 0.0, 0.0]),
        fixed_point(2, [0.0, 0.4, 0.0]),
        fixed_point(3, [0.0, 0.0, 0.5]),
        // The plane through three points: x/30 + y/40 + z/50 = 1.
        json!({"index": 4, "name": "Through", "objectType": "ConstructionPlane",
               "detail": {"definition": {"_type": "ConstructionPlaneThreePointsDefinition",
                                         "pointEntityOne": point_ref(1),
                                         "pointEntityTwo": point_ref(2),
                                         "pointEntityThree": point_ref(3)},
                          "geometry": {"type": "Plane", "origin": [0.3, 0.0, 0.0],
                                       "normal": [0.4, 0.3, 0.24]}}}),
        // Parallel to the XY plane through the third point.
        json!({"index": 5, "name": "Level", "objectType": "ConstructionPlane",
               "detail": {"definition": {"_type": "ConstructionPlaneOffsetThroughPointDefinition",
                                         "planarEntity": origin_plane("XY"),
                                         "point": point_ref(3)},
                          "geometry": {"type": "Plane", "origin": [0.0, 0.0, 0.5],
                                       "normal": [0.0, 0.0, 1.0]}}}),
        // Where the XY and XZ planes meet: the X axis, in the file's
        // direction whichever way Mitcad's rule runs.
        json!({"index": 6, "name": "Edge", "objectType": "ConstructionAxis",
               "detail": {"definition": {"_type": "ConstructionAxisTwoPlaneDefinition",
                                         "planarEntityOne": origin_plane("XY"),
                                         "planarEntityTwo": origin_plane("XZ")},
                          "geometry": {"type": "InfiniteLine3D", "origin": [0.0, 0.0, 0.0],
                                       "direction": [1.0, 0.0, 0.0]}}}),
    ];
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &timeline_dump(items));
    let read = |i: usize| report.items[i].input_provenance;
    for (i, outcome) in [
        (0, Outcome::Parametric),
        (1, Outcome::Partial),
        (2, Outcome::Partial),
        (3, Outcome::Partial),
        (4, Outcome::Parametric),
        (5, Outcome::Parametric),
        (6, Outcome::Parametric),
    ] {
        assert_eq!(report.items[i].outcome, outcome, "{}", report.text());
    }
    let def = |name: &str| {
        let uid = doc.features().find(|f| f.name == name).unwrap().uid;
        let v: Value = serde_json::from_str(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                .unwrap(),
        )
        .unwrap();
        v["def"].clone()
    };
    // Each is the definition the file has, read from its inputs.
    assert_eq!(def("Center")["definition"]["type"], "three_planes");
    assert_eq!(read(0), Some(InputProvenance::Read));
    assert_eq!(def("Through")["definition"]["type"], "three_points");
    assert_eq!(read(4), Some(InputProvenance::Read));
    assert_eq!(def("Level")["definition"]["type"], "offset_through_point");
    assert_eq!(read(5), Some(InputProvenance::Read));
    assert_eq!(def("Edge")["definition"]["type"], "two_planes");
    // The axis lies on the file's line in the file's direction.
    let axis = doc.datum(doc.features().find(|f| f.name == "Edge").unwrap().uid);
    match axis {
        Some(mitcad_model::datum::Datum::Axis(a)) => {
            assert!((a.direction[0] - 1.0).abs() < 1e-9, "{a:?}");
        }
        other => panic!("{other:?}"),
    }
    let level = doc.datum(doc.features().find(|f| f.name == "Level").unwrap().uid);
    match level {
        Some(mitcad_model::datum::Datum::Plane(p)) => {
            assert!((p.origin[2] - 5.0).abs() < 1e-9, "{p:?}");
        }
        other => panic!("{other:?}"),
    }
}

/// mitcad#186: what ties a text box's lines to their text, or a control
/// point spline's polygon to its spline, constrains no geometry and
/// leaves the sketch whole; a horizontal line on construction lines is a
/// constraint as any other.
#[test]
fn text_box_and_control_polygon_constraints_leave_the_sketch_whole() {
    let line = |id: &str, a: &str, b: &str| {
        let mut c = gline(id, a, b);
        c["isConstruction"] = json!(true);
        c
    };
    let points = vec![
        gpoint("p0", 0.0, 0.0),
        gpoint("p1", 4.0, 0.0),
        gpoint("p2", 4.0, 1.0),
        gpoint("p3", 0.0, 1.0),
    ];
    let curves = vec![
        line("c0", "p0", "p1"),
        line("c1", "p1", "p2"),
        line("c2", "p2", "p3"),
        line("c3", "p3", "p0"),
    ];
    let constraints = json!([
        {"id": "k0", "type": "TextBoxConstraint",
         "refs": {"entities": ["c0", "c1", "c2", "c3"]}},
        {"id": "k1", "type": "ControlPolygonConstraint",
         "refs": {"entities": ["c0", "c1"]}},
        {"id": "k2", "type": "HorizontalConstraint", "refs": {"entities": ["c0"]}},
    ]);
    let dump = group_sketch_dump(Vec::new(), points, curves, constraints, json!([]));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let s = sketch_query(&doc);
    let kinds: Vec<&str> = s["constraints"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|k| k["type"].as_str())
        .collect();
    assert_eq!(kinds, ["horizontal"], "{s}");
}

#[test]
fn three_collinear_points_lie_on_the_line_between_two_of_them() {
    // A centre line from p1 to p2 and the point p0 that it passes: the
    // stored constraint names the three points, not the line.
    let points = vec![
        gpoint("p0", 0.0, 0.0),
        gpoint("p1", 0.0, 2.0),
        gpoint("p2", 0.0, -0.3),
        gpoint("p3", 1.0, 0.5),
    ];
    let curves = vec![gline("c0", "p1", "p2")];
    let constraints = json!([
        {"id": "k0", "type": "CollinearConstraint",
         "refs": {"entities": ["p1", "p0", "p2"]}},
    ]);
    let dump = group_sketch_dump(
        Vec::new(),
        points.clone(),
        curves.clone(),
        constraints,
        json!([]),
    );
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let s = sketch_query(&doc);
    let kept: Vec<&Value> = s["constraints"].as_array().unwrap().iter().collect();
    assert_eq!(kept.len(), 1, "{s}");
    assert_eq!(kept[0]["type"], "coincident", "{s}");
    let line = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "line")
        .unwrap();
    assert_eq!(kept[0]["entity"], line["id"], "{s}");
    assert_eq!(kept[0]["point"], point_at(&s, [0.0, 0.0]), "{s}");

    // No line between two of the points: the constraint is left out, and
    // the sketch says so.
    let constraints = json!([
        {"id": "k0", "type": "CollinearConstraint",
         "refs": {"entities": ["p1", "p0", "p3"]}},
    ]);
    let dump = group_sketch_dump(Vec::new(), points, curves, constraints, json!([]));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert!(
        report.text().contains("collinear points without a line"),
        "{}",
        report.text()
    );
}

#[test]
fn an_offset_without_curves_binds_nothing() {
    // The offset's curves are gone: its stored list is empty. The circle
    // keeps its other constraints and the sketch is whole.
    let points = vec![gfixed("p0", 0.0, 0.0)];
    let curves = vec![gcircle("c0", "p0", 2.0)];
    let constraints = json!([
        {"id": "k0", "type": "OffsetConstraint", "refs": {"entities": ["c0"]},
         "props": {"parentCurves": ["c0"], "childCurves": [], "distance": 0.5}},
    ]);
    let dump = group_sketch_dump(Vec::new(), points, curves, constraints, json!([]));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    assert!(report.items[0].note.is_none(), "{}", report.text());
    let s = sketch_query(&doc);
    assert!(s["offsets"].as_array().unwrap().is_empty(), "{s}");
}

/// A sketch whose stored positions break one of its constraints (not
/// solved again after the geometry it follows changed) comes in with the
/// others, that one left out, not without any.
#[test]
fn constraints_the_stored_positions_break_are_left_out_alone() {
    let points = vec![
        gpoint("p0", 0.0, 0.0),
        gpoint("p1", 4.0, 0.0),
        gpoint("p2", 4.0, 2.0),
        gpoint("p3", 0.0, 2.5),
    ];
    let curves = vec![
        gline("c0", "p0", "p1"),
        gline("c1", "p1", "p2"),
        gline("c2", "p2", "p3"),
    ];
    let constraints = json!([
        {"id": "k0", "type": "HorizontalConstraint", "refs": {"entities": ["c0"]}},
        {"id": "k1", "type": "VerticalConstraint", "refs": {"entities": ["c1"]}},
        // The file's p2 and p3 are 5 mm apart in height.
        {"id": "k2", "type": "HorizontalConstraint", "refs": {"entities": ["c2"]}},
    ]);
    let dump = group_sketch_dump(Vec::new(), points, curves, constraints, json!([]));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_eq!(
        report.items[0].outcome,
        Outcome::Partial,
        "{}",
        report.text()
    );
    let note = report.items[0].note.as_deref().unwrap_or_default();
    assert!(note.contains("1 constraints and dimensions"), "{note}");
    let s = sketch_query(&doc);
    let kinds: Vec<&str> = s["constraints"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|k| k["type"].as_str())
        .collect();
    assert_eq!(kinds, ["horizontal", "vertical"], "{s}");
    // The positions are the file's.
    assert_eq!(at(&s, &point_at(&s, [40.0, 20.0])), [40.0, 20.0]);
}

/// A profile's corner the file holds on a linked spline lies 1.3 µm off
/// the spline's stored control points (mitcad#396): solved with the
/// coincidence, the corner moves that far, within the import's geometric
/// tolerance (mitcad#224) and with the sketch's regions the file's, so the
/// coincidence stays and the sketch is parametric. 50 µm off, beyond the
/// tolerance, it is left out as before.
#[test]
fn a_corner_micrometres_off_a_linked_spline_keeps_its_coincidence() {
    for (off, kept) in [(1.3e-4, true), (5e-3, false)] {
        // The spline (4t, 4t(1 - t)) cm; its point at t = 1/4 is (1, 0.75).
        let mut spline = json!({
            "id": "c0", "type": "SketchFixedSpline", "isLinked": true, "isReference": true,
            "geometry": {"type": "NurbsCurve3D", "nurbs": {
                "degree": 2, "isRational": false, "isPeriodic": false, "controlPointCount": 3,
                "knots": [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                "controlPoints": [[0.0, 0.0, 0.0], [2.0, 2.0, 0.0], [4.0, 0.0, 0.0]],
                "weights": []}}});
        spline["startSketchPoint"] = json!("p0");
        spline["endSketchPoint"] = json!("p1");
        let points = vec![
            gpoint("p0", 0.0, 0.0),
            gpoint("p1", 4.0, 0.0),
            gpoint("p3", 1.0, 0.75 + off),
            gpoint("p4", 1.0, 3.0),
            gpoint("p5", 3.0, 3.0),
        ];
        let curves = vec![
            spline,
            gline("c1", "p3", "p4"),
            gline("c2", "p4", "p5"),
            gline("c3", "p5", "p3"),
        ];
        let constraints = json!([
            {"id": "k0", "type": "CoincidentConstraint", "refs": {"entities": ["p3", "c0"]}},
            {"id": "k1", "type": "VerticalConstraint", "refs": {"entities": ["c1"]}},
        ]);
        let dump = group_sketch_dump(Vec::new(), points, curves, constraints, json!([]));
        let mut doc = Document::new(MockKernel::default());
        let report = import(&mut doc, &dump);
        let note = report.items[0].note.clone().unwrap_or_default();
        let s = sketch_query(&doc);
        let kinds: Vec<&str> = s["constraints"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|k| k["type"].as_str())
            .collect();
        if kept {
            assert_parametric(&report);
            assert!(
                note.contains("solved within the geometric tolerance"),
                "{note}"
            );
            assert!(kinds.contains(&"coincident"), "{s}");
            // The corner on the spline, 1.3 µm from the file's.
            let uid = report.items[0]
                .features
                .iter()
                .filter_map(|f| f.parse::<mitcad_model::FeatureUid>().ok())
                .find(|uid| doc.sketch_output(*uid).is_some())
                .expect("the sketch");
            let corner = doc
                .sketch_output(uid)
                .unwrap()
                .solved
                .points
                .values()
                .copied()
                .find(|p| (p[0] - 10.0).hypot(p[1] - 7.5) < 0.01)
                .unwrap_or_else(|| panic!("{s}"));
            let moved = (corner[0] - 10.0).hypot(corner[1] - 7.5 - off * 10.0);
            assert!(moved > 1e-4 && moved < 2e-3, "{corner:?}");
            // On the spline: y = x (1 - x / 40) mm.
            let on = corner[1] - corner[0] * (1.0 - corner[0] / 40.0);
            assert!(on.abs() < 1e-7, "{corner:?}");
        } else {
            assert_eq!(report.items[0].outcome, Outcome::Partial, "{note}");
            assert!(note.contains("1 constraints and dimensions"), "{note}");
            assert!(!kinds.contains(&"coincident"), "{s}");
        }
    }
}

/// A solve within the geometric tolerance that changes the sketch's
/// regions is not the file's sketch (mitcad#396): a line 2 µm short of a
/// rectangle's top edge, its length dimensioned 4 µm longer than the
/// file's positions make it, would cross the edge and split the
/// rectangle; the dimension stays driven.
#[test]
fn a_solve_within_the_tolerance_that_changes_the_regions_drives_the_dimension() {
    for (end, kept) in [(1.5, true), (1.9998, false)] {
        let points = vec![
            gpoint("p0", 0.0, 0.0),
            gpoint("p1", 4.0, 0.0),
            gpoint("p2", 4.0, 2.0),
            gpoint("p3", 0.0, 2.0),
            gfixed("p4", 2.0, -0.5),
            gpoint("p5", 2.0, end),
        ];
        let curves = vec![
            gline("c0", "p0", "p1"),
            gline("c1", "p1", "p2"),
            gline("c2", "p2", "p3"),
            gline("c3", "p3", "p0"),
            gline("c4", "p4", "p5"),
        ];
        let constraints = json!([
            {"id": "k0", "type": "HorizontalConstraint", "refs": {"entities": ["c0"]}},
            {"id": "k1", "type": "VerticalConstraint", "refs": {"entities": ["c1"]}},
            {"id": "k2", "type": "HorizontalConstraint", "refs": {"entities": ["c2"]}},
            {"id": "k3", "type": "VerticalConstraint", "refs": {"entities": ["c4"]}},
        ]);
        // The file's length plus 4 µm.
        let length = (end + 0.5) * 10.0 + 4e-3;
        let dimensions = json!([
            {"id": "d0", "type": "SketchLinearDimension",
             "parameter": param("d1", &format!("{length} mm"), length / 10.0, "mm"),
             "props": {"orientation": "VerticalDimensionOrientation"},
             "refs": {"entities": ["p4", "p5"]}, "textPosition": [2.5, 0.5, 0.0]},
        ]);
        let dump = group_sketch_dump(
            vec![json!({"name": "d1", "expression": format!("{length} mm"),
                        "value": length / 10.0, "unit": "mm"})],
            points,
            curves,
            constraints,
            dimensions,
        );
        let mut doc = Document::new(MockKernel::default());
        let report = import(&mut doc, &dump);
        let note = report.items[0].note.clone().unwrap_or_default();
        if kept {
            assert_parametric(&report);
            assert!(
                note.contains("solved within the geometric tolerance"),
                "{note}"
            );
        } else {
            assert_eq!(report.items[0].outcome, Outcome::Partial, "{note}");
            assert!(note.contains("1 constraints and dimensions"), "{note}");
        }
    }
}

#[test]
fn a_tangent_distance_dimension_is_the_gap_between_a_circle_and_a_line() {
    // A circle of 12 mm about the fixed origin and a line 37 mm below its
    // centre: the gap is 25 mm, with the entities in either order.
    let points = vec![
        gfixed("p0", 0.0, 0.0),
        gpoint("p1", -3.0, -3.7),
        gpoint("p2", 3.0, -3.7),
    ];
    let curves = vec![gcircle("c0", "p0", 1.2), gline("c1", "p1", "p2")];
    for order in [["c0", "c1"], ["c1", "c0"]] {
        let dump = group_sketch_dump(
            vec![json!({"name": "d1", "expression": "25 mm", "value": 2.5, "unit": "mm"})],
            points.clone(),
            curves.clone(),
            json!([]),
            json!([{"id": "d0", "type": "SketchTangentDistanceDimension",
                    "parameter": param("d1", "25 mm", 2.5, "mm"),
                    "refs": {"entities": order}}]),
        );
        let mut doc = Document::new(MockKernel::default());
        let report = import(&mut doc, &dump);
        assert_parametric(&report);
        let s = sketch_query(&doc);
        let dimension = &s["dimensions"][0];
        assert_eq!(dimension["type"], "tangent_distance", "{s}");
        assert!((dimension["measured"].as_f64().unwrap() - 25.0).abs() < 1e-6);
        // The parameter drives the gap: the line moves away from the
        // circle, whose size and place the sketch keeps.
        doc.set_parameter_expression("d1", "30 mm", None).unwrap();
        let s = sketch_query(&doc);
        assert!((s["dimensions"][0]["measured"].as_f64().unwrap() - 30.0).abs() < 1e-6);
    }
}

/// A cut and paste of a body into a component that is not its own: the
/// body and the features that made it move into the existing component,
/// where the body keeps its shape and its place in the design.
#[test]
fn a_cut_and_paste_moves_the_bodies_into_an_existing_component() {
    let mut first = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    for item in &mut first {
        item["component"] = json!("Assembly");
    }
    let mut reference = Document::new(TestKernel {
        edge_points: true,
        ..TestKernel::default()
    });
    import_design(
        &mut reference,
        &timeline_dump(first.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let shape = shapes(&reference)[0].clone();
    let points: Vec<[f64; 3]> = shape
        .edges
        .iter()
        .take(3)
        .map(|(a, b)| edge_point(a, b).map(|x| x / 10.0))
        .collect();
    let body = json!({"kind": "body", "objectType": "BRepBody",
                      "_f3d": {"recipe": "body", "entities": [[{"tag": "304", "kind": 7, "ops": []}]],
                               "edge_points": points}});
    let matrix = |x: f64| {
        json!([
            [1.0, 0.0, 0.0, x],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0]
        ])
    };
    let mut items = first;
    items.push(
        json!({"index": 2, "name": "CutPasteBodies1", "objectType": "CopyPasteBodies",
                      "component": "Box", "detail": {"sourceBody": [body]},
                      "_f3d": {"base_name": "CutPasteBodies"}}),
    );
    let mut value = serde_json::to_value(timeline_dump(items)).unwrap();
    value["document"] = json!({"root_component": "Assembly"});
    value["components"] = json!([
        {"name": "Assembly", "is_root": true, "_f3d": {"object_id": 3}},
        {"name": "Box", "_f3d": {"object_id": 10}}]);
    value["occurrences"] = json!([
        {"component": "Box", "transform": matrix(5.0), "_f3d": {"component_object": 10},
         "children": []}]);
    let dump = Dump::from_json(&value.to_string()).unwrap();
    let mut doc = Document::new(TestKernel {
        edge_points: true,
        ..TestKernel::default()
    });
    let report = import_design(&mut doc, &dump, &mut NoGeometry, &Options::default());
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    // The body and the features that made it are the box's now, the
    // body's identity and the design's world placement kept.
    let a = doc.assembly();
    let boxed = a.components[0].uid;
    assert_eq!(a.component(boxed).map(|c| c.name.as_str()), Some("Box"));
    let body = doc.bodies()[0].uid;
    assert_eq!(doc.body_component(body), Some(boxed));
    assert!(doc.features().all(|f| f.component == boxed));
    assert!(doc.features().all(|f| doc.status(f.uid).unwrap().is_ok()));
    assert_eq!(doc.instances().len(), 1);
    assert_eq!(doc.instances()[0].transform.translation, [50.0, 0.0, 0.0]);
    // The note tells what moved; no sketch was copied for the item.
    assert!(
        report.items[2]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("moves its body into Box")),
        "{}",
        report.text()
    );
    assert!(report.items[2].features.is_empty());
}

/// Reference curves the file locks that do not meet exactly: two elliptical
/// arcs share their ends, which lie on one of them and a tenth of a micrometre off the other.
/// No solution keeps both in place, so they are imported free.
#[test]
fn linked_arcs_that_do_not_meet_exactly_are_not_locked() {
    let (a, b) = (2.0_f64, 1.0_f64);
    let on_first = |angle: f64| {
        let t = angle.to_radians();
        [a * t.cos(), b * t.sin(), 0.0]
    };
    let points = vec![
        json!({"id": "p0", "xyz": [0.0, 0.0, 0.0]}),
        json!({"id": "p1", "xyz": on_first(20.0)}),
        json!({"id": "p2", "xyz": on_first(80.0)}),
    ];
    let arc = |id: &str, minor: f64| {
        json!({"id": id, "type": "SketchEllipticalArc", "isReference": true, "isLinked": true,
               "startSketchPoint": "p1", "endSketchPoint": "p2", "centerSketchPoint": "p0",
               "majorAxisRadius": a, "minorAxisRadius": minor, "majorAxis": [a, 0.0, 0.0],
               "geometry": {"type": "EllipticalArc3D", "center": [0.0, 0.0, 0.0],
                            "normal": [0.0, 0.0, 1.0], "majorAxis": [1.0, 0.0, 0.0],
                            "majorRadius": a, "minorRadius": minor,
                            "startAngle": 0.3, "endAngle": 1.4}})
    };
    let dump = group_sketch_dump(
        vec![],
        points,
        vec![arc("c0", b), arc("c1", b * 1.00003)],
        json!([]),
        json!([]),
    );
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let note = report.items[0].note.as_deref().unwrap_or_default();
    assert!(note.contains("not locked"), "{note}");
    let s = sketch_query(&doc);
    let arcs = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "elliptical_arc")
        .count();
    assert_eq!(arcs, 2, "{s}");
}

/// The midpoint constraints of a fitted spline's fit points with the
/// tangent handles (lines the sketch does not list) bind nothing in a
/// sketch that keeps the spline's curve: the sketch is imported whole.
#[test]
fn tangent_handles_of_a_fitted_spline_are_no_constraints_left_out() {
    let handle = json!({"kind": "sketch_entity", "objectType": "SketchLine", "id": null,
                        "geometry": {"type": "Line3D", "startPoint": [-0.3, 0.0, 0.0],
                                     "endPoint": [0.3, 0.0, 0.0]}});
    let spline = json!({"id": "c0", "type": "SketchFittedSpline", "fitPoints": ["p1", "p2"],
        "startSketchPoint": "p1", "endSketchPoint": "p2",
        "geometry": {"type": "NurbsCurve3D", "nurbs": {
            "degree": 3, "isRational": false, "isPeriodic": false, "controlPointCount": 4,
            "knots": [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            "controlPoints": [[0.0, 0.0, 0.0], [1.0, 2.0, 0.0], [3.0, 2.0, 0.0],
                              [4.0, 0.0, 0.0]]}}});
    let dump = group_sketch_dump(
        vec![],
        vec![gpoint("p1", 0.0, 0.0), gpoint("p2", 4.0, 0.0)],
        vec![spline],
        json!([{"id": "k0", "type": "MidPointConstraint",
                "refs": {"entities": [handle, "p1"]}}]),
        json!([]),
    );
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    assert_eq!(report.items[0].note, None, "{}", report.text());
}

#[test]
fn datums_follow_several_sketch_curves_and_the_plane_of_a_profile() {
    let curve = |id: &str| {
        json!({"kind": "sketch_entity", "objectType": "SketchLine", "sketch": "Sketch1",
               "sketch_timeline_index": 0, "id": id})
    };
    let items = vec![
        rectangle_sketch(0),
        // 50 mm along the bottom and the right side of the rectangle: on
        // the right side, 10 mm up, normal to it.
        json!({"index": 1, "name": "Along", "objectType": "ConstructionPlane",
               "detail": {"definition": {"_type": "ConstructionPlaneDistanceOnPathDefinition",
                                         "distance": param("d9", "50 mm", 5.0, "mm"),
                                         "pathEntities": [curve("c0"), curve("c1")]},
                          "geometry": {"type": "Plane", "origin": [4.0, 1.0, 0.0],
                                       "normal": [0.0, 1.0, 0.0]}}}),
        // Normal to the rectangle's plane at its second corner.
        json!({"index": 2, "name": "Upright", "objectType": "ConstructionAxis",
               "detail": {"definition": {"_type": "ConstructionAxisNormalToFaceAtPointDefinition",
                                         "face": {"kind": "profile", "sketch": "Sketch1",
                                                  "sketch_timeline_index": 0},
                                         "pointEntity": {"kind": "sketch_entity",
                                                         "objectType": "SketchPoint",
                                                         "sketch": "Sketch1",
                                                         "sketch_timeline_index": 0, "id": "p1"}},
                          "geometry": {"type": "InfiniteLine3D", "origin": [4.0, 0.0, 0.0],
                                       "direction": [0.0, 0.0, 1.0]}}}),
    ];
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &timeline_dump(items));
    for i in [1, 2] {
        assert_eq!(
            report.items[i].outcome,
            Outcome::Parametric,
            "{}",
            report.text()
        );
    }
    let def = |name: &str| {
        let uid = doc.features().find(|f| f.name == name).unwrap().uid;
        let v: Value = serde_json::from_str(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                .unwrap(),
        )
        .unwrap();
        v["def"].clone()
    };
    let along = def("Along");
    assert_eq!(along["definition"]["type"], "along_path");
    assert_eq!(
        along["definition"]["path"]["curves"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        def("Upright")["definition"]["type"],
        "normal_to_face_at_point"
    );
    assert_eq!(
        report.items[2].input_provenance,
        Some(InputProvenance::Read)
    );
}

/// A midpoint constraint of two lines (the file's API names only the first
/// as `midPointCurve`): the first line's midpoint lies on the second.
#[test]
fn a_midpoint_of_two_lines_puts_the_midpoint_of_the_first_on_the_second() {
    let dump = group_sketch_dump(
        vec![],
        vec![
            gpoint("p0", 0.0, 0.0),
            gpoint("p1", 0.0, 1.0),
            gpoint("p2", -0.3, 0.5),
            gpoint("p3", 0.3, 0.5),
        ],
        vec![gline("c0", "p0", "p1"), gline("c1", "p2", "p3")],
        json!([{"id": "k0", "type": "MidPointConstraint",
                "refs": {"entities": ["c0", "c1"]}}]),
        json!([]),
    );
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let s = sketch_query(&doc);
    let kinds: Vec<&str> = s["constraints"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|k| k["type"].as_str())
        .collect();
    assert_eq!(kinds, ["midpoint", "coincident"], "{s}");
    // The helper point is at the middle of the first line.
    let helper = point_at(&s, [0.0, 5.0]);
    assert_eq!(at(&s, &helper), [0.0, 5.0]);
}

/// An offset with a round corner: the chain of two lines turns left, its
/// offset to the right goes round the corner by an arc of the distance
/// about it. The offset is Mitcad's derived one, computed from the chain,
/// and follows its distance.
#[test]
fn an_offset_with_a_round_corner_is_a_derived_offset() {
    let points = vec![
        gpoint("p0", 0.0, 0.0),
        gpoint("p1", 1.0, 0.0),
        gpoint("p2", 1.0, 1.0),
        gpoint("p3", 0.0, -0.1),
        gpoint("p4", 1.0, -0.1),
        gpoint("p5", 1.0, 0.0),
        gpoint("p6", 1.1, 0.0),
        gpoint("p7", 1.1, 1.0),
    ];
    let curves = vec![
        gline("c0", "p0", "p1"),
        gline("c1", "p1", "p2"),
        gline("c2", "p3", "p4"),
        json!({"id": "c3", "type": "SketchArc", "centerSketchPoint": "p5",
               "startSketchPoint": "p4", "endSketchPoint": "p6"}),
        gline("c4", "p6", "p7"),
    ];
    let dump = group_sketch_dump(
        vec![json!({"name": "d1", "expression": "1 mm", "value": 0.1, "unit": "mm"})],
        points,
        curves,
        json!([{"id": "k0", "type": "OffsetConstraint",
                "refs": {"entities": ["c0", "c2", "c1", "c4", "c3"]},
                "props": {"distance": 0.1, "dimension": "d0",
                          "parentCurves": ["c0", "c1"],
                          "childCurves": ["c2", "c3", "c4"]}}]),
        json!([{"id": "d0", "type": "SketchOffsetCurvesDimension",
                "parameter": param("d1", "1 mm", 0.1, "mm"), "refs": {"entities": ["c0"]}}]),
    );
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_parametric(&report);
    let s = sketch_query(&doc);
    let offsets = s["offsets"].as_array().unwrap();
    assert_eq!(offsets.len(), 1, "{s}");
    assert_eq!(offsets[0]["derived"], true);
    assert_eq!(offsets[0]["corners"].as_array().map(Vec::len), Some(1));
    assert_eq!(offsets[0]["distance"], "d1");
    // The distance drives the offset: the corner arc grows with it.
    let arc = offsets[0]["corners"][0]["arc"].as_str().unwrap().to_owned();
    doc.set_parameter_expression("d1", "2 mm", None).unwrap();
    let s = sketch_query(&doc);
    assert!((radius(&s, &arc) - 2.0).abs() < 1e-6, "{s}");
}

#[test]
fn a_join_may_change_the_volume_by_the_bodies_it_merges() {
    // A prism of 26 mm³ whose join merges a body of 230 mm³ that the
    // history's state still holds: a change of 256 mm³ is possible for a
    // join, not for a cut.
    let candidate = |operation: &str| {
        let mut c = Candidate::new(json!({"type": "extrude", "operation": operation,
            "extent": {"type": "distance", "distance": 1.0}}));
        c.predicted = Some(26.0);
        c
    };
    let kept = |operation: &str, mergeable: f64| {
        feasible("Extrude", vec![candidate(operation)], 256.0, mergeable).len()
    };
    assert_eq!(kept("join", 0.0), 0);
    assert_eq!(kept("join", 230.0), 1);
    assert_eq!(kept("cut", 230.0), 0);
}

/// Imports one primitive or coil item and returns its definition and the
/// provenance of its inputs (mitcad#25).
fn primitive_item(object_type: &str, detail: Value) -> (Value, Option<InputProvenance>) {
    let item = json!({"index": 0, "name": "Primitive1", "objectType": object_type,
                      "detail": detail});
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(vec![item]),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[0].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F1"}"#).unwrap()).unwrap();
    (def["def"].clone(), report.items[0].input_provenance)
}

fn xy_plane() -> Value {
    json!({"kind": "construction_plane", "name": "XY", "origin": "XY"})
}

#[test]
fn primitives_come_in_from_their_matrices() {
    // A box on the XY plane whose own frame is turned a quarter and moved to
    // x = 2 cm: its width (the own x) runs along the plane's y and its
    // length along the plane's -x, so it spans x 0..20 and y 0..10.
    let (def, provenance) = primitive_item(
        "BoxFeature",
        json!({"operation": "NewBodyFeatureOperation", "plane": xy_plane(),
               "placement": [[0.0, -1.0, 0.0, 2.0], [1.0, 0.0, 0.0, 0.0],
                             [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
               "distanceX": param("d7", "10 mm", 1.0, "mm"),
               "distanceY": param("d8", "20 mm", 2.0, "mm"),
               "distanceZ": param("d9", "15 mm", 1.5, "mm")}),
    );
    assert_eq!(provenance, Some(InputProvenance::Read));
    assert_eq!(def["plane"], "xy");
    assert_eq!(def["corner"], json!([0.0, 0.0]));
    // The sizes are the file's parameters (the length is `DistanceY`, the
    // width `DistanceX`: created in the order the import reads them).
    assert!(def["length"].is_string() && def["width"].is_string() && def["height"].is_string());
    assert_ne!(def["length"], def["width"]);
    assert_eq!(def["operation"], "new_body");

    // A cylinder on a face-like frame turned half round, centred at (1, 1)
    // cm of it, against the plane's normal (a negative height).
    let (def, provenance) = primitive_item(
        "CylinderFeature",
        json!({"operation": "NewBodyFeatureOperation", "plane": xy_plane(),
               "frame": [[-1.0, 0.0, 0.0, 0.0], [0.0, -1.0, 0.0, 0.0],
                         [0.0, 0.0, -1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
               "placement": [[1.0, 0.0, 0.0, -1.0], [0.0, 1.0, 0.0, -2.0],
                             [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
               "height": param("d7", "15 mm", 1.5, "mm"),
               "diameter": param("d8", "10 mm", 1.0, "mm")}),
    );
    assert_eq!(provenance, Some(InputProvenance::Read));
    assert_eq!(def["center"], json!([10.0, 20.0]));
    // The height is a parameter of its own: the file's, negated.
    assert!(def["height"].is_string() && def["height"] != def["diameter"]);
    assert_eq!(def["operation"], "new_body");

    // A sphere and a torus inside the diameter at the plane's origin.
    let (def, _) = primitive_item(
        "SphereFeature",
        json!({"operation": "NewBodyFeatureOperation", "plane": xy_plane(),
               "diameter": param("d7", "20 mm", 2.0, "mm")}),
    );
    assert_eq!(
        (def["type"].clone(), def["center"].clone()),
        (json!("sphere"), json!([0.0, 0.0]))
    );
    let (def, _) = primitive_item(
        "TorusFeature",
        json!({"operation": "NewBodyFeatureOperation", "plane": xy_plane(),
               "majorDiameter": param("d8", "40 mm", 4.0, "mm"),
               "minorDiameter": param("d7", "6 mm", 0.6, "mm"),
               "torusPosition": "InsideTorusPosition"}),
    );
    assert_eq!(def["position"], "inside");
}

#[test]
fn coils_come_in_with_their_choices() {
    let (def, provenance) = primitive_item(
        "CoilFeature",
        json!({"operation": "NewBodyFeatureOperation", "plane": xy_plane(),
               "coilType": "PitchAndRevolutionCoilType",
               "sectionType": "SquareCoilSectionType",
               "sectionPosition": "OutsideCoilSectionPosition",
               "isClockwise": false,
               "diameter": param("d1", "40 mm", 4.0, "mm"),
               "sectionSize": param("d2", "4 mm", 0.4, "mm"),
               "angle": param("d3x", "0 deg", 0.0, "deg"),
               "revolutions": param("d4", "3", 3.0, ""),
               "pitch": param("d5x", "10 mm", 1.0, "mm")}),
    );
    assert_eq!(provenance, Some(InputProvenance::Read));
    assert_eq!(def["type"], "coil");
    assert_eq!(
        (
            def["section"].clone(),
            def["section_position"].clone(),
            def["clockwise"].clone()
        ),
        (json!("square"), json!("outside"), json!(true))
    );
    assert_eq!(def["helix"]["type"], "revolutions_and_pitch");
}

/// A cut that divides its target in two: the target keeps the record of
/// the item that made it, so the cut's own records are its new pieces
/// only, and a removal naming the cut's first record takes the first new
/// piece, not the target's remaining one (the pieces come out in the
/// replay's order, which is not the file's).
#[test]
fn a_removal_by_a_cuts_record_takes_a_new_piece_not_the_target() {
    let produced = |producer: i64, index: i64| {
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"producer": producer, "body_index": index}})
    };
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        extrude(2, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 3, "name": "Combine1", "objectType": "CombineFeature",
               "detail": {"targetBody": produced(1, 0), "toolBodies": [produced(2, 0)],
                          "operation": "CutFeatureOperation", "isKeepToolBodies": true}}),
        json!({"index": 4, "name": "RemoveBody-Body3", "objectType": "RemoveFeature",
               "detail": {"itemToRemove": produced(3, 0)}}),
    ];
    let mut doc = Document::new(TestKernel::default());
    doc.kernel().mock.split.set(Some(2));
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    let combine = &report.items[3];
    assert_eq!(combine.outcome, Outcome::Parametric, "{}", report.text());
    let remove = &report.items[4];
    assert_eq!(remove.outcome, Outcome::Parametric, "{}", report.text());
    let def: Value = serde_json::from_str(
        &doc.query(&json!({"query": "feature", "uid": remove.features[0]}).to_string())
            .unwrap(),
    )
    .unwrap();
    // The new piece of the cut (the cut's own body), not the target F2.b0.
    assert_eq!(
        def["def"]["bodies"],
        json!([format!("{}.b0", combine.features[0])]),
        "{}",
        report.text()
    );
}

/// An occurrence inside a component placed several times (Pin in Plate,
/// Plate placed 10 cm apart twice): the file gives the occurrence's
/// placement after each item per path, in the root component. Its
/// placement in its parent is each step made relative to the parent along
/// the step's own path (Pin 1 cm along X in Plate), not a root placement
/// taken as one in the parent (11 cm, where the second path puts it).
#[test]
fn an_occurrence_in_a_component_placed_twice_keeps_its_place_in_it() {
    let mut value: Value = serde_json::to_value(assembly_dump(true)).unwrap();
    let matrix = |x: f64| {
        json!([
            [1.0, 0.0, 0.0, x],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0]
        ])
    };
    let step = |index: i64, x: f64| json!([{"index": index, "object_id": 900 + index, "transform": matrix(x)}]);
    for (k, (plate, x)) in [(11, 0.0), (12, 10.0)].into_iter().enumerate() {
        let index = 2 * k as i64;
        let occurrence = &mut value["occurrences"][k];
        occurrence["_f3d"]["object_id"] = json!(plate);
        occurrence["_f3d"]["placements"] = step(index, x);
        let pin = &mut occurrence["children"][0]["_f3d"];
        pin["object_id"] = json!(21);
        pin["placements"] = step(index + 1, x + 1.0);
    }
    let dump = Dump::from_json(&value.to_string()).unwrap();
    let mut doc = Document::new(MockKernel::default());
    let report = import_design(
        &mut doc,
        &dump,
        &mut ComponentHistory::default(),
        &Options::default(),
    );
    let assembly = doc.assembly();
    let pin = assembly
        .occurrences
        .iter()
        .find(|o| assembly.occurrence_name(o.uid).starts_with("Pin"))
        .expect("the pin's occurrence");
    let x = pin.transform.translation[0];
    assert!((x - 10.0).abs() < 1e-9, "{x} mm: {}", report.text());
}

/// The file's own base feature (bodies without a history in the file)
/// comes in as a base feature of the bodies the file stores for it: that
/// is its translation, not a fallback standing in for it, so its feature
/// gets no import note.
#[test]
fn the_files_base_feature_is_its_stored_bodies() {
    let stored = MockShape::imported("stored body", 6);
    let items = vec![json!({"index": 0, "name": "Base Feature1", "objectType": "BaseFeature"})];
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), vec![stored]],
        items: HashMap::from([(0, 1)]),
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let item = &report.items[0];
    assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(item.input_provenance, Some(InputProvenance::Read));
    let uid: FeatureUid = item.features[0].parse().unwrap();
    assert_eq!(doc.feature(uid).unwrap().def.type_name(), "base");
    assert_eq!(doc.import_note(uid), None, "{}", report.text());
}

// Appearances and materials (mitcad#228).

/// The stored bodies as the file's blob holds them: `BREP.test.smbh#7`,
/// `#8`, ... (the decoder's body map names them so).
struct BlobGeometry(TestGeometry);

impl StoredGeometry<MockShape> for BlobGeometry {
    fn state_count(&mut self) -> usize {
        self.0.state_count()
    }

    fn state(&mut self, index: usize) -> Result<Vec<StoredBody<MockShape>>, String> {
        Ok(self
            .0
            .state(index)?
            .into_iter()
            .enumerate()
            .map(|(k, mut b)| {
                b.source = format!("BREP.test.smbh#{}", 7 + k);
                b
            })
            .collect())
    }

    fn item_state(&mut self, index: i64) -> Option<usize> {
        self.0.item_state(index)
    }

    fn final_bodies(&mut self) -> Result<Vec<StoredBody<MockShape>>, String> {
        let last = self.0.state_count() - 1;
        self.state(last)
    }
}

/// The decoder's appearance data of one body: a red paint, the steel
/// material, and one face (`point`, cm) in the steel's satin look.
fn appearance_data(point: [f64; 3]) -> Value {
    let paint = "23303B60-6683-46D0-BC5B-B92F9F3CA4E8_Post2015";
    let satin = "703A8DBC-1CFC-4F45-A969-681245623F44_Post2015";
    json!({
        "assets": [
            {"id": paint, "schema": "PrismOpaqueSchema", "name": "Prism-093",
             "ui_name": "Prism-093", "description": "Paint - enamel red glossy",
             "properties": {"opaque_albedo": [0.7674, 0.206, 0.1517, 1.0],
                            "opaque_albedo_colorspace": 1, "opaque_f0": 0.06027025,
                            "surface_roughness": 0.0775, "opaque_translucency": false}},
            {"id": satin, "schema": "PrismMetalSchema",
             "name": "PrismMaterial-018_physmat_aspects:Prism-256", "ui_name": "Prism-256",
             "description": "Steel - satin",
             "properties": {"metal_f0": [0.6262, 0.6262, 0.6262, 1.0],
                            "surface_roughness": 0.2646}},
            {"id": "4177D984-5662-4715-B8AF-E5D65A04EDEE", "schema": "PhysMatSchema",
             "name": "PrismMaterial-018", "description": "Steel, mill finish",
             "properties": {"physmat_classification": "Metal.Ferrous.Carbon Steel"},
             "connections": {"physmat_aspects": ["F3981BFD-5548-497A-A042-21FC315C9862"]}},
            {"id": "F3981BFD-5548-497A-A042-21FC315C9862", "schema": "StructuralMetalSchema",
             "name": "PhysicalMetal_124",
             "properties": {"structural_Density": {"unit": 69633, "value": 7850.0}}}
        ],
        "bodies": [{
            "entity": 300, "name": "Block", "blob": "BREP.test.smbh", "selector": 0,
            "record": 7,
            "appearance": {"token": "23303B60-6683-46D0-BC5B-B92F9F3CA4E8", "asset": paint,
                           "preset": "Prism-093"},
            "material": {"token": "4177D984-5662-4715-B8AF-E5D65A04EDEE",
                         "asset": "4177D984-5662-4715-B8AF-E5D65A04EDEE",
                         "preset": "PrismMaterial-018"},
            "faces": [{"guid": "0438b4bf-d97c-4f0d-9ef2-713f87b07603",
                       "appearance": {"asset": satin},
                       "faces": [{"point_on_face": point}]}]
        }]
    })
}

/// The file's appearances and material come in on the replay's body
/// (matched with the stored body the body map names) and on its face
/// (found through the stored face's point); each asset makes one document
/// appearance with the asset's parameters on Mitcad's set.
#[test]
fn appearances_and_materials_come_in_on_bodies_and_faces() {
    let (items, geometry) = block_history(Vec::new());
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    // A face of the block, as the stored design has it.
    let mut reference = Document::new(kernel());
    let mut g = BlobGeometry(block_history(Vec::new()).1);
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut g,
        &Options::default(),
    );
    let body = reference.bodies()[0].shape.clone();
    let face = body.faces[0].clone();
    let point = face_point(&face).map(|x| x / 10.0);

    let mut source = serde_json::to_value(timeline_dump(items)).unwrap();
    source["_f3d"] = json!({"appearance": appearance_data(point)});
    let mut doc = Document::new(kernel());
    let mut geometry = BlobGeometry(geometry);
    let report = import_design(&mut doc, &dump(source), &mut geometry, &Options::default());
    let a = &report.appearances;
    assert_eq!((a.bodies_set, a.bodies), (1, 1), "{}", report.text());
    assert_eq!((a.faces_set, a.faces), (1, 1), "{}", report.text());
    assert_eq!((a.materials_set, a.materials), (1, 1), "{}", report.text());
    assert!(a.gaps.is_empty(), "{:?}", a.gaps);
    assert_eq!(a.physical_materials[0].density, Some(7850.0));
    assert!(report.text().contains("appearances: bodies 1/1, faces 1/1"));

    let uid = doc.bodies()[0].uid;
    let attributes = doc.body_attributes(uid);
    assert_eq!(attributes.material.as_deref(), Some("steel"));
    let id = attributes.appearance.clone().unwrap();
    assert_eq!(id, "f3d_paint_enamel_red_glossy");
    let red = doc.appearance(&id).unwrap();
    assert_eq!(red.name, "Paint - enamel red glossy");
    assert_eq!(red.base_color, [0.7674, 0.206, 0.1517]);
    assert_eq!(red.metalness, 0.0);
    assert!((red.ior - 1.6506).abs() < 1e-3, "{}", red.ior);
    assert!((red.roughness - 0.0775).abs() < 1e-12);
    // The face, and the faces the test kernel takes for pieces of it (its
    // faces all have one normal).
    let faces = &attributes.face_appearances;
    assert_eq!(
        faces.get(&face.to_string()).map(String::as_str),
        Some("f3d_steel_satin")
    );
    assert!(faces.values().all(|a| a == "f3d_steel_satin"));
    let satin = doc.appearance("f3d_steel_satin").unwrap();
    assert_eq!(satin.metalness, 1.0);
    assert_eq!(satin.base_color, [0.6262; 3]);
    assert_eq!(doc.custom_appearances().len(), 2);
}

/// A dimension the file keeps driven, whose parameter the file has (`d4`):
/// that parameter becomes the dimension's reference (mitcad#250), owned by
/// the sketch with the value the sketch measures, instead of a user
/// parameter beside a reference of the dimension's own; `half` = `d4 / 2`
/// follows it.
#[test]
fn a_driven_dimensions_parameter_of_the_file_is_its_reference() {
    use mitcad_model::expr::ParamKind;
    let mut sketch = rectangle_sketch(0);
    sketch["detail"]["dimensions"].as_array_mut().unwrap().push(
        json!({"id": "d1", "type": "SketchLinearDimension", "isDriving": false,
                     "parameter": param("d4", "20 mm", 2.0, "mm"),
                     "props": {"orientation": "VerticalDimensionOrientation"},
                     "refs": {"entities": ["p1", "p2"]}, "textPosition": [5.0, 1.0, 0.0]}),
    );
    let source = dump(json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "driven.f3d"},
        "parameters": {
            "user": [{"name": "half", "expression": "d4 / 2", "value": 1.0, "unit": "mm"}],
            "model": [
                {"name": "d1", "expression": "40 mm", "value": 4.0, "unit": "mm",
                 "createdBy": {"kind": "sketch_dimension", "sketch": "Sketch1",
                               "sketch_timeline_index": 0, "id": "d0"}},
                {"name": "d4", "expression": "20 mm", "value": 2.0, "unit": "mm",
                 "createdBy": {"kind": "sketch_dimension", "sketch": "Sketch1",
                               "sketch_timeline_index": 0, "id": "d1"}}
            ]
        },
        "timeline": {"items": [sketch]}
    }));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &source);
    assert!(
        report.parameters.mismatched.is_empty(),
        "{:?}",
        report.parameters
    );
    let def = doc.query(r#"{"query": "feature", "uid": "F1"}"#).unwrap();
    let def: Value = serde_json::from_str(&def).unwrap();
    let driven: Vec<&Value> = def["def"]["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["driven"] == true)
        .collect();
    assert_eq!(driven.len(), 1, "{def}\n{}", report.text());
    assert_eq!(driven[0]["reference"], "d4", "{def}");
    let params = doc.parameters();
    let d4 = params.find("d4").unwrap();
    assert_eq!(params.get(d4).unwrap().kind(), ParamKind::Reference);
    assert_eq!(params.owner(d4), Some(FeatureUid(1)));
    assert!((params.value(d4).unwrap() - 20.0).abs() < 1e-9);
    let half = params.find("half").unwrap();
    assert!((params.value(half).unwrap() - 10.0).abs() < 1e-9);
    let references = params
        .iter()
        .filter(|p| p.kind() == ParamKind::Reference)
        .count();
    assert_eq!(references, 1);
    // The driving width's parameter is the sketch's model parameter.
    assert_eq!(
        params.get(params.find("d1").unwrap()).unwrap().kind(),
        ParamKind::Model
    );
}

/// A sketch definition without its driven dimensions' references, as an
/// import falls back on when the model refuses one (mitcad#250).
#[test]
fn sketch_definitions_lose_their_references_for_the_fallback() {
    let def = json!({"type": "sketch", "dimensions": [
        {"id": "k1", "type": "length", "line": "c1", "value": "d1"},
        {"id": "k2", "type": "length", "line": "c2", "driven": true, "reference": "d4"}]});
    assert!(crate::sketch::has_references(&def));
    let plain = crate::sketch::without_references(&def);
    assert!(!crate::sketch::has_references(&plain));
    assert_eq!(plain["dimensions"][1]["driven"], true);
    assert_eq!(plain["dimensions"][0]["value"], "d1");
}

/// Angle dimensions take their side from the file (mitcad#296): the sector
/// of the lines' extensions their value is shown in, when its angle is the
/// file's value, else the neighbouring sector nearer to the text.
#[test]
fn an_angle_dimensions_side_comes_from_the_file() {
    // A trapezoid's corner: the bottom c0 from the corner to the right, the
    // side c1 from up right down to the corner; inside, 63.43 degrees.
    let p = |id: &str, x: f64, y: f64| json!({"id": id, "xyz": [x, y, 0.0]});
    let line = |id: &str, a: &str, b: &str| json!({"id": id, "type": "SketchLine", "startSketchPoint": a, "endSketchPoint": b});
    let inner = 2.0f64.atan();
    let angle = |id: &str, value: f64, text: [f64; 2]| {
        json!({"id": id, "type": "SketchAngularDimension", "isDriving": false,
               "parameter": param(id, "", value, "deg"),
               "refs": {"lineOne": "c0", "lineTwo": "c1"}, "textPosition": [text[0], text[1], 0.0]})
    };
    let sketch = json!({
        "index": 0, "name": "Sketch1", "objectType": "Sketch",
        "detail": {
            "referencePlane": {"kind": "construction_plane", "name": "XY", "origin": "XY"},
            "model_frame": {"sketch_to_model": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                                                [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]},
            "points": [p("p0", 0.0, 0.0), p("p1", 4.0, 0.0), p("p2", 1.0, 2.0)],
            "curves": [line("c0", "p0", "p1"), line("c1", "p2", "p0")],
            "dimensions": [
                // Inside the corner.
                angle("d0", inner, [0.6, 0.6]),
                // Between the bottom's extension and the side.
                angle("d1", std::f64::consts::PI - inner, [-0.2, 0.6]),
                // The supplement shown inside, near the bottom: the sector
                // below the bottom.
                angle("d2", std::f64::consts::PI - inner, [0.6, 0.3]),
                // A value neither angle has: no side.
                angle("d3", 0.5, [0.6, 0.6])
            ]
        }
    });
    let source = dump(json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "angles.f3d"},
        "parameters": {"user": [], "model": []},
        "timeline": {"items": [sketch]}
    }));
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &source);
    let def = doc.query(r#"{"query": "feature", "uid": "F1"}"#).unwrap();
    let def: Value = serde_json::from_str(&def).unwrap();
    let dimensions = def["def"]["dimensions"].as_array().unwrap();
    assert_eq!(dimensions.len(), 4, "{def}\n{}", report.text());
    let side = |d: &Value| (d.get("reverse_a").cloned(), d.get("reverse_b").cloned());
    let given = |a: bool, b: bool| (Some(json!(a)), Some(json!(b)));
    assert_eq!(side(&dimensions[0]), given(false, true), "{def}");
    assert_eq!(side(&dimensions[1]), given(true, true), "{def}");
    assert_eq!(side(&dimensions[2]), given(false, false), "{def}");
    assert_eq!(side(&dimensions[3]), (None, None), "{def}");
}

#[test]
fn symptoms_and_kernel_failures_of_definitions() {
    // What only says the result is not the file's (mitcad#325) ...
    assert!(symptom(NOT_IN_HISTORY));
    assert!(symptom(
        "the extrusion does not cut into any participant body"
    ));
    assert!(symptom("there are no bodies in the extrude direction"));
    assert!(!symptom(
        "fillet failed; the radius may be too large for the edges"
    ));
    // ... and what the geometry kernel failed in.
    assert!(kernel_failure(
        "the boolean operation produced an invalid shape (face: BRepCheck_UnorientableShape)"
    ));
    assert!(kernel_failure(
        "fillet failed; the radius may be too large for the edges"
    ));
    assert!(kernel_failure("fillet: TopoDS_Builder::Add"));
    assert!(kernel_failure(
        "the taper failed; the angle may close the profile"
    ));
    assert!(!kernel_failure(NOT_IN_HISTORY));
    assert!(!kernel_failure(
        "the extrusion does not cut into any participant body"
    ));
}

// Derives: bodies of other documents (mitcad#324).
mod derive_tests;

/// A construction plane offset from a face of another component's body,
/// linked to it (mitcad#389), goes into the item's component, where its
/// link's target leads: not into the face's component, whose coordinates
/// put it where the other component is placed instead.
#[test]
fn a_linked_plane_goes_into_the_items_component() {
    let mut doc = Document::new(TestKernel::default());
    let (a, _) = doc
        .create_component(
            Some("A"),
            mitcad_model::Transform::translation([0.0, 0.0, 10.0]),
            true,
        )
        .unwrap();
    let sketch = doc
        .add_sketch(mitcad_model::features::SketchPlane::Xy)
        .unwrap()
        .uid;
    let region = doc
        .add_rectangle(
            sketch,
            [0.0, 0.0],
            &mitcad_model::ValueInput::Number(10.0),
            &mitcad_model::ValueInput::Number(10.0),
        )
        .unwrap()
        .region;
    let added: Value = serde_json::from_str(
        &doc.command(
            &json!({"cmd": "add_feature", "def": {"type": "extrude",
                    "profiles": [{"sketch": sketch.to_string(), "region": region.to_string()}],
                    "extent": {"type": "distance", "distance": 5}, "operation": "new_body"}})
            .to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    let body = format!("{}.b0", added["uid"].as_str().unwrap());
    doc.activate_component(mitcad_model::ComponentUid::ROOT)
        .unwrap();
    let (b, _) = doc
        .create_component(Some("B"), mitcad_model::Transform::IDENTITY, true)
        .unwrap();
    let face = format!("{}:end({region})", added["uid"].as_str().unwrap());
    let plane = json!({"type": "construction_plane",
                       "definition": {"type": "offset", "plane": {"body": body, "face": face},
                                      "distance": 5},
                       "links": {body.clone(): {"source": "O1", "target": "O2"}}});
    assert_eq!(def_component(&doc, &plane, b), b);
    // Without the link the face's body names its component.
    let mut unlinked = plane.clone();
    unlinked.as_object_mut().unwrap().remove("links");
    assert_eq!(def_component(&doc, &unlinked, b), a);
}
