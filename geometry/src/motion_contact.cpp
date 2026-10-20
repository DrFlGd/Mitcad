// SPDX-License-Identifier: MIT
#include "mitcad/geometry/motion_contact.hpp"

#include <algorithm>
#include <map>
#include <stdexcept>

#include <BRepAlgoAPI_Common.hxx>
#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <NCollection_List.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS_Shape.hxx>

#include "mitcad/analysis/common.hpp"

namespace mitcad::geometry {

namespace {

// A body's solids, oriented, and its box, both in its own coordinates.
struct Prepared {
  TopoDS_Shape solids;
  Bnd_Box box;
  bool has_solid = false;
};

} // namespace

std::vector<double> overlap_volumes(const std::vector<PlacedPair>& pairs, double tolerance) {
  // Each body once, however many pairs it is in.
  std::map<const Shape*, Prepared> prepared;
  const auto prepare = [&prepared](const Shape* shape) -> const Prepared& {
    auto found = prepared.find(shape);
    if (found != prepared.end()) {
      return found->second;
    }
    Prepared p;
    p.solids = analysis::oriented_solids(shape->occt());
    p.has_solid = TopExp_Explorer(p.solids, TopAbs_SOLID).More();
    if (p.has_solid) {
      BRepBndLib::Add(p.solids, p.box);
    }
    return prepared.emplace(shape, std::move(p)).first->second;
  };
  std::vector<double> volumes;
  volumes.reserve(pairs.size());
  for (const PlacedPair& pair : pairs) {
    if (pair.a == nullptr || pair.b == nullptr) {
      throw std::runtime_error("overlap of bodies: a body is missing");
    }
    const Prepared& a = prepare(pair.a);
    const Prepared& b = prepare(pair.b);
    if (!a.has_solid || !b.has_solid) {
      volumes.push_back(0.0);
      continue;
    }
    Bnd_Box box_a = a.box.Transformed(pair.a_placement);
    Bnd_Box box_b = b.box.Transformed(pair.b_placement);
    box_a.Enlarge(tolerance);
    if (box_a.IsOut(box_b)) {
      volumes.push_back(0.0);
      continue;
    }
    NCollection_List<TopoDS_Shape> object;
    NCollection_List<TopoDS_Shape> tool;
    object.Append(a.solids.Moved(TopLoc_Location(pair.a_placement)));
    tool.Append(b.solids.Moved(TopLoc_Location(pair.b_placement)));
    BRepAlgoAPI_Common common;
    common.SetArguments(object);
    common.SetTools(tool);
    // The model's bodies, which other results share, stay as they are.
    common.SetNonDestructive(true);
    common.SetRunParallel(true);
    if (tolerance > 0.0) {
      common.SetFuzzyValue(tolerance);
    }
    common.Build();
    if (!common.IsDone() || common.HasErrors()) {
      throw std::runtime_error("the overlap of two bodies could not be computed");
    }
    const GProp_GProps props = analysis::volume_properties(common.Shape());
    volumes.push_back(std::max(0.0, props.Mass()));
  }
  return volumes;
}

} // namespace mitcad::geometry
