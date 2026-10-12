# SPDX-License-Identifier: MIT
"""Unit tests of apiref.py on synthetic dumps (no corpus data).

Run: python3 tools/f3d-learn/test_apiref.py
"""

import contextlib
import gzip
import io
import json
import math
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import apiref  # noqa: E402


class ValueTests(unittest.TestCase):
    def test_same_tolerance_and_types(self):
        self.assertTrue(apiref.same(1.0, 1.0 + 1e-6))
        self.assertFalse(apiref.same(1.0, 1.001))
        self.assertTrue(apiref.same(None, None))
        self.assertFalse(apiref.same(None, 0))
        self.assertFalse(apiref.same("a", 1.0))
        self.assertTrue(apiref.same((1.0, ("x", 2.0)), (1.0, ("x", 2.0 + 1e-7))))
        self.assertFalse(apiref.same((1.0, 2.0), (1.0, 2.0, 3.0)))

    def test_closed_circle_edge_matches_by_its_circle(self):
        # The file's middle lies opposite its seam, a replay's elsewhere.
        circle = {
            "kind": "edge",
            "geometry": {"type": "Circle3D", "center": [0, 0, 0.15], "normal": [0, 0, -1], "radius": 2.75},
            "length": 17.27876,
            "start_point": [-2.75, 0, 0.15],
            "end_point": [-2.75, 3e-16, 0.15],
            "mid_point": [2.75, 0, 0.15],
        }
        entity = apiref.Canon({}, "api").entity(circle)
        self.assertIsNotNone(entity)
        twin = ("edge", (-2.75, 0.0, 0.15), 17.27876)
        quarter = ("edge", (0.0, 2.75, 0.15), 17.27876)
        wrong_radius = ("edge", (0.0, 2.9, 0.15), 17.27876)
        shorter = ("edge", (-2.75, 0.0, 0.15), 8.0)
        self.assertTrue(apiref.same(twin, entity))
        self.assertTrue(apiref.same(entity, quarter))
        self.assertFalse(apiref.same(entity, wrong_radius))
        self.assertFalse(apiref.same(entity, shorter))
        self.assertTrue(apiref.same(apiref.Bag([entity]), apiref.Bag([twin])))
        # An open edge still needs the same middle.
        arc = {"kind": "edge", "mid_point": [1, 0, 0], "length": 3.0}
        open_edge = apiref.Canon({}, "api").entity(arc)
        self.assertFalse(apiref.same(open_edge, ("edge", (-1.0, 0.0, 0.0), 3.0)))

    def test_pieces_of_an_edge_fold_into_it(self):
        size = (("radius", 0.2),)
        canon = apiref.Canon({}, "api")
        circle = canon.entity({
            "kind": "edge",
            "geometry": {"type": "Circle3D", "center": [0, 0, 1], "normal": [0, 0, 1], "radius": 2.0},
            "length": 4 * math.pi, "start_point": [2, 0, 1], "end_point": [2, 0, 1], "mid_point": [-2, 0, 1],
        })
        line = canon.entity({
            "kind": "edge", "geometry": {"type": "Line3D"},
            "length": 4.5, "start_point": [0, 0, 0], "end_point": [4.5, 0, 0], "mid_point": [2.25, 0, 0],
        })
        api = apiref.Bag([(circle, size), (line, size)])
        halves = [(("edge", (0.0, 2.0, 1.0), 2 * math.pi), size), (("edge", (0.0, -2.0, 1.0), 2 * math.pi), size)]
        thirds = [(("edge", (0.75 + 1.5 * k, 0.0, 0.0), 1.5), size) for k in range(3)]
        other = apiref.Bag(halves + thirds)
        self.assertFalse(apiref.same(api, other))
        folded = apiref.fold_pieces(api, other)
        self.assertTrue(apiref.same(api, folded))
        # Pieces that do not add up, are on another curve or have another size stay.
        short = apiref.Bag(halves[:1] + thirds[:2])
        self.assertEqual(len(apiref.fold_pieces(api, short)), 3)
        off = apiref.Bag([(("edge", (0.75 + 1.5 * k, 1.0, 0.0), 1.5), size) for k in range(3)])
        self.assertEqual(len(apiref.fold_pieces(apiref.Bag([(line, size)]), off)), 3)
        other_size = apiref.Bag([(e, (("radius", 0.3),)) for e, _ in thirds])
        self.assertEqual(len(apiref.fold_pieces(apiref.Bag([(line, size)]), other_size)), 3)

    def test_face_selection_and_failed_references(self):
        size = (("radius", 0.2),)
        canon = apiref.Canon({}, "api")
        face = canon.entity({"kind": "face", "geometry": {"type": "Plane", "normal": [0, 0, 1], "origin": [0, 0, 2]}})
        api = {"edges": apiref.Bag([(face, size)]), "edges.count": 1}
        edges = [(("edge", (x, 0.0, 2.0), 1.0), size) for x in (0.0, 1.0, 2.0)]
        decoder = {"edges": apiref.Bag(edges), "edges.count": 3}
        apiref.dressup_edges(api, decoder)
        self.assertTrue(apiref.same(api["edges"], decoder["edges"]))
        self.assertNotIn("edges.count", api)
        self.assertNotIn("edges.count", decoder)
        # An edge off the face is not the face.
        off = {"edges": apiref.Bag(edges + [(("edge", (0.0, 0.0, 3.0), 1.0), size)]), "edges.count": 4}
        api = {"edges": apiref.Bag([(face, size)]), "edges.count": 1}
        apiref.dressup_edges(api, off)
        self.assertFalse(apiref.same(api["edges"], off["edges"]))
        # No edges listed by the API: nothing to compare.
        api = {"edges": apiref.Bag([]), "edges.count": 0}
        apiref.dressup_edges(api, decoder)
        self.assertNotIn("edges", api)
        self.assertNotIn("edges.count", api)

    def test_a_spline_edge_matches_with_a_middle_off_along_it(self):
        spline = apiref.Canon({}, "api").entity({
            "kind": "edge", "geometry": {"type": "NurbsCurve3D"},
            "length": 2.0, "start_point": [0, 0, 0], "end_point": [2, 0, 0], "mid_point": [1.0, 0.0, 0.0],
        })
        self.assertTrue(apiref.same(spline, ("edge", (1.1, 0.05, 0.0), 2.0)))
        self.assertFalse(apiref.same(spline, ("edge", (1.5, 0.0, 0.0), 2.0)))
        self.assertFalse(apiref.same(spline, ("edge", (1.0, 0.0, 0.0), 2.1)))

    def test_census_answer_with_a_feature_per_chain_is_one_input(self):
        def make(edges):
            return {"type": "fillet", "body": "T1.b0", "rolling_ball_corners": True, "tangent_chain": True,
                    "sets": [{"edges": edges, "size": {"type": "constant", "radius": "d1"}, "tangent_chain": True}]}
        line = {"answer": {"defs": [make(["E1"]), make(["E2", "E3"])]}, "context": {"edges": {}}}
        d, sets = apiref.cen_dressup_sets(line, "fillet")
        self.assertEqual(len(sets), 1)
        self.assertEqual(sets[0]["edges"], ["E1", "E2", "E3"])
        two_sizes = {"answer": {"defs": [make(["E1"]), dict(make(["E2"]), sets=[
            {"edges": ["E2"], "size": {"type": "constant", "radius": "d2"}, "tangent_chain": True}])]}}
        self.assertEqual(len(apiref.cen_dressup_sets(two_sizes, "fillet")[1]), 2)

    def test_bag_is_unordered_and_counts_duplicates(self):
        a = apiref.Bag([("e", 1.0), ("e", 2.0), ("e", 2.0)])
        b = apiref.Bag([("e", 2.0), ("e", 1.0), ("e", 2.0 + 1e-7)])
        self.assertTrue(apiref.same(a, b))
        self.assertFalse(apiref.same(a, apiref.Bag([("e", 1.0), ("e", 2.0), ("e", 3.0)])))
        self.assertFalse(apiref.same(a, apiref.Bag([("e", 1.0), ("e", 2.0)])))
        left_a, left_b = apiref.bag_match([1.0, 2.0, 3.0], [3.0, 5.0])
        self.assertEqual((left_a, left_b), ([1.0, 2.0], [5.0]))

    def test_within_may_be_exceeded(self):
        api = apiref.Within([(1, 0)])
        self.assertTrue(apiref.same(api, apiref.Bag([(1, 0), (2, 0)])))
        self.assertFalse(apiref.same(api, apiref.Bag([(2, 0)])))
        self.assertFalse(apiref.same(api, None))

    def test_uniq_and_classify(self):
        self.assertEqual(apiref.uniq([1.0, 1.0 + 1e-7, 2.0]), [1.0, 2.0])
        self.assertIsNone(apiref.classify(None, None))
        self.assertEqual(apiref.classify(1.0, None), "dec_miss")
        self.assertEqual(apiref.classify(None, 1.0), "api_miss")
        self.assertEqual(apiref.classify(1.0, 1.0), "agree")
        self.assertEqual(apiref.classify(1.0, 2.0), "differs")

    def test_fnv_hash_of_the_learning_dump_names(self):
        # FNV-1a 64 of "a" is af63dc4c8601ec8c; the dump keeps the low 32 bits.
        self.assertEqual(apiref.fnv_hash("a"), "8601ec8c")
        self.assertEqual(apiref.fnv_hash(""), "84222325")


class GeometryTests(unittest.TestCase):
    def test_plane_key_ignores_the_stored_origin(self):
        a = {"type": "Plane", "origin": [1.0, 2.0, 3.0], "normal": [0.0, 0.0, 1.0]}
        b = {"type": "Plane", "origin": [-4.0, 9.0, 3.0], "normal": [0.0, 0.0, 1.0]}
        c = {"type": "Plane", "origin": [0.0, 0.0, 3.5], "normal": [0.0, 0.0, 1.0]}
        self.assertTrue(apiref.same(apiref.geometry_key(a), apiref.geometry_key(b)))
        self.assertFalse(apiref.same(apiref.geometry_key(a), apiref.geometry_key(c)))

    def test_cylinder_key_ignores_the_point_on_the_axis_and_its_sign(self):
        a = {"type": "Cylinder", "origin": [1.0, 0.0, 0.0], "axis": [0.0, 0.0, 1.0], "radius": 2.0}
        b = {"type": "Cylinder", "origin": [1.0, 0.0, 7.0], "axis": [0.0, 0.0, -1.0], "radius": 2.0}
        self.assertTrue(apiref.same(apiref.geometry_key(a), apiref.geometry_key(b)))

    def test_arc_key_from_angles_matches_the_end_points(self):
        api = {"id": "c0", "type": "SketchArc", "geometry": {
            "type": "Arc3D", "center": [0.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0],
            "referenceVector": [1.0, 0.0, 0.0], "radius": 2.0,
            "startPoint": [2.0, 0.0, 0.0], "endPoint": [0.0, 2.0, 0.0],
            "startAngle": 0.0, "endAngle": math.pi / 2}}
        dec = {"id": "c5", "type": "SketchArc", "geometry": {
            "type": "Arc3D", "center": [0.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0],
            "referenceVector": [1.0, 0.0, 0.0], "radius": 2.0,
            "startAngle": 0.0, "endAngle": math.pi / 2}}
        self.assertTrue(apiref.same(apiref.curve_key(api), apiref.curve_key(dec)))
        dec["geometry"]["endAngle"] = math.pi
        self.assertFalse(apiref.same(apiref.curve_key(api), apiref.curve_key(dec)))

    def test_line_key_ignores_direction(self):
        a = {"id": "c0", "type": "SketchLine", "geometry": {"type": "Line3D", "startPoint": [0, 0, 0], "endPoint": [1, 0, 0]}}
        b = {"id": "c9", "type": "SketchLine", "geometry": {"type": "Line3D", "startPoint": [1, 0, 0], "endPoint": [0, 0, 0]}}
        self.assertTrue(apiref.same(apiref.curve_key(a), apiref.curve_key(b)))

    def test_axis_lines(self):
        axis = {"kind": "construction_axis", "origin": "Z"}
        cyl = {"kind": "face", "geometry": {"type": "Cylinder", "origin": [0, 0, 5], "axis": [0, 0, -1], "radius": 1}}
        self.assertTrue(apiref.same(apiref.axis_line_of(axis), apiref.axis_line_of(cyl)))
        off = {"kind": "face", "geometry": {"type": "Cylinder", "origin": [1, 0, 5], "axis": [0, 0, 1], "radius": 1}}
        self.assertFalse(apiref.same(apiref.axis_line_of(axis), apiref.axis_line_of(off)))

    def test_mirror_plane_is_sign_free(self):
        a = ("face", ("plane", (-1.0, 0.0, 0.0), 2.0))
        b = ("face", ("plane", (1.0, 0.0, 0.0), -2.0))
        self.assertTrue(apiref.same(apiref.plane_sign_free(a), apiref.plane_sign_free(b)))


def item(index, type_, name, **kw):
    d = {"index": index, "objectType": type_, "name": name}
    d.update(kw)
    return d


class AlignTests(unittest.TestCase):
    def test_extra_decoder_items_and_class_named_occurrences(self):
        api = [item(0, "Occurrence", " Part:1"), item(1, "Sketch", "Sketch1"), item(2, "ExtrudeFeature", "Extrude1")]
        dec = [item(0, "DerivedContext", ""), item(1, "ComponentInsert", "Part"),
               item(2, "Sketch", "Sketch1"), item(3, "ExtrudeFeature", "Extrude1")]
        pairs = apiref.align(api, dec)
        got = [(a["name"] if a else None, d["name"] if d else None) for a, d in pairs]
        self.assertEqual(got, [(None, ""), (" Part:1", "Part"), ("Sketch1", "Sketch1"), ("Extrude1", "Extrude1")])

    def test_different_types_never_pair(self):
        pairs = apiref.align([item(0, "Sketch", "A")], [item(0, "ExtrudeFeature", "A")])
        self.assertEqual(len(pairs), 2)
        self.assertTrue(all(a is None or d is None for a, d in pairs))


# ---------------------------------------------------------------------------
# A synthetic design: a sketch with a circle and a diameter dimension, and a
# through-all cut of its profile.

PARAM = {"kind": "parameter", "name": "d1", "expression": "20 mm", "value": 2.0, "unit": "mm"}


def api_dump():
    sketch = item(0, "Sketch", "Sketch1", isSuppressed=False, healthState="HealthyFeatureHealthState",
                  isRolledBack=False, detail={
                      "name": "Sketch1", "origin": [0, 0, 0], "xDirection": [1, 0, 0], "yDirection": [0, 1, 0],
                      "points": [{"id": "p0", "xyz": [0, 0, 0], "isFixed": True},
                                 {"id": "p1", "xyz": [1, 0, 0], "isFixed": False}],
                      "curves": [{"id": "c0", "type": "SketchCircle", "isConstruction": False, "isFixed": False,
                                  "isReference": False, "isLinked": False, "radius": 1.0,
                                  "geometry": {"type": "Circle3D", "center": [0, 0, 0], "normal": [0, 0, 1],
                                               "radius": 1.0}},
                                 {"id": "c1", "type": "SketchLine", "isConstruction": True, "isFixed": False,
                                  "isReference": False, "isLinked": False,
                                  "geometry": {"type": "Line3D", "startPoint": [0, 0, 0], "endPoint": [1, 0, 0]}}],
                      "constraints": [{"id": "k0", "type": "HorizontalConstraint", "refs": {"line": "c1"}, "props": {}}],
                      "dimensions": [{"id": "d0", "type": "SketchDiameterDimension", "parameter": PARAM,
                                      "isDriving": True, "value": 2.0, "textPosition": [0, 0, 0],
                                      "refs": {"entity": "c0"}, "props": {}}]})
    cut = item(1, "ExtrudeFeature", "Extrude1", isSuppressed=False, healthState="HealthyFeatureHealthState",
               isRolledBack=False, detail={
                   "operation": "CutFeatureOperation",
                   "profile": {"kind": "profile", "sketch": "Sketch1", "sketch_timeline_index": 0, "profile_index": 0,
                               "loops": [{"isOuter": True, "curves": ["c0"]}], "area": 3.14159},
                   "extentType": "OneSideFeatureExtentType",
                   "extentOne": {"_type": "ThroughAllExtentDefinition", "isPositiveDirection": False},
                   "taperAngleOne": {"kind": "parameter", "name": "d2", "expression": "0 deg", "value": 0.0,
                                     "unit": "deg"},
                   "isThinExtrude": False,
                   "participantBodies": "<BodyVector; proxy of <Swig Object"})
    steps = [{"timeline_index": 0, "bodies": []},
             {"timeline_index": 1, "bodies": [{"name": "Body1", "occurrence": "", "volume": 10.0}]}]
    return {"timeline": {"items": [sketch, cut]},
            "parameters": {"user": [], "model": [dict(PARAM, role="Diameter", comment="", isFavorite=False,
                                                      createdBy={"kind": "sketch", "name": "Sketch1"},
                                                      dependents=[])]},
            "steps": {"items": steps}}


def ir_dump(flag=1):
    sketch = item(0, "Sketch", "Sketch1", detail={
        "origin": [0, 0, 0], "xDirection": [1, 0, 0], "yDirection": [0, 1, 0],
        "points": [{"id": "p0", "xyz": [0, 0, 0]}, {"id": "p1", "xyz": [1, 0, 0]}],
        "curves": [{"id": "c0", "type": "SketchCircle", "radius": 1.0,
                    "geometry": {"type": "Circle3D", "center": [0, 0, 0], "normal": [0, 0, 1], "radius": 1.0}},
                   {"id": "c1", "type": "SketchLine", "isConstruction": True,
                    "geometry": {"type": "Line3D", "startPoint": [1, 0, 0], "endPoint": [0, 0, 0]}}],
        "constraints": [{"id": "k0", "type": "HorizontalConstraint", "refs": {"entities": ["c1"]}}],
        "dimensions": [{"id": "d0", "type": "SketchDiameterDimension", "parameter": PARAM,
                        "textPosition": [0, 0, 0], "refs": {"entities": ["c0"]}}]})
    cut = item(1, "ExtrudeFeature", "Extrude1", detail={
        "operation": "CutFeatureOperation",
        "profile": [{"kind": "profile", "sketch": "Sketch1", "sketch_timeline_index": 0}],
        "taperAngleOne": {"kind": "parameter", "name": "d2", "expression": "0 deg", "value": 0.0, "unit": "deg"},
        "participantBodies": [{"kind": "body", "_f3d": {"producer": 5, "body_index": 0}}],
        "_f3d_profile_loops": [{"children": [], "loops": [{"curves": [{"id": "c0"}], "outer": True}]}]},
        _f3d={"extrude": {"extent_a": 1, "sides": [4, 0], "flag": flag, "direction": 1.0}})
    return {"timeline": {"items": [sketch, cut]},
            "parameters": {"user": [], "model": [dict(PARAM, role="Diameter",
                                                      createdBy={"kind": "sketch_dimension", "sketch": "Sketch1"})]}}


CENSUS_LINE = {
    "item": 1, "type": "ExtrudeFeature", "status": "settled",
    "answer": {"defs": [{"type": "extrude", "operation": "cut", "extent": {"type": "through_all"}, "flip": True,
                         "profiles": [{"sketch": "T0", "region": 0}]}]},
    "context": {"sketches": {"T0": {"regions": [{"outer": ["c0"], "holes": [], "area": 314.159}]}}},
}


def write_design(root, id_, api, ir, census_lines=None, source="f3d/part.f3d"):
    os.makedirs(os.path.join(root, "api"), exist_ok=True)
    os.makedirs(os.path.join(root, "ir"), exist_ok=True)
    with gzip.open(os.path.join(root, "api", id_ + ".json.gz"), "wt", encoding="utf-8") as f:
        json.dump(api, f)
    with open(os.path.join(root, "ir", id_ + ".json"), "w", encoding="utf-8") as f:
        json.dump(ir, f)
    with open(os.path.join(root, "index.tsv"), "w", encoding="utf-8") as f:
        f.write("id\tsource\nx\ty\n%s\t%s\n" % (id_, source))
    tag = apiref.fnv_hash(os.path.join(root, source))
    for status in ("settled", "unsettled"):
        lines = [l for l in census_lines or [] if l.get("status", "settled") == status]
        if lines:
            d = os.path.join(root, "census", status)
            os.makedirs(d, exist_ok=True)
            with open(os.path.join(d, "part.f3d-%s.jsonl" % tag), "w", encoding="utf-8") as f:
                for line in lines:
                    f.write(json.dumps(line) + "\n")


class CompareTests(unittest.TestCase):
    def compare(self, api, ir, census=None):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        write_design(tmp.name, "t01", api, ir, census)
        task = ("t01", os.path.join(tmp.name, "api", "t01.json.gz"), os.path.join(tmp.name, "ir", "t01.json"),
                "f3d/part.f3d", os.path.join(tmp.name, "census") if census else None, tmp.name, None)
        _, acc, items = apiref.compare_design(task)
        return acc, items, tmp.name

    def count(self, acc, group, field, state):
        return acc.counts[(group, field)][state]

    def test_extrusion_fields(self):
        acc, items, _ = self.compare(api_dump(), ir_dump(flag=1))
        self.assertEqual(acc.notes["items_paired"], 2)
        g = "extrusion"
        self.assertEqual(self.count(acc, g, "operation", "agree"), 1)
        # through all: the stored flag byte 1 is against the sketch normal
        self.assertEqual(self.count(acc, g, "extentOne.isPositiveDirection", "agree"), 1)
        self.assertEqual(self.count(acc, g, "direction", "agree"), 1)
        # the dump's extent type is left out for the application's; the decoder's extent code 1 is one side
        self.assertEqual(self.count(acc, g, "extentType", "agree"), 1)
        self.assertEqual(self.count(acc, g, "isThinExtrude", "dec_miss"), 1)
        self.assertEqual(self.count(acc, g, "taperAngleOne", "agree"), 1)
        # the profile: same curves
        self.assertEqual(self.count(acc, g, "profile.regions", "agree"), 1)
        self.assertEqual(self.count(acc, g, "profile.region.exact", "agree"), 1)
        self.assertEqual(self.count(acc, g, "profile.sketch", "agree"), 1)
        # participants: the cut changed Body1, created by item 1 (not by the decoder's item 5)
        self.assertEqual(self.count(acc, g, "participants", "differs"), 0)

    def test_wrong_flag_is_a_mismatch_row(self):
        acc, _, _ = self.compare(api_dump(), ir_dump(flag=0))
        self.assertEqual(self.count(acc, "extrusion", "extentOne.isPositiveDirection", "differs"), 1)
        rows = acc.rows[("extrusion", "direction")]
        self.assertEqual(len(rows), 1)
        design, idx, name, field, source, state, dec, api = rows[0]
        self.assertEqual((design, idx, name, source, state), ("t01", 1, "Extrude1", "decoder", "differs"))
        self.assertEqual((dec, api), ('"positive"', '"negative"'))

    def test_sketch_elements_matched_by_geometry(self):
        acc, _, _ = self.compare(api_dump(), ir_dump())
        # the line is stored the other way round in the decoder's dump
        self.assertEqual(self.count(acc, "sketch.curves", "geometry:SketchLine", "agree"), 1)
        self.assertEqual(self.count(acc, "sketch.curves", "geometry:SketchCircle", "agree"), 1)
        self.assertEqual(self.count(acc, "sketch.curves", "isConstruction", "agree"), 2)
        # flags the decoder leaves out are false
        self.assertEqual(self.count(acc, "sketch.curves", "isReference", "agree"), 2)
        self.assertEqual(self.count(acc, "sketch.curves", "isFixed", "agree"), 2)
        self.assertEqual(self.count(acc, "sketch.constraints", "HorizontalConstraint", "agree"), 1)
        self.assertEqual(self.count(acc, "sketch.dimensions", "SketchDiameterDimension", "agree"), 1)
        self.assertEqual(self.count(acc, "sketch.dimensions", "isDriving", "agree"), 1)
        self.assertEqual(self.count(acc, "sketch.points", "position", "agree"), 2)

    def test_missing_curve_and_construction_flag_differences(self):
        ir = ir_dump()
        ir["timeline"]["items"][0]["detail"]["curves"].pop(0)
        ir["timeline"]["items"][0]["detail"]["curves"][0].pop("isConstruction")
        acc, _, _ = self.compare(api_dump(), ir)
        self.assertEqual(self.count(acc, "sketch.curves", "geometry:SketchCircle", "dec_miss"), 1)
        self.assertEqual(self.count(acc, "sketch.curves", "isConstruction", "differs"), 1)

    def test_linked_and_fixed_flag_differences(self):
        ir = ir_dump()
        curves = ir["timeline"]["items"][0]["detail"]["curves"]
        curves[0]["isLinked"] = True
        curves[0]["isReference"] = True
        curves[1]["isFixed"] = True
        acc, _, _ = self.compare(api_dump(), ir)
        self.assertEqual(self.count(acc, "sketch.curves", "isLinked", "differs"), 1)
        self.assertEqual(self.count(acc, "sketch.curves", "isLinked", "agree"), 1)
        self.assertEqual(self.count(acc, "sketch.curves", "isReference", "differs"), 1)
        self.assertEqual(self.count(acc, "sketch.curves", "isFixed", "differs"), 1)

    def test_parameters(self):
        acc, _, _ = self.compare(api_dump(), ir_dump())
        self.assertEqual(self.count(acc, "parameters", "exists", "agree"), 1)
        self.assertEqual(self.count(acc, "parameters", "expression", "agree"), 1)
        self.assertEqual(self.count(acc, "parameters", "createdBy.kind", "agree"), 1)
        self.assertEqual(self.count(acc, "parameters", "isFavorite", "agree"), 1)

    def test_parameters_of_other_designs_and_repeated_names(self):
        """The application lists the parameters of the other designs a file
        links to; their names repeat the root's. Parameters pair by creator,
        a foreign one without a counterpart is left out, a favourite and an
        indirect dependent agree."""
        api = api_dump()
        def mk(name, expr, role, kind, owner, idx, favorite=False):
            return dict(PARAM, name=name, expression=expr, role=role, comment="", isFavorite=favorite,
                        dependents=[], createdBy={"kind": kind, "name": owner, "timeline_index": idx})
        api["parameters"]["model"] = [
            mk("d1", "99 mm", "Other", "feature", "Extrude7", 5),  # another design's, first in the list
            mk("d1", "20 mm", "Radius", "feature", "Extrude1", 1, favorite=True),
        ]
        ir = ir_dump()
        ir["parameters"]["model"] = [mk("d1", "20 mm", "Radius", "feature", "Extrude1", 1, favorite=True)]
        acc, _, _ = self.compare(api, ir)
        self.assertEqual(self.count(acc, "parameters", "exists", "agree"), 1)
        self.assertEqual(self.count(acc, "parameters", "exists", "dec_miss"), 0)
        self.assertEqual(self.count(acc, "parameters", "expression", "agree"), 1)
        self.assertEqual(self.count(acc, "parameters", "isFavorite", "agree"), 1)
        self.assertEqual(acc.notes["parameters_linked_designs"], 1)

    def test_parameters_with_a_wrong_creator_still_pair_by_role(self):
        api = api_dump()
        api["parameters"]["model"][0]["createdBy"] = {"kind": "sketch", "name": "Sketch1", "timeline_index": 0}
        ir = ir_dump()
        ir["parameters"]["model"][0]["createdBy"] = {"kind": "feature", "objectType": "Sketch", "name": "Sketch1",
                                                     "timeline_index": 0}
        acc, _, _ = self.compare(api, ir)
        # a sketch's own parameter is a feature of type Sketch in the decoder's dump
        self.assertEqual(self.count(acc, "parameters", "createdBy.kind", "agree"), 1)
        ir["parameters"]["model"][0]["createdBy"] = {"kind": "feature", "objectType": "ExtrudeFeature",
                                                     "name": "Extrude1", "timeline_index": 3}
        acc, _, _ = self.compare(api, ir)
        self.assertEqual(self.count(acc, "parameters", "exists", "agree"), 1)
        self.assertEqual(self.count(acc, "parameters", "createdBy.kind", "differs"), 1)

    def test_driven_dimension_values_are_measurements(self):
        api = api_dump()
        api["timeline"]["items"][0]["detail"]["dimensions"][0]["isDriving"] = False
        ir = ir_dump()
        ir["timeline"]["items"][0]["detail"]["dimensions"][0]["isDriving"] = False
        ir["timeline"]["items"][0]["detail"]["dimensions"][0]["parameter"] = dict(PARAM, expression="19.9 mm", value=1.99)
        ir["parameters"]["model"][0] = dict(ir["parameters"]["model"][0], expression="19.9 mm", value=1.99)
        acc, _, _ = self.compare(api, ir)
        self.assertEqual(self.count(acc, "sketch.dimensions", "parameter.value", "differs"), 0)
        self.assertEqual(self.count(acc, "parameters", "value", "differs"), 0)
        self.assertEqual(acc.notes["driven_dimension_values"], 1)

    def test_a_failed_parameters_section_is_left_out(self):
        api = api_dump()
        api["errors"] = [{"section": "parameters", "error": "Traceback"}]
        api["parameters"] = {"user": [], "model": []}
        acc, _, _ = self.compare(api, ir_dump())
        self.assertEqual(self.count(acc, "parameters", "exists", "api_miss"), 0)
        self.assertEqual(acc.notes["parameters_sections_failed"], 1)

    def test_item_types_and_occurrence_names_compare_by_family(self):
        api = api_dump()
        api["timeline"]["items"].append(item(2, "Occurrence", " Part:1", isSuppressed=False, isRolledBack=False))
        ir = ir_dump()
        ir["timeline"]["items"].append(item(2, "ComponentInsert", "Part"))
        acc, _, _ = self.compare(api, ir)
        self.assertEqual(self.count(acc, "timeline", "type", "agree"), 3)
        self.assertEqual(self.count(acc, "timeline", "name", "agree"), 3)
        api["timeline"]["items"][2]["name"] = "Component Insert Part:1"
        ir["timeline"]["items"][2]["name"] = "Component Insert1"
        acc, _, _ = self.compare(api, ir)
        self.assertEqual(self.count(acc, "timeline", "name", "agree"), 3)

    def test_sweeps_pipes_lofts_and_moves(self):
        canon = apiref.Canon({}, "api")
        entity = {"kind": "sketch_entity", "objectType": "SketchLine", "sketch": "S", "sketch_timeline_index": 1,
                  "id": "c0"}
        sweep_api = {"detail": {
            "path": [{"_type": "PathEntity", "entity": entity, "curve": {"type": "NurbsCurve3D"},
                      "curveType": "Line3DCurveType", "isOpposedToEntity": True, "parentPath": [{}]}],
            "profile": [{"kind": "profile", "sketch": "P", "profile_index": 0},
                        {"kind": "profile", "sketch": "P", "profile_index": 1}],
            "guideSurfaces": "<BRepFaceVector; proxy of <Swig Object>"}}
        sweep_dec = {"detail": {"path": [{"_type": "PathEntity", "entity": entity}],
                                "profile": [{"kind": "profile", "sketch": "P"}]}}
        fa, fd = apiref.f_sweep(canon, sweep_api, "api"), apiref.f_sweep(canon, sweep_dec, "ir")
        self.assertTrue(apiref.same(fa["path"], fd["path"]))
        self.assertTrue(apiref.same(fa["profile.sketch"], fd["profile.sketch"]))
        sec = lambda cond: {"detail": {"loftSections": [  # noqa: E731
            {"_type": "LoftSection", "index": 0, "entity": {"kind": "profile", "sketch": "A"}, "endCondition": cond}]}}
        la = apiref.f_loft(canon, sec({"_type": "LoftFreeEndCondition", "parentLoftSection": {"_type": "LoftSection"}}),
                           "api")
        ld = apiref.f_loft(canon, sec({"_type": "LoftFreeEndCondition"}), "ir")
        self.assertTrue(apiref.same(la["loftSections"], ld["loftSections"]))
        move = apiref.f_move(canon, {"detail": {"transform": [[1.0]], "definition": {
            "_type": "MoveFeatureFreeMoveDefinition", "transform": [[1.0]],
            "parentMoveFeature": {"kind": "feature", "name": "Move1"}}}}, "api")
        self.assertEqual(sorted(move), ["definition._type", "transform"])

    def test_census_answer_against_the_api(self):
        acc, _, _ = self.compare(api_dump(), ir_dump(), [CENSUS_LINE])
        g = "extrusion"
        self.assertEqual(self.count(acc, g, "direction", "c_agree"), 1)
        self.assertEqual(self.count(acc, g, "operation", "c_agree"), 1)
        self.assertEqual(self.count(acc, g, "profile.regions", "c_agree"), 1)
        # Mitcad's region area (mm2) against the application's profile area (cm2)
        self.assertEqual(self.count(acc, g, "profile.areas", "c_agree"), 1)
        wrong = json.loads(json.dumps(CENSUS_LINE))
        wrong["answer"]["defs"][0]["flip"] = False
        acc, _, _ = self.compare(api_dump(), ir_dump(), [wrong])
        self.assertEqual(self.count(acc, g, "direction", "c_differs"), 1)
        census_rows = [r for r in acc.rows[(g, "direction")] if r[4] == "census"]
        self.assertEqual(len(census_rows), 1)

    def test_unsettled_lines_are_not_counted(self):
        line = dict(CENSUS_LINE, status="unsettled")
        acc2, _, _ = self.compare(api_dump(), ir_dump(), [line])
        self.assertEqual(self.count(acc2, "extrusion", "direction", "c_agree"), 0)

    def test_suppressed_items(self):
        api = api_dump()
        api["timeline"]["items"][1]["isSuppressed"] = True
        api["timeline"]["items"][1]["healthState"] = "SuppressedFeatureHealthState"
        acc, _, _ = self.compare(api, ir_dump())
        self.assertEqual(self.count(acc, "timeline", "isSuppressed", "differs"), 1)
        ir = ir_dump()
        ir["timeline"]["items"][1]["isSuppressed"] = True
        acc, _, _ = self.compare(api, ir)
        self.assertEqual(self.count(acc, "timeline", "isSuppressed", "agree"), 2)
        self.assertEqual(self.count(acc, "timeline", "healthState", "dec_miss"), 2)

    def test_command_line_writes_the_report(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        write_design(tmp.name, "t01", api_dump(), ir_dump(flag=0), [CENSUS_LINE])
        out = os.path.join(tmp.name, "out")
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            apiref.main(["compare", "--api", os.path.join(tmp.name, "api"), "--ir", os.path.join(tmp.name, "ir"),
                         "--census", os.path.join(tmp.name, "census"),
                         "--index", os.path.join(tmp.name, "index.tsv"), "--root", tmp.name,
                         "--out", out, "--jobs", "1"])
        with open(os.path.join(out, "summary.txt"), encoding="utf-8") as f:
            summary = f.read()
        self.assertIn("== extrusion", summary)
        self.assertIn("extentOne.isPositiveDirection", summary)
        self.assertIn("2 items paired", summary)
        with open(os.path.join(out, "mismatches", "extrusion.tsv"), encoding="utf-8") as f:
            rows = f.read().splitlines()
        self.assertTrue(rows[0].startswith("design\titem\tname\tfield\tsource\tstate"))
        self.assertTrue(any("\tdirection\tdecoder\tdiffers\t" in r for r in rows))
        self.assertTrue(os.path.exists(os.path.join(out, "items.tsv")))


def plane_face(normal, d):
    """A face reference whose plane is `normal` through `normal * d`."""
    origin = [c * d for c in normal]
    return {"kind": "face", "geometry": {"type": "Plane", "origin": origin, "normal": list(normal)}}


class FieldTests(unittest.TestCase):
    """The fields of the groups on small items."""

    def canon(self, side="api", steps=()):
        dump = {"timeline": {"items": []}, "steps": {"items": list(steps)}}
        return apiref.Canon(dump, side)

    def test_shell_and_construction_planes_have_no_side(self):
        canon = self.canon()
        shell = lambda faces: {"detail": {"inputEntities": faces}}
        a = apiref.f_shell(canon, shell([plane_face((0.0, 0.0, 1.0), 2.0)]), "api")
        b = apiref.f_shell(canon, shell([plane_face((0.0, 0.0, -1.0), -2.0)]), "ir")
        self.assertTrue(apiref.same(a["inputEntities"], b["inputEntities"]))
        offset = lambda face: {"detail": {"definition": {"_type": "ConstructionPlaneOffsetDefinition",
                                                          "planarEntity": face}}}
        a = apiref.f_construction(canon, offset(plane_face((1.0, 0.0, 0.0), 3.0)), "api")
        b = apiref.f_construction(canon, offset(plane_face((-1.0, 0.0, 0.0), -3.0)), "ir")
        self.assertTrue(apiref.same(a["definition.planarEntity"], b["definition.planarEntity"]))

    def test_extrusion_extent_faces_have_no_side_and_bodies_come_with_their_maker(self):
        face = lambda n, d: {"detail": {"extentOne": {"_type": "ToEntityExtentDefinition",
                                                      "entity": plane_face(n, d)}}}
        a = apiref.f_extrusion(self.canon(), face((0.0, 0.0, 1.0), 1.0), "api")
        b = apiref.f_extrusion(self.canon("ir"), face((0.0, 0.0, -1.0), -1.0), "ir")
        self.assertTrue(apiref.same(a["extentOne.entity"], b["extentOne.entity"]))
        # a body the extent goes up to: the application names it, the
        # decoder gives its producer
        steps = [{"timeline_index": 3, "bodies": [{"name": "Body9", "occurrence": "A:1", "volume": 1.0}]}]
        api_body = {"detail": {"extentOne": {"_type": "ToEntityExtentDefinition",
                                             "entity": {"kind": "body", "name": "Body9", "occurrence": None}}}}
        ir_body = {"detail": {"extentOne": {"_type": "ToEntityExtentDefinition",
                                            "entity": {"kind": "body",
                                                       "_f3d": {"producer": 3, "body_index": 0}}}}}
        a = apiref.f_extrusion(self.canon(steps=steps), api_body, "api")
        b = apiref.f_extrusion(self.canon("ir"), ir_body, "ir")
        self.assertEqual(a["extentOne.entity"], ("body", (3, 0)))
        self.assertTrue(apiref.same(a["extentOne.entity"], b["extentOne.entity"]))

    def test_an_empty_decoder_list_of_participants_says_nothing(self):
        detail = {"operation": "CutFeatureOperation", "participantBodies": []}
        out = apiref.f_extrusion(self.canon("ir"), {"detail": detail}, "ir")
        self.assertNotIn("participants", out)

    def test_hole_thread_and_construction_path_follow_the_application(self):
        hole = lambda thread: {"detail": {"holeType": "SimpleHoleType", "thread": thread}}
        a = apiref.f_hole(self.canon(), hole(["feature", "Hole1-Thread1"]), "api")
        b = apiref.f_hole(self.canon("ir"), hole({"_type": "ThreadFeature", "isModeled": False}), "ir")
        self.assertEqual(a["thread"], b["thread"])
        edge = {"kind": "edge", "mid_point": [0.0, 0.0, 0.0], "length": 2.0}
        plane = lambda key, value: {"detail": {"definition": {
            "_type": "ConstructionPlaneDistanceOnPathDefinition", key: value}}}
        a = apiref.f_construction(self.canon(), plane("pathEntity", edge), "api")
        b = apiref.f_construction(self.canon("ir"), plane("pathEntities", [edge]), "ir")
        self.assertTrue(apiref.same(a["definition.pathEntity"], b["definition.pathEntity"]))

    def test_regions_in_another_loop_structure_count_as_found(self):
        # one hole against two holes over the same curves, and an outline
        # that holds the application's separate regions
        outer = apiref.Bag([("L", 1.0), ("L", 2.0)])
        api = ("region", outer, apiref.Bag([apiref.Bag([("L", 3.0), ("L", 4.0)])]))
        dec = ("region", outer, apiref.Bag([apiref.Bag([("L", 3.0)]), apiref.Bag([("L", 4.0)])]))
        acc = apiref.Acc()
        apiref.region_units(acc, "t01", 1, "Extrude1", "extrusion", {"_regions": [api]}, {"_regions": [dec]})
        self.assertEqual(acc.counts[("extrusion", "profile.region.loops")]["agree"], 1)
        self.assertEqual(acc.counts[("extrusion", "profile.region.missing")]["dec_miss"], 0)
        part = ("region", apiref.Bag([("L", 1.0)]), apiref.Bag([]))
        union = ("region", apiref.Bag([("L", 1.0), ("L", 5.0)]), apiref.Bag([]))
        acc = apiref.Acc()
        apiref.region_units(acc, "t01", 1, "Extrude1", "extrusion", {"_regions": [part, part]}, {"_regions": [union]})
        self.assertEqual(acc.counts[("extrusion", "profile.region.relaxed")]["agree"], 1)
        self.assertEqual(acc.counts[("extrusion", "profile.region.union")]["agree"], 1)
        self.assertEqual(acc.counts[("extrusion", "profile.region.missing")]["dec_miss"], 0)


if __name__ == "__main__":
    unittest.main()
